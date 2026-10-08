//! Commands accepted by the serialized runtime worker.

use std::path::PathBuf;
use std::time::Duration;

use petsona_core::config::{
    ConversationConfig, DeepSeekConfig, GreetingConfig, MemoryConfig, WindowPosition,
};
use petsona_core::deepseek::GeneratedPersonaProfile;
use petsona_core::memory::MemorySuggestion;
use petsona_core::persona::PersonaStyleProfile;
use petsona_core::persona_source::ParsedPersonaSource;
use petsona_core::pet::PetState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonaPatch {
    pub name: Option<String>,
    pub tone: Option<String>,
    pub verbosity: Option<String>,
    pub emoji: Option<bool>,
    pub greeting: Option<String>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonaCreate {
    pub id: String,
    pub name: String,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaDuplicate {
    pub source_id: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFactInput {
    pub key: String,
    pub value: String,
    pub confidence: Option<f32>,
}

/// Which part of a persona's memory a clear command targets (REQ-S15).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemoryScope {
    #[default]
    All,
    Facts,
    Events,
}

impl MemoryScope {
    pub fn from_wire(value: f64) -> Self {
        match value as i64 {
            1 => Self::Facts,
            2 => Self::Events,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFactUpdate {
    pub id: String,
    pub key: String,
    pub value: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRequest {
    pub request_id: String,
    pub pet_id: String,
    pub text: String,
    #[serde(default)]
    pub retry_turn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCandidateReview {
    pub candidate_id: String,
    pub accept: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaSourceParseRequest {
    pub request_id: String,
    pub label: String,
    pub format: String,
    pub path: Option<PathBuf>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaProfileRequest {
    pub request_id: String,
    pub source_id: String,
    pub kind: String,
    pub label: String,
    pub description: String,
    pub target_speaker: String,
    #[serde(default)]
    pub target_speaker_label: String,
    pub start_index: usize,
    pub end_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPersonaDraftRequest {
    pub draft_id: String,
    pub pet_id: String,
    pub name: String,
    pub style: PersonaStyleProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaPreviewRequest {
    pub request_id: String,
    pub pet_id: String,
    pub draft_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct ImportConflict {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

impl ImportConflict {
    pub fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "path": self.path,
        })
    }
}

#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    /// A UI/event-loop wakeup. The worker also ticks on its own deadline.
    Wake,
    /// Ask the worker to advance TTLs and the animation clock immediately.
    Tick,
    SetVisibility(bool),
    SetClickThrough(bool),
    SetScale(f32),
    SetPosition {
        x: f32,
        y: f32,
    },
    SetWindowPosition(WindowPosition),
    SetAutoWalk(bool),
    SetGravity(bool),
    SetAlwaysOnTop(bool),
    SetGazeTarget {
        dx: f32,
        dy: f32,
    },
    ClearGaze,
    SetState {
        state: PetState,
        ttl: Option<Duration>,
    },
    ShowBubble {
        text: String,
        ttl: Duration,
    },
    SetBubblePaused(bool),
    ClearBubble,
    RefreshPets,
    ScanCodexPets,
    ImportPet {
        path: PathBuf,
        overwrite: bool,
    },
    ClearImportConflict,
    ExportPet {
        id: String,
        path: PathBuf,
    },
    SelectPet(String),
    DeletePet(String),
    UpdatePersona(Box<PersonaPatch>),
    SavePersona,
    ResetPersona,
    RefreshPersonas,
    CreatePersona(PersonaCreate),
    DuplicatePersona(PersonaDuplicate),
    SelectPersona(String),
    DeletePersona(String),
    ImportPersona {
        path: PathBuf,
        overwrite: bool,
    },
    /// Apply an imported persona document to the active pet's stable persona id.
    ApplyImportedPersona(PathBuf),
    ExportPersona {
        id: String,
        path: PathBuf,
    },
    UpdateDeepSeekConfig(DeepSeekConfig),
    UpdateMemoryConfig(MemoryConfig),
    UpdateConversationConfig(ConversationConfig),
    UpdateGreetingConfig(GreetingConfig),
    RememberFact(MemoryFactInput),
    ForgetFact(String),
    UpdateFact(MemoryFactUpdate),
    ClearMemory,
    ClearMemoryScope(MemoryScope),
    ExportMemory(PathBuf),
    ImportMemory(PathBuf),
    SaveDeepSeekKey(String),
    /// Result of the asynchronous OS credential-presence probe. Startup must
    /// not block the worker on a locked or unavailable system keychain; the
    /// request id prevents older probes from replacing a newer configuration.
    KeyPresenceResult {
        provider: String,
        request_id: u64,
        present: bool,
    },
    ListModels,
    ModelsResult(Result<Vec<String>, String>),
    SendConversation(String),
    StartConversation(ConversationRequest),
    CancelConversation(String),
    ClearConversationHistory(String),
    LoadEarlierConversationHistory(String),
    ConversationStreamChunk {
        request_id: String,
        pet_id: String,
        persona_id: String,
        chunk: String,
    },
    ConversationStreamFinished {
        request_id: String,
        pet_id: String,
        persona_id: String,
        result: Result<(), String>,
    },
    ReviewMemoryCandidate(MemoryCandidateReview),
    MemoryLearningFinished {
        request_id: String,
        pet_id: String,
        persona_id: String,
        through_turn_id: String,
        result: Result<Vec<MemorySuggestion>, String>,
    },
    ParsePersonaSource(PersonaSourceParseRequest),
    PersonaSourceParseFinished {
        request_id: String,
        result: Result<ParsedPersonaSource, String>,
    },
    GeneratePersonaProfile(PersonaProfileRequest),
    PersonaProfileFinished {
        request_id: String,
        pet_id: String,
        persona_id: String,
        result: Result<GeneratedPersonaProfile, String>,
    },
    ApplyPersonaDraft(ApplyPersonaDraftRequest),
    ClearPersonaDraft,
    PreviewPersonaDraftRequest(PersonaPreviewRequest),
    PersonaPreviewFinished {
        request_id: String,
        pet_id: String,
        draft_id: String,
        result: Result<String, String>,
    },
    ConversationResult(Result<String, String>),
    GreetingResult(Result<String, String>),
    Stop,
}
