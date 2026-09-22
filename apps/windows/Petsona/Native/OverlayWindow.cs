using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Petsona.Native;

/// <summary>
/// Non-activating layered window used for the speech bubble and the edit
/// strip. Rendering stays in GDI+ so the overlays can remain pixel-transparent
/// and WS_EX_NOACTIVATE.
/// </summary>
internal sealed unsafe class OverlayWindow : IDisposable
{
    private const string ClassName = "PetsonaOverlayWindow";
    private const int BubbleFadeMs = 150;
    private const int StripAnimationMs = 120;
    private const float BubblePaddingX = 16f;
    private const float BubblePaddingY = 12f;
    private const float BubbleMaxTextWidth = 300f;
    private const float BubbleRadius = 12f;
    private const float ProgressBarHeight = 3f;
    private static readonly object ClassLock = new();
    private static bool _classRegistered;
    private static readonly nint ArrowCursor =
        NativeWin32.LoadCursorW(0, (nint)NativeWin32.IDC_ARROW);

    private readonly bool _isBubble;
    private GCHandle _selfHandle;
    private nint _hwnd;
    private bool _visible;
    private bool _hovered;
    private bool _trackingMouse;
    private bool _disposed;

    private string _bubbleText = string.Empty;
    private long _bubbleGeneration = -1;
    private long _bubbleFadeStartedAt;
    private byte _bubbleOpacity;
    private string _bubbleRenderKey = string.Empty;
    private byte[]? _bubblePixels;
    private int _bubblePixelWidth;
    private int _bubblePixelHeight;

    private bool _stripVertical;
    private float _stripExpansion;
    private float _stripTarget;
    private long _stripAnimationStartedAt;
    private string _stripRenderKey = string.Empty;
    private bool _stripStateInitialized;

    public OverlayWindow(bool isBubble)
    {
        _isBubble = isBubble;
        _selfHandle = GCHandle.Alloc(this);
        EnsureClassRegistered();

        _hwnd = NativeWin32.CreateWindowExW(
            NativeWin32.WS_EX_LAYERED | NativeWin32.WS_EX_TOOLWINDOW | NativeWin32.WS_EX_NOACTIVATE | NativeWin32.WS_EX_TOPMOST,
            ClassName,
            "Petsona",
            NativeWin32.WS_POPUP,
            0,
            0,
            1,
            1,
            0,
            0,
            NativeWin32.GetModuleHandleW(null),
            0);
        if (_hwnd == 0)
        {
            _selfHandle.Free();
            throw new InvalidOperationException("Cannot create an overlay window.");
        }

        NativeWin32.SetWindowLongPtrW(_hwnd, NativeWin32.GWLP_USERDATA, GCHandle.ToIntPtr(_selfHandle));
    }

    public event Action? Clicked;

    public event Action<bool>? HoverChanged;

    public int Width { get; private set; }

    public int Height { get; private set; }

    public bool IsHovered => _hovered;

    public void MoveTo(int x, int y)
    {
        NativeWin32.SetWindowPos(
            _hwnd, 0, x, y, 0, 0,
            NativeWin32.SWP_NOSIZE | NativeWin32.SWP_NOACTIVATE | 0x0004);
    }

    public void Show(bool visible)
    {
        if (visible != _visible)
        {
            _visible = visible;
            _ = NativeWin32.ShowWindow(_hwnd, visible ? NativeWin32.SW_SHOWNOACTIVATE : NativeWin32.SW_HIDE);
        }

        if (!visible)
        {
            _trackingMouse = false;
            SetHovered(false);
        }
    }

    /// <summary>
    /// Updates the bubble from the runtime projection. Progress is supplied by
    /// the runtime rather than by a local timer, so hover pause and the actual
    /// expiry remain in sync.
    /// </summary>
    public void UpdateBubble(
        string text,
        long remainingMilliseconds,
        long totalMilliseconds,
        long generation,
        long nowMilliseconds)
    {
        if (!_isBubble)
        {
            return;
        }

        if (string.IsNullOrEmpty(text))
        {
            Show(false);
            _bubbleText = string.Empty;
            _bubbleGeneration = -1;
            _bubbleRenderKey = string.Empty;
            _bubblePixels = null;
            _bubbleOpacity = 0;
            return;
        }

        if (generation != _bubbleGeneration)
        {
            _bubbleGeneration = generation;
            _bubbleText = text;
            _bubbleFadeStartedAt = nowMilliseconds;
            _bubbleOpacity = 0;
            _bubbleRenderKey = string.Empty;
        }
        else if (text != _bubbleText)
        {
            _bubbleText = text;
            _bubbleRenderKey = string.Empty;
        }

        var elapsed = Math.Max(0, nowMilliseconds - _bubbleFadeStartedAt);
        _bubbleOpacity = (byte)Math.Clamp((elapsed * 255L) / BubbleFadeMs, 0, 255);
        var progress = totalMilliseconds <= 0
            ? 1f
            : Math.Clamp((float)remainingMilliseconds / totalMilliseconds, 0f, 1f);
        var renderKey = $"{_bubbleText}\u001f{(int)Math.Round(progress * 120f)}";
        if (renderKey != _bubbleRenderKey || _bubblePixels is null)
        {
            _bubbleRenderKey = renderKey;
            RenderBubble(_bubbleText, progress);
        }

        if (_bubblePixels is not null && _bubblePixelWidth > 0 && _bubblePixelHeight > 0)
        {
            Width = _bubblePixelWidth;
            Height = _bubblePixelHeight;
            LayeredPresenter.Present(_hwnd, _bubblePixels, Width, Height, _bubbleOpacity);
        }

        Show(true);
    }

