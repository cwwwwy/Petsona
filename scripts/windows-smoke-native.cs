using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace PetsonaSmoke
{
    public sealed class WindowInfo
    {
        public IntPtr Hwnd;
        public string Title;
        public int ProcessId;
        public bool Visible;
        public uint Style;
        public uint ExStyle;
        public int Left;
        public int Top;
        public int Width;
        public int Height;
    }

    public static class Native
    {
        public const uint WS_POPUP = 0x80000000u;
        public const uint WS_CAPTION = 0x00C00000u;
        public const uint WS_BORDER = 0x00800000u;
        public const uint WS_DLGFRAME = 0x00400000u;
        public const uint WS_FRAME = WS_CAPTION | WS_BORDER | WS_DLGFRAME;
        public const uint WS_EX_NOACTIVATE = 0x08000000u;
        public const uint WS_EX_TOPMOST = 0x00000008u;
        public const uint WS_EX_TOOLWINDOW = 0x00000080u;
        public const uint WS_EX_APPWINDOW = 0x00040000u;

        private const int GWL_STYLE = -16;
        private const int GWL_EXSTYLE = -20;

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT
        {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        private struct POINT
        {
            public int X;
            public int Y;
        }

        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextLengthW(IntPtr hWnd);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int maxCount);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        [DllImport("user32.dll")]
        private static extern bool AttachThreadInput(uint attachThread, uint attachToThread, bool attach);

        [DllImport("user32.dll")]
        private static extern bool BringWindowToTop(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr SetActiveWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr SetFocus(IntPtr hWnd);

        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentThreadId();

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
        private static extern IntPtr GetWindowLongPtr64(IntPtr hWnd, int index);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
        private static extern int GetWindowLong32(IntPtr hWnd, int index);

        private static IntPtr GetWindowLongPtr(IntPtr hWnd, int index)
        {
            if (IntPtr.Size == 8)
            {
                return GetWindowLongPtr64(hWnd, index);
            }
            return new IntPtr(GetWindowLong32(hWnd, index));
        }

        public static bool HasAll(uint value, uint bits)
        {
            return (value & bits) == bits;
        }

        public static bool HasAny(uint value, uint bits)
        {
            return (value & bits) != 0;
        }

        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern void keybd_event(
            byte virtualKey,
            byte scanCode,
            uint flags,
            UIntPtr extraInfo);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int command);

        [DllImport("user32.dll")]
        private static extern bool SetCursorPos(int x, int y);

        [DllImport("user32.dll")]
        private static extern bool GetCursorPos(out POINT point);

        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int index);

        [DllImport("user32.dll")]
        private static extern void mouse_event(
            uint flags,
            uint dx,
            uint dy,
            uint data,
            UIntPtr extraInfo);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern uint SendInput(
            uint inputCount,
            INPUT[] inputs,
            int inputSize);

        [DllImport("user32.dll")]
        private static extern short GetAsyncKeyState(int virtualKey);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageW(
            IntPtr hWnd,
            uint message,
            UIntPtr wParam,
            IntPtr lParam);

        [StructLayout(LayoutKind.Sequential)]
        private struct INPUT
        {
            public uint Type;
            public INPUTUNION Data;
        }

        [StructLayout(LayoutKind.Explicit)]
        private struct INPUTUNION
        {
            [FieldOffset(0)]
            public MOUSEINPUT Mouse;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct MOUSEINPUT
        {
            public int Dx;
            public int Dy;
            public uint MouseData;
            public uint Flags;
            public uint Time;
            public UIntPtr ExtraInfo;
        }

        public static IntPtr ForegroundWindow()
        {
            return GetForegroundWindow();
        }

        public static bool ActivateWindow(IntPtr hWnd)
        {
            uint ignored;
            uint currentThread = GetCurrentThreadId();
            uint foregroundThread = GetWindowThreadProcessId(GetForegroundWindow(), out ignored);
            uint targetThread = GetWindowThreadProcessId(hWnd, out ignored);
            bool attachedForeground = foregroundThread != currentThread &&
                AttachThreadInput(currentThread, foregroundThread, true);
            bool attachedTarget = targetThread != currentThread &&
                AttachThreadInput(currentThread, targetThread, true);

            ShowWindow(hWnd, 9);
            BringWindowToTop(hWnd);
            keybd_event(0x12, 0, 0, UIntPtr.Zero);
            bool activated = SetForegroundWindow(hWnd);
            SetActiveWindow(hWnd);
            SetFocus(hWnd);
            keybd_event(0x12, 0, 0x0002u, UIntPtr.Zero);

            if (attachedTarget)
            {
                AttachThreadInput(currentThread, targetThread, false);
            }
            if (attachedForeground)
            {
                AttachThreadInput(currentThread, foregroundThread, false);
            }
            return activated;
        }

        public static void SetCursorPosition(int x, int y)
        {
            SetCursorPos(x, y);
        }

        public static int[] CursorPosition()
        {
            POINT point;
            GetCursorPos(out point);
            return new int[] { point.X, point.Y };
        }

        // SetCursorPos bypasses WH_MOUSE_LL; send real input so the app updates
        // its cached pointer position the same way as a physical mouse.
        public static bool MoveCursorWithInput(int x, int y)
        {
            int virtualX = GetSystemMetrics(76);
            int virtualY = GetSystemMetrics(77);
            int virtualWidth = Math.Max(1, GetSystemMetrics(78) - 1);
            int virtualHeight = Math.Max(1, GetSystemMetrics(79) - 1);
            int normalizedX = (int)Math.Round((x - virtualX) * 65535.0 / virtualWidth);
            int normalizedY = (int)Math.Round((y - virtualY) * 65535.0 / virtualHeight);

            INPUT input = new INPUT();
            input.Type = 0;
            input.Data.Mouse.Flags = 0x0001u | 0x8000u | 0x4000u;
            input.Data.Mouse.Dx = normalizedX;
            input.Data.Mouse.Dy = normalizedY;
            INPUT[] inputs = new INPUT[] { input };
            return SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT))) == 1;
        }

        private static void SendMouseInput(uint flags)
        {
            INPUT input = new INPUT();
            input.Type = 0;
            input.Data.Mouse.Flags = flags;
            INPUT[] inputs = new INPUT[] { input };
            SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT)));
        }

        public static void LeftMouseDown()
        {
            SendMouseInput(0x0002u);
        }

        public static void LeftMouseUp()
        {
            SendMouseInput(0x0004u);
        }

        public static bool LeftMouseIsDown()
        {
            return (GetAsyncKeyState(0x01) & 0x8000) != 0;
        }

        public static int MouseActivateResult(IntPtr hWnd)
        {
            return (int)SendMessageW(hWnd, 0x0021u, UIntPtr.Zero, IntPtr.Zero);
        }

        public static List<WindowInfo> GetTopLevelWindows(int processId)
        {
            return GetWindows(processId);
        }

        public static List<WindowInfo> GetAllTopLevelWindows()
        {
            return GetWindows(-1);
        }

        private static List<WindowInfo> GetWindows(int processId)
        {
            List<WindowInfo> windows = new List<WindowInfo>();
            EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
            {
                uint owner;
                GetWindowThreadProcessId(hWnd, out owner);
                if (processId >= 0 && (int)owner != processId)
                {
                    return true;
                }

                int length = GetWindowTextLengthW(hWnd);
                StringBuilder title = new StringBuilder(Math.Max(length + 1, 256));
                GetWindowTextW(hWnd, title, title.Capacity);

                RECT rect;
                GetWindowRect(hWnd, out rect);
                windows.Add(new WindowInfo
                {
                    Hwnd = hWnd,
                    Title = title.ToString(),
                    ProcessId = (int)owner,
                    Visible = IsWindowVisible(hWnd),
                    Style = unchecked((uint)GetWindowLongPtr(hWnd, GWL_STYLE).ToInt64()),
                    ExStyle = unchecked((uint)GetWindowLongPtr(hWnd, GWL_EXSTYLE).ToInt64()),
                    Left = rect.Left,
                    Top = rect.Top,
                    Width = rect.Right - rect.Left,
                    Height = rect.Bottom - rect.Top
                });
                return true;
            }, IntPtr.Zero);
            return windows;
        }
    }
}
