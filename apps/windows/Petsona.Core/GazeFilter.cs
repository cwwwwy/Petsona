namespace Petsona.Core;

/// <summary>
/// Cursor-gaze ellipse with hysteresis and a forward dead zone. The outer
/// reach is the doubled user-requested range (enter 80% / release 100% of the
/// short side). The inner dead zone was widened from 22% to 35%: when the
/// cursor is very close to the pet, a few pixels of movement change the look
/// angle dramatically, so this personal space keeps the pet neutral instead
/// of flipping look frames.
/// </summary>
public static class GazeFilter
{
    public static bool IsInside(double dx, double dy, double width, double height, bool wasActive)
    {
        var shortSide = Math.Min(width, height);
        var margin = shortSide * (wasActive ? 1.00 : 0.80);
        var radiusX = (width * 0.5) + margin;
        var radiusY = (height * 0.5) + margin;
        if (radiusX <= 0 || radiusY <= 0)
        {
            return false;
        }

        return ((dx * dx) / (radiusX * radiusX)) + ((dy * dy) / (radiusY * radiusY)) <= 1.0;
    }

    public static bool IsInDeadZone(double dx, double dy, double width, double height)
    {
        var deadZone = Math.Min(width, height) * 0.35;
        return Math.Sqrt((dx * dx) + (dy * dy)) <= deadZone;
    }
}
