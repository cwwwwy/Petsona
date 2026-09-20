using Microsoft.UI.Dispatching;

namespace Petsona.Rendering;

/// <summary>
/// Schedules one engine tick per runtime deadline on the UI thread. The
/// callback returns the next delay so empty libraries or hidden pets stay at
/// a low wake-up rate.
/// </summary>
internal sealed class FrameScheduler : IDisposable
{
    private readonly DispatcherQueueTimer _timer;
    private readonly Func<uint> _tick;
    private bool _disposed;

    public FrameScheduler(DispatcherQueue dispatcher, Func<uint> tick)
    {
        _tick = tick;
        _timer = dispatcher.CreateTimer();
        _timer.IsRepeating = false;
        _timer.Tick += OnTick;
    }

    public void Start()
    {
        Schedule(16);
    }

    /// <summary>Re-arm the pending tick at the shortest interval.</summary>
    public void Wake()
    {
        Schedule(10);
    }

    private void OnTick(DispatcherQueueTimer sender, object args)
    {
        Schedule(_tick());
    }

    private void Schedule(uint delay)
    {
        if (_disposed)
        {
            return;
        }

        _timer.Interval = TimeSpan.FromMilliseconds(Math.Clamp(delay, 10, 60000));
        _timer.Start();
    }

    public void Dispose()
    {
        _disposed = true;
        _timer.Stop();
        _timer.Tick -= OnTick;
    }
}
