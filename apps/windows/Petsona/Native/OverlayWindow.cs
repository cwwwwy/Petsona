using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Petsona.Native;

/// <summary>
/// Non-activating layered window used for the speech bubble (above the pet)
/// and the edit button (below the pet). The macOS shadow-to-button animation
/// stays deferred; the button itself is always rendered in place.
/// </summary>
internal sealed unsafe class OverlayWindow : IDisposable
{
    private const string ClassName = "PetsonaOverlayWindow";
    private const int ButtonSize = 40;
    private static readonly object ClassLock = new();
    private static bool _classRegistered;
    private static readonly nint ArrowCursor =
        NativeWin32.LoadCursorW(0, (nint)NativeWin32.IDC_ARROW);

    private readonly bool _isBubble;
    private GCHandle _selfHandle;
    private nint _hwnd;
    private bool _visible;
    private string _renderedKey = string.Empty;
    private bool _disposed;

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

    public int Width { get; private set; }

    public int Height { get; private set; }

    /// <summary>Render the bubble text or the edit button glyph once.</summary>
    public void Render(string text)
    {
        var key = _isBubble ? text : "edit";
        if (key == _renderedKey && Width > 0)
        {
            return;
        }

        _renderedKey = key;
        if (_isBubble)
        {
            RenderBubble(text);
        }
        else
        {
            RenderButton();
        }
    }

    public void MoveTo(int x, int y)
    {
        NativeWin32.SetWindowPos(
            _hwnd, 0, x, y, 0, 0,
            NativeWin32.SWP_NOSIZE | NativeWin32.SWP_NOACTIVATE | 0x0004);
    }

    public void Show(bool visible)
    {
        if (visible == _visible)
        {
            return;
        }

        _visible = visible;
        _ = NativeWin32.ShowWindow(_hwnd, visible ? NativeWin32.SW_SHOWNOACTIVATE : NativeWin32.SW_HIDE);
    }

    private void RenderBubble(string text)
    {
        using var measureFont = CreateFont(14);
        float textWidth;
        float textHeight;
        using (var probe = Graphics.FromHwnd(0))
        {
            var measured = probe.MeasureString(text, measureFont, new SizeF(250f, 240f));
            textWidth = measured.Width;
            textHeight = measured.Height;
        }

        var width = (int)Math.Clamp(Math.Ceiling(textWidth) + 28, 120, 280);
        var height = (int)Math.Ceiling(textHeight) + 22;
        using var bitmap = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
            using var path = RoundedRect(new RectangleF(0.5f, 0.5f, width - 1f, height - 1f), 12f);
            using var fill = new SolidBrush(Color.FromArgb(242, 255, 255, 255));
            graphics.FillPath(fill, path);
            using var border = new Pen(Color.FromArgb(70, 0, 0, 0), 1f);
            graphics.DrawPath(border, path);
            using var textBrush = new SolidBrush(Color.FromArgb(32, 32, 32));
            graphics.DrawString(text, measureFont, textBrush, new RectangleF(14f, 11f, width - 28f, height - 22f));
        }

        Width = width;
        Height = height;
        Present(bitmap);
    }

    private void RenderButton()
    {
        using var bitmap = new Bitmap(ButtonSize, ButtonSize, PixelFormat.Format32bppPArgb);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
            using var circle = new GraphicsPath();
            circle.AddEllipse(0.5f, 0.5f, ButtonSize - 1f, ButtonSize - 1f);
            using var fill = new SolidBrush(Color.FromArgb(240, 255, 255, 255));
            graphics.FillPath(fill, circle);
            using var border = new Pen(Color.FromArgb(70, 0, 0, 0), 1f);
            graphics.DrawPath(border, circle);
            using var glyphFont = new Font("Segoe UI Symbol", 15f, FontStyle.Regular, GraphicsUnit.Point);
            using var glyphBrush = new SolidBrush(Color.FromArgb(40, 40, 40));
            using var format = new StringFormat
            {
                Alignment = StringAlignment.Center,
                LineAlignment = StringAlignment.Center,
            };
            graphics.DrawString("✎", glyphFont, glyphBrush, new RectangleF(0f, 0f, ButtonSize, ButtonSize), format);
        }

        Width = ButtonSize;
        Height = ButtonSize;
        Present(bitmap);
    }

    private void Present(Bitmap bitmap)
    {
        var data = bitmap.LockBits(
            new Rectangle(0, 0, bitmap.Width, bitmap.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppPArgb);
        try
        {
            var pixels = new byte[bitmap.Width * bitmap.Height * 4];
            Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
            LayeredPresenter.Present(_hwnd, pixels, bitmap.Width, bitmap.Height);
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

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static nint WndProc(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        var data = NativeWin32.GetWindowLongPtrW(hWnd, NativeWin32.GWLP_USERDATA);
        if (data != 0 && GCHandle.FromIntPtr(data).Target is OverlayWindow self)
        {
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
