use petsona_runtime::snapshot::{RuntimeSnapshot, RuntimeTextField};

use crate::handles::Engine;
use crate::types::{PetsonaSnapshot, PetsonaTextField, ABI_VERSION};

pub fn snapshot_of(engine: &Engine) -> PetsonaSnapshot {
    let runtime = engine.runtime.snapshot();
    let mut snapshot = from_runtime(runtime);
    snapshot.abi_version = ABI_VERSION;
    if engine.terminal_error.is_some() {
        snapshot.ready = 1;
        snapshot.faulted = 1;
    }
    snapshot
}

fn from_runtime(runtime: RuntimeSnapshot) -> PetsonaSnapshot {
    PetsonaSnapshot {
        abi_version: ABI_VERSION,
        revision: runtime.revision,
        ready: u8::from(runtime.ready),
        faulted: u8::from(runtime.faulted),
        has_pet: u8::from(runtime.has_pet),
        pet_visible: u8::from(runtime.pet_visible),
        click_through: u8::from(runtime.click_through),
        auto_walk: u8::from(runtime.auto_walk),
        gravity_enabled: u8::from(runtime.gravity_enabled),
        always_on_top: u8::from(runtime.always_on_top),
        conversation_inflight: u8::from(runtime.conversation_inflight),
        scale: runtime.scale,
        sprite_index: runtime.sprite_index,
        atlas_width: runtime.atlas_width,
        atlas_height: runtime.atlas_height,
        cell_width: runtime.cell_width,
        cell_height: runtime.cell_height,
        next_frame_ms: runtime.next_frame_ms,
        conversation_history_len: runtime.conversation_history_len,
        state_server_port: runtime.state_server_port,
        reserved_tail: 0,
    }
}

pub fn text_field(field: u32) -> Option<(PetsonaTextField, RuntimeTextField)> {
    let field = match field {
        x if x == PetsonaTextField::State as u32 => {
            (PetsonaTextField::State, RuntimeTextField::State)
        }
        x if x == PetsonaTextField::PetId as u32 => {
            (PetsonaTextField::PetId, RuntimeTextField::PetId)
        }
        x if x == PetsonaTextField::PetName as u32 => {
            (PetsonaTextField::PetName, RuntimeTextField::PetName)
        }
        x if x == PetsonaTextField::Bubble as u32 => {
            (PetsonaTextField::Bubble, RuntimeTextField::Bubble)
        }
        x if x == PetsonaTextField::AtlasPath as u32 => {
            (PetsonaTextField::AtlasPath, RuntimeTextField::AtlasPath)
        }
        x if x == PetsonaTextField::Error as u32 => {
            (PetsonaTextField::Error, RuntimeTextField::Error)
        }
        x if x == PetsonaTextField::Pets as u32 => (PetsonaTextField::Pets, RuntimeTextField::Pets),
        x if x == PetsonaTextField::PersonaId as u32 => {
            (PetsonaTextField::PersonaId, RuntimeTextField::PersonaId)
        }
        x if x == PetsonaTextField::PersonaName as u32 => {
            (PetsonaTextField::PersonaName, RuntimeTextField::PersonaName)
        }
        x if x == PetsonaTextField::Status as u32 => {
            (PetsonaTextField::Status, RuntimeTextField::Status)
        }
        x if x == PetsonaTextField::Position as u32 => {
            (PetsonaTextField::Position, RuntimeTextField::Position)
        }
        x if x == PetsonaTextField::CodexPets as u32 => {
            (PetsonaTextField::CodexPets, RuntimeTextField::CodexPets)
        }
        x if x == PetsonaTextField::Persona as u32 => {
            (PetsonaTextField::Persona, RuntimeTextField::Persona)
        }
        x if x == PetsonaTextField::Personas as u32 => {
            (PetsonaTextField::Personas, RuntimeTextField::Personas)
        }
        x if x == PetsonaTextField::DeepSeekConfig as u32 => (
            PetsonaTextField::DeepSeekConfig,
            RuntimeTextField::DeepSeekConfig,
        ),
        x if x == PetsonaTextField::Memory as u32 => {
            (PetsonaTextField::Memory, RuntimeTextField::Memory)
        }
        x if x == PetsonaTextField::ImportConflict as u32 => (
            PetsonaTextField::ImportConflict,
            RuntimeTextField::ImportConflict,
        ),
        x if x == PetsonaTextField::Models as u32 => {
            (PetsonaTextField::Models, RuntimeTextField::Models)
        }
        _ => return None,
    };
    Some(field)
}
