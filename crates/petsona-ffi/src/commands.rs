use std::path::PathBuf;
use std::time::Duration;

use petsona_core::config::{ConversationConfig, DeepSeekConfig, GreetingConfig, MemoryConfig};
use petsona_core::pet::PetState;
use petsona_runtime::commands::{
    ApplyPersonaDraftRequest, ConversationRequest, MemoryCandidateReview, MemoryFactInput,
    MemoryFactUpdate, MemoryScope, PersonaCreate, PersonaDuplicate, PersonaPatch,
    PersonaPreviewRequest, PersonaProfileRequest, PersonaSourceParseRequest, RuntimeCommand,
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
        x if x == PetsonaCommandKind::SetWindowPosition as u32 => {
            let position: petsona_core::config::WindowPosition = serde_json::from_str(&text)
                .map_err(|error| {
                    (
                        PetsonaStatus::InvalidArgument,
                        format!("window position is not valid JSON: {error}"),
                    )
                })?;
            if !position.x.is_finite()
                || !position.y.is_finite()
                || !position.backing_scale.is_finite()
                || position.backing_scale <= 0.0
            {
                return Err((
                    PetsonaStatus::InvalidArgument,
                    "window position coordinates and scale must be finite".to_string(),
                ));
            }
            Ok(RuntimeCommand::SetWindowPosition(position))
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
        x if x == PetsonaCommandKind::SetBubblePaused as u32 => {
            Ok(RuntimeCommand::SetBubblePaused(command.value >= 0.5))
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
        x if x == PetsonaCommandKind::ResetPersona as u32 => Ok(RuntimeCommand::ResetPersona),
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
        x if x == PetsonaCommandKind::UpdateGreetingConfig as u32 => {
            let config: GreetingConfig = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("greeting config is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdateGreetingConfig(config))
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
        x if x == PetsonaCommandKind::UpdateConversationConfig as u32 => {
            let config: ConversationConfig = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("conversation config is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdateConversationConfig(config))
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
        x if x == PetsonaCommandKind::UpdateMemoryFact as u32 => {
            let update: MemoryFactUpdate = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("memory fact update is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::UpdateFact(update))
        }
        x if x == PetsonaCommandKind::ClearMemoryScope as u32 => Ok(
            RuntimeCommand::ClearMemoryScope(MemoryScope::from_wire(command.value)),
        ),
        x if x == PetsonaCommandKind::ExportMemory as u32 => {
            Ok(RuntimeCommand::ExportMemory(PathBuf::from(text)))
        }
        x if x == PetsonaCommandKind::ImportMemory as u32 => {
            Ok(RuntimeCommand::ImportMemory(PathBuf::from(text)))
        }
        x if x == PetsonaCommandKind::ForgetFact as u32 => Ok(RuntimeCommand::ForgetFact(text)),
        x if x == PetsonaCommandKind::ClearMemory as u32 => Ok(RuntimeCommand::ClearMemory),
        x if x == PetsonaCommandKind::SaveDeepSeekKey as u32 => {
            Ok(RuntimeCommand::SaveDeepSeekKey(text))
        }
        x if x == PetsonaCommandKind::ListModels as u32 => Ok(RuntimeCommand::ListModels),
        x if x == PetsonaCommandKind::SendConversation as u32 => {
            Ok(RuntimeCommand::SendConversation(text))
        }
        x if x == PetsonaCommandKind::StartConversation as u32 => {
            let request: ConversationRequest = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("conversation request is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::StartConversation(request))
        }
        x if x == PetsonaCommandKind::CancelConversation as u32 => {
            Ok(RuntimeCommand::CancelConversation(text))
        }
        x if x == PetsonaCommandKind::ClearConversationHistory as u32 => {
            Ok(RuntimeCommand::ClearConversationHistory(text))
        }
        x if x == PetsonaCommandKind::LoadEarlierConversationHistory as u32 => {
            Ok(RuntimeCommand::LoadEarlierConversationHistory(text))
        }
        x if x == PetsonaCommandKind::ReviewMemoryCandidate as u32 => {
            let review: MemoryCandidateReview = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("memory candidate review is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::ReviewMemoryCandidate(review))
        }
        x if x == PetsonaCommandKind::ParsePersonaSource as u32 => {
            let request: PersonaSourceParseRequest =
                serde_json::from_str(&text).map_err(|error| {
                    (
                        PetsonaStatus::InvalidArgument,
                        format!("persona source request is not valid JSON: {error}"),
                    )
                })?;
            Ok(RuntimeCommand::ParsePersonaSource(request))
        }
        x if x == PetsonaCommandKind::GeneratePersonaProfile as u32 => {
            let request: PersonaProfileRequest = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("persona generation request is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::GeneratePersonaProfile(request))
        }
        x if x == PetsonaCommandKind::ApplyPersonaDraft as u32 => {
            let request: ApplyPersonaDraftRequest =
                serde_json::from_str(&text).map_err(|error| {
                    (
                        PetsonaStatus::InvalidArgument,
                        format!("persona draft is not valid JSON: {error}"),
                    )
                })?;
            Ok(RuntimeCommand::ApplyPersonaDraft(request))
        }
        x if x == PetsonaCommandKind::ClearPersonaDraft as u32 => {
            Ok(RuntimeCommand::ClearPersonaDraft)
        }
        x if x == PetsonaCommandKind::PreviewPersonaDraft as u32 => {
            let request: PersonaPreviewRequest = serde_json::from_str(&text).map_err(|error| {
                (
                    PetsonaStatus::InvalidArgument,
                    format!("persona preview request is not valid JSON: {error}"),
                )
            })?;
            Ok(RuntimeCommand::PreviewPersonaDraftRequest(request))
        }
        x if x == PetsonaCommandKind::ApplyImportedPersona as u32 => {
            Ok(RuntimeCommand::ApplyImportedPersona(PathBuf::from(text)))
        }
        _ => Err((
            PetsonaStatus::InvalidArgument,
            format!("unknown command kind: {}", command.kind),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PetsonaStringView;

    #[test]
    fn display_position_command_decodes_physical_offset_and_monitor_identity() {
        let bytes = br#"{"x":240,"y":360,"displayId":"display-a","backingScale":2}"#;
        let command = PetsonaCommand {
            kind: PetsonaCommandKind::SetWindowPosition as u32,
            reserved: 0,
            value: 0.0,
            ttl_ms: 0,
            text: PetsonaStringView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            },
        };
        let parsed = convert(&command).expect("position payload");
        let RuntimeCommand::SetWindowPosition(position) = parsed else {
            panic!("wrong command");
        };
        assert_eq!(position.x, 240.0);
        assert_eq!(position.y, 360.0);
        assert_eq!(position.display_id.as_deref(), Some("display-a"));
        assert_eq!(position.backing_scale, 2.0);
    }

    #[test]
    fn display_position_command_rejects_nonpositive_scale() {
        let bytes = br#"{"x":240,"y":360,"displayId":"display-a","backingScale":0}"#;
        let command = PetsonaCommand {
            kind: PetsonaCommandKind::SetWindowPosition as u32,
            reserved: 0,
            value: 0.0,
            ttl_ms: 0,
            text: PetsonaStringView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            },
        };
        assert!(convert(&command).is_err());
    }

    #[test]
    fn start_conversation_payload_keeps_the_request_and_retry_identity() {
        let json =
            r#"{"requestId":"req-1","petId":"pet-a","text":"继续刚才的话","retryTurnId":"turn-1"}"#;
        let bytes = json.as_bytes();
        let command = PetsonaCommand {
            kind: PetsonaCommandKind::StartConversation as u32,
            reserved: 0,
            value: 0.0,
            ttl_ms: 0,
            text: PetsonaStringView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            },
        };
        let parsed = convert(&command).expect("conversation payload");
        let RuntimeCommand::StartConversation(request) = parsed else {
            panic!("wrong command");
        };
        assert_eq!(request.request_id, "req-1");
        assert_eq!(request.pet_id, "pet-a");
        assert_eq!(request.text, "继续刚才的话");
        assert_eq!(request.retry_turn_id.as_deref(), Some("turn-1"));
    }

    #[test]
    fn conversation_history_config_defaults_to_saving_history() {
        let bytes = br#"{}"#;
        let command = PetsonaCommand {
            kind: PetsonaCommandKind::UpdateConversationConfig as u32,
            reserved: 0,
            value: 0.0,
            ttl_ms: 0,
            text: PetsonaStringView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            },
        };
        assert!(matches!(
            convert(&command).expect("conversation config"),
            RuntimeCommand::UpdateConversationConfig(config) if config.save_history
        ));
    }
}
