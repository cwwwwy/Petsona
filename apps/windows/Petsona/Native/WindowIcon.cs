using System.Drawing;

namespace Petsona.Native;

/// <summary>
/// The application icon embedded in the executable, handed to every top-level
/// window. Windows shows a blank/generic icon for windows whose class declares
/// no icon, which is what the taskbar and the settings window looked like
/// before this helper (bug report 2026-09-22).
///
/// The <see cref="Icon"/> instances are kept alive as statics on purpose: the
/// HICON is owned by the managed object, so disposing it would leave the window
/// with a dangling handle.
/// </summary>
internal static class WindowIcon
{
    private static Icon? _big;
    private static Icon? _small;

    internal static nint Big => Handle(ref _big, 32);

    internal static nint Small => Handle(ref _small, 16);

    /// <summary>Applies both icon sizes to one window.</summary>
    internal static void Apply(nint hwnd)
    {
        if (hwnd == 0)
        {
            return;
        }

        var big = Big;
        if (big != 0)
        {
            _ = NativeWin32.SendMessageW(hwnd, NativeWin32.WM_SETICON, NativeWin32.ICON_BIG, big);
        }

        var small = Small;
        if (small != 0)
        {
            _ = NativeWin32.SendMessageW(hwnd, NativeWin32.WM_SETICON, NativeWin32.ICON_SMALL, small);
        }
    }

    private static nint Handle(ref Icon? slot, int size)
    {
        slot ??= Load(size);
        return slot.Handle;
    }

    private static Icon Load(int size)
    {
        try
        {
            var path = Environment.ProcessPath;
            if (!string.IsNullOrEmpty(path))
            {
                using var extracted = Icon.ExtractAssociatedIcon(path);
                if (extracted is not null)
                {
                    return new Icon(extracted, new Size(size, size));
                }
            }
        }
        catch (Exception)
        {
            // A missing icon must never stop the app from starting.
        }

        return SystemIcons.Application;
    }
}
