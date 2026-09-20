namespace Petsona.Core;

/// <summary>
/// Decodes a <c>Shell_NotifyIcon</c> callback. With NOTIFYICON_VERSION_4 the
/// low word of lParam is the icon id and the high word is the event, so a
/// right-click arrives as <c>WM_CONTEXTMENU</c> (with screen coordinates in
/// wParam) instead of <c>WM_RBUTTONUP</c>. Older shells use the inverse word
/// order and keep the mouse event in the low word.
/// </summary>
public static class TrayCallbackParser
{
    public const uint WmContextMenu = 0x007B;
    public const uint WmMouseMove = 0x0200;
    public const uint WmLButtonDown = 0x0201;
    public const uint WmLButtonUp = 0x0202;
    public const uint WmLButtonDoubleClick = 0x0203;
    public const uint WmRButtonDown = 0x0204;
    public const uint WmRButtonUp = 0x0205;
    public const uint NinSelect = 0x0400;
    public const uint NinKeySelect = 0x0401;

    public readonly record struct Notification(uint Event, int X, int Y, bool Version4);

    public static Notification Parse(nint wParam, nint lParam, uint iconId)
    {
        var raw = lParam.ToInt64();
        var low = (uint)(raw & 0xFFFF);
        var high = (uint)((raw >> 16) & 0xFFFF);
        var version4 = low == iconId && high != 0;
        var code = version4 ? high : low;

        var x = 0;
        var y = 0;
        if (code == WmContextMenu)
        {
            var packed = wParam.ToInt64();
            x = (short)(packed & 0xFFFF);
            y = (short)((packed >> 16) & 0xFFFF);
        }

        return new Notification(code, x, y, version4);
    }

    public static bool OpensMenu(uint code) =>
        code is WmContextMenu or WmRButtonUp or WmLButtonUp or NinSelect or NinKeySelect;
}
