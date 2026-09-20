using Microsoft.UI.Xaml.Media;

namespace Petsona.Views;

/// <summary>Row model for the pet library and Codex candidate lists.</summary>
public sealed class PetListItem
{
    public required string Id { get; init; }

    public required string Label { get; init; }

    /// <summary>Codex candidates carry the import path.</summary>
    public string? Path { get; init; }

    public ImageSource? Thumbnail { get; set; }
}
