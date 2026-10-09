//! Settings-facing IPC. The TypeScript settings UI never owns business
//! state: every read is a projection of the runtime, and every write is a
//! `RuntimeCommand` executed by the existing serialized worker.

use std::io::Cursor;
use std::path::PathBuf;

use petsona_core::config::{
    AppPaths, ConversationConfig, DeepSeekConfig, GreetingConfig, MemoryConfig,
};
use petsona_core::persona::PersonaStyleProfile;
use petsona_runtime::commands::{
    ApplyPersonaDraftRequest, ConversationRequest, MemoryCandidateReview, MemoryFactInput,
    MemoryFactUpdate, MemoryScope, PersonaPatch, PersonaPreviewRequest, PersonaProfileRequest,
    PersonaSourceParseRequest, RuntimeCommand,
};
use petsona_runtime::snapshot::RuntimeTextField;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};

use super::Engine;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub app_version: String,
    pub platform: String,
    pub arch: String,
    pub debug_build: bool,
    pub revision: u64,
    pub ready: bool,
    pub faulted: bool,
    pub error: String,
    pub status: String,
    pub has_pet: bool,
    pub pet_visible: bool,
    pub click_through: bool,
    pub scale: f32,
    pub auto_walk: bool,
    pub gravity_enabled: bool,
    pub always_on_top: bool,
    pub state_server_port: u16,
    pub pet_id: String,
    pub pet_name: String,
    pub settings: Value,
    pub pets: Value,
    pub codex_pets: Value,
    pub persona: Value,
    pub personas: Value,
    pub deepseek: Value,
    pub memory: Value,
    pub models: Value,
    pub conversation: Value,
    pub persona_source: Value,
    pub persona_draft: Value,
    pub persona_preview: Value,
    pub import_conflict: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SettingsAction {
    SetScale {
        value: f32,
    },
    SetClickThrough {
        value: bool,
    },
    SetAlwaysOnTop {
        value: bool,
    },
    SetGravity {
        value: bool,
    },
    SetAutoWalk {
        value: bool,
    },
    SetVisibility {
        value: bool,
    },
    SelectPet {
        id: String,
    },
    RefreshPets,
    ScanCodexPets,
    ImportPet {
        path: String,
        overwrite: bool,
    },
    ClearImportConflict,
    ExportPet {
        id: String,
        path: String,
    },
    DeletePet {
        id: String,
    },
    UpdatePersona {
        patch: PersonaPatch,
    },
    SavePersona,
    ResetPersona,
    CopyPersonaToPet {
        target_pet_id: String,
    },
    ImportPersona {
        path: String,
        overwrite: bool,
    },
    ExportPersona {
        id: String,
        path: String,
    },
    ParsePersonaSource {
        request_id: String,
        label: String,
        format: String,
        path: Option<String>,
        text: Option<String>,
    },
    GeneratePersonaProfile {
        request_id: String,
        source_id: String,
        kind: String,
        label: String,
        description: String,
        target_speaker: String,
        target_speaker_label: String,
        start_index: usize,
        end_index: usize,
    },
    ApplyPersonaDraft {
        draft_id: String,
        pet_id: String,
        name: String,
        style: PersonaStyleProfile,
    },
    PreviewPersonaDraft {
        request_id: String,
        pet_id: String,
        draft_id: String,
        prompt: String,
    },
    ClearPersonaDraft,
    UpdateDeepSeek {
        config: DeepSeekConfig,
    },
    SaveDeepSeekKey {
        key: String,
    },
    ListModels,
    UpdateGreeting {
        config: GreetingConfig,
    },
    UpdateMemoryConfig {
        config: MemoryConfig,
    },
    ReviewMemoryCandidate {
        candidate_id: String,
        accept: bool,
    },
    ImportMemory {
        path: String,
    },
    ExportMemory {
        path: String,
    },
    UpdateConversation {
        config: ConversationConfig,
    },
    StartConversation {
        request_id: String,
        pet_id: String,
        text: String,
        retry_turn_id: Option<String>,
    },
    CancelConversation {
        request_id: String,
    },
    ClearConversationHistory {
        pet_id: String,
    },
    LoadEarlierConversationHistory {
        pet_id: String,
    },
    ClearMemory {
        scope: u8,
    },
    ForgetFact {
        id: String,
    },
    UpdateFact {
        fact: MemoryFactUpdate,
    },
    RememberFact {
        fact: MemoryFactInput,
    },
}

fn parse_json(text: String) -> Value {
    if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }
}

