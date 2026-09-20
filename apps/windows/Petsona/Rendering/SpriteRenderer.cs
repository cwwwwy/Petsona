using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using Petsona.Core;
using Petsona.Core.Interop;
using Windows.Graphics.Imaging;
using Windows.Storage;

namespace Petsona.Rendering;

/// <summary>
/// Decodes the pet sprite sheet with GDI+ and prepares premultiplied BGRA
/// frames for <c>UpdateLayeredWindow</c>. The hit mask is the union of the
/// current frame and the idle row (row 0) so a gaze pose cannot make the
/// resting body unclickable.
/// </summary>
internal sealed class SpriteRenderer : IDisposable
{
    private const byte AlphaThreshold = 13; // ~0.05 alpha

    private readonly Dictionary<string, Bitmap> _atlases = new(StringComparer.Ordinal);
    private readonly HashSet<string> _pendingDecodes = new(StringComparer.Ordinal);
    private readonly Dictionary<FrameKey, Frame> _frames = [];
    private readonly Dictionary<UnionKey, bool[]> _idleUnions = [];

    internal sealed class Frame
    {
        public required byte[] Pixels { get; init; }

        /// <summary>Clickable mask: current frame OR the idle-row union.</summary>
        public required bool[] HitMask { get; init; }

        public required int Width { get; init; }

        public required int Height { get; init; }

        public bool IsHit(int x, int y)
        {
            return x >= 0 && y >= 0 && x < Width && y < Height && HitMask[(y * Width) + x];
        }
    }

    /// <summary>Frame for the current snapshot at the window's DPI scale, or null.</summary>
    public Frame? GetFrame(PetsonaSnapshot snapshot, string atlasPath, double dpiScale)
    {
        if (string.IsNullOrEmpty(atlasPath) || snapshot.CellWidth == 0 || snapshot.CellHeight == 0)
        {
            return null;
        }

        var atlas = LoadAtlas(atlasPath);
        if (atlas is null)
        {
            return null;
        }

        var userScale = Math.Max(0.1, snapshot.Scale);
        var width = Math.Max(1, (int)Math.Round(snapshot.CellWidth * userScale * dpiScale));
        var height = Math.Max(1, (int)Math.Round(snapshot.CellHeight * userScale * dpiScale));

        var key = new FrameKey(atlasPath, snapshot.SpriteIndex, width, height);
        if (_frames.TryGetValue(key, out var cached))
        {
            return cached;
        }

        var (originX, originY) = SpriteFrame.SourceOrigin(snapshot, snapshot.SpriteIndex);
        var (pixels, mask) = RenderCell(
            atlas, originX, originY, (int)snapshot.CellWidth, (int)snapshot.CellHeight, width, height);
        var idleUnion = GetIdleUnion(atlas, snapshot, atlasPath, width, height);

        var hitMask = new bool[mask.Length];
        for (var i = 0; i < mask.Length; i++)
        {
            hitMask[i] = mask[i] || idleUnion[i];
        }

        var frame = new Frame { Pixels = pixels, HitMask = hitMask, Width = width, Height = height };
        if (_frames.Count > 256)
        {
            _frames.Clear();
        }

        _frames[key] = frame;
        return frame;
    }

    private bool[] GetIdleUnion(Bitmap atlas, PetsonaSnapshot snapshot, string atlasPath, int width, int height)
    {
        var key = new UnionKey(atlasPath, width, height);
        if (_idleUnions.TryGetValue(key, out var cached))
        {
            return cached;
        }

        var union = new bool[width * height];
        var columns = SpriteFrame.Columns(snapshot);
        for (var column = 0; column < columns; column++)
        {
            var (originX, originY) = SpriteFrame.CellOrigin(snapshot, SpriteFrame.IdleRow, column);
            var (_, mask) = RenderCell(
                atlas, originX, originY, (int)snapshot.CellWidth, (int)snapshot.CellHeight, width, height);
            for (var i = 0; i < union.Length; i++)
            {
                union[i] |= mask[i];
            }
        }

        if (_idleUnions.Count > 16)
        {
            _idleUnions.Clear();
        }

        _idleUnions[key] = union;
        return union;
    }

