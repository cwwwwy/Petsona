using System.Globalization;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Petsona.Core;
using Petsona.Core.Interop;
using Petsona.Native;
using Petsona.Rendering;
using Petsona.Views;

namespace Petsona;

/// <summary>
/// Wires the runtime engine to the pet window, tray menu and frame scheduler
/// on the UI thread. Batch B1 covers the minimal product path: sprite window,
/// click/double-click/drag, position persistence, tray menu, empty-library
/// first run and the state protocol that the runtime already serves.
/// </summary>
internal sealed unsafe class AppController : IDisposable
{
    private const uint ActivityTtlMs = 8000;
    private const uint DragStateTtlMs = 300;

    private readonly EngineClient _engine;
    private readonly SpriteRenderer _renderer;
    private readonly PetWindow _window;
    private readonly OverlayWindow _bubble;
    private readonly OverlayWindow _editButton;
    private readonly TrayService _tray;
    private readonly FrameScheduler _scheduler;

    private SettingsWindow? _settings;
    private ComposerWindow? _composer;
    private PetsonaSnapshot _lastSnapshot;
    private string _draft = string.Empty;
    private string _atlasPath = string.Empty;
    private long _lastDragRenderTick;
    private readonly GazeStabilizer _gazeStabilizer = new();
    private bool _initialPositionApplied;
    private bool _emptyLibraryHandled;
    private bool _manuallyHidden;
    private bool _gazeActive;
    private bool _draggingPet;
    private bool _faultReported;
    private bool _disposed;

    private readonly DispatcherQueue _dispatcher;

    public AppController(DispatcherQueue dispatcher)
    {
        _dispatcher = dispatcher;
        _engine = new EngineClient();
        _renderer = new SpriteRenderer();
        _window = new PetWindow(_renderer);
        _bubble = new OverlayWindow(isBubble: true);
        _editButton = new OverlayWindow(isBubble: false);
        _tray = new TrayService(dispatcher, OnTrayCommand);
        _scheduler = new FrameScheduler(dispatcher, Tick);

        _window.Clicked += OnClicked;
        _window.DoubleClicked += OnDoubleClicked;
        _window.DragStateChanged += OnDragStateChanged;
        _window.PositionChanged += OnPositionChanged;
        _window.ContextMenuRequested += _tray.RequestMenu;
        _window.DragMoved += OnDragMoved;
        _window.DragStarted += OnDragStarted;
        _window.DragEnded += OnDragEnded;
        _bubble.Clicked += OpenComposer;
        _editButton.Clicked += OpenComposer;

    }

    public void Start()
    {
        _scheduler.Start();
    }

    private uint Tick()
    {
        try
        {
            return TickCore();
        }
        catch (Exception ex)
        {
            // One failing tick must not stop the frame chain, but it must
            // not disappear silently either.
            TryWriteErrorLog("tick", ex);
            return 1000;
        }
    }

    private uint TickCore()
    {
        _engine.Tick();
        PetsonaSnapshot snapshot;
        try
        {
            snapshot = _engine.Snapshot();
        }
        catch (EngineException error) when (
            error.Status is PetsonaStatus.RuntimeFailed or PetsonaStatus.Panic)
        {
            // A terminal worker (for example a second instance on the same
            // data directory) must surface its error instead of leaving an
            // empty desktop.
            HandleFault();
            return 1000;
        }

        _lastSnapshot = snapshot;

        if (snapshot.Faulted != 0)
        {
            HandleFault();
            return 1000;
        }

        _atlasPath = _engine.Text(PetsonaTextField.AtlasPath);

        _window.Hidden = _manuallyHidden;
        _window.Update(snapshot, _atlasPath);
        UpdateOverlays(snapshot);

        // Position after the sprite frame produced a real window size; with a
        // webp sheet the first frames decode asynchronously, so the placeholder
        // 1x1 window must not be used for the default placement.
        if (!_initialPositionApplied && snapshot.Ready != 0 && _window.HasAppliedSize)
        {
            ApplyInitialPosition();
        }

        if (snapshot.Ready != 0 && snapshot.HasPet == 0 && !_emptyLibraryHandled)
        {
            _emptyLibraryHandled = true;
            OpenSettings();
        }

        // The same tick that renders the pet also samples the global cursor.
        // Keeping both in one loop means a gaze command is followed by a
        // render tick within the poll interval instead of the animation
        // deadline (which can be 280 ms idle or 1 s while holding a pose).
        SampleGaze();
        return NextDelay(snapshot);
    }

