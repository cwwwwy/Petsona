using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Petsona.Core;
using Petsona.Native;
using Windows.System;
using Windows.UI.Core;

namespace Petsona.Views;

/// <summary>
/// Compact, chrome-free composer. Enter sends, Shift+Enter inserts a newline,
/// Esc closes and the draft survives closing. IME composition keeps Enter for
/// candidate confirmation.
/// </summary>
public sealed partial class ComposerWindow : Window
{
    private bool _imeComposing;

    public ComposerWindow(string draft)
    {
        InitializeComponent();

        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.SetBorderAndTitleBar(false, false);
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
        }

        AppWindow.Resize(new Windows.Graphics.SizeInt32(
            OverlayLayout.ComposerDesiredWidth,
            OverlayLayout.ComposerHeight));
        AppWindow.IsShownInSwitchers = false;

        var handle = WinRT.Interop.WindowNative.GetWindowHandle(this);
        NativeWin32.MakeToolWindow(handle);
        NativeWin32.MakeBorderlessPopup(handle);
        WindowIcon.Apply(handle);
        Petsona.Native.WindowTheme.Apply(handle, ElementTheme.Default);
        Petsona.Native.WindowTheme.RemoveBorder(handle);
        Activated += (_, _) =>
        {
            NativeWin32.MakeBorderlessPopup(handle);
            Petsona.Native.WindowTheme.RemoveBorder(handle);
        };

        InputBox.Text = draft;
        // PreviewKeyDown tunnels ahead of the TextBox default action; a plain
        // KeyDown handler still lets WinUI insert the newline first.
        InputBox.PreviewKeyDown += OnInputKeyDown;
        InputBox.SelectionChanged += (_, _) => CaretMoved?.Invoke();
        InputBox.TextChanged += (_, _) => CaretMoved?.Invoke();
        InputBox.TextCompositionStarted += (_, _) => _imeComposing = true;
        InputBox.TextCompositionEnded += (_, _) => _imeComposing = false;
    }

    public event Action<string>? Submitted;

    public event Action? CaretMoved;

    public string CurrentText => InputBox.Text;

    public void FocusInput()
    {
        InputBox.Focus(FocusState.Programmatic);
        InputBox.SelectionStart = InputBox.Text.Length;
    }

    public void ClearInput()
    {
        InputBox.Text = string.Empty;
    }

    private void OnSendClick(object sender, RoutedEventArgs e)
    {
        Submit();
    }

    private void OnInputKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Escape)
        {
            e.Handled = true;
            Close();
            return;
        }

        if (e.Key != VirtualKey.Enter)
        {
            return;
        }

        if (_imeComposing)
        {
            // The IME already consumed this key to confirm its candidate.
            // Marking it handled stops the TextBox default action (newline)
            // while leaving the committed text in place.
            e.Handled = true;
            return;
        }

        var shift = InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Shift);
        if (shift.HasFlag(CoreVirtualKeyStates.Down))
        {
            return; // Shift+Enter keeps the default newline behaviour.
        }

        e.Handled = true;
        Submit();
    }

    private void Submit()
    {
        var text = InputBox.Text.Trim();
        if (text.Length == 0)
        {
            return;
        }

        Submitted?.Invoke(text);
    }
}
