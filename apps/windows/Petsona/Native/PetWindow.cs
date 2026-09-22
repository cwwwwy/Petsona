using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Petsona.Core.Interop;
using Petsona.Rendering;

namespace Petsona.Native;

/// <summary>
/// Layered, non-activating Win32 window that renders one sprite frame and
/// mirrors the macOS interaction rules: single click after 320 ms, jump on
/// double click, drag past 4 px moves the window, and pixel-level
/// pass-through that keeps the idle body clickable.
/// </summary>
internal sealed unsafe class PetWindow : IDisposable
{
    private const string ClassName = "PetsonaPetWindow";
    private const nuint ClickTimerId = 1;
    private const nuint HitTestTimerId = 2;
    private const int ClickDelayMs = 320;
    private const int HitTestIntervalMs = 50;
    private const int DragThreshold = 4;
    private const int DirectionFlipThreshold = 3;
    private const uint SwpNoZorder = 0x0004;

    private static readonly object ClassLock = new();
    private static bool _classRegistered;
    private static readonly nint ArrowCursor =
        NativeWin32.LoadCursorW(0, (nint)NativeWin32.IDC_ARROW);

    private readonly SpriteRenderer _renderer;
    private GCHandle _selfHandle;
    private nint _hwnd;
    private bool _disposed;

    private SpriteRenderer.Frame? _frame;
    private SpriteRenderer.Frame? _appliedFrame;
    private int _appliedWidth;
    private int _appliedHeight;
    private bool _appliedVisible;
    private bool _appliedTopmost;
    private bool _appliedTransparent;
    private bool _clickThrough;

    private bool _mouseDown;
    private bool _dragging;
    private NativeWin32.POINT _dragStartCursor;
    private NativeWin32.POINT _dragStartWindow;
    private NativeWin32.POINT _lastMoveCursor;
    private int _directionFlipAccumulator;
    private string _lastDragState = string.Empty;
    private long _lastDragStateTick;
    private long _lastClickTick;

    public PetWindow(SpriteRenderer renderer)
    {
        _renderer = renderer;
        _selfHandle = GCHandle.Alloc(this);
        EnsureClassRegistered();

        _hwnd = NativeWin32.CreateWindowExW(
            NativeWin32.WS_EX_LAYERED | NativeWin32.WS_EX_TOOLWINDOW | NativeWin32.WS_EX_NOACTIVATE,
            ClassName,
            "Petsona",
            NativeWin32.WS_POPUP,
            0,
            0,
            1,
            1,
            0,
            0,
            NativeWin32.GetModuleHandleW(null),
            0);
        if (_hwnd == 0)
        {
            _selfHandle.Free();
            throw new InvalidOperationException("Cannot create the pet window.");
        }

        NativeWin32.SetWindowLongPtrW(_hwnd, NativeWin32.GWLP_USERDATA, GCHandle.ToIntPtr(_selfHandle));
        NativeWin32.SetTimer(_hwnd, HitTestTimerId, HitTestIntervalMs, 0);
    }

    public nint Handle => _hwnd;

    /// <summary>True while the user hid the pet from the tray menu.</summary>
    public bool Hidden { get; set; }

    /// <summary>
    /// While true the window stays hidden even when a frame is ready. The
    /// controller uses it to hold the pet back until the remembered position is
    /// applied, so it never flashes at the creation coordinates (bug 2026-09-22).
    /// </summary>
    public bool DeferShow { get; set; }

    /// <summary>The sprite window has a real frame size (not the 1x1 placeholder).</summary>
    public bool HasAppliedSize => _appliedWidth > 1 && _appliedHeight > 1;

    /// <summary>True while the pet window is actually shown.</summary>
    public bool IsVisible => _appliedVisible;

    public event Action? Clicked;

    public event Action? DoubleClicked;

    public event Action<string>? DragStateChanged;

    public event Action<int, int>? PositionChanged;

    public event Action? ContextMenuRequested;

    /// <summary>Raised when a press becomes a drag, before any run state is sent.</summary>
    public event Action? DragStarted;

    /// <summary>Raised when a drag ends, after capture is released.</summary>
    public event Action? DragEnded;

    /// <summary>
    /// Raised after a drag move was applied. The UI renders the latest
    /// runtime frame synchronously because a high-rate mouse stream can
    /// starve the dispatcher timer while the button is held.
    /// </summary>
    public event Action? DragMoved;

