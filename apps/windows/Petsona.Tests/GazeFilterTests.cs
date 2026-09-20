using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class GazeFilterTests
{
    private const double Width = 192;
    private const double Height = 208;

    [Fact]
    public void DeadZoneCoversTheImmediateFront()
    {
        var deadZone = Math.Min(Width, Height) * 0.35;

        Assert.True(GazeFilter.IsInDeadZone(deadZone - 1, 0, Width, Height));
        Assert.False(GazeFilter.IsInDeadZone(deadZone + 1, 0, Width, Height));
    }

    [Fact]
    public void HysteresisKeepsTheReleaseRadiusLargerThanTheEnterRadius()
    {
        // Enter radius X = 96 + 153.6 = 249.6; release radius X = 96 + 192 = 288.
        Assert.False(GazeFilter.IsInside(260, 0, Width, Height, wasActive: false));
        Assert.True(GazeFilter.IsInside(260, 0, Width, Height, wasActive: true));
    }

    [Fact]
    public void DoubledRangeCoversPointsAboveThePet()
    {
        // The user asked for twice the reach; a point 220 px above the centre
        // was outside the old 181 px radius and is inside the new 258 px one.
        Assert.True(GazeFilter.IsInside(0, -220, Width, Height, wasActive: false));
        Assert.False(GazeFilter.IsInside(0, -300, Width, Height, wasActive: false));
    }

    [Fact]
    public void PointsOutsideTheEllipseDoNotTrigger()
    {
        Assert.False(GazeFilter.IsInside(500, 0, Width, Height, wasActive: true));
        Assert.False(GazeFilter.IsInside(0, 500, Width, Height, wasActive: true));
    }

    [Fact]
    public void PointsInsideTheEllipseTrigger()
    {
        Assert.True(GazeFilter.IsInside(100, 40, Width, Height, wasActive: false));
    }
}
