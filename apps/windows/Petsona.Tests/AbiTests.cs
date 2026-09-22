using System.Runtime.InteropServices;
using Petsona.Core.Interop;
using Xunit;

namespace Petsona.Tests;

/// <summary>Layout and ordinal checks against contracts/petsona.h (ABI 3).</summary>
public sealed class AbiTests
{
    [Fact]
    public void AbiVersionMatchesTheContract()
    {
        Assert.Equal(3u, NativeMethods.AbiVersion);
    }

    [Fact]
    public unsafe void StructLayoutsMatchTheRustContract()
    {
        Assert.Equal(16, sizeof(PetsonaStringView));
        Assert.Equal(24, sizeof(PetsonaEngineOptions));
        Assert.Equal(40, sizeof(PetsonaCommand));
        Assert.Equal(64, sizeof(PetsonaSnapshot));

        PetsonaSnapshot snapshot;
        Assert.Equal(8, (int)((byte*)&snapshot.Revision - (byte*)&snapshot));
        Assert.Equal(28, (int)((byte*)&snapshot.Scale - (byte*)&snapshot));
        Assert.Equal(60, (int)((byte*)&snapshot.StateServerPort - (byte*)&snapshot));

        PetsonaCommand command;
        Assert.Equal(8, (int)((byte*)&command.Value - (byte*)&command));
        Assert.Equal(16, (int)((byte*)&command.TtlMs - (byte*)&command));
        Assert.Equal(24, (int)((byte*)&command.Text - (byte*)&command));
    }

    [Fact]
    public void EnumerationsMatchTheAbiOrdinals()
    {
        Assert.Equal(0, (int)PetsonaTextField.State);
        Assert.Equal(16, (int)PetsonaTextField.ImportConflict);
        Assert.Equal(18, (int)PetsonaTextField.BubbleTiming);
        Assert.Equal(1u, (uint)PetsonaCommandKind.SetVisibility);
        Assert.Equal(10u, (uint)PetsonaCommandKind.SetAlwaysOnTop);
        Assert.Equal(20u, (uint)PetsonaCommandKind.SetGazeTarget);
        Assert.Equal(35u, (uint)PetsonaCommandKind.ClearImportConflict);
        Assert.Equal(43u, (uint)PetsonaCommandKind.SetBubblePaused);
        Assert.Equal(0, (int)PetsonaStatus.Ok);
        Assert.Equal(7, (int)PetsonaStatus.Stopped);
    }

    [Fact]
    public void StatusEnumMatchesTheHeader()
    {
        Assert.Equal(PetsonaStatus.InvalidArgument, PetsonaStatus.InvalidArgument);
        Assert.Equal(5, (int)PetsonaStatus.RuntimeFailed);
        Assert.Equal(6, (int)PetsonaStatus.Panic);
    }
}
