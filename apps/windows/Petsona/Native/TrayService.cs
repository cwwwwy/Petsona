using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Microsoft.UI.Dispatching;
using Petsona.Core;

namespace Petsona.Native;

/// <summary>
/// Owns the notification-area icon and the native popup menu on a dedicated
/// STA thread, matching the legacy shell: TrackPopupMenu blocks the calling
/// thread, so it must not run on the WinUI dispatcher thread. Commands are
/// posted back to the UI thread.
///
/// The shell delivers the icon callback to the window procedure (not just to
/// the message queue), so the window procedure must dispatch
/// <see cref="TrayCallbackMessage"/> itself. With NOTIFYICON_VERSION_4 a
/// right-click arrives as WM_CONTEXTMENU with the screen point in wParam.
/// </summary>
internal sealed unsafe class TrayService : IDisposable
{
    public const int CommandSettings = 1;
    public const int CommandChangePet = 2;
    public const int CommandToggleVisibility = 3;
    public const int CommandActivity = 4;
    public const int CommandQuit = 5;

    private const string ClassName = "PetsonaTrayWindow";
    private const uint TrayCallbackMessage = NativeWin32.WM_APP + 2;
    private const uint MenuRequestMessage = NativeWin32.WM_APP + 3;
    private const uint TrayIconId = 1;
    private const uint Version4 = 4;

    private static readonly Guid TrayIconGuid = new("7f3b9d2a-6c4e-4f38-9a5b-2d1e8c7b0a64");

    private readonly DispatcherQueue _dispatcher;
    private readonly Action<int> _onCommand;
    private readonly ManualResetEventSlim _ready = new(false);
    private readonly Thread _thread;
    private GCHandle _selfHandle;
    private nint _ownerWindow;
    private uint _threadId;
    private nint _icon;
    private bool _iconOwned;
    private bool _pendingMenuPoint;
    private int _pendingMenuX;
    private int _pendingMenuY;
    private bool _disposed;

    public TrayService(DispatcherQueue dispatcher, Action<int> onCommand)
    {
        _dispatcher = dispatcher;
        _onCommand = onCommand;
        _selfHandle = GCHandle.Alloc(this);
        _thread = new Thread(ThreadMain)
        {
            IsBackground = true,
            Name = "Petsona tray",
        };
        _thread.SetApartmentState(ApartmentState.STA);
        _thread.Start();
        _ = _ready.Wait(TimeSpan.FromSeconds(5));
    }

    /// <summary>Ask the tray thread to open the menu at the cursor.</summary>
    public void RequestMenu()
    {
        if (_ownerWindow != 0)
        {
            _ = NativeWin32.PostMessageW(_ownerWindow, MenuRequestMessage, 0, 0);
        }
    }