    /// <summary>Applies one runtime snapshot; blits only when the frame changes.</summary>
    public void Update(PetsonaSnapshot snapshot, string atlasPath)
    {
        var dpiScale = Math.Max(1.0, NativeWin32.GetDpiForWindow(_hwnd) / 96.0);
        var frame = _renderer.GetFrame(snapshot, atlasPath, dpiScale);
        _frame = frame;
        _clickThrough = snapshot.ClickThrough != 0;

        var sizeChanged = frame is not null &&
            (frame.Width != _appliedWidth || frame.Height != _appliedHeight);
        var spriteChanged = frame is not null && !ReferenceEquals(frame, _appliedFrame);
        if (frame is not null && (sizeChanged || spriteChanged))
        {
            if (sizeChanged && _appliedWidth > 0 && _appliedHeight > 0)
            {
                // Scale anchor: bottom center, matching the macOS controller.
                var rect = CurrentRect();
                var centerX = rect.Left + (rect.Width / 2);
                var newLeft = centerX - (frame.Width / 2);
                var newTop = rect.Bottom - frame.Height;
                NativeWin32.SetWindowPos(
                    _hwnd, 0, newLeft, newTop, frame.Width, frame.Height,
                    NativeWin32.SWP_NOACTIVATE | SwpNoZorder);
            }

            Blit(frame);
            _appliedWidth = frame.Width;
            _appliedHeight = frame.Height;
            _appliedFrame = frame;
        }

        var visible = !Hidden && !DeferShow && snapshot.Ready != 0 && snapshot.HasPet != 0 && snapshot.PetVisible != 0 && frame is not null;
        if (visible != _appliedVisible)
        {
            _ = NativeWin32.ShowWindow(_hwnd, visible ? NativeWin32.SW_SHOWNOACTIVATE : NativeWin32.SW_HIDE);
            _appliedVisible = visible;
        }

        var topmost = snapshot.AlwaysOnTop != 0;
        if (topmost != _appliedTopmost)
        {
            NativeWin32.SetWindowPos(
                _hwnd,
                topmost ? NativeWin32.HWND_TOPMOST : NativeWin32.HWND_NOTOPMOST,
                0, 0, 0, 0,
                NativeWin32.SWP_NOMOVE | NativeWin32.SWP_NOSIZE | NativeWin32.SWP_NOACTIVATE);
            _appliedTopmost = topmost;
        }

        // Re-check pass-through on every frame as well: the WM_TIMER poller
        // can be delayed by render load, and a late release makes the pet
        // feel unclickable.
        UpdatePassThrough();
    }

    public void MoveTo(int x, int y)
    {
        NativeWin32.SetWindowPos(
            _hwnd, 0, x, y, 0, 0,
            NativeWin32.SWP_NOSIZE | NativeWin32.SWP_NOACTIVATE | SwpNoZorder);
    }

    public NativeWin32.RECT CurrentRect()
    {
        NativeWin32.RECT rect;
        _ = NativeWin32.GetWindowRect(_hwnd, &rect);
        return rect;
    }

    /// <summary>Screen point under the cursor inside the window, in window pixels.</summary>
    public bool TryGetCursorInWindow(out int x, out int y)
    {
        NativeWin32.POINT cursor;
        _ = NativeWin32.GetCursorPos(&cursor);
        var rect = CurrentRect();
        x = cursor.X - rect.Left;
        y = cursor.Y - rect.Top;
        return x >= 0 && y >= 0 && cursor.X < rect.Right && cursor.Y < rect.Bottom;
    }

    private void Blit(SpriteRenderer.Frame frame)
    {
        LayeredPresenter.Present(_hwnd, frame.Pixels, frame.Width, frame.Height);
    }

    private void UpdatePassThrough()
    {
        var transparent = _clickThrough;
        NativeWin32.POINT cursor;
        _ = NativeWin32.GetCursorPos(&cursor);
        var rect = CurrentRect();
        var localX = cursor.X - rect.Left;
        var localY = cursor.Y - rect.Top;
        var inWindow = localX >= 0 && localY >= 0 && cursor.X < rect.Right && cursor.Y < rect.Bottom;
        var hit = inWindow && _frame is not null && _frame.IsHit(localX, localY);
        if (transparent && _appliedVisible && _frame is not null && hit)
        {
            transparent = false;
        }

        if (transparent == _appliedTransparent)
        {
            return;
        }

        var style = (uint)NativeWin32.GetWindowLongPtrW(_hwnd, NativeWin32.GWL_EXSTYLE);
        var updated = transparent
            ? style | NativeWin32.WS_EX_TRANSPARENT
            : style & ~NativeWin32.WS_EX_TRANSPARENT;
        _ = NativeWin32.SetWindowLongPtrW(_hwnd, NativeWin32.GWL_EXSTYLE, (nint)updated);
        _appliedTransparent = transparent;
    }

    private static int _dragDirectionDiagLines;

