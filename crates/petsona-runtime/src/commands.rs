//! Commands accepted by the serialized runtime worker.

use std::path::PathBuf;
use std::time::Duration;

use petsona_core::config::{DeepSeekConfig, MemoryConfig};
use petsona_core::pet::PetState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonaPatch {
    pub description: Option<String>,
    pub avatar_pet: Option<String>,
    pub name: Option<String>,
    pub tone: Option<String>,
    pub verbosity: Option<String>,
    pub language: Option<String>,
    pub emoji: Option<bool>,
    pub greeting: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub memory_enabled: Option<bool>,
    pub memory_window_turns: Option<u32>,
    pub memory_long_term: Option<bool>,
    pub memory_summarize_after_turns: Option<u32>,
    pub tts_enabled: Option<bool>,
    pub tts_voice: Option<String>,
    pub tts_rate: Option<f32>,
    pub proactive_enabled: Option<bool>,
    pub proactive_idle_minutes: Option<u32>,
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
    RememberFact(MemoryFactInput),
    ForgetFact(String),
    ClearMemory,
    SaveDeepSeekKey(String),
    SendConversation(String),
    ConversationResult(Result<String, String>),
    Stop,
}