fn parse_json_object(text: String) -> Value {
    match parse_json(text) {
        Value::Object(object) => Value::Object(object),
        _ => serde_json::json!({}),
    }
}

fn parse_json_array(text: String) -> Value {
    match parse_json(text) {
        Value::Array(array) => Value::Array(array),
        _ => serde_json::json!([]),
    }
}

fn pet_asset_is_allowed(path: &std::path::Path) -> bool {
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return false;
    };
    let mut roots = vec![AppPaths::default().pets_dir];
    if let Some(codex) = petsona_core::pet::codex_pets_dir() {
        roots.push(codex);
    }
    roots.into_iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|root| canonical.starts_with(root))
            .unwrap_or(false)
    })
}

/// Returns a small PNG data payload for the first sprite frame. The frontend
/// turns it into a Blob URL; paths outside the pet/Codex roots are rejected.
#[tauri::command]
pub fn pet_preview(path: String, frame_width: u32, frame_height: u32) -> Result<Vec<u8>, String> {
    let path = PathBuf::from(path);
    if !pet_asset_is_allowed(&path) {
        return Err("预览路径不在宠物库或 Codex 宠物目录中".to_string());
    }

    let source = image::open(&path).map_err(|error| format!("无法读取宠物图集：{error}"))?;
    let width = source.width().max(1);
    let height = source.height().max(1);
    let frame_width = frame_width.clamp(1, width);
    let frame_height = frame_height.clamp(1, height);
    let crop = source.crop_imm(0, 0, frame_width, frame_height);
    let target_width = 128u32.min(frame_width);
    let target_height = ((u64::from(frame_height) * u64::from(target_width))
        / u64::from(frame_width))
    .max(1) as u32;
    let preview = crop.resize(
        target_width,
        target_height,
        image::imageops::FilterType::Lanczos3,
    );
    let mut output = Cursor::new(Vec::new());
    preview
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| format!("无法生成宠物预览：{error}"))?;
    Ok(output.into_inner())
}

#[tauri::command]
pub fn settings_snapshot(
    app: AppHandle,
    engine: State<'_, Engine>,
) -> Result<SettingsSnapshot, String> {
    let engine = engine
        .lock()
        .map_err(|_| "runtime engine lock is poisoned".to_string())?;
    let snapshot = engine.snapshot();
    let text = |field| engine.text(field);
    let import_conflict_text = text(RuntimeTextField::ImportConflict);

    Ok(SettingsSnapshot {
        app_version: app.package_info().version.to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        debug_build: cfg!(debug_assertions),
        revision: snapshot.revision,
        ready: snapshot.ready,
        faulted: snapshot.faulted,
        error: text(RuntimeTextField::Error),
        status: text(RuntimeTextField::Status),
        has_pet: snapshot.has_pet,
        pet_visible: snapshot.pet_visible,
        click_through: snapshot.click_through,
        scale: snapshot.scale,
        auto_walk: snapshot.auto_walk,
        gravity_enabled: snapshot.gravity_enabled,
        always_on_top: snapshot.always_on_top,
        state_server_port: snapshot.state_server_port,
        pet_id: text(RuntimeTextField::PetId),
        pet_name: text(RuntimeTextField::PetName),
        settings: parse_json_object(text(RuntimeTextField::Settings)),
        pets: parse_json_array(text(RuntimeTextField::Pets)),
        codex_pets: parse_json_array(text(RuntimeTextField::CodexPets)),
        persona: parse_json_object(text(RuntimeTextField::Persona)),
        personas: parse_json_array(text(RuntimeTextField::Personas)),
        deepseek: parse_json_object(text(RuntimeTextField::DeepSeekConfig)),
        memory: parse_json_object(text(RuntimeTextField::Memory)),
        models: parse_json_array(text(RuntimeTextField::Models)),
        conversation: parse_json_object(text(RuntimeTextField::Conversation)),
        persona_source: parse_json_object(text(RuntimeTextField::PersonaSource)),
        persona_draft: parse_json_object(text(RuntimeTextField::PersonaDraft)),
        persona_preview: parse_json_object(text(RuntimeTextField::PersonaPreview)),
        import_conflict: parse_json(import_conflict_text),
    })
}

