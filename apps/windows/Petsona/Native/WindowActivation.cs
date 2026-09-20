using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using WinRT.Interop;

namespace Petsona.Native;

/// <summary>
/// Re-asserts foreground ownership after a WinUI window is shown. The pet
/// window is WS_EX_NOACTIVATE, so Settings/Composer can appear without
/// keyboard focus even though Window.Activate() returned. Retry briefly, like
/// the legacy Win32 shell did for the settings window, but never fight another
/// foreground app indefinitely.
/// </summary>
internal static class WindowActivation
{
    private const int RetryIntervalMilliseconds = 50;

    public static void EnsureForeground(
        Window window,
        Action? afterFocus = null,
        int timeoutMilliseconds = 1500)
    {
        nint handle;
        try
        {
            handle = WindowNative.GetWindowHandle(window);
        }
        catch (Exception)
        {
            return;
        }

        if (handle == 0)
        {
            return;
        }

        TryActivate(handle, afterFocus);
        if (NativeWin32.GetForegroundWindow() == handle)
        {
            return;
        }

        var deadline = Environment.TickCount64 + Math.Max(0, timeoutMilliseconds);
        DispatcherQueueTimer? timer = null;
        timer = window.DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(RetryIntervalMilliseconds);
        timer.IsRepeating = true;
        timer.Tick += (_, _) =>
        {
            TryActivate(handle, afterFocus);
            if (NativeWin32.GetForegroundWindow() == handle || Environment.TickCount64 >= deadline)
            {
                timer!.Stop();
            }
        };
        window.Closed += (_, _) => timer!.Stop();
        timer.Start();
    }

    private static void TryActivate(nint handle, Action? afterFocus)
    {
        _ = NativeWin32.SetForegroundWindow(handle);
        _ = NativeWin32.SetFocus(handle);
        afterFocus?.Invoke();
    }
}
