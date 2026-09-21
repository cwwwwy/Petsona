use std::ptr;

pub const ABI_VERSION: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PetsonaStringView {
    pub ptr: *const u8,
    pub len: usize,
}

impl Default for PetsonaStringView {
    fn default() -> Self {
        Self {
            ptr: ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PetsonaEngineOptions {
    pub abi_version: u32,
    pub home: PetsonaStringView,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PetsonaSnapshot {
    pub abi_version: u32,
    pub revision: u64,
    pub ready: u8,
    pub faulted: u8,
    pub has_pet: u8,
    pub pet_visible: u8,
    pub click_through: u8,
    pub auto_walk: u8,
    pub gravity_enabled: u8,
    pub always_on_top: u8,
    pub conversation_inflight: u8,
    pub scale: f32,
    pub sprite_index: u32,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub next_frame_ms: u32,
    pub conversation_history_len: u32,
    pub state_server_port: u16,
    pub reserved_tail: u16,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsonaTextField {
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
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsonaCommandKind {
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
    SetGazeTarget = 20,
    ClearGaze = 21,
    ScanCodexPets = 22,
    RefreshPets = 11,
    ImportPet = 12,
    ExportPet = 13,
    SelectPet = 14,
    DeletePet = 15,
    UpdatePersona = 16,
    SavePersona = 17,
    SaveDeepSeekKey = 18,
    SendConversation = 19,
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
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PetsonaCommand {
    pub kind: u32,
    pub reserved: u32,
    pub value: f64,
    pub ttl_ms: u64,
    pub text: PetsonaStringView,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsonaStatus {
    Ok = 0,
    InvalidArgument = 1,
    InvalidHandle = 2,
    AlreadyRunning = 3,
    InitializationFailed = 4,
    RuntimeFailed = 5,
    Panic = 6,
    Stopped = 7,
}
