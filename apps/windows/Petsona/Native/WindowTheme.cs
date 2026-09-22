using Microsoft.UI.Xaml;

namespace Petsona.Native;

/// <summary>
/// Keeps the Win32/DWM title bar in step with the app theme.
///
/// A WinUI 3 unpackaged window paints its non-client area from the system
/// setting, so forcing the client area dark (diagnostics) or switching Windows
/// to dark mode left a light title bar behind (acceptance report 2026-09-22).
/// DWM exposes the switch as <c>DWMWA_USE_IMMERSIVE_DARK_MODE</c>.
/// </summary>
internal static class WindowTheme
{
    private const uint DwmwaUseImmersiveDarkMode = 20;
    private const uint DwmwaUseImmersiveDarkModeBefore20H1 = 19;
    private const uint DwmwaBorderColor = 34;

    internal static void RemoveBorder(nint hwnd)
    {
        if (hwnd == 0)
        {
            return;
        }

        unsafe
        {
            var none = unchecked((int)0xFFFFFFFE);
            _ = NativeWin32.DwmSetWindowAttribute(hwnd, DwmwaBorderColor, &none, sizeof(int));
        }
    }

    internal static void Apply(nint hwnd, ElementTheme theme)
    {
        if (hwnd == 0)
        {
            return;
        }

        var effective = theme switch
        {
            ElementTheme.Dark => 1,
            ElementTheme.Light => 0,
            // "Default" means "follow Windows": the framework already resolved
            // it on the application object, which tracks the system setting.
            _ => Application.Current.RequestedTheme == ApplicationTheme.Dark ? 1 : 0,
        };

        unsafe
        {
            // Attribute 20 is the Windows 10 20H1+ contract; older builds only
            // understand 19, so try both and ignore failures.
            _ = NativeWin32.DwmSetWindowAttribute(hwnd, DwmwaUseImmersiveDarkMode, &effective, sizeof(int));
            _ = NativeWin32.DwmSetWindowAttribute(hwnd, DwmwaUseImmersiveDarkModeBefore20H1, &effective, sizeof(int));
        }
    }
}