    /// <summary>Advances the strip expansion animation; call once per UI tick.</summary>
    public void UpdateEditStrip(bool vertical, long nowMilliseconds)
    {
        if (_isBubble)
        {
            return;
        }

        if (!_stripStateInitialized || vertical != _stripVertical)
        {
            _stripStateInitialized = true;
            _stripVertical = vertical;
            _stripExpansion = _hovered ? 1f : 0f;
            _stripTarget = _stripExpansion;
            _stripAnimationStartedAt = nowMilliseconds;
        }

        var target = _hovered ? 1f : 0f;
        if (Math.Abs(target - _stripTarget) > float.Epsilon)
        {
            _stripTarget = target;
            _stripAnimationStartedAt = nowMilliseconds;
        }

        var elapsed = Math.Clamp(nowMilliseconds - _stripAnimationStartedAt, 0, StripAnimationMs);
        var from = _stripExpansion;
        var progress = StripAnimationMs == 0 ? _stripTarget : (float)elapsed / StripAnimationMs;
        _stripExpansion = from + ((_stripTarget - from) * progress);
        if (elapsed >= StripAnimationMs)
        {
            _stripExpansion = _stripTarget;
        }

        var renderKey = $"{vertical}{(int)Math.Round(_stripExpansion * 100f)}";
        if (renderKey != _stripRenderKey || Width == 0 || Height == 0)
        {
            _stripRenderKey = renderKey;
            RenderStrip(_stripVertical, _stripExpansion);
        }
    }

    private void RenderBubble(string text, float progress)
    {
        using var measureFont = CreateFont(14);
        float textWidth;
        float textHeight;
        using (var probe = Graphics.FromHwnd(0))
        {
            var measured = probe.MeasureString(text, measureFont, new SizeF(BubbleMaxTextWidth, 240f));
            textWidth = measured.Width;
            textHeight = measured.Height;
        }

        var width = (int)Math.Clamp(
            Math.Ceiling(textWidth) + (BubblePaddingX * 2),
            120f,
            BubbleMaxTextWidth + (BubblePaddingX * 2));
        var height = (int)Math.Ceiling(textHeight) + (int)(BubblePaddingY * 2) + (int)ProgressBarHeight + 2;
        var palette = BubblePalette.Current();
        using var bitmap = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
            using var path = RoundedRect(new RectangleF(0.5f, 0.5f, width - 1f, height - 1f), BubbleRadius);
            using var fill = new SolidBrush(palette.Fill);
            graphics.FillPath(fill, path);
            using var border = new Pen(palette.Border, 1f);
            graphics.DrawPath(border, path);

            using var textBrush = new SolidBrush(palette.Text);
            graphics.DrawString(
                text,
                measureFont,
                textBrush,
                new RectangleF(
                    BubblePaddingX,
                    BubblePaddingY,
                    width - (BubblePaddingX * 2),
                    height - (BubblePaddingY * 2) - ProgressBarHeight - 2));

            var barLeft = BubblePaddingX;
            var barTop = height - ProgressBarHeight - 3f;
            var barWidth = width - (BubblePaddingX * 2);
            using var track = new SolidBrush(palette.ProgressTrack);
            graphics.FillRectangle(track, barLeft, barTop, barWidth, ProgressBarHeight);
            using var progressBrush = new SolidBrush(palette.Accent);
            graphics.FillRectangle(
                progressBrush,
                barLeft,
                barTop,
                Math.Max(0f, barWidth * progress),
                ProgressBarHeight);
        }

