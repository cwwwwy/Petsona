using System.Drawing;
using System.Drawing.Imaging;
using Microsoft.UI.Xaml;

namespace Petsona.Native;

/// <summary>
/// Colours for the hand-drawn speech bubble, plus the diagnostics snapshot hook.
///
/// The bubble is painted with GDI+ instead of XAML, so it does not pick up the
/// theme by itself; before this helper it stayed white-on-black-text in dark
/// mode (polish 3.4). `PETSONA_BUBBLE_SNAPSHOT=<path>` writes the rendered
/// bitmap to disk — a layered window cannot be captured with PrintWindow.
/// </summary>
internal readonly record struct BubblePalette(
    Color Fill,
    Color Border,
    Color Text,
    Color Accent,
    Color ProgressTrack)
{
    internal static BubblePalette Current()
    {
        var dark = Environment.GetEnvironmentVariable("PETSONA_SETTINGS_THEME") switch
        {
            "dark" => true,
            "light" => false,
            _ => Application.Current.RequestedTheme == ApplicationTheme.Dark,
        };

        return dark
            ? new BubblePalette(
                Color.FromArgb(242, 43, 43, 43),
                Color.FromArgb(70, 255, 255, 255),
                Color.FromArgb(240, 240, 240),
                Color.FromArgb(0, 153, 255),
                Color.FromArgb(70, 255, 255, 255))
            : new BubblePalette(
                Color.FromArgb(242, 255, 255, 255),
                Color.FromArgb(70, 0, 0, 0),
                Color.FromArgb(32, 32, 32),
                Color.FromArgb(0, 120, 212),
                Color.FromArgb(45, 0, 0, 0));
    }

    /// <summary>Diagnostics: dump the freshly rendered bubble for review.</summary>
    internal static void Snapshot(Bitmap bitmap)
    {
        var path = Environment.GetEnvironmentVariable("PETSONA_BUBBLE_SNAPSHOT");
        if (string.IsNullOrWhiteSpace(path))
        {
            return;
        }

        try
        {
            bitmap.Save(path, ImageFormat.Png);
        }
        catch (Exception)
        {
            // Diagnostics must never break rendering.
        }
    }
}