    private void UpdateOverlays(PetsonaSnapshot snapshot)
    {
        // The bubble is protocol-driven and stays useful even without a
        // rendered sprite, but the edit button must only appear when the pet
        // itself is actually on screen.
        var petListed = !_manuallyHidden && snapshot.Ready != 0 && snapshot.HasPet != 0 && snapshot.PetVisible != 0;
        if (!petListed)
        {
            _bubble.Show(false);
            _editButton.Show(false);
            return;
        }

        var petVisible = _window.IsVisible;

        var rect = _window.CurrentRect();

        var bubbleText = _engine.Text(PetsonaTextField.Bubble);
        if (bubbleText.Length == 0)
        {
            _bubble.Show(false);
        }
        else
        {
            _bubble.Render(bubbleText);
            _bubble.MoveTo(rect.Left + ((rect.Width - _bubble.Width) / 2), rect.Top - _bubble.Height - 10);
            _bubble.Show(true);
        }

        if (_composer is null && petVisible)
        {
            _editButton.Render(string.Empty);
            _editButton.MoveTo(rect.Left + ((rect.Width - _editButton.Width) / 2), rect.Bottom + 16);
            _editButton.Show(true);
        }
        else
        {
            _editButton.Show(false);
        }
    }

    /// <summary>
    /// Render the latest runtime frame from inside the drag message. The drag
    /// stream can deliver mouse messages faster than the dispatcher timer, so
    /// waiting for the scheduled tick makes the running animation look frozen
    /// until the button stops moving. Throttle to ~120 Hz.
    /// </summary>
    private void OnDragStarted()
    {
        // The gaze (priority 20) would otherwise override the running rows
        // (priority 10) as soon as the cursor leaves the dead zone, hiding the
        // direction change the user is performing.
        _draggingPet = true;
        ClearGaze();
    }

    private void OnDragEnded()
    {
        _draggingPet = false;
    }

    private void OnDragMoved()
    {
        var now = Environment.TickCount64;
        if (now - _lastDragRenderTick < 8)
        {
            return;
        }

        _lastDragRenderTick = now;
        try
        {
            var snapshot = _engine.Snapshot();
            _lastSnapshot = snapshot;
            _window.Hidden = _manuallyHidden;
            _window.Update(snapshot, _atlasPath);
            DragDiag(snapshot);
        }
        catch (EngineException)
        {
            // The periodic tick owns fault reporting.
        }
    }

    private static int _dragDiagLines;

    /// <summary>Capped drag diagnostic; shows whether the engine publishes new frames while the mouse moves.</summary>
    private void DragDiag(PetsonaSnapshot snapshot)
    {
        if (_dragDiagLines > 400)
        {
            return;
        }

        _dragDiagLines++;
        try
        {
            var configuredHome = Environment.GetEnvironmentVariable("PETSONA_HOME");
            var baseDirectory = string.IsNullOrWhiteSpace(configuredHome)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Petsona")
                : configuredHome;
            var logDirectory = Path.Combine(baseDirectory, "logs");
            Directory.CreateDirectory(logDirectory);
            var state = _engine.Text(PetsonaTextField.State);
            File.AppendAllText(
                Path.Combine(logDirectory, "windows-native-drag.log"),
                $"{DateTime.Now:HH:mm:ss.fff} sprite={snapshot.SpriteIndex} state={state} next={snapshot.NextFrameMs}{Environment.NewLine}");
        }
        catch (Exception)
        {
            // diagnostics must never break the drag
        }
    }

    private void SampleGaze()
    {
        var snapshot = _lastSnapshot;
        if (snapshot.Faulted != 0 || _composer is not null || _draggingPet || _manuallyHidden ||
            snapshot.Ready == 0 || snapshot.HasPet == 0 || snapshot.PetVisible == 0)
        {
            ClearGaze();
            return;
        }

        NativeWin32.POINT cursor;
        _ = NativeWin32.GetCursorPos(&cursor);
        var rect = _window.CurrentRect();
        if (rect.Width <= 0 || rect.Height <= 0)
        {
            return;
        }

        var dx = cursor.X - (rect.Left + (rect.Width / 2.0));
        var dy = cursor.Y - (rect.Top + (rect.Height / 2.0));
        var near = GazeFilter.IsInside(dx, dy, rect.Width, rect.Height, _gazeActive)
            && !GazeFilter.IsInDeadZone(dx, dy, rect.Width, rect.Height);
        if (near)
        {
            // Stabilize the official 22.5-degree direction before sending it.
            // Re-issue the held direction every poll so a multi-step cross-row
            // transition can finish even when the cursor stops moving; the
            // stabilizer prevents small movements from flipping the pose.
            var direction = _gazeStabilizer.Update(dx, dy);
            var (unitX, unitY) = GazeStabilizer.UnitVector(direction);
            _engine.Send(
                PetsonaCommandKind.SetGazeTarget,
                text: $"{unitX:0.####},{unitY:0.####}");
            _gazeActive = true;
            return;
        }

        ClearGaze();
    }

