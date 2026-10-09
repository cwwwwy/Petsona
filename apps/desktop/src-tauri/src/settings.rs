//! Settings-facing IPC. The TypeScript settings UI never owns business
//! state: every read is a projection of the runtime, and every write is a
//! `RuntimeCommand` executed by the existing serialized worker.

use std::io::Cursor;
use std::path::PathBuf;

use petsona_core::config::{
    AppPaths, ConversationConfig, DeepSeekConfig, GreetingConfig, MemoryConfig,
};
use petsona_runtime::commands::{
    MemoryFactInput, MemoryFactUpdate, MemoryScope, PersonaPatch, RuntimeCommand,
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
    pub import_conflict: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SettingsAction {
    SetScale { value: f32 },
    SetClickThrough { value: bool },
    SetAlwaysOnTop { value: bool },
    SetGravity { value: bool },
    SetAutoWalk { value: bool },
    SetVisibility { value: bool },
    SelectPet { id: String },
    RefreshPets,
    ScanCodexPets,
    ImportPet { path: String, overwrite: bool },
    ClearImportConflict,
    ExportPet { id: String, path: String },
    DeletePet { id: String },
    UpdatePersona { patch: PersonaPatch },
    SavePersona,
    ResetPersona,
    UpdateDeepSeek { config: DeepSeekConfig },
    SaveDeepSeekKey { key: String },
    ListModels,
    UpdateGreeting { config: GreetingConfig },
    UpdateMemoryConfig { config: MemoryConfig },
    UpdateConversation { config: ConversationConfig },
    ClearMemory { scope: u8 },
    ForgetFact { id: String },
    UpdateFact { fact: MemoryFactUpdate },
    RememberFact { fact: MemoryFactInput },
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
        SettingsAction::UpdateDeepSeek { config } => RuntimeCommand::UpdateDeepSeekConfig(config),
        SettingsAction::SaveDeepSeekKey { key } => RuntimeCommand::SaveDeepSeekKey(key),
        SettingsAction::ListModels => RuntimeCommand::ListModels,
        SettingsAction::UpdateGreeting { config } => RuntimeCommand::UpdateGreetingConfig(config),
        SettingsAction::UpdateMemoryConfig { config } => RuntimeCommand::UpdateMemoryConfig(config),
        SettingsAction::UpdateConversation { config } => {
            RuntimeCommand::UpdateConversationConfig(config)
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
