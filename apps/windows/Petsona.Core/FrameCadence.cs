namespace Petsona.Core;

/// <summary>
/// Windows frame scheduling cadence. Mirrors the macOS tick loop: the native
/// UI also polls the global cursor, so while a pet is on screen it never
/// sleeps longer than the gaze interval even when the engine's next frame is
/// far away (idle holds can be 280-320 ms and a held gaze reports 1 s).
/// </summary>
public static class FrameCadence
{
    public const uint IdlePollMilliseconds = 33;
    public const uint ActiveGazePollMilliseconds = 16;

    public static uint NextDelayMilliseconds(
        uint nextFrameMilliseconds,
        bool hasPet,
        bool petVisible,
        bool manuallyHidden,
        bool gazeActive)
    {
        if (!hasPet || !petVisible || manuallyHidden)
        {
            return 1000;
        }

        var frameDelay = nextFrameMilliseconds == 0 ? 16u : nextFrameMilliseconds;
        var pollDelay = gazeActive ? ActiveGazePollMilliseconds : IdlePollMilliseconds;
        return Math.Clamp(Math.Min(frameDelay, pollDelay), 10u, 60_000u);
    }
}
