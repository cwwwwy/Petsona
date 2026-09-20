use std::path::PathBuf;
use std::time::Duration;

use petsona_core::config::{DeepSeekConfig, MemoryConfig};
use petsona_core::pet::PetState;
use petsona_runtime::commands::{
    MemoryFactInput, PersonaCreate, PersonaDuplicate, PersonaPatch, RuntimeCommand,
};

use crate::buffers::view_string;
use crate::types::{PetsonaCommand, PetsonaCommandKind, PetsonaStatus};

pub fn convert(command: &PetsonaCommand) -> Result<RuntimeCommand, (PetsonaStatus, String)> {
    let text = view_string(command.text).map_err(|error| {
        (
            PetsonaStatus::InvalidArgument,
            format!("command text {error}"),
        )
    })?;
    match command.kind {
        x if x == PetsonaCommandKind::SetVisibility as u32 => {
            Ok(RuntimeCommand::SetVisibility(command.value >= 0.5))
        }
        x if x == PetsonaCommandKind::SetClickThrough as u32 => {
            Ok(RuntimeCommand::SetClickThrough(command.value >= 0.5))
        }
        x if x == PetsonaCommandKind::SetScale as u32 => {
            if !command.value.is_finite() {
                return Err((
                    PetsonaStatus::InvalidArgument,
                    "scale must be finite".to_string(),
                ));
            }
            Ok(RuntimeCommand::SetScale(command.value as f32))
        }
        x if x == PetsonaCommandKind::SetPosition as u32 => {
            let (x, y) = text.split_once(',').ok_or_else(|| {
                (
                    PetsonaStatus::InvalidArgument,
                    "position must be encoded as x,y".to_string(),
                )
            })?;
            let x = x.trim().parse::<f32>().map_err(|_| {
                (
                    PetsonaStatus::InvalidArgument,
                    "position x is not a number".to_string(),
                )
            })?;
            let y = y.trim().parse::<f32>().map_err(|_| {
                (
                    PetsonaStatus::InvalidArgument,
                    "position y is not a number".to_string(),
                )
            })?;
            if !x.is_finite() || !y.is_finite() {
                return Err((
                    PetsonaStatus::InvalidArgument,
                    "position must be finite".to_string(),
                ));
            }
            Ok(RuntimeCommand::SetPosition { x, y })
        }
        x if x == PetsonaCommandKind::SetAutoWalk as u32 => {
            Ok(RuntimeCommand::SetAutoWalk(command.value >= 0.5))
        }
        x if x == PetsonaCommandKind::SetGravity as u32 => {
            Ok(RuntimeCommand::SetGravity(command.value >= 0.5))
        }
        x if x == PetsonaCommandKind::SetAlwaysOnTop as u32 => {
            Ok(RuntimeCommand::SetAlwaysOnTop(command.value >= 0.5))
        }
        x if x == PetsonaCommandKind::SetGazeTarget as u32 => {
            let (dx, dy) = text.split_once(',').ok_or_else(|| {
                (
                    PetsonaStatus::InvalidArgument,
                    "gaze target must be encoded as dx,dy".to_string(),
                )
            })?;
            let dx = dx.trim().parse::<f32>().map_err(|_| {
                (
                    PetsonaStatus::InvalidArgument,
                    "gaze dx is not a number".to_string(),
                )
            })?;
            let dy = dy.trim().parse::<f32>().map_err(|_| {
                (
                    PetsonaStatus::InvalidArgument,
                    "gaze dy is not a number".to_string(),
                )
            })?;
            if !dx.is_finite() || !dy.is_finite() {
                return Err((
                    PetsonaStatus::InvalidArgument,
                    "gaze target must be finite".to_string(),
                ));
            }
            Ok(RuntimeCommand::SetGazeTarget { dx, dy })
        }
        x if x == PetsonaCommandKind::ClearGaze as u32 => Ok(RuntimeCommand::ClearGaze),
        x if x == PetsonaCommandKind::SetState as u32 => {
            let state = PetState::from_name(&text).ok_or_else(|| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("unknown pet state: {text}"),
                )
            })?;
            let ttl = match command.ttl_ms {
                0 => None,
                milliseconds => Some(Duration::from_millis(milliseconds)),
            };
            Ok(RuntimeCommand::SetState { state, ttl })
        }
        x if x == PetsonaCommandKind::ShowBubble as u32 => {
            let ttl = if command.ttl_ms == 0 {
                Duration::from_secs(8)
            } else {
                Duration::from_millis(command.ttl_ms)
            };
            Ok(RuntimeCommand::ShowBubble { text, ttl })
        }
        x if x == PetsonaCommandKind::ClearBubble as u32 => Ok(RuntimeCommand::ClearBubble),
        x if x == PetsonaCommandKind::RefreshPets as u32 => Ok(RuntimeCommand::RefreshPets),
        x if x == PetsonaCommandKind::ScanCodexPets as u32 => Ok(RuntimeCommand::ScanCodexPets),
        x if x == PetsonaCommandKind::ImportPet as u32 => Ok(RuntimeCommand::ImportPet {
            path: PathBuf::from(text),
            overwrite: command.value >= 0.5,
        }),
        x if x == PetsonaCommandKind::ClearImportConflict as u32 => {
            Ok(RuntimeCommand::ClearImportConflict)
        }
        x if x == PetsonaCommandKind::ExportPet as u32 => {
            let (id, path) = text.split_once('\n').ok_or_else(|| {
                (
                    PetsonaStatus::InvalidArgument,
                    "export must be encoded as id\\npath".to_string(),
                )
            })?;
            Ok(RuntimeCommand::ExportPet {
                id: id.to_string(),
                path: PathBuf::from(path),
            })
        }
        x if x == PetsonaCommandKind::SelectPet as u32 => Ok(RuntimeCommand::SelectPet(text)),
        x if x == PetsonaCommandKind::DeletePet as u32 => Ok(RuntimeCommand::DeletePet(text)),
        x if x == PetsonaCommandKind::UpdatePersona as u32 => {
            let patch: PersonaPatch = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("persona patch is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdatePersona(Box::new(patch)))
        }
        x if x == PetsonaCommandKind::SavePersona as u32 => Ok(RuntimeCommand::SavePersona),
        x if x == PetsonaCommandKind::RefreshPersonas as u32 => Ok(RuntimeCommand::RefreshPersonas),
        x if x == PetsonaCommandKind::CreatePersona as u32 => {
            let spec: PersonaCreate = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("persona create payload is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::CreatePersona(spec))
        }
        x if x == PetsonaCommandKind::DuplicatePersona as u32 => {
            let spec: PersonaDuplicate = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("persona duplicate payload is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::DuplicatePersona(spec))
        }
        x if x == PetsonaCommandKind::SelectPersona as u32 => {
            Ok(RuntimeCommand::SelectPersona(text))
        }
        x if x == PetsonaCommandKind::DeletePersona as u32 => {
            Ok(RuntimeCommand::DeletePersona(text))
        }
        x if x == PetsonaCommandKind::ImportPersona as u32 => Ok(RuntimeCommand::ImportPersona {
            path: PathBuf::from(text),
            overwrite: command.value >= 0.5,
        }),
        x if x == PetsonaCommandKind::ExportPersona as u32 => {
            let (id, path) = text.split_once('\n').ok_or_else(|| {
                (
                    PetsonaStatus::InvalidArgument,
                    "persona export must be encoded as id\\npath".to_string(),
                )
            })?;
            Ok(RuntimeCommand::ExportPersona {
                id: id.to_string(),
                path: PathBuf::from(path),
            })
        }
        x if x == PetsonaCommandKind::UpdateDeepSeekConfig as u32 => {
            let config: DeepSeekConfig = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("DeepSeek config is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdateDeepSeekConfig(config))
        }
        x if x == PetsonaCommandKind::UpdateMemoryConfig as u32 => {
            let config: MemoryConfig = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("memory config is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdateMemoryConfig(config))
        }
        x if x == PetsonaCommandKind::RememberFact as u32 => {
            let input: MemoryFactInput = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("memory fact is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::RememberFact(input))
        }
        x if x == PetsonaCommandKind::ForgetFact as u32 => Ok(RuntimeCommand::ForgetFact(text)),
        x if x == PetsonaCommandKind::ClearMemory as u32 => Ok(RuntimeCommand::ClearMemory),
        x if x == PetsonaCommandKind::SaveDeepSeekKey as u32 => {
            Ok(RuntimeCommand::SaveDeepSeekKey(text))
        }
        x if x == PetsonaCommandKind::SendConversation as u32 => {
            Ok(RuntimeCommand::SendConversation(text))
        }
        _ => Err((
            PetsonaStatus::InvalidArgument,
            format!("unknown command kind: {}", command.kind),
        )),
    }
}
