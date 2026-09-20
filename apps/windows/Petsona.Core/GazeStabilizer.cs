namespace Petsona.Core;

/// <summary>
/// Maps a cursor vector to the 16 official Codex look directions and holds it
/// with angular hysteresis. The desktop app quantizes the angle to 22.5-degree
/// steps with only a 1 px dead zone; that raw mapping flips the pose when the
/// cursor jitters near a boundary. The stabilizer keeps the current direction
/// until the cursor has both moved at least <see cref="MinMovementPixels"/>
/// and left a wider band around that direction.
/// </summary>
public sealed class GazeStabilizer
{
    public const double StepDegrees = 22.5;
    public const double HysteresisDegrees = 7.0;
    public const double MinMovementPixels = 2.0;

    private int _direction = -1;
    private double _lastDx;
    private double _lastDy;

    /// <summary>Currently held direction (0 = up, clockwise), or -1 before the first update.</summary>
    public int Direction => _direction;

    public static double AngleDegrees(double dx, double dy) =>
        ((Math.Atan2(dx, -dy) * 180.0 / Math.PI) + 360.0) % 360.0;

    public static int Quantize(double dx, double dy) =>
        (int)(Math.Round(AngleDegrees(dx, dy) / StepDegrees) % 16.0);

    /// <summary>Unit vector for a direction; feeding it back through the engine reproduces the same pose.</summary>
    public static (double X, double Y) UnitVector(int direction)
    {
        var normalized = ((direction % 16) + 16) % 16;
        var radians = normalized * StepDegrees * Math.PI / 180.0;
        return (Math.Sin(radians), -Math.Cos(radians));
    }

    /// <summary>
    /// Update with a cursor vector relative to the pet centre. Returns the
    /// direction the caller should send to the engine every poll; the value is
    /// stable until the hysteresis band is crossed.
    /// </summary>
    public int Update(double dx, double dy)
    {
        var raw = Quantize(dx, dy);
        if (_direction < 0)
        {
            _direction = raw;
            _lastDx = dx;
            _lastDy = dy;
            return _direction;
        }

        var moveX = dx - _lastDx;
        var moveY = dy - _lastDy;
        if ((moveX * moveX) + (moveY * moveY) < MinMovementPixels * MinMovementPixels)
        {
            return _direction;
        }

        _lastDx = dx;
        _lastDy = dy;
        var delta = NormalizeSigned(AngleDegrees(dx, dy) - (_direction * StepDegrees));
        if (Math.Abs(delta) > (StepDegrees / 2.0) + HysteresisDegrees)
        {
            _direction = raw;
        }

        return _direction;
    }

    public void Reset()
    {
        _direction = -1;
        _lastDx = 0;
        _lastDy = 0;
    }

    private static double NormalizeSigned(double degrees)
    {
        var value = degrees % 360.0;
        if (value > 180.0)
        {
            value -= 360.0;
        }
        else if (value < -180.0)
        {
            value += 360.0;
        }

        return value;
    }
}
