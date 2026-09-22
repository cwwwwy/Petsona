namespace Petsona.Core;

/// <summary>A physical-pixel rectangle. Windows work areas and window APIs are in this unit.</summary>
public readonly record struct PixelRect(int Left, int Top, int Right, int Bottom)
{
    public int Width => Right - Left;

    public int Height => Bottom - Top;

    public static PixelRect FromBounds(int left, int top, int width, int height)
    {
        return new PixelRect(left, top, left + width, top + height);
    }
}
