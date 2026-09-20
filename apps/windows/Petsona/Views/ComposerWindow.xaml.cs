using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;
using Windows.UI.Core;

namespace Petsona.Views;

/// <summary>
/// Composer input window. Enter sends, Shift+Enter inserts a newline, Esc
/// closes and the draft survives closing. IME composition keeps Enter for
/// candidate confirmation (the runtime never sees that key).
/// </summary>
public sealed partial class ComposerWindow : Window
{
    private bool _imeComposing;

    public ComposerWindow(string draft)
    {
        InitializeComponent();
        AppWindow.Resize(new Windows.Graphics.SizeInt32(380, 190));
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