    private static void DragDirectionDiag(string message)
    {
        if (_dragDirectionDiagLines > 200)
        {
            return;
        }

        _dragDirectionDiagLines++;
        try
        {
            var configuredHome = Environment.GetEnvironmentVariable("PETSONA_HOME");
            var baseDirectory = string.IsNullOrWhiteSpace(configuredHome)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Petsona")
                : configuredHome;
            var logDirectory = Path.Combine(baseDirectory, "logs");
            Directory.CreateDirectory(logDirectory);
            File.AppendAllText(
                Path.Combine(logDirectory, "windows-native-drag-direction.log"),
                $"{DateTime.Now:HH:mm:ss.fff} {message}{Environment.NewLine}");
        }
        catch (Exception)
        {
        }
    }

    private static int _clickDiagLines;

    private static void ClickDiag(string message)
    {
        if (_clickDiagLines > 40)
        {
            return;
        }

        _clickDiagLines++;
        try
        {
            var configuredHome = Environment.GetEnvironmentVariable("PETSONA_HOME");
            var baseDirectory = string.IsNullOrWhiteSpace(configuredHome)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Petsona")
                : configuredHome;
            var logDirectory = Path.Combine(baseDirectory, "logs");
            Directory.CreateDirectory(logDirectory);
            File.AppendAllText(
                Path.Combine(logDirectory, "windows-native-click.log"),
                $"{DateTime.Now:HH:mm:ss.fff} {message}{Environment.NewLine}");
        }
        catch (Exception)
        {
        }
    }

    private void OnLeftDown()
    {
        ClickDiag("LBUTTONDOWN");
        var now = Environment.TickCount64;
        if (now - _lastClickTick <= ClickDelayMs)
        {
            _lastClickTick = 0;
            CancelClickTimer();
            DoubleClicked?.Invoke();
            return;
        }

        _lastClickTick = now;
        _mouseDown = true;
        _dragging = false;
        NativeWin32.POINT cursor;
        _ = NativeWin32.GetCursorPos(&cursor);
        _dragStartCursor = cursor;
        _lastMoveCursor = cursor;
        _directionFlipAccumulator = 0;
        var rect = CurrentRect();
        _dragStartWindow = new NativeWin32.POINT { X = rect.Left, Y = rect.Top };
        StartClickTimer();
        _ = NativeWin32.SetCapture(_hwnd);
    }

    private void OnMouseMove()
    {
        if (!_mouseDown)
        {
            return;
        }

        NativeWin32.POINT cursor;
        _ = NativeWin32.GetCursorPos(&cursor);
        var dx = cursor.X - _dragStartCursor.X;
        var dy = cursor.Y - _dragStartCursor.Y;
        var stepX = cursor.X - _lastMoveCursor.X;
        _lastMoveCursor = cursor;

        if (!_dragging && (Math.Abs(dx) > DragThreshold || Math.Abs(dy) > DragThreshold))
        {
            _dragging = true;
            CancelClickTimer();
            DragStarted?.Invoke();
        }

        if (!_dragging)
        {
            return;
        }

        NativeWin32.SetWindowPos(
            _hwnd, 0, _dragStartWindow.X + dx, _dragStartWindow.Y + dy, 0, 0,
            NativeWin32.SWP_NOSIZE | NativeWin32.SWP_NOACTIVATE | SwpNoZorder);
        DragMoved?.Invoke();

        // Direction comes from the latest movement, not the total offset from
        // mouse-down: reversing in place must flip the run direction without
        // first crossing the original press point. A 3 px counter-movement
        // filter keeps hand jitter from flapping the pose.
        var now = Environment.TickCount64;
        if (stepX != 0)
        {
            var candidate = stepX > 0 ? "running-right" : "running-left";
            if (_lastDragState.Length == 0)
            {
                DragDirectionDiag($"step={stepX} candidate={candidate} last=<empty> decision=initial");
                SendDragState(candidate, now);
            }
            else if (candidate == _lastDragState)
            {
                _directionFlipAccumulator = 0;
                DragDirectionDiag($"step={stepX} candidate={candidate} last={_lastDragState} decision=keep");
            }
            else
            {
                _directionFlipAccumulator += Math.Abs(stepX);
                var flip = _directionFlipAccumulator >= DirectionFlipThreshold;
                DragDirectionDiag($"step={stepX} candidate={candidate} last={_lastDragState} acc={_directionFlipAccumulator} decision={(flip ? "flip" : "wait")}");
                if (flip)
                {
                    SendDragState(candidate, now);
                }
            }
        }

        if (_lastDragState.Length != 0 && now - _lastDragStateTick >= 80)
        {
            SendDragState(_lastDragState, now);
        }
    }

    private void SendDragState(string state, long now)
    {
        _lastDragState = state;
        _lastDragStateTick = now;
        _directionFlipAccumulator = 0;
        DragStateChanged?.Invoke(state);
    }