    private void ClearGaze()
    {
        if (_gazeActive)
        {
            _engine.Send(PetsonaCommandKind.ClearGaze);
            _gazeActive = false;
        }

        _gazeStabilizer.Reset();
    }

    /// <summary>
    /// The worker is terminal. Show the worker error once on a locally
    /// rendered bubble, stop the gaze loop and keep the UI at a slow
    /// heartbeat; the runtime has already rejected further commands.
    /// </summary>
    private void HandleFault()
    {
        if (_faultReported)
        {
            return;
        }

        _faultReported = true;
        _gazeActive = false;
        _editButton.Show(false);
        var error = _engine.Text(PetsonaTextField.Error);
        var lockConflict = error.Contains("lock", StringComparison.OrdinalIgnoreCase);
        if (error.Length == 0)
        {
            error = "Petsona 引擎发生故障，已停止处理命令。";
        }
        else if (lockConflict)
        {
            error = "已有一个 Petsona 实例在使用同一数据目录，本窗口将在 3 秒后退出。";
        }
        else
        {
            error = $"Petsona 无法继续：\n{error}";
        }

        var rect = _window.CurrentRect();
        _bubble.Render(error);
        _bubble.MoveTo(
            Math.Max(8, rect.Left + ((rect.Width - _bubble.Width) / 2)),
            Math.Max(8, rect.Top - _bubble.Height - 10));
        _bubble.Show(true);

        if (lockConflict)
        {
            // A stray second instance should not linger on the desktop.
            var exitTimer = _dispatcher.CreateTimer();
            exitTimer.Interval = TimeSpan.FromSeconds(3);
            exitTimer.IsRepeating = false;
            exitTimer.Tick += (_, _) =>
            {
                exitTimer.Stop();
                Dispose();
                Application.Current.Exit();
            };
            exitTimer.Start();
        }
    }

    private void OpenComposer()
    {
        if (_composer is not null)
        {
            WindowActivation.EnsureForeground(_composer, _composer.FocusInput);
            return;
        }

        var composer = new ComposerWindow(_draft);
        _composer = composer;
        composer.Submitted += OnComposerSubmitted;
        composer.CaretMoved += OnComposerCaretMoved;
        composer.Closed += (_, _) =>
        {
            _draft = composer.CurrentText;
            if (ReferenceEquals(_composer, composer))
            {
                _composer = null;
            }

            ClearGaze();
        };
        PositionComposer(composer);
        composer.Activate();
        WindowActivation.EnsureForeground(composer, composer.FocusInput);
    }

    private void PositionComposer(ComposerWindow composer)
    {
        var rect = _window.CurrentRect();
        var size = composer.AppWindow.Size;
        var width = size.Width > 0 ? size.Width : 380;
        var height = size.Height > 0 ? size.Height : 190;
        var x = rect.Left + ((rect.Width - width) / 2);
        var y = rect.Bottom + 24;

        NativeWin32.RECT work;
        if (NativeWin32.SystemParametersInfoW(NativeWin32.SPI_GETWORKAREA, 0, &work, 0) != 0)
        {
            if (y + height > work.Bottom)
            {
                y = rect.Top - height - 24;
            }

            x = Math.Clamp(x, work.Left, Math.Max(work.Left, work.Right - width));
            y = Math.Clamp(y, work.Top, Math.Max(work.Top, work.Bottom - height));
        }

        composer.AppWindow.Move(new Windows.Graphics.PointInt32(x, y));
    }

    private void OnComposerSubmitted(string text)
    {
        _draft = string.Empty;
        _composer?.ClearInput();
        _engine.Send(PetsonaCommandKind.SendConversation, text: text);
    }

    private void OnComposerCaretMoved()
    {
        if (_composer is null)
        {
            return;
        }

        var petRect = _window.CurrentRect();
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(_composer);
        NativeWin32.RECT composerRect;
        if (NativeWin32.GetWindowRect(handle, &composerRect) == 0)
        {
            return;
        }

        var targetX = composerRect.Left + (composerRect.Width / 2.0);
        var targetY = composerRect.Top + (composerRect.Height / 2.0);
        var dx = targetX - (petRect.Left + (petRect.Width / 2.0));
        var dy = targetY - (petRect.Top + (petRect.Height / 2.0));
        _engine.Send(PetsonaCommandKind.SetGazeTarget, text: $"{dx:0.##},{dy:0.##}");
        _gazeActive = true;
    }

