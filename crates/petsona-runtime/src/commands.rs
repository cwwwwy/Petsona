//! Commands accepted by the serialized runtime worker.

use std::path::PathBuf;
use std::time::Duration;

use petsona_core::config::{DeepSeekConfig, GreetingConfig, MemoryConfig};
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
    ExportPersona {
        id: String,
        path: PathBuf,
    },
    UpdateDeepSeekConfig(DeepSeekConfig),
    UpdateMemoryConfig(MemoryConfig),
    UpdateGreetingConfig(GreetingConfig),
    RememberFact(MemoryFactInput),
    ForgetFact(String),
    UpdateFact(MemoryFactUpdate),
    ClearMemory,
    ClearMemoryScope(MemoryScope),
    ExportMemory(PathBuf),
    ImportMemory(PathBuf),
    SaveDeepSeekKey(String),
    ListModels,
    ModelsResult(Result<Vec<String>, String>),
    SendConversation(String),
    ConversationResult(Result<String, String>),
    GreetingResult(Result<String, String>),
    Stop,
}