#[tauri::command]
pub fn settings_action(engine: State<'_, Engine>, action: SettingsAction) -> Result<(), String> {
    let command = match action {
        SettingsAction::SetScale { value } => RuntimeCommand::SetScale(value),
        SettingsAction::SetClickThrough { value } => RuntimeCommand::SetClickThrough(value),
        SettingsAction::SetAlwaysOnTop { value } => RuntimeCommand::SetAlwaysOnTop(value),
        SettingsAction::SetGravity { value } => RuntimeCommand::SetGravity(value),
        SettingsAction::SetAutoWalk { value } => RuntimeCommand::SetAutoWalk(value),
        SettingsAction::SetVisibility { value } => RuntimeCommand::SetVisibility(value),
        SettingsAction::SelectPet { id } => RuntimeCommand::SelectPet(id),
        SettingsAction::RefreshPets => RuntimeCommand::RefreshPets,
        SettingsAction::ScanCodexPets => RuntimeCommand::ScanCodexPets,
        SettingsAction::ImportPet { path, overwrite } => RuntimeCommand::ImportPet {
            path: PathBuf::from(path),
            overwrite,
        },
        SettingsAction::ClearImportConflict => RuntimeCommand::ClearImportConflict,
        SettingsAction::ExportPet { id, path } => RuntimeCommand::ExportPet {
            id,
            path: PathBuf::from(path),
        },
        SettingsAction::DeletePet { id } => RuntimeCommand::DeletePet(id),
        SettingsAction::UpdatePersona { patch } => RuntimeCommand::UpdatePersona(Box::new(patch)),
        SettingsAction::SavePersona => RuntimeCommand::SavePersona,
        SettingsAction::ResetPersona => RuntimeCommand::ResetPersona,
        SettingsAction::CopyPersonaToPet { target_pet_id } => {
            RuntimeCommand::CopyPersonaToPet(target_pet_id)
        }
        SettingsAction::ImportPersona { path, overwrite } => RuntimeCommand::ImportPersona {
            path: PathBuf::from(path),
            overwrite,
        },
        SettingsAction::ExportPersona { id, path } => RuntimeCommand::ExportPersona {
            id,
            path: PathBuf::from(path),
        },
        SettingsAction::ParsePersonaSource {
            request_id,
            label,
            format,
            path,
            text,
        } => RuntimeCommand::ParsePersonaSource(PersonaSourceParseRequest {
            request_id,
            label,
            format,
            path: path.map(PathBuf::from),
            text,
        }),
        SettingsAction::GeneratePersonaProfile {
            request_id,
            source_id,
            kind,
            label,
            description,
            target_speaker,
            target_speaker_label,
            start_index,
            end_index,
        } => RuntimeCommand::GeneratePersonaProfile(PersonaProfileRequest {
            request_id,
            source_id,
            kind,
            label,
            description,
            target_speaker,
            target_speaker_label,
            start_index,
            end_index,
        }),
        SettingsAction::ApplyPersonaDraft {
            draft_id,
            pet_id,
            name,
            style,
        } => RuntimeCommand::ApplyPersonaDraft(ApplyPersonaDraftRequest {
            draft_id,
            pet_id,
            name,
            style,
        }),
        SettingsAction::PreviewPersonaDraft {
            request_id,
            pet_id,
            draft_id,
            prompt,
        } => RuntimeCommand::PreviewPersonaDraftRequest(PersonaPreviewRequest {
            request_id,
            pet_id,
            draft_id,
            prompt,
        }),
        SettingsAction::ClearPersonaDraft => RuntimeCommand::ClearPersonaDraft,
        SettingsAction::UpdateDeepSeek { config } => RuntimeCommand::UpdateDeepSeekConfig(config),
        SettingsAction::SaveDeepSeekKey { key } => RuntimeCommand::SaveDeepSeekKey(key),
        SettingsAction::ListModels => RuntimeCommand::ListModels,
        SettingsAction::UpdateGreeting { config } => RuntimeCommand::UpdateGreetingConfig(config),
        SettingsAction::UpdateMemoryConfig { config } => RuntimeCommand::UpdateMemoryConfig(config),
        SettingsAction::ReviewMemoryCandidate {
            candidate_id,
            accept,
        } => RuntimeCommand::ReviewMemoryCandidate(MemoryCandidateReview {
            candidate_id,
            accept,
        }),
        SettingsAction::ImportMemory { path } => RuntimeCommand::ImportMemory(PathBuf::from(path)),
        SettingsAction::ExportMemory { path } => RuntimeCommand::ExportMemory(PathBuf::from(path)),
        SettingsAction::UpdateConversation { config } => {
            RuntimeCommand::UpdateConversationConfig(config)
        }
        SettingsAction::StartConversation {
            request_id,
            pet_id,
            text,
            retry_turn_id,
        } => RuntimeCommand::StartConversation(ConversationRequest {
            request_id,
            pet_id,
            text,
            retry_turn_id,
        }),
        SettingsAction::CancelConversation { request_id } => {
            RuntimeCommand::CancelConversation(request_id)
        }
        SettingsAction::ClearConversationHistory { pet_id } => {
            RuntimeCommand::ClearConversationHistory(pet_id)
        }
        SettingsAction::LoadEarlierConversationHistory { pet_id } => {
            RuntimeCommand::LoadEarlierConversationHistory(pet_id)
        }
        SettingsAction::ClearMemory { scope } => {
            RuntimeCommand::ClearMemoryScope(MemoryScope::from_wire(scope as f64))
        }
        SettingsAction::ForgetFact { id } => RuntimeCommand::ForgetFact(id),
        SettingsAction::UpdateFact { fact } => RuntimeCommand::UpdateFact(fact),
        SettingsAction::RememberFact { fact } => RuntimeCommand::RememberFact(fact),
    };

    let engine = engine
        .lock()
        .map_err(|_| "runtime engine lock is poisoned".to_string())?;
    engine.send(command).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("只允许打开 http/https 链接".to_string());
    }

    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开链接：{error}"))
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开链接：{error}"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开链接：{error}"))
    }
}