        BubblePalette.Snapshot(bitmap);
        _bubblePixels = BitmapPixels(bitmap);
        _bubblePixelWidth = bitmap.Width;
        _bubblePixelHeight = bitmap.Height;
    }

    private void RenderStrip(bool vertical, float expansion)
    {
        var length = (int)Math.Round(36f + ((72f - 36f) * Math.Clamp(expansion, 0f, 1f)));
        var width = vertical ? 6 : length;
        var height = vertical ? length : 6;
        var palette = BubblePalette.Current();
        using var bitmap = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            var alpha = (int)Math.Round(90f + (110f * expansion));
            using var fill = new SolidBrush(Color.FromArgb(alpha, palette.Text));
            using var path = RoundedRect(new RectangleF(0.5f, 0.5f, width - 1f, height - 1f), Math.Min(width, height) / 2f);
            graphics.FillPath(fill, path);
        }

        Width = width;
        Height = height;
        Present(bitmap, 220);
    }

    private void Present(Bitmap bitmap, byte opacity)
    {
        LayeredPresenter.Present(_hwnd, BitmapPixels(bitmap), bitmap.Width, bitmap.Height, opacity);
    }

    private static byte[] BitmapPixels(Bitmap bitmap)
    {
        var data = bitmap.LockBits(
            new Rectangle(0, 0, bitmap.Width, bitmap.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppPArgb);
        try
        {
            var pixels = new byte[bitmap.Width * bitmap.Height * 4];
            Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
            return pixels;
        }
        finally
        {
            bitmap.UnlockBits(data);
        }
    }

    private static Font CreateFont(float size)
    {
        try
        {
            return new Font("Microsoft YaHei UI", size, FontStyle.Regular, GraphicsUnit.Point);
        }
        catch (ArgumentException)
        {
            return new Font("Segoe UI", size, FontStyle.Regular, GraphicsUnit.Point);
        }
    }

    private static GraphicsPath RoundedRect(RectangleF bounds, float radius)
    {
        var diameter = radius * 2f;
        var path = new GraphicsPath();
        path.AddArc(bounds.Left, bounds.Top, diameter, diameter, 180f, 90f);
        path.AddArc(bounds.Right - diameter, bounds.Top, diameter, diameter, 270f, 90f);
        path.AddArc(bounds.Right - diameter, bounds.Bottom - diameter, diameter, diameter, 0f, 90f);
        path.AddArc(bounds.Left, bounds.Bottom - diameter, diameter, diameter, 90f, 90f);
        path.CloseFigure();
        return path;
    }

    private static void EnsureClassRegistered()
    {
        lock (ClassLock)
        {
            if (_classRegistered)
            {
                return;
            }

            fixed (char* className = ClassName)
            {
                var wndClass = new NativeWin32.WNDCLASSEXW
                {
                    CbSize = (uint)sizeof(NativeWin32.WNDCLASSEXW),
                    LpfnWndProc = (nint)(delegate* unmanaged[Stdcall]<nint, uint, nint, nint, nint>)&WndProc,
                    HInstance = NativeWin32.GetModuleHandleW(null),
                    HCursor = ArrowCursor,
                    HIcon = WindowIcon.Big,
                    HIconSm = WindowIcon.Small,
                    LpszClassName = className,
                };
                if (NativeWin32.RegisterClassExW(&wndClass) == 0)
                {
                    throw new InvalidOperationException("Cannot register the overlay window class.");
                }
            }

            _classRegistered = true;
        }
    }

    private void SetHovered(bool hovered)
    {
        if (_hovered == hovered)
        {
            return;
        }

        _hovered = hovered;
        HoverChanged?.Invoke(hovered);
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static nint WndProc(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        var data = NativeWin32.GetWindowLongPtrW(hWnd, NativeWin32.GWLP_USERDATA);
        if (data != 0 && GCHandle.FromIntPtr(data).Target is OverlayWindow self)
        {
            if (msg == NativeWin32.WM_MOUSEMOVE)
            {
                if (!self._trackingMouse)
                {
                    var track = new NativeWin32.TRACKMOUSEEVENT
                    {
                        CbSize = (uint)sizeof(NativeWin32.TRACKMOUSEEVENT),
                        DwFlags = NativeWin32.TME_LEAVE,
                        HwndTrack = hWnd,
                    };
                    _ = NativeWin32.TrackMouseEvent(&track);
                    self._trackingMouse = true;
                }

                self.SetHovered(true);
                return 0;
            }

            if (msg == NativeWin32.WM_MOUSELEAVE)
            {
                self._trackingMouse = false;
                self.SetHovered(false);
                return 0;
            }

            if (msg == NativeWin32.WM_LBUTTONUP)
            {
                self.Clicked?.Invoke();
                return 0;
            }

            if (msg == NativeWin32.WM_SETCURSOR &&
                (lParam.ToInt64() & 0xFFFF) == NativeWin32.HTCLIENT)
            {
                _ = NativeWin32.SetCursor(ArrowCursor);
                return 1;
            }
        }

        return NativeWin32.DefWindowProcW(hWnd, msg, wParam, lParam);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        if (_hwnd != 0)
        {
            _ = NativeWin32.DestroyWindow(_hwnd);
            _hwnd = 0;
        }

        if (_selfHandle.IsAllocated)
        {
            _selfHandle.Free();
        }
    }
}
