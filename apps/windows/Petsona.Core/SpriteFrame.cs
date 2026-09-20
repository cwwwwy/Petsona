using Petsona.Core.Interop;

namespace Petsona.Core;

/// <summary>
/// Pure projection of a runtime snapshot onto the sprite-sheet grid. The
/// sheet is row-major; row 0 is the idle animation used for the clickable
/// union fallback.
/// </summary>
public static class SpriteFrame
{
    public const int IdleRow = 0;

    /// <summary>Column count of the sheet, derived from atlas and cell width.</summary>
    public static int Columns(PetsonaSnapshot snapshot)
    {
        var cellWidth = Math.Max(1u, snapshot.CellWidth);
        return (int)Math.Max(1u, snapshot.AtlasWidth / cellWidth);
    }

    public static int Row(PetsonaSnapshot snapshot, uint spriteIndex)
    {
        return (int)(spriteIndex / (uint)Columns(snapshot));
    }

    public static int Column(PetsonaSnapshot snapshot, uint spriteIndex)
    {
        return (int)(spriteIndex % (uint)Columns(snapshot));
    }

    /// <summary>Top-left origin of one sprite inside the sheet, in sheet pixels.</summary>
    public static (int X, int Y) SourceOrigin(PetsonaSnapshot snapshot, uint spriteIndex)
    {
        return (Column(snapshot, spriteIndex) * (int)snapshot.CellWidth,
                Row(snapshot, spriteIndex) * (int)snapshot.CellHeight);
    }

    /// <summary>Top-left origin of a cell in a specific row, in sheet pixels.</summary>
    public static (int X, int Y) CellOrigin(PetsonaSnapshot snapshot, int row, int column)
    {
        return (column * (int)snapshot.CellWidth, row * (int)snapshot.CellHeight);
    }

    /// <summary>Maps a point inside the scaled window onto cell-local pixels.</summary>
    public static (int X, int Y) CellPoint(PetsonaSnapshot snapshot, int pointX, int pointY, int windowWidth, int windowHeight)
    {
        var cellWidth = Math.Max(1, (int)snapshot.CellWidth);
        var cellHeight = Math.Max(1, (int)snapshot.CellHeight);
        var x = Clamp(pointX * cellWidth / Math.Max(1, windowWidth), 0, cellWidth - 1);
        var y = Clamp(pointY * cellHeight / Math.Max(1, windowHeight), 0, cellHeight - 1);
        return (x, y);
    }

    private static int Clamp(int value, int min, int max)
    {
        return Math.Min(max, Math.Max(min, value));
    }
}
