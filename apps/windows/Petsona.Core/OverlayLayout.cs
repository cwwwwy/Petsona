namespace Petsona.Core;

public enum OverlaySide
{
    Bottom,
    Left,
    Right,
}

/// <summary>
/// Pure physical-pixel layout rules for the pet-relative overlays. Keeping
/// these calculations out of the WinUI view makes the edge cases testable
/// without a desktop session.
/// </summary>
public static class OverlayLayout
{
    public const int Gap = 12;
    public const int EdgeMargin = 8;
    public const int ComposerHeight = 56;
    public const int ComposerDesiredWidth = 360;
    public const int ComposerMinWidth = 280;
    public const int StripThickness = 6;
    public const int StripLength = 36;
    public const int StripExpandedLength = 72;

    public static PixelRect ClampToWorkArea(PixelRect rect, PixelRect work)
    {
        var width = Math.Min(rect.Width, work.Width);
        var height = Math.Min(rect.Height, work.Height);
        var left = rect.Left;
        var top = rect.Top;

        if (width <= 0 || height <= 0 || work.Width <= 0 || work.Height <= 0)
        {
            return rect;
        }

        left = Math.Clamp(left, work.Left, work.Right - width);
        top = Math.Clamp(top, work.Top, work.Bottom - height);
        return PixelRect.FromBounds(left, top, width, height);
    }

    public static OverlaySide ChooseSide(
        PixelRect pet,
        PixelRect work,
        int panelWidth,
        int panelHeight,
        OverlaySide? current = null)
    {
        var below = work.Bottom - pet.Bottom - Gap - EdgeMargin;
        var right = work.Right - pet.Right - Gap - EdgeMargin;
        var left = pet.Left - work.Left - Gap - EdgeMargin;
        var bottomFits = below >= panelHeight;

        if (current == OverlaySide.Bottom)
        {
            return bottomFits ? OverlaySide.Bottom : (right >= left ? OverlaySide.Right : OverlaySide.Left);
        }

        if (current is OverlaySide.Left or OverlaySide.Right)
        {
            // Hysteresis: require a little more room to return to the bottom
            // so a pet near the threshold does not flip sides continuously.
            if (below >= panelHeight + 16)
            {
                return OverlaySide.Bottom;
            }

            var currentSpace = current == OverlaySide.Right ? right : left;
            var otherSpace = current == OverlaySide.Right ? left : right;
            return currentSpace >= ComposerMinWidth || currentSpace >= otherSpace
                ? current.Value
                : current == OverlaySide.Right ? OverlaySide.Left : OverlaySide.Right;
        }

        if (bottomFits)
        {
            return OverlaySide.Bottom;
        }

        return right >= left ? OverlaySide.Right : OverlaySide.Left;
    }

    public static int SidePanelWidth(PixelRect pet, PixelRect work, OverlaySide side, int desiredWidth)
    {
        var space = side == OverlaySide.Right
            ? work.Right - pet.Right - Gap - EdgeMargin
            : pet.Left - work.Left - Gap - EdgeMargin;
        if (space < ComposerMinWidth)
        {
            return ComposerMinWidth;
        }

        return Math.Clamp(desiredWidth, ComposerMinWidth, Math.Max(ComposerMinWidth, space));
    }

    public static PixelRect PositionPanel(
        PixelRect pet,
        PixelRect work,
        int width,
        int height,
        OverlaySide side)
    {
        var x = side switch
        {
            OverlaySide.Left => pet.Left - width - Gap,
            OverlaySide.Right => pet.Right + Gap,
            _ => pet.Left + ((pet.Width - width) / 2),
        };
        var y = side == OverlaySide.Bottom
            ? pet.Bottom + Gap
            : pet.Bottom - height + 6;
        return ClampToWorkArea(PixelRect.FromBounds(x, y, width, height), work);
    }

    public static PixelRect PositionStrip(
        PixelRect pet,
        PixelRect work,
        OverlaySide side,
        bool expanded,
        bool vertical)
    {
        var length = expanded ? StripExpandedLength : StripLength;
        var width = vertical ? StripThickness : length;
        var height = vertical ? length : StripThickness;
        var centerY = pet.Bottom - 22;
        var x = side switch
        {
            OverlaySide.Left => pet.Left - width - EdgeMargin,
            OverlaySide.Right => pet.Right + EdgeMargin,
            _ => pet.Left + ((pet.Width - width) / 2),
        };
        var y = side == OverlaySide.Bottom
            ? pet.Bottom + EdgeMargin
            : centerY - (height / 2);
        return ClampToWorkArea(PixelRect.FromBounds(x, y, width, height), work);
    }

    public static PixelRect PositionBubble(PixelRect pet, PixelRect work, int width, int height)
    {
        var x = pet.Left + ((pet.Width - width) / 2);
        var y = pet.Top - height - 10;
        if (y < work.Top + EdgeMargin)
        {
            y = pet.Bottom + 10;
        }

        return ClampToWorkArea(PixelRect.FromBounds(x, y, width, height), work);
    }
}
