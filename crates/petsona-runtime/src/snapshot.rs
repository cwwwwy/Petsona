//! Immutable data projected from the runtime for native frontends.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTextField {
    State,
    PetId,
    PetName,
    Bubble,
    AtlasPath,
    Error,
    Pets,
    PersonaId,
    PersonaName,
    Status,
    Position,
    CodexPets,
    Persona,
    Personas,
    DeepSeekConfig,
    Memory,
    ImportConflict,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSnapshot {
    pub ready: bool,
    pub faulted: bool,
    pub revision: u64,
    pub has_pet: bool,
    pub pet_visible: bool,
    pub click_through: bool,
    pub scale: f32,
    pub sprite_index: u32,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub next_frame_ms: u32,
    pub state_server_port: u16,
    pub first_run: bool,
    pub auto_walk: bool,
    pub gravity_enabled: bool,
    pub always_on_top: bool,
    pub conversation_inflight: bool,
    pub conversation_history_len: u32,
    pub pet_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeTexts {
    pub state: String,
    pub pet_id: String,
    pub pet_name: String,
    pub bubble: String,
    pub atlas_path: String,
    pub error: String,
    pub pets: String,
    pub persona_id: String,
    pub persona_name: String,
    pub status: String,
    pub position: String,
    pub codex_pets: String,
    pub persona: String,
    pub personas: String,
    pub deepseek_config: String,
    pub memory: String,
    pub import_conflict: String,
}

impl RuntimeTexts {
    pub fn get(&self, field: RuntimeTextField) -> &str {
        match field {
            RuntimeTextField::State => &self.state,
            RuntimeTextField::PetId => &self.pet_id,
            RuntimeTextField::PetName => &self.pet_name,
            RuntimeTextField::Bubble => &self.bubble,
            RuntimeTextField::AtlasPath => &self.atlas_path,
            RuntimeTextField::Error => &self.error,
            RuntimeTextField::Pets => &self.pets,
            RuntimeTextField::PersonaId => &self.persona_id,
            RuntimeTextField::PersonaName => &self.persona_name,
            RuntimeTextField::Status => &self.status,
            RuntimeTextField::Position => &self.position,
            RuntimeTextField::CodexPets => &self.codex_pets,
            RuntimeTextField::Persona => &self.persona,
            RuntimeTextField::Personas => &self.personas,
            RuntimeTextField::DeepSeekConfig => &self.deepseek_config,
            RuntimeTextField::Memory => &self.memory,
            RuntimeTextField::ImportConflict => &self.import_conflict,
        }
    }
}
