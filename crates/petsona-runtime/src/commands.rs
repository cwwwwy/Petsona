//! Commands accepted by the serialized runtime worker.

use std::path::PathBuf;
use std::time::Duration;

use petsona_core::pet::PetState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonaPatch {
    pub name: Option<String>,
    pub tone: Option<String>,
    pub language: Option<String>,
    pub greeting: Option<String>,
    pub system_prompt: Option<String>,
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
    ExportPet {
        id: String,
        path: PathBuf,
    },
    SelectPet(String),
    DeletePet(String),
    UpdatePersona(PersonaPatch),
    SavePersona,
    SaveDeepSeekKey(String),
    SendConversation(String),
    ConversationResult(Result<String, String>),
    Stop,
}