    private static (byte[] Pixels, bool[] Mask) RenderCell(
        Bitmap atlas, int sourceX, int sourceY, int sourceWidth, int sourceHeight, int width, int height)
    {
        using var composed = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
        using (var graphics = Graphics.FromImage(composed))
        {
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.DrawImage(
                atlas,
                new Rectangle(0, 0, width, height),
                new Rectangle(sourceX, sourceY, sourceWidth, sourceHeight),
                GraphicsUnit.Pixel);
        }

        var data = composed.LockBits(
            new Rectangle(0, 0, width, height), ImageLockMode.ReadOnly, PixelFormat.Format32bppPArgb);
        try
        {
            var pixels = new byte[width * height * 4];
            Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
            var mask = new bool[width * height];
            for (var i = 0; i < mask.Length; i++)
            {
                mask[i] = pixels[(i * 4) + 3] > AlphaThreshold;
            }

            return (pixels, mask);
        }
        finally
        {
            composed.UnlockBits(data);
        }
    }

    private Bitmap? LoadAtlas(string path)
    {
        if (_atlases.TryGetValue(path, out var cached))
        {
            return cached;
        }

        if (!File.Exists(path))
        {
            return null;
        }

        var bitmap = TryDecodeWithGdiPlus(path);
        if (bitmap is not null)
        {
            Cache(path, bitmap);
            return bitmap;
        }

        // GDI+ cannot decode every Codex sheet format (webp is common). Fall
        // back to the system imaging codecs through WinRT; the decode runs
        // asynchronously and the next frame picks the cached bitmap up.
        if (_pendingDecodes.Add(path))
        {
            _ = DecodeAtlasAsync(path);
        }

        return null;
    }

    private void Cache(string path, Bitmap bitmap)
    {
        if (_atlases.Count > 8)
        {
            foreach (var old in _atlases.Values)
            {
                old.Dispose();
            }

            _atlases.Clear();
            _frames.Clear();
            _idleUnions.Clear();
        }

        _atlases[path] = bitmap;
    }

    private static Bitmap? TryDecodeWithGdiPlus(string path)
    {
        try
        {
            using var stream = File.OpenRead(path);
            using var source = new Bitmap(stream);
            var copy = new Bitmap(source.Width, source.Height, PixelFormat.Format32bppPArgb);
            using (var graphics = Graphics.FromImage(copy))
            {
                graphics.DrawImage(source, 0, 0, source.Width, source.Height);
            }

            return copy;
        }
        catch (Exception ex) when (ex is IOException or ArgumentException or OutOfMemoryException or ExternalException)
        {
            return null;
        }
    }

    private async Task DecodeAtlasAsync(string path)
    {
        try
        {
            var file = await StorageFile.GetFileFromPathAsync(path);
            using var stream = await file.OpenReadAsync();
            var decoder = await BitmapDecoder.CreateAsync(stream);
            var pixelData = await decoder.GetPixelDataAsync(
                BitmapPixelFormat.Bgra8,
                BitmapAlphaMode.Premultiplied,
                new BitmapTransform(),
                ExifOrientationMode.IgnoreExifOrientation,
                ColorManagementMode.DoNotColorManage);
            var bytes = pixelData.DetachPixelData();
            var width = (int)decoder.PixelWidth;
            var height = (int)decoder.PixelHeight;
            var bitmap = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
            var data = bitmap.LockBits(
                new Rectangle(0, 0, width, height), ImageLockMode.WriteOnly, PixelFormat.Format32bppPArgb);
            try
            {
                var rowBytes = width * 4;
                if (data.Stride == rowBytes)
                {
                    Marshal.Copy(bytes, 0, data.Scan0, bytes.Length);
                }
                else
                {
                    for (var row = 0; row < height; row++)
                    {
                        Marshal.Copy(bytes, row * rowBytes, data.Scan0 + (row * data.Stride), rowBytes);
                    }
                }
            }
            finally
            {
                bitmap.UnlockBits(data);
            }

            Cache(path, bitmap);
        }
        catch (Exception error)
        {
            AppController.TryWriteErrorLog("atlas-decode", error);
        }
        finally
        {
            _pendingDecodes.Remove(path);
        }
    }

    public void Dispose()
    {
        foreach (var bitmap in _atlases.Values)
        {
            bitmap.Dispose();
        }

        _atlases.Clear();
        _frames.Clear();
        _idleUnions.Clear();
    }

    private readonly record struct FrameKey(string AtlasPath, uint SpriteIndex, int Width, int Height);

    private readonly record struct UnionKey(string AtlasPath, int Width, int Height);
}
