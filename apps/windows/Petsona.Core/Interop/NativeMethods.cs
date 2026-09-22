using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Petsona.Core.Interop;

/// <summary>Mirrors <c>PetsonaStatus</c> in <c>contracts/petsona.h</c> (ABI 3).</summary>
public enum PetsonaStatus
{
    Ok = 0,
    InvalidArgument = 1,
    InvalidHandle = 2,
    AlreadyRunning = 3,
    InitializationFailed = 4,
    RuntimeFailed = 5,
    Panic = 6,
    Stopped = 7,
}

/// <summary>Mirrors <c>PetsonaTextField</c> in <c>contracts/petsona.h</c> (ABI 3).</summary>
public enum PetsonaTextField
{
    State = 0,
    PetId = 1,
    PetName = 2,
    Bubble = 3,
    AtlasPath = 4,
    Error = 5,
    Pets = 6,
    PersonaId = 7,
    PersonaName = 8,
    Status = 9,
    Position = 10,
    CodexPets = 11,
    Persona = 12,
    Personas = 13,
    DeepSeekConfig = 14,
    Memory = 15,
    ImportConflict = 16,
    Models = 17,
    BubbleTiming = 18,
}

/// <summary>Mirrors <c>PetsonaCommandKind</c> in <c>contracts/petsona.h</c> (ABI 3).</summary>
public enum PetsonaCommandKind : uint
{
    SetVisibility = 1,
    SetClickThrough = 2,
    SetScale = 3,
    SetState = 4,
    ShowBubble = 5,
    ClearBubble = 6,
    SetPosition = 7,
    SetAutoWalk = 8,
    SetGravity = 9,
    SetAlwaysOnTop = 10,
    RefreshPets = 11,
    ImportPet = 12,
    ExportPet = 13,
    SelectPet = 14,
    DeletePet = 15,
    UpdatePersona = 16,
    SavePersona = 17,
    SaveDeepSeekKey = 18,
    SendConversation = 19,
    SetGazeTarget = 20,
    ClearGaze = 21,
    ScanCodexPets = 22,
    UpdateDeepSeekConfig = 23,
    UpdateMemoryConfig = 24,
    RememberFact = 25,
    ForgetFact = 26,
    ClearMemory = 27,
    RefreshPersonas = 28,
    CreatePersona = 29,
    DuplicatePersona = 30,
    SelectPersona = 31,
    DeletePersona = 32,
    ImportPersona = 33,
    ExportPersona = 34,
    ClearImportConflict = 35,
    UpdateGreetingConfig = 36,
    UpdateMemoryFact = 37,
    ClearMemoryScope = 38,
    ExportMemory = 39,
    ImportMemory = 40,
    ListModels = 41,
    ResetPersona = 42,
    SetBubblePaused = 43,
}

/// <summary>Mirrors <c>PetsonaStringView</c>: caller-owned bytes borrowed for one call.</summary>
[StructLayout(LayoutKind.Sequential)]
public unsafe struct PetsonaStringView
{
    public byte* Ptr;
    public nuint Len;
}

/// <summary>Mirrors <c>PetsonaEngineOptions</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
public struct PetsonaEngineOptions
{
    public uint AbiVersion;
    public PetsonaStringView Home;
}

/// <summary>Mirrors <c>PetsonaSnapshot</c>; layout is verified by AbiTests.</summary>
[StructLayout(LayoutKind.Sequential)]
public struct PetsonaSnapshot
{
    public uint AbiVersion;
    public ulong Revision;
    public byte Ready;
    public byte Faulted;
    public byte HasPet;
    public byte PetVisible;
    public byte ClickThrough;
    public byte AutoWalk;
    public byte GravityEnabled;
    public byte AlwaysOnTop;
    public byte ConversationInflight;
    public float Scale;
    public uint SpriteIndex;
    public uint AtlasWidth;
    public uint AtlasHeight;
    public uint CellWidth;
    public uint CellHeight;
    public uint NextFrameMs;
    public uint ConversationHistoryLen;
    public ushort StateServerPort;
    public ushort ReservedTail;
}

/// <summary>Mirrors <c>PetsonaCommand</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
public struct PetsonaCommand
{
    public uint Kind;
    public uint Reserved;
    public double Value;
    public ulong TtlMs;
    public PetsonaStringView Text;
}

/// <summary>Raw ABI surface. All calls must run on the creating thread.</summary>
internal static unsafe partial class NativeMethods
{
    public const uint AbiVersion = 3;

    private const string LibraryName = "petsona_ffi";

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial PetsonaStatus petsona_engine_create(PetsonaEngineOptions* options, nint* outEngine);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void petsona_engine_destroy(nint engine);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial PetsonaStatus petsona_engine_tick(nint engine);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial PetsonaStatus petsona_engine_snapshot(nint engine, PetsonaSnapshot* destination);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nuint petsona_engine_copy_text(nint engine, uint field, byte* destination, nuint capacity);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial PetsonaStatus petsona_engine_command(nint engine, PetsonaCommand* command);

    [LibraryImport(LibraryName)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nuint petsona_last_error_copy(byte* destination, nuint capacity);
}