    private void ApplyInitialPosition()
    {
        _initialPositionApplied = true;
        if (TryParsePosition(_engine.Text(PetsonaTextField.Position), out var savedX, out var savedY))
        {
            _window.MoveTo(savedX, savedY);
            return;
        }

        NativeWin32.RECT work;
        if (NativeWin32.SystemParametersInfoW(NativeWin32.SPI_GETWORKAREA, 0, &work, 0) != 0)
        {
            var rect = _window.CurrentRect();
            _window.MoveTo(
                work.Right - rect.Width - 40,
                work.Bottom - rect.Height - 40);
        }
    }

    private void OnClicked()
    {
        _engine.Send(PetsonaCommandKind.SetState, text: "waving");
        _engine.Send(PetsonaCommandKind.ShowBubble, ttlMilliseconds: 5000, text: "你好，我在这里");
    }

    private void OnDoubleClicked()
    {
        _engine.Send(PetsonaCommandKind.SetState, text: "jumping");
    }

    private void OnDragStateChanged(string state)
    {
        var ttl = state == "idle" ? 1u : DragStateTtlMs;
        _engine.Send(PetsonaCommandKind.SetState, ttlMilliseconds: ttl, text: state);
    }

    private void OnPositionChanged(int x, int y)
    {
        _engine.Send(PetsonaCommandKind.SetPosition, text: $"{x},{y}");
    }

    private void OnTrayCommand(int command)
    {
        switch (command)
        {
            case TrayService.CommandSettings:
            case TrayService.CommandChangePet:
                OpenSettings();
                break;
            case TrayService.CommandToggleVisibility:
                _manuallyHidden = !_manuallyHidden;
                // Hidden pets schedule a slow heartbeat; wake immediately so
                // showing the pet again is not delayed by up to a second.
                _scheduler.Wake();
                break;
            case TrayService.CommandActivity:
                _engine.Send(PetsonaCommandKind.SetState, ttlMilliseconds: ActivityTtlMs, text: "running");
                break;
            case TrayService.CommandQuit:
                Quit();
                break;
        }
    }

    private void OpenSettings()
    {
        if (_settings is null)
        {
            _settings = new SettingsWindow(_engine);
            _settings.Closed += (_, _) => _settings = null;
        }

        _settings.Activate();
        WindowActivation.EnsureForeground(_settings, _settings.FocusRoot);
    }

    private void Quit()
    {
        Dispose();
        Application.Current.Exit();
    }

    internal static void TryWriteErrorLog(string stage, Exception exception)
    {
        try
        {
            var configuredHome = Environment.GetEnvironmentVariable("PETSONA_HOME");
            var baseDirectory = string.IsNullOrWhiteSpace(configuredHome)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Petsona")
                : configuredHome;
            var logDirectory = Path.Combine(baseDirectory, "logs");
            Directory.CreateDirectory(logDirectory);
            File.AppendAllText(
                Path.Combine(logDirectory, "windows-native-error.log"),
                $"{DateTime.Now:O} {stage}: {exception}{Environment.NewLine}");
        }
        catch (Exception)
        {
            // Logging must never mask the original failure.
        }
    }

    private uint NextDelay(PetsonaSnapshot snapshot)
    {
        if (snapshot.Faulted != 0)
        {
            return 1000;
        }

        if (snapshot.Ready == 0)
        {
            return 50;
        }

        // Mirror the macOS tick loop: while a pet is on screen, never sleep
        // past the global-cursor poll (33 ms idle, 16 ms while gazing) even
        // when the engine's next animation frame is hundreds of ms away.
        return FrameCadence.NextDelayMilliseconds(
            snapshot.NextFrameMs,
            snapshot.HasPet != 0,
            snapshot.PetVisible != 0,
            _manuallyHidden,
            _gazeActive);
    }

    private static bool TryParsePosition(string text, out int x, out int y)
    {
        x = 0;
        y = 0;
        if (string.IsNullOrWhiteSpace(text))
        {
            return false;
        }

        var parts = text.Split(',');
        return parts.Length == 2
            && int.TryParse(parts[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out x)
            && int.TryParse(parts[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out y);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _scheduler.Dispose();
        _settings?.Close();
        _composer?.Close();
        _bubble.Dispose();
        _editButton.Dispose();
        _tray.Dispose();
        _window.Dispose();
        _renderer.Dispose();
        _engine.Dispose();
    }
}
