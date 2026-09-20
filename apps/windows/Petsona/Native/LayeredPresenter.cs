using System.Runtime.InteropServices;

namespace Petsona.Native;

/// <summary>Presents premultiplied BGRA pixels on a layered window.</summary>
internal static unsafe class LayeredPresenter
{
    public static void Present(nint hwnd, byte[] pixels, int width, int height)
    {
        var screenDc = NativeWin32.GetDC(0);
        var memDc = NativeWin32.CreateCompatibleDC(screenDc);
        nint dib = 0;
        try
        {
            void* bits = null;
            var info = new NativeWin32.BITMAPINFO
            {
                Header = new NativeWin32.BITMAPINFOHEADER
                {
                    BiSize = (uint)sizeof(NativeWin32.BITMAPINFOHEADER),
                    BiWidth = width,
                    BiHeight = -height,
                    BiPlanes = 1,
                    BiBitCount = 32,
                    BiCompression = NativeWin32.BI_RGB,
                },
            };
            dib = NativeWin32.CreateDIBSection(memDc, &info, NativeWin32.DIB_RGB_COLORS, &bits, 0, 0);
            if (dib == 0 || bits is null)
            {
                return;
            }

            _ = NativeWin32.SelectObject(memDc, dib);
            Marshal.Copy(pixels, 0, (nint)bits, pixels.Length);

            var size = new NativeWin32.SIZE { Cx = width, Cy = height };
            var source = new NativeWin32.POINT { X = 0, Y = 0 };
            var blend = new NativeWin32.BLENDFUNCTION
            {
                BlendOp = 0,
                BlendFlags = 0,
                SourceConstantAlpha = 255,
                AlphaFormat = 1,
            };
            _ = NativeWin32.UpdateLayeredWindow(hwnd, screenDc, null, &size, memDc, &source, 0, &blend, NativeWin32.ULW_ALPHA);
        }
        finally
        {
            if (dib != 0)
            {
                _ = NativeWin32.DeleteObject(dib);
            }

            _ = NativeWin32.DeleteDC(memDc);
            _ = NativeWin32.ReleaseDC(0, screenDc);
        }
    }
}