    private static void Log(string message)
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
                Path.Combine(logDirectory, "windows-native-tray.log"),
                $"{DateTime.Now:O} {message}{Environment.NewLine}");
        }
        catch (Exception)
        {
            // diagnostics must never break the tray
        }
    }

    private void ThreadMain()
    {
        _threadId = NativeWin32.GetCurrentThreadId();
        Log($"tray thread start id={_threadId}");
        var hInstance = NativeWin32.GetModuleHandleW(null);
        fixed (char* className = ClassName)
        {
            var wndClass = new NativeWin32.WNDCLASSEXW
            {
                CbSize = (uint)sizeof(NativeWin32.WNDCLASSEXW),
                LpfnWndProc = (nint)(delegate* unmanaged[Stdcall]<nint, uint, nint, nint, nint>)&WndProc,
                HInstance = hInstance,
                LpszClassName = className,
            };
            _ = NativeWin32.RegisterClassExW(&wndClass);
        }

        _ownerWindow = NativeWin32.CreateWindowExW(
            0, ClassName, "Petsona", NativeWin32.WS_POPUP, 0, 0, 0, 0, 0, 0, hInstance, 0);
        if (_ownerWindow == 0)
        {
            Log($"tray owner window creation failed err={Marshal.GetLastWin32Error()}");
            _ready.Set();
            return;
        }

        _ = NativeWin32.SetWindowLongPtrW(
            _ownerWindow, NativeWin32.GWLP_USERDATA, GCHandle.ToIntPtr(_selfHandle));
        AddIcon();
        _ready.Set();

        NativeWin32.MSG msg;
        while (NativeWin32.GetMessageW(&msg, 0, 0, 0) > 0)
        {
            _ = NativeWin32.TranslateMessage(&msg);
            _ = NativeWin32.DispatchMessageW(&msg);
        }

        RemoveIcon();
        if (_ownerWindow != 0)
        {
            _ = NativeWin32.DestroyWindow(_ownerWindow);
            _ownerWindow = 0;
        }
    }

    private nint HandleMessage(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        switch (msg)
        {
            case TrayCallbackMessage:
                OnTrayCallback(hWnd, wParam, lParam);
                return 0;

            case MenuRequestMessage:
                var hasPoint = _pendingMenuPoint;
                var x = _pendingMenuX;
                var y = _pendingMenuY;
                _pendingMenuPoint = false;
                ShowMenu(hasPoint, x, y);
                return 0;

            default:
                return NativeWin32.DefWindowProcW(hWnd, msg, wParam, lParam);
        }
    }

    private void OnTrayCallback(nint hWnd, nint wParam, nint lParam)
    {
        var notification = TrayCallbackParser.Parse(wParam, lParam, TrayIconId);
        Log(
            $"tray callback raw=0x{lParam.ToInt64():X} event=0x{notification.Event:X} " +
            $"version4={notification.Version4} point={notification.X},{notification.Y}");
        if (!TrayCallbackParser.OpensMenu(notification.Event))
        {
            return;
        }

        if (notification.Event == TrayCallbackParser.WmContextMenu)
        {
            _pendingMenuPoint = true;
            _pendingMenuX = notification.X;
            _pendingMenuY = notification.Y;
        }

        // Never run the modal menu inside the shell's sent callback: post it
        // back to this thread and return so the shell can continue.
        _ = NativeWin32.PostMessageW(hWnd, MenuRequestMessage, 0, 0);
    }

    private void ShowMenu(bool hasPoint, int x, int y)
    {
        var menu = NativeWin32.CreatePopupMenu();
        if (menu == 0)
        {
            return;
        }

        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_STRING, CommandSettings, "打开设置");
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_STRING, CommandChangePet, "更换宠物");
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_SEPARATOR, 0, null);
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_STRING, CommandToggleVisibility, "隐藏 / 显示");
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_STRING, CommandActivity, "立即活动");
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_SEPARATOR, 0, null);
        _ = NativeWin32.AppendMenuW(menu, NativeWin32.MF_STRING, CommandQuit, "退出");

        if (!hasPoint)
        {
            NativeWin32.POINT cursor;
            _ = NativeWin32.GetCursorPos(&cursor);
            x = cursor.X;
            y = cursor.Y;
        }

        var previous = NativeWin32.GetForegroundWindow();
        _ = NativeWin32.SetForegroundWindow(_ownerWindow);
        Log($"show menu at {x},{y} fg={NativeWin32.GetForegroundWindow()} owner={_ownerWindow}");
        var command = NativeWin32.TrackPopupMenu(
            menu,
            NativeWin32.TPM_RETURNCMD | NativeWin32.TPM_RIGHTBUTTON,
            x,
            y,
            0,
            _ownerWindow,
            null);
        Log($"menu command={command} err={Marshal.GetLastWin32Error()}");
        _ = NativeWin32.PostMessageW(_ownerWindow, NativeWin32.WM_NULL, 0, 0);
        if (command != CommandSettings && command != CommandChangePet)
        {
            _ = NativeWin32.SetForegroundWindow(previous);
        }

        _ = NativeWin32.DestroyMenu(menu);

        if (command != 0)
        {
            var id = (int)command;
            _dispatcher.TryEnqueue(() => _onCommand(id));
        }
    }

    private void AddIcon()
    {
        (_icon, _iconOwned) = LoadAppIcon();
        var data = new NativeWin32.NOTIFYICONDATAW
        {
            CbSize = (uint)sizeof(NativeWin32.NOTIFYICONDATAW),
            HWnd = _ownerWindow,
            UID = TrayIconId,
            UFlags = NativeWin32.NIF_MESSAGE | NativeWin32.NIF_ICON | NativeWin32.NIF_TIP,
            UCallbackMessage = TrayCallbackMessage,
            HIcon = _icon,
            // A stable GUID identity lets Windows remember this icon's
            // "show in taskbar" choice across launches.
            GuidItem = TrayIconGuid,
        };
        var tip = new Span<char>(data.SzTip, 128);
        "Petsona".AsSpan().CopyTo(tip);
        var added = NativeWin32.Shell_NotifyIconW(NativeWin32.NIM_ADD, &data);
        Log($"icon add result={added} hwnd={_ownerWindow} err={Marshal.GetLastWin32Error()}");

        // Version 4 is the modern contract on Windows 10/11: the low word of
        // the callback lParam carries the icon id and the high word carries
        // the mouse event. Explicitly setting it makes the delivery format
        // deterministic instead of leaving it up to the shell default.
        data.UVersionOrTimeout = Version4;
        var versioned = NativeWin32.Shell_NotifyIconW(NativeWin32.NIM_SETVERSION, &data);
        Log($"icon set version4 result={versioned}");
    }

    private void RemoveIcon()
    {
        if (_icon == 0)
        {
            return;
        }

        var data = new NativeWin32.NOTIFYICONDATAW
        {
            CbSize = (uint)sizeof(NativeWin32.NOTIFYICONDATAW),
            HWnd = _ownerWindow,
            UID = TrayIconId,
            GuidItem = TrayIconGuid,
        };
        _ = NativeWin32.Shell_NotifyIconW(NativeWin32.NIM_DELETE, &data);
        if (_iconOwned)
        {
            _ = NativeWin32.DestroyIcon(_icon);
        }

        _icon = 0;
    }

    private static (nint Icon, bool Owned) LoadAppIcon()
    {
        foreach (var path in IconCandidates())
        {
            if (!File.Exists(path))
            {
                continue;
            }

            var icon = NativeWin32.LoadImageW(
                0, path, NativeWin32.IMAGE_ICON, 0, 0, NativeWin32.LR_LOADFROMFILE | NativeWin32.LR_DEFAULTSIZE);
            if (icon != 0)
            {
                return (icon, true);
            }
        }

        return (System.Drawing.SystemIcons.Application.Handle, false);
    }

    private static IEnumerable<string> IconCandidates()
    {
        yield return Path.Combine(AppContext.BaseDirectory, "Petsona.ico");
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                yield return Path.Combine(directory.FullName, "packaging", "windows", "Petsona.ico");
                yield break;
            }
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static nint WndProc(nint hWnd, uint msg, nint wParam, nint lParam)
    {
        var data = NativeWin32.GetWindowLongPtrW(hWnd, NativeWin32.GWLP_USERDATA);
        if (data != 0 && GCHandle.FromIntPtr(data).Target is TrayService self)
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
        if (_threadId != 0)
        {
            _ = NativeWin32.PostThreadMessageW(_threadId, NativeWin32.WM_QUIT, 0, 0);
        }

        if (_thread.IsAlive)
        {
            _ = _thread.Join(TimeSpan.FromSeconds(2));
        }

        if (_selfHandle.IsAllocated)
        {
            _selfHandle.Free();
        }

        _ready.Dispose();
    }
}
