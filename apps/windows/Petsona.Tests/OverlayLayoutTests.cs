using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class OverlayLayoutTests
{
    [Fact]
    public void ClampKeepsAWindowInsideANegativeCoordinateWorkArea()
    {
        var work = new PixelRect(-1920, -200, 0, 880);
        var clamped = OverlayLayout.ClampToWorkArea(PixelRect.FromBounds(-2200, -500, 120, 100), work);

        Assert.Equal(work.Left, clamped.Left);
        Assert.Equal(work.Top, clamped.Top);
        Assert.True(clamped.Right <= work.Right);
        Assert.True(clamped.Bottom <= work.Bottom);
    }

    [Fact]
    public void BottomIsPreferredWhenTheComposerFits()
    {
        var work = new PixelRect(0, 0, 1920, 1040);
        var pet = PixelRect.FromBounds(800, 400, 120, 160);

        var side = OverlayLayout.ChooseSide(
            pet,
            work,
            OverlayLayout.ComposerDesiredWidth,
            OverlayLayout.ComposerHeight);

        Assert.Equal(OverlaySide.Bottom, side);
    }

    [Fact]
    public void NearbyTaskbarMovesTheComposerToTheLargerSide()
    {
        var work = new PixelRect(0, 0, 1920, 1040);
        var nearBottomLeft = PixelRect.FromBounds(40, 900, 120, 130);
        var nearBottomRight = PixelRect.FromBounds(1700, 900, 120, 130);

        Assert.Equal(
            OverlaySide.Right,
            OverlayLayout.ChooseSide(nearBottomLeft, work, OverlayLayout.ComposerDesiredWidth, OverlayLayout.ComposerHeight));
        Assert.Equal(
            OverlaySide.Left,
            OverlayLayout.ChooseSide(nearBottomRight, work, OverlayLayout.ComposerDesiredWidth, OverlayLayout.ComposerHeight));
    }

    [Fact]
    public void CurrentSideHasHysteresisBeforeReturningBottom()
    {
        var work = new PixelRect(0, 0, 1920, 1040);
        var justEnoughForBottom = PixelRect.FromBounds(800, 864, 120, 100);
        var comfortablyAboveBottom = PixelRect.FromBounds(800, 820, 120, 100);

        Assert.Equal(
            OverlaySide.Right,
            OverlayLayout.ChooseSide(
                justEnoughForBottom,
                work,
                OverlayLayout.ComposerDesiredWidth,
                OverlayLayout.ComposerHeight,
                OverlaySide.Right));
        Assert.Equal(
            OverlaySide.Right,
            OverlayLayout.ChooseSide(
                new PixelRect(800, 880, 920, 980),
                work,
                OverlayLayout.ComposerDesiredWidth,
                OverlayLayout.ComposerHeight,
                OverlaySide.Right));
        Assert.Equal(
            OverlaySide.Bottom,
            OverlayLayout.ChooseSide(
                comfortablyAboveBottom,
                work,
                OverlayLayout.ComposerDesiredWidth,
                OverlayLayout.ComposerHeight,
                OverlaySide.Right));
    }

    [Fact]
    public void BubbleFlipsBelowThePetWhenTheTopHasNoRoom()
    {
        var work = new PixelRect(0, 0, 800, 600);
        var pet = PixelRect.FromBounds(40, 12, 100, 120);

        var bubble = OverlayLayout.PositionBubble(pet, work, 220, 80);

        Assert.Equal(pet.Bottom + 10, bubble.Top);
        Assert.True(bubble.Left >= work.Left);
        Assert.True(bubble.Right <= work.Right);
        Assert.True(bubble.Bottom <= work.Bottom);
    }

    [Fact]
    public void SidePanelWidthShrinksToTheMinimumWhenSpaceIsTight()
    {
        var work = new PixelRect(0, 0, 700, 600);
        var pet = PixelRect.FromBounds(200, 240, 100, 120);

        var width = OverlayLayout.SidePanelWidth(
            pet,
            work,
            OverlaySide.Right,
            OverlayLayout.ComposerDesiredWidth);
        var panel = OverlayLayout.PositionPanel(
            pet,
            work,
            width,
            OverlayLayout.ComposerHeight,
            OverlaySide.Right);

        Assert.InRange(width, OverlayLayout.ComposerMinWidth, OverlayLayout.ComposerDesiredWidth);
        Assert.True(panel.Right <= work.Right);
    }
}
