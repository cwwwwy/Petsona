using Petsona.Core;
using Xunit;

namespace Petsona.Tests;

public sealed class FrameCadenceTests
{
    [Fact]
    public void IdleFramesNeverSleepPastTheGazePoll()
    {
        Assert.Equal(
            FrameCadence.IdlePollMilliseconds,
            FrameCadence.NextDelayMilliseconds(320, hasPet: true, petVisible: true, manuallyHidden: false, gazeActive: false));
    }

    [Fact]
    public void ActiveGazeUsesTheFastPollEvenWhileTheEngineHoldsForASecond()
    {
        Assert.Equal(
            FrameCadence.ActiveGazePollMilliseconds,
            FrameCadence.NextDelayMilliseconds(1000, hasPet: true, petVisible: true, manuallyHidden: false, gazeActive: true));
    }

    [Fact]
    public void EngineFrameDeadlinesShorterThanThePollWin()
    {
        Assert.Equal(
            12u,
            FrameCadence.NextDelayMilliseconds(12, hasPet: true, petVisible: true, manuallyHidden: false, gazeActive: false));
        Assert.Equal(
            16u,
            FrameCadence.NextDelayMilliseconds(0, hasPet: true, petVisible: true, manuallyHidden: false, gazeActive: false));
    }

    [Fact]
    public void HiddenOrMissingPetsFallBackToTheSlowHeartbeat()
    {
        Assert.Equal(1000u, FrameCadence.NextDelayMilliseconds(16, hasPet: false, petVisible: true, manuallyHidden: false, gazeActive: false));
        Assert.Equal(1000u, FrameCadence.NextDelayMilliseconds(16, hasPet: true, petVisible: false, manuallyHidden: false, gazeActive: false));
        Assert.Equal(1000u, FrameCadence.NextDelayMilliseconds(16, hasPet: true, petVisible: true, manuallyHidden: true, gazeActive: false));
    }
}