    private void OnLeftUp()
    {
        if (!_mouseDown)
        {
            return;
        }

        ClickDiag($"LBUTTONUP dragging={_dragging}");
        DragDirectionDiag($"up dragging={_dragging} state={_lastDragState}");
        _mouseDown = false;
        _ = NativeWin32.ReleaseCapture();
        if (_dragging)
        {
            _dragging = false;
            _lastDragState = string.Empty;
            _directionFlipAccumulator = 0;
            DragStateChanged?.Invoke("idle");
            DragEnded?.Invoke();
            var rect = CurrentRect();
            PositionChanged?.Invoke(rect.Left, rect.Top);
        }
    }

    private void StartClickTimer()
    {
        _ = NativeWin32.SetTimer(_hwnd, ClickTimerId, ClickDelayMs, 0);
    }

    private void CancelClickTimer()
    {
        _ = NativeWin32.KillTimer(_hwnd, ClickTimerId);
    }

    private nint HandleMessage(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        switch (msg)
        {
            case NativeWin32.WM_SETCURSOR:
                // The shell shows an "app starting" cursor while the process
                // boots. A window class without a cursor leaves whatever was
                // set before in place, so a hover over the pet would keep the
                // busy pointer. Answer a client hit test with a plain arrow.
                if ((lParam.ToInt64() & 0xFFFF) == NativeWin32.HTCLIENT)
                {
                    _ = NativeWin32.SetCursor(ArrowCursor);
                    return 1;
                }

                return NativeWin32.DefWindowProcW(hWnd, msg, wParam, lParam);

            case NativeWin32.WM_TIMER:
                if ((nuint)wParam == ClickTimerId)
                {
                    CancelClickTimer();
                    ClickDiag("CLICK FIRED");
                    Clicked?.Invoke();
                }
                else if ((nuint)wParam == HitTestTimerId)
                {
                    UpdatePassThrough();
                }

                return 0;

            case NativeWin32.WM_LBUTTONDOWN:
                OnLeftDown();
                return 0;

            case NativeWin32.WM_MOUSEMOVE:
                OnMouseMove();
                return 0;

            case NativeWin32.WM_LBUTTONUP:
                OnLeftUp();
                return 0;

            case NativeWin32.WM_RBUTTONUP:
                ContextMenuRequested?.Invoke();
                return 0;

            case NativeWin32.WM_NCHITTEST:
                return HitTest(lParam);

            default:
                return NativeWin32.DefWindowProcW(hWnd, msg, wParam, lParam);
        }
    }

    private nint HitTest(nint lParam)
    {
        if (!_clickThrough || _frame is null)
        {
            return NativeWin32.HTCLIENT;
        }

        var value = lParam.ToInt64();
        var screenX = (short)(value & 0xFFFF);
        var screenY = (short)((value >> 16) & 0xFFFF);
        var rect = CurrentRect();
        return _frame.IsHit(screenX - rect.Left, screenY - rect.Top)
            ? NativeWin32.HTCLIENT
            : NativeWin32.HTTRANSPARENT;
    }

    private static void EnsureClassRegistered()
    {
        lock (ClassLock)
        {
            if (_classRegistered)
            {
                return;
            }

            fixed (char* className = ClassName)
            {
                var wndClass = new NativeWin32.WNDCLASSEXW
                {
                    CbSize = (uint)sizeof(NativeWin32.WNDCLASSEXW),
                    LpfnWndProc = (nint)(delegate* unmanaged[Stdcall]<nint, uint, nint, nint, nint>)&WndProc,
                    HInstance = NativeWin32.GetModuleHandleW(null),
                    HCursor = ArrowCursor,
                    HIcon = WindowIcon.Big,
                    HIconSm = WindowIcon.Small,
                    LpszClassName = className,
                };
                if (NativeWin32.RegisterClassExW(&wndClass) == 0)
                {
                    throw new InvalidOperationException("Cannot register the pet window class.");
                }
            }

            _classRegistered = true;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static nint WndProc(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        var data = NativeWin32.GetWindowLongPtrW(hWnd, NativeWin32.GWLP_USERDATA);
        if (data != 0 && GCHandle.FromIntPtr(data).Target is PetWindow self)
        {
            return self.HandleMessage(hWnd, msg, wParam, lParam);
        }

        return NativeWin32.DefWindowProcW(hWnd, msg, wParam, lParam);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        if (_hwnd != 0)
        {
            _ = NativeWin32.KillTimer(_hwnd, HitTestTimerId);
            _ = NativeWin32.KillTimer(_hwnd, ClickTimerId);
            _ = NativeWin32.DestroyWindow(_hwnd);
            _hwnd = 0;
        }

        if (_selfHandle.IsAllocated)
        {
            _selfHandle.Free();
        }
    }
}