#[tauri::command]
pub fn open_data_path(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("路径不存在：{}", path.display()));
    }

    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("explorer.exe");
        if path.is_dir() {
            command.arg(&path);
        } else {
            command.arg(format!("/select,{}", path.display()));
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开路径：{error}"))
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开目录：{error}"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开目录：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_action_accepts_memory_candidate_wire_shape() {
        let action: SettingsAction = serde_json::from_str(
            r#"{"type":"reviewMemoryCandidate","candidate_id":"c1","accept":true}"#,
        )
        .expect("memory candidate wire shape");
        match action {
            SettingsAction::ReviewMemoryCandidate {
                candidate_id,
                accept,
            } => {
                assert_eq!(candidate_id, "c1");
                assert!(accept);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn settings_action_accepts_persona_source_wire_shape() {
        let action: SettingsAction = serde_json::from_str(
            r#"{"type":"parsePersonaSource","request_id":"p1","label":"sample","format":"txt","path":null,"text":"A: hello"}"#,
        )
        .expect("persona source wire shape");
        match action {
            SettingsAction::ParsePersonaSource {
                request_id,
                path,
                text,
                ..
            } => {
                assert_eq!(request_id, "p1");
                assert!(path.is_none());
                assert_eq!(text.as_deref(), Some("A: hello"));
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn settings_action_accepts_persona_style_wire_shape() {
        let action: SettingsAction = serde_json::from_str(
            r#"{"type":"applyPersonaDraft","draft_id":"d1","pet_id":"boba","name":"风格","style":{"personality":"冷静","expressionStyle":"简洁","responseHabits":"先回答重点","relationship":"伙伴","examples":["好的"]}}"#,
        )
        .expect("persona style wire shape");
        match action {
            SettingsAction::ApplyPersonaDraft { style, .. } => {
                assert_eq!(style.expression_style, "简洁");
                assert_eq!(style.examples, vec!["好的"]);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn settings_action_uses_snake_case_for_variant_fields() {
        let action: SettingsAction =
            serde_json::from_str(r#"{"type":"copyPersonaToPet","target_pet_id":"rocky"}"#)
                .expect("copy action wire shape");
        match action {
            SettingsAction::CopyPersonaToPet { target_pet_id } => {
                assert_eq!(target_pet_id, "rocky");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn settings_action_accepts_chat_wire_shape() {
        let action: SettingsAction = serde_json::from_str(
            r#"{"type":"startConversation","request_id":"r1","pet_id":"boba","text":"hi","retry_turn_id":null}"#,
        )
        .expect("chat action wire shape");
        match action {
            SettingsAction::StartConversation {
                request_id,
                pet_id,
                text,
                retry_turn_id,
            } => {
                assert_eq!(request_id, "r1");
                assert_eq!(pet_id, "boba");
                assert_eq!(text, "hi");
                assert!(retry_turn_id.is_none());
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn empty_runtime_text_normalises_to_collections() {
        assert_eq!(parse_json_array(String::new()), serde_json::json!([]));
        assert_eq!(parse_json_object(String::new()), serde_json::json!({}));
        assert_eq!(
            parse_json_array("not json".to_string()),
            serde_json::json!([])
        );
        assert_eq!(
            parse_json_object("not json".to_string()),
            serde_json::json!({})
        );
    }
}
