using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class GazeStabilizerTests
{
    [Fact]
    public void QuantizesTheOfficialCardinalDirections()
    {
        Assert.Equal(0, GazeStabilizer.Quantize(0, -1));
        Assert.Equal(4, GazeStabilizer.Quantize(1, 0));
        Assert.Equal(8, GazeStabilizer.Quantize(0, 1));
        Assert.Equal(12, GazeStabilizer.Quantize(-1, 0));
    }

    [Fact]
    public void UnitVectorRoundTripsThroughTheQuantizer()
    {
        for (var direction = 0; direction < 16; direction++)
        {
            var (x, y) = GazeStabilizer.UnitVector(direction);
            Assert.Equal(direction, GazeStabilizer.Quantize(x, y));
        }
    }

    [Fact]
    public void SmallJitterNearTheBoundaryKeepsTheHeldDirection()
    {
        var stabilizer = new GazeStabilizer();
        Assert.Equal(4, stabilizer.Update(1, 0)); // right / 90°

        // 75 degrees is already in the raw 67.5-degree bucket, but only 15
        // degrees off the held centre: the widened band must keep 90 degrees.
        var jitterRadians = 75.0 * Math.PI / 180.0;
        Assert.Equal(
            4,
            stabilizer.Update(Math.Sin(jitterRadians) * 100, -Math.Cos(jitterRadians) * 100));

        // 65 degrees is 25 degrees away, a deliberate move that must switch.
        var movedRadians = 65.0 * Math.PI / 180.0;
        Assert.NotEqual(
            4,
            stabilizer.Update(Math.Sin(movedRadians) * 100, -Math.Cos(movedRadians) * 100));
    }

    [Fact]
    public void MovementBelowTwoPixelsDoesNotRetarget()
    {
        var stabilizer = new GazeStabilizer();
        Assert.Equal(0, stabilizer.Update(0, -100));
        // 1.5 px sideways: below the movement floor, so the held direction wins.
        Assert.Equal(0, stabilizer.Update(1.5, -100));
    }

    [Fact]
    public void ResetClearsTheHeldDirection()
    {
        var stabilizer = new GazeStabilizer();
        stabilizer.Update(0, -100);
        stabilizer.Reset();
        Assert.Equal(-1, stabilizer.Direction);
        Assert.Equal(4, stabilizer.Update(100, 0));
    }
}
