using Petsona.Core;
using Petsona.Core.Interop;
using Xunit;

namespace Petsona.Tests;

public sealed class SpriteFrameTests
{
    [Fact]
    public void ColumnsComeFromAtlasAndCellWidth()
    {
        Assert.Equal(8, SpriteFrame.Columns(Snapshot(1536, 1408, 192, 128)));
        Assert.Equal(1, SpriteFrame.Columns(Snapshot(0, 0, 0, 0)));
    }

    [Fact]
    public void SpriteIndicesAreRowMajor()
    {
        var snapshot = Snapshot(1536, 1408, 192, 128);

        Assert.Equal(0, SpriteFrame.Row(snapshot, 0));
        Assert.Equal(0, SpriteFrame.Column(snapshot, 0));
        Assert.Equal(0, SpriteFrame.Row(snapshot, 7));
        Assert.Equal(7, SpriteFrame.Column(snapshot, 7));
        Assert.Equal(1, SpriteFrame.Row(snapshot, 8));
        Assert.Equal(0, SpriteFrame.Column(snapshot, 8));
        Assert.Equal(10, SpriteFrame.Row(snapshot, 80));
        Assert.Equal(0, SpriteFrame.Column(snapshot, 80));
    }

    [Fact]
    public void SourceOriginUsesTheCellGrid()
    {
        var snapshot = Snapshot(1536, 1408, 192, 128);

        Assert.Equal((0, 0), SpriteFrame.SourceOrigin(snapshot, 0));
        Assert.Equal((192 * 3, 0), SpriteFrame.SourceOrigin(snapshot, 3));
        Assert.Equal((192 * 2, 128 * 9), SpriteFrame.SourceOrigin(snapshot, 9 * 8 + 2));
        Assert.Equal((192 * 1, 0), SpriteFrame.CellOrigin(snapshot, SpriteFrame.IdleRow, 1));
    }

    [Fact]
    public void CellPointClampsInsideTheCell()
    {
        var snapshot = Snapshot(1536, 1408, 192, 128);

        Assert.Equal((0, 0), SpriteFrame.CellPoint(snapshot, 0, 0, 384, 256));
        Assert.Equal((191, 127), SpriteFrame.CellPoint(snapshot, 383, 255, 384, 256));
        Assert.Equal((95, 63), SpriteFrame.CellPoint(snapshot, 191, 127, 384, 256));
        Assert.Equal((0, 0), SpriteFrame.CellPoint(snapshot, -5, -5, 384, 256));
        Assert.Equal((191, 127), SpriteFrame.CellPoint(snapshot, 9999, 9999, 384, 256));
    }

    private static PetsonaSnapshot Snapshot(uint atlasWidth, uint atlasHeight, uint cellWidth, uint cellHeight)
    {
        return new PetsonaSnapshot
        {
            AtlasWidth = atlasWidth,
            AtlasHeight = atlasHeight,
            CellWidth = cellWidth,
            CellHeight = cellHeight,
        };
    }
}
