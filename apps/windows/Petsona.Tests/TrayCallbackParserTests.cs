using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class TrayCallbackParserTests
{
    private const uint IconId = 1;
    private const uint WmContextMenu = 0x007B;
    private const uint WmRButtonUp = 0x0205;
    private const uint WmLButtonUp = 0x0202;

    [Fact]
    public void Version4RightClickArrivesAsContextMenuWithScreenCoordinates()
    {
        var notification = TrayCallbackParser.Parse(
            MakeWordPack(1234, 567),
            MakeWordPack(IconId, WmContextMenu),
            IconId);

        Assert.True(notification.Version4);
        Assert.Equal(WmContextMenu, notification.Event);
        Assert.True(TrayCallbackParser.OpensMenu(notification.Event));
        Assert.Equal(1234, notification.X);
        Assert.Equal(567, notification.Y);
    }

    [Fact]
    public void Version4LeftClickArrivesAsSelect()
    {
        var notification = TrayCallbackParser.Parse(
            0,
            MakeWordPack(IconId, TrayCallbackParser.NinSelect),
            IconId);

        Assert.True(notification.Version4);
        Assert.Equal(TrayCallbackParser.NinSelect, notification.Event);
        Assert.True(TrayCallbackParser.OpensMenu(notification.Event));
    }

    [Fact]
    public void LegacyLayoutKeepsTheMouseEventInTheLowWord()
    {
        var notification = TrayCallbackParser.Parse(0, MakeWordPack(WmRButtonUp, IconId), IconId);

        Assert.False(notification.Version4);
        Assert.Equal(WmRButtonUp, notification.Event);
        Assert.True(TrayCallbackParser.OpensMenu(notification.Event));
    }

    [Fact]
    public void MouseMoveAndDoubleClickDoNotOpenTheMenu()
    {
        var move = TrayCallbackParser.Parse(0, MakeWordPack(IconId, TrayCallbackParser.WmMouseMove), IconId);
        var doubleClick = TrayCallbackParser.Parse(0, MakeWordPack(IconId, TrayCallbackParser.WmLButtonDoubleClick), IconId);

        Assert.False(TrayCallbackParser.OpensMenu(move.Event));
        Assert.False(TrayCallbackParser.OpensMenu(doubleClick.Event));
        Assert.True(TrayCallbackParser.OpensMenu(WmLButtonUp));
    }

    private static nint MakeWordPack(uint low, uint high) =>
        (nint)((high << 16) | (low & 0xFFFF));
}
