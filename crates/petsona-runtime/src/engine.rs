//! Serialized runtime worker used by native frontends.
//!
//! The worker owns all disk/network/protocol state. Native UI threads only
//! enqueue commands and read immutable projections, so slow pet discovery,
//! config migration or a protocol request cannot run inside a UI callback.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use petsona_core::config::{
    AppConfig, AppPaths, DeepSeekConfig, GreetingConfig, MemoryConfig, WindowPosition,
};
use petsona_core::deepseek::DeepSeekClient;
use petsona_core::memory::{extract_preference, EventKind, GreetingContext};
use petsona_core::persona::{templates, Persona, PersonaSource};
use petsona_core::persona_source::ParsedPersonaSource;
use petsona_core::pet::PetLibrary;
use petsona_core::pet::PetState;

use crate::commands::{
    ConversationRequest, ImportConflict, MemoryFactInput, MemoryFactUpdate, MemoryScope,
    PersonaCreate, PersonaPatch, PersonaPreviewRequest, PersonaProfileRequest, RuntimeCommand,
};
use crate::events::RuntimeWaker;
use crate::instance_lock::InstanceLock;
use crate::logging;
use crate::persona_source::{ActivePersonaGeneration, ActivePersonaPreview, PersonaDraft};
use crate::session::PetsonaRuntime;
use crate::session::{ActiveConversation, ActiveMemoryLearning, ConversationTurn};
use crate::snapshot::{RuntimeSnapshot, RuntimeTextField, RuntimeTexts};

const INITIAL_SNAPSHOT_DELAY: Duration = Duration::from_secs(1);
const IDLE_WORKER_WAIT: Duration = Duration::from_secs(1);
pub const SCALE_PRESETS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

fn normalize_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }
    SCALE_PRESETS
        .iter()
        .copied()
        .min_by(|left, right| {
            (value - *left)
                .abs()
                .partial_cmp(&(value - *right).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(1.0)
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeEngineError {
    #[error("runtime engine has stopped")]
    Stopped,
    #[error("runtime engine failed: {0}")]
    Fault(String),
    #[error("runtime command queue is closed")]
    QueueClosed,
}

struct SharedProjection {
    snapshot: RwLock<RuntimeSnapshot>,
    texts: RwLock<RuntimeTexts>,
}

/// A handle to the serialized runtime worker.
pub struct RuntimeEngine {
    command_tx: Sender<RuntimeCommand>,
    projection: Arc<SharedProjection>,
    stopped: bool,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeEngine {
    /// Spawn the worker and return immediately. `home` is only read by the
    /// worker; construction therefore does not touch the user's filesystem on
    /// the caller's UI thread.
    pub fn spawn<F>(home: Option<PathBuf>, external_wake: F) -> Result<Self>
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self::spawn_with_key_presence(home, external_wake, petsona_core::deepseek::api_key_present)
    }

    fn spawn_with_key_presence<F, K>(
        home: Option<PathBuf>,
        external_wake: F,
        key_presence: K,
    ) -> Result<Self>
    where
        F: Fn() + Send + Sync + 'static,
        K: Fn(&DeepSeekConfig) -> bool + Clone + Send + Sync + 'static,
    {
        let (command_tx, command_rx) = mpsc::channel();
        let projection = Arc::new(SharedProjection {
            snapshot: RwLock::new(RuntimeSnapshot {
                next_frame_ms: INITIAL_SNAPSHOT_DELAY.as_millis() as u32,
                ..RuntimeSnapshot::default()
            }),
            texts: RwLock::new(RuntimeTexts::default()),
        });
        let worker_projection = Arc::clone(&projection);
        let worker_tx = command_tx.clone();
        let external_wake = Arc::new(external_wake);
        let worker = thread::Builder::new()
            .name("petsona-runtime".to_string())
            .spawn(move || {
                run_worker(
                    home,
                    command_rx,
                    worker_tx,
                    worker_projection,
                    external_wake,
                    key_presence,
                )
            })
            .context("cannot spawn the Petsona runtime worker")?;

        Ok(Self {
            command_tx,
            projection,
            stopped: false,
            worker: Some(worker),
        })
    }

    pub fn waker(&self) -> RuntimeWaker {
        RuntimeWaker::new(self.command_tx.clone())
    }

    pub fn send(&self, command: RuntimeCommand) -> Result<(), RuntimeEngineError> {
        if self.stopped {
            return Err(RuntimeEngineError::Stopped);
        }
        if self.snapshot().faulted {
            return Err(RuntimeEngineError::Fault(
                self.text(RuntimeTextField::Error),
            ));
        }
        self.command_tx
            .send(command)
            .map_err(|_| RuntimeEngineError::QueueClosed)
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.projection
            .snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|_| RuntimeSnapshot {
                faulted: true,
                ..RuntimeSnapshot::default()
            })
    }

    pub fn text(&self, field: RuntimeTextField) -> String {
        self.projection
            .texts
            .read()
            .map(|texts| texts.get(field).to_string())
            .unwrap_or_default()
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        let _ = self.command_tx.send(RuntimeCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for RuntimeEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_worker<K>(
    home: Option<PathBuf>,
    command_rx: Receiver<RuntimeCommand>,
    command_tx: Sender<RuntimeCommand>,
    projection: Arc<SharedProjection>,
    external_wake: Arc<dyn Fn() + Send + Sync>,
    key_presence: K,
) where
    K: Fn(&DeepSeekConfig) -> bool + Clone + Send + Sync + 'static,
{
    let result = load_runtime(home, &command_tx, &external_wake);
    let (mut runtime, _instance_lock) = match result {
        Ok(value) => value,
        Err(error) => {
            set_fault(&projection, format!("{error:#}"));
            return;
        }
    };

    let mut key_presence_request_id = 0;
    let request_id = next_key_presence_request_id(&mut key_presence_request_id);
    schedule_key_presence_check(
        &runtime.config.deepseek,
        &command_tx,
        &key_presence,
        request_id,
    );

    let mut revision = 1u64;
    tick_runtime(&mut runtime, &command_tx, &projection, &mut revision);

    loop {
        let wait = next_wait(&runtime);
        match command_rx.recv_timeout(wait) {
            Ok(RuntimeCommand::Stop) => {
                cancel_active_conversation(&mut runtime, "");
                break;
            }
            Ok(command) => {
                if apply_command(
                    command,
                    &mut runtime,
                    &command_tx,
                    &key_presence,
                    &mut key_presence_request_id,
                ) {
                    tick_runtime(&mut runtime, &command_tx, &projection, &mut revision);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                tick_runtime(&mut runtime, &command_tx, &projection, &mut revision);
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn load_runtime(
    home: Option<PathBuf>,
    command_tx: &Sender<RuntimeCommand>,
    external_wake: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(PetsonaRuntime, InstanceLock)> {
    let paths = home.map(AppPaths::resolve).unwrap_or_default();
    paths
        .ensure()
        .context("cannot prepare Petsona data directory")?;
    logging::init(&paths.logs_dir);
    let lock_path = paths.config_dir.join("petsona.lock");
    let instance_lock = InstanceLock::acquire(&lock_path)
        .context("cannot acquire the Petsona data-directory lock")?;
    let config = AppConfig::load(&paths.config_file).context("cannot load Petsona config")?;
    let mut runtime = PetsonaRuntime::load(paths, config).context("cannot load Petsona runtime")?;
    runtime.key_configured = env_key_present(&runtime.config.deepseek);
    // REQ-P05: honour the retention window on every start.
    let retention = runtime.config.memory.event_retention_days;
    let _ = runtime.memory.prune_events(&runtime.persona.id, retention);
    let wake_tx = command_tx.clone();
    let external_wake = Arc::clone(external_wake);
    runtime.sync_state_server(move || {
        let _ = wake_tx.send(RuntimeCommand::Wake);
        external_wake();
    });
    Ok((runtime, instance_lock))
}

fn env_key_present(config: &DeepSeekConfig) -> bool {
    std::env::var(&config.api_key_env)
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

fn schedule_key_presence_check<K>(
    config: &DeepSeekConfig,
    command_tx: &Sender<RuntimeCommand>,
    key_presence: &K,
    request_id: u64,
) where
    K: Fn(&DeepSeekConfig) -> bool + Clone + Send + Sync + 'static,
{
    let config = config.clone();
    let provider = config.provider.clone();
    let command_tx = command_tx.clone();
    let key_presence = key_presence.clone();
    let _ = thread::Builder::new()
        .name("petsona-key-presence".to_string())
        .spawn(move || {
            let present = key_presence(&config);
            let _ = command_tx.send(RuntimeCommand::KeyPresenceResult {
                provider,
                request_id,
                present,
            });
        });
}

fn next_key_presence_request_id(current: &mut u64) -> u64 {
    *current = current.wrapping_add(1).max(1);
    *current
}

fn key_presence_result_is_current(
    configured_provider: &str,
    latest_request_id: u64,
    result_provider: &str,
    result_request_id: u64,
) -> bool {
    configured_provider == result_provider && latest_request_id == result_request_id
}

fn apply_command<K>(
    command: RuntimeCommand,
    runtime: &mut PetsonaRuntime,
    command_tx: &Sender<RuntimeCommand>,
    key_presence: &K,
    key_presence_request_id: &mut u64,
) -> bool
where
    K: Fn(&DeepSeekConfig) -> bool + Clone + Send + Sync + 'static,
{
    match command {
        RuntimeCommand::Wake | RuntimeCommand::Tick => true,
        RuntimeCommand::SetVisibility(visible) => {
            runtime.pet_visible = visible;
            true
        }
        RuntimeCommand::SetClickThrough(enabled) => {
            runtime.config.window.click_through = enabled;
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存点击穿透设置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetScale(scale) => {
            runtime.config.window.scale = normalize_scale(scale);
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存缩放设置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetPosition { x, y } => {
            runtime.config.window.start_position = Some(WindowPosition {
                x,
                y,
                ..WindowPosition::default()
            });
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存位置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetWindowPosition(position) => {
            if !position.x.is_finite()
                || !position.y.is_finite()
                || !position.backing_scale.is_finite()
                || position.backing_scale <= 0.0
            {
                runtime.status = "保存位置失败：显示器坐标无效".to_string();
                return true;
            }
            match save_macos_window_position(&runtime.paths.config_dir, &position) {
                Ok(()) => {
                    runtime.macos_window_position = Some(position);
                    runtime.status.clear();
                }
                Err(error) => {
                    runtime.status = format!("保存 Mac 显示器位置失败：{error:#}");
                }
            }
            true
        }
        RuntimeCommand::SetAutoWalk(enabled) => {
            runtime.config.window.auto_walk.enabled = enabled;
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存活动提醒设置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetGravity(enabled) => {
            runtime.config.window.gravity_enabled = enabled;
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存重力设置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetAlwaysOnTop(enabled) => {
            runtime.config.window.always_on_top = enabled;
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存置顶设置失败：{error}");
            }
            true
        }
        RuntimeCommand::SetGazeTarget { dx, dy } => {
            if let Some(pet) = &mut runtime.pet {
                let now = Instant::now();
                let changed = if pet.engine.gaze_direction().is_some() {
                    pet.engine.retarget_gaze(dx, dy)
                } else {
                    pet.engine.glance_towards(dx, dy, now).is_some()
                };
                if changed {
                    pet.anim_started = now;
                    pet.last_state = pet.engine.current();
                }
            }
            true
        }
        RuntimeCommand::ClearGaze => {
            if let Some(pet) = &mut runtime.pet {
                let now = Instant::now();
                if pet.engine.release_gaze(now) {
                    pet.anim_started = now;
                    pet.last_state = pet.engine.current();
                }
            }
            true
        }
        RuntimeCommand::SetState { state, ttl } => {
            if let Some(pet) = &mut runtime.pet {
                let now = Instant::now();
                let previous = pet.engine.current();
                let transition = pet.engine.raise(state, "native", None, ttl, now);
                let current = pet.engine.current();
                // Drag sends the same state every 80 ms. Restart the clock
                // only for a real visible change (or an explicit one-shot
                // retrigger); otherwise the running frames rewind to frame 0
                // before the first 120 ms frame can advance.
                if transition.is_some() && (current != previous || state.is_one_shot()) {
                    pet.anim_started = now;
                    pet.last_state = current;
                }
                runtime.last_user_action = now;
            } else {
                runtime.status = "尚未导入宠物".to_string();
            }
            true
        }
        RuntimeCommand::ShowBubble { text, ttl } => {
            runtime.show_bubble(text, ttl);
            true
        }
        RuntimeCommand::SetBubblePaused(paused) => {
            runtime.set_bubble_paused(paused);
            true
        }
        RuntimeCommand::ClearBubble => {
            runtime.bubble = None;
            true
        }
        RuntimeCommand::RefreshPets => {
            runtime.pets = runtime.library.list();
            runtime.publish_health();
            true
        }
        RuntimeCommand::ScanCodexPets => {
            runtime.codex_pets = petsona_core::pet::codex_pets_dir()
                .map(|directory| {
                    PetLibrary::scan_dir(&directory, petsona_core::pet::RootKind::Codex)
                })
                .unwrap_or_default();
            true
        }
        RuntimeCommand::ImportPet { path, overwrite } => {
            runtime.import_conflict = None;
            if !overwrite {
                match PetLibrary::import_identity(&path) {
                    Ok((id, name)) if runtime.pets.iter().any(|pet| pet.id == id) => {
                        runtime.import_conflict = Some(ImportConflict { id, name, path });
                        runtime.status = "本地已有同 ID 宠物，等待确认是否覆盖".to_string();
                        return true;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        runtime.status = format!("导入失败：{error:#}");
                        return true;
                    }
                }
            }
            let result = if path.is_dir() {
                runtime.library.import_dir(&path, overwrite)
            } else {
                runtime.library.import_zip(&path, overwrite)
            };
            match result {
                Ok(entry) => {
                    runtime.pets = runtime.library.list();
                    if runtime.selected_pet.as_deref() != Some(entry.id.as_str()) {
                        cancel_active_conversation(runtime, "");
                    }
                    runtime.config.active_pet = Some(entry.id.clone());
                    runtime.config.first_run = false;
                    match runtime.save_config() {
                        Ok(()) => match crate::pet::PetSession::load(entry.clone()) {
                            Ok(pet) => {
                                runtime.pet = Some(pet);
                                runtime.selected_pet = Some(entry.id.clone());
                                load_pet_conversation_history(runtime, &entry.id);
                                runtime.status = format!("已导入并切换到 {}", entry.display_name);
                            }
                            Err(error) => runtime.status = format!("导入后加载失败：{error:#}"),
                        },
                        Err(error) => runtime.status = format!("导入后保存配置失败：{error}"),
                    }
                }
                Err(error) => runtime.status = format!("导入失败：{error:#}"),
            }
            runtime.publish_health();
            true
        }
        RuntimeCommand::ClearImportConflict => {
            runtime.import_conflict = None;
            if runtime.status.contains("等待确认是否覆盖") {
                runtime.status.clear();
            }
            true
        }
        RuntimeCommand::ExportPet { id, path } => {
            match runtime.library.export_zip(&id, &path) {
                Ok(()) => runtime.status = format!("已导出 {}", path.display()),
                Err(error) => runtime.status = format!("导出失败：{error:#}"),
            }
            true
        }
        RuntimeCommand::SelectPet(id) => {
            if runtime.selected_pet.as_deref() != Some(id.as_str()) {
                cancel_active_conversation(runtime, "");
            }
            match runtime.pets.iter().find(|pet| pet.id == id).cloned() {
                Some(entry) => match crate::pet::PetSession::load(entry.clone()) {
                    Ok(pet) => {
                        runtime.pet = Some(pet);
                        runtime.selected_pet = Some(id.clone());
                        load_pet_conversation_history(runtime, &id);
                        runtime.config.active_pet = Some(id.clone());
                        runtime.config.first_run = false;
                        // One persona per pet (REQ-P02): follow the binding, or
                        // adopt the current speaking style for a new pet.
                        match runtime.config.persona_by_pet.get(&id).cloned() {
                            Some(persona_id) => {
                                if let Ok(Some(bound)) = runtime.personas.get(&persona_id) {
                                    runtime.persona = bound.clone();
                                    runtime.config.active_persona = Some(bound.id);
                                }
                            }
                            None => {
                                // REQ-P02: a pet without a binding gets its own
                                // copy of the current style, so editing one
                                // pet's speaking style never leaks into
                                // another. An existing file with that id is
                                // reused instead of being overwritten.
                                let style = match runtime.personas.get(&id).ok().flatten() {
                                    Some(existing) => existing,
                                    None => {
                                        let mut copy = runtime.persona.clone();
                                        copy.id = id.clone();
                                        copy.builtin = false;
                                        copy.name = runtime
                                            .pets
                                            .iter()
                                            .find(|pet| pet.id == id)
                                            .map(|pet| pet.display_name.clone())
                                            .unwrap_or_else(|| copy.name.clone());
                                        let _ = runtime.personas.save(&copy);
                                        copy
                                    }
                                };
                                runtime.persona = style;
                                runtime.config.active_persona = Some(id.clone());
                                runtime.config.persona_by_pet.insert(id.clone(), id.clone());
                            }
                        }
                    }
                    Err(error) => runtime.status = format!("加载宠物失败：{error:#}"),
                },
                None => runtime.status = "本地库中没有这个宠物".to_string(),
            }
            runtime.publish_health();
            true
        }
        RuntimeCommand::DeletePet(id) => {
            if runtime.selected_pet.as_deref() == Some(id.as_str()) {
                cancel_active_conversation(runtime, "");
            }
            match runtime.library.remove_local(&id) {
                Ok(()) => {
                    runtime.pets = runtime.library.list();
                    // REQ-P04: dropping a pet removes its binding only. The
                    // persona file and its memory are kept on purpose — user
                    // data is never deleted silently (explicit clear lives in
                    // the settings page).
                    runtime.config.persona_by_pet.remove(&id);
                    if runtime.config.active_pet.as_deref() == Some(id.as_str()) {
                        if let Some(next) = runtime.pets.first().cloned() {
                            runtime.config.active_pet = Some(next.id.clone());
                            runtime.pet = crate::pet::PetSession::load(next).ok();
                            runtime.selected_pet = runtime.config.active_pet.clone();
                            if let Some(pet_id) = runtime.selected_pet.clone() {
                                load_pet_conversation_history(runtime, &pet_id);
                            }
                        } else {
                            runtime.config.active_pet = None;
                            runtime.pet = None;
                            runtime.selected_pet = None;
                            runtime.conversation_history.clear();
                            runtime.conversation_context_start = 0;
                        }
                    }
                    runtime.status = format!("已删除 {id}");
                    let _ = runtime.save_config();
                }
                Err(error) => runtime.status = format!("删除失败：{error:#}"),
            }
            runtime.publish_health();
            true
        }
        RuntimeCommand::UpdatePersona(patch) => {
            cancel_active_conversation(runtime, "");
            apply_persona_patch(&mut runtime.persona, *patch);
            runtime.status = "人格已更新（待保存）".to_string();
            true
        }
        RuntimeCommand::SavePersona => {
            match runtime.personas.save(&runtime.persona) {
                Ok(()) => runtime.status = "人格已保存".to_string(),
                Err(error) => runtime.status = format!("保存人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::RefreshPersonas => true,
        RuntimeCommand::CreatePersona(spec) => {
            let result = create_persona(&runtime.personas, spec);
            match result {
                Ok(persona) => {
                    runtime.persona = persona.clone();
                    runtime.config.active_persona = Some(persona.id.clone());
                    runtime.status = format!("已创建人格：{}", persona.name);
                    if let Err(error) = runtime.save_config() {
                        runtime.status = format!("保存当前人格失败：{error}");
                    }
                }
                Err(error) => runtime.status = format!("创建人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::DuplicatePersona(spec) => {
            match runtime
                .personas
                .duplicate(&spec.source_id, &spec.id, &spec.name)
            {
                Ok(persona) => {
                    runtime.persona = persona.clone();
                    runtime.config.active_persona = Some(persona.id.clone());
                    let _ = runtime.save_config();
                    runtime.status = format!("已复制人格：{}", persona.name);
                }
                Err(error) => runtime.status = format!("复制人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::CopyPersonaToPet(target_pet_id) => {
            let target = runtime
                .pets
                .iter()
                .find(|pet| pet.id == target_pet_id)
                .cloned();
            let Some(target) = target else {
                runtime.status = "目标宠物不在本地宠物库中".to_string();
                return true;
            };
            let source_id = runtime.persona.id.clone();
            let copy_id = format!("{source_id}--{target_pet_id}");
            let copy_name = format!("{}（{}）", runtime.persona.name, target.display_name);
            match runtime.personas.duplicate(&source_id, &copy_id, &copy_name) {
                Ok(copy) => {
                    runtime
                        .config
                        .persona_by_pet
                        .insert(target_pet_id.clone(), copy.id.clone());
                    match runtime.save_config() {
                        Ok(()) => {
                            runtime.status = format!("已将说话方式复制到 {}", target.display_name);
                        }
                        Err(error) => {
                            runtime.status = format!("复制后保存绑定失败：{error}");
                        }
                    }
                }
                Err(error) => {
                    runtime.status = format!("复制人格失败：{error}");
                }
            }
            true
        }
        RuntimeCommand::ResetPersona => {
            cancel_active_conversation(runtime, "");
            // REQ-P03: 「重置为内置」restores the shipped speaking style but
            // keeps the persona identity (id) and any binding.
            let builtin = Persona::default();
            let id = runtime.persona.id.clone();
            let name = runtime.persona.name.clone();
            runtime.persona = Persona {
                id,
                name,
                builtin: false,
                ..builtin
            };
            match runtime.personas.save(&runtime.persona) {
                Ok(()) => runtime.status = "人格已重置为内置".to_string(),
                Err(error) => runtime.status = format!("重置失败：{error}"),
            }
            true
        }
        RuntimeCommand::SelectPersona(id) => {
            if runtime.persona.id != id {
                cancel_active_conversation(runtime, "");
            }
            match runtime.personas.get(&id) {
                Ok(Some(persona)) => {
                    runtime.persona = persona;
                    runtime.config.active_persona = Some(id.clone());
                    // REQ-P02: choosing a speaking style binds it to the pet you
                    // are looking at, so switching pets restores it later.
                    if let Some(pet) = runtime.selected_pet.clone() {
                        runtime.config.persona_by_pet.insert(pet, id.clone());
                    }
                    let _ = runtime.save_config();
                    runtime.status = format!("已切换人格：{}", runtime.persona.name);
                }
                Ok(None) => runtime.status = format!("人格不存在：{id}"),
                Err(error) => runtime.status = format!("读取人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::DeletePersona(id) => {
            match runtime.personas.delete(&id) {
                Ok(()) => {
                    if runtime.config.active_persona.as_deref() == Some(id.as_str()) {
                        let fallback = runtime
                            .personas
                            .get(petsona_core::persona::DEFAULT_PERSONA_ID)
                            .ok()
                            .flatten()
                            .or_else(|| {
                                runtime
                                    .personas
                                    .list()
                                    .ok()
                                    .and_then(|p| p.into_iter().next())
                            });
                        if let Some(persona) = fallback {
                            runtime.persona = persona.clone();
                            runtime.config.active_persona = Some(persona.id);
                            let _ = runtime.save_config();
                        }
                    }
                    runtime.status = format!("已删除人格：{id}");
                }
                Err(error) => runtime.status = format!("删除人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::ImportPersona { path, overwrite } => {
            cancel_active_conversation(runtime, "");
            match runtime.personas.import_file(&path, overwrite) {
                Ok(persona) => {
                    runtime.persona = persona.clone();
                    runtime.config.active_persona = Some(persona.id.clone());
                    let _ = runtime.save_config();
                    runtime.status = format!("已导入人格：{}", persona.name);
                }
                Err(error) => runtime.status = format!("导入人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::ApplyImportedPersona(path) => {
            cancel_active_conversation(runtime, "");
            let Some(pet_id) = runtime.selected_pet.clone() else {
                runtime.status = "请先选择宠物，再导入人格".to_string();
                return true;
            };
            match import_persona_into_stable_identity(&runtime.personas, &path, &runtime.persona.id)
            {
                Ok(persona) => {
                    runtime.persona = persona.clone();
                    runtime.config.active_persona = Some(persona.id.clone());
                    runtime
                        .config
                        .persona_by_pet
                        .insert(pet_id, persona.id.clone());
                    let _ = runtime.save_config();
                    runtime.status = format!("已导入人格：{}", persona.name);
                }
                Err(error) => runtime.status = format!("导入人格失败：{error:#}"),
            }
            true
        }
        RuntimeCommand::ExportPersona { id, path } => {
            match runtime.personas.export_file(&id, &path) {
                Ok(()) => runtime.status = format!("已导出人格：{}", path.display()),
                Err(error) => runtime.status = format!("导出人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateDeepSeekConfig(config) => {
            cancel_active_conversation(runtime, "");
            runtime.config.deepseek = sanitize_deepseek_config(config);
            runtime.key_configured = env_key_present(&runtime.config.deepseek);
            let request_id = next_key_presence_request_id(key_presence_request_id);
            schedule_key_presence_check(
                &runtime.config.deepseek,
                command_tx,
                key_presence,
                request_id,
            );
            match runtime.save_config() {
                Ok(()) => runtime.status = "DeepSeek 配置已保存".to_string(),
                Err(error) => runtime.status = format!("保存 DeepSeek 配置失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateGreetingConfig(config) => {
            runtime.config.greeting = sanitize_greeting_config(config);
            match runtime.save_config() {
                Ok(()) => runtime.status = "问候设置已保存".to_string(),
                Err(error) => runtime.status = format!("保存问候设置失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateMemoryConfig(config) => {
            let was_enabled = runtime.config.memory.enabled;
            runtime.config.memory = sanitize_memory_config(config);
            if !runtime.config.memory.enabled {
                runtime.active_memory_learning = None;
                if was_enabled {
                    preserve_memory_learning_boundary(runtime);
                }
            }
            let retention = runtime.config.memory.event_retention_days;
            let _ = runtime.memory.prune_events(&runtime.persona.id, retention);
            compress_facts_if_enabled(runtime);
            match runtime.save_config() {
                Ok(()) => runtime.status = "记忆设置已保存".to_string(),
                Err(error) => runtime.status = format!("保存记忆设置失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateConversationConfig(config) => {
            runtime.config.conversation = config;
            if let Err(error) = runtime.save_config() {
                runtime.conversation_error = format!("保存聊天设置失败：{error}");
            } else {
                runtime.conversation_error.clear();
            }
            true
        }
        RuntimeCommand::RememberFact(MemoryFactInput {
            key,
            value,
            confidence,
        }) => {
            match runtime.memory.remember_fact_from(
                &runtime.persona.id,
                key.trim(),
                value.trim(),
                confidence.unwrap_or(0.8).clamp(0.0, 1.0),
                petsona_core::memory::FACT_SOURCE_MANUAL,
            ) {
                Ok(_) => {
                    runtime.status = "已保存一条用户偏好".to_string();
                    compress_facts_if_enabled(runtime);
                }
                Err(error) => runtime.status = format!("保存偏好失败：{error}"),
            }
            true
        }
        RuntimeCommand::ForgetFact(id) => {
            match runtime.memory.forget_fact(&runtime.persona.id, &id) {
                Ok(true) => runtime.status = "已删除记忆偏好".to_string(),
                Ok(false) => runtime.status = "记忆偏好不存在".to_string(),
                Err(error) => runtime.status = format!("删除偏好失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateFact(MemoryFactUpdate {
            id,
            key,
            value,
            confidence,
        }) => {
            match runtime.memory.update_fact(
                &runtime.persona.id,
                &id,
                key.trim(),
                value.trim(),
                confidence.unwrap_or(0.8).clamp(0.0, 1.0),
            ) {
                Ok(Some(_)) => runtime.status = "已更新这条记忆偏好".to_string(),
                Ok(None) => runtime.status = "这条偏好已经不存在".to_string(),
                Err(error) => runtime.status = format!("更新偏好失败：{error}"),
            }
            true
        }
        RuntimeCommand::ClearMemoryScope(scope) => {
            if scope == MemoryScope::Facts || scope == MemoryScope::All {
                runtime.active_memory_learning = None;
            }
            let result = match scope {
                MemoryScope::All => runtime
                    .memory
                    .clear_persona(&runtime.persona.id)
                    .map(|()| "已清空当前人格的全部记忆".to_string()),
                MemoryScope::Facts => runtime
                    .memory
                    .clear_facts(&runtime.persona.id)
                    .map(|removed| format!("已清空 {removed} 条偏好（事件保留）")),
                MemoryScope::Events => runtime
                    .memory
                    .clear_events(&runtime.persona.id)
                    .map(|removed| format!("已清空 {removed} 条互动事件（偏好保留）")),
            };
            let cleared = result.is_ok();
            runtime.status = match result {
                Ok(message) => message,
                Err(error) => format!("清空记忆失败：{error}"),
            };
            if cleared && matches!(scope, MemoryScope::Facts | MemoryScope::All) {
                preserve_memory_learning_boundary(runtime);
            }
            true
        }
        RuntimeCommand::ExportMemory(path) => {
            match runtime.memory.export_persona(&runtime.persona.id, &path) {
                Ok(memory) => {
                    runtime.status = format!(
                        "已导出记忆：{} 条偏好 / {} 条事件",
                        memory.facts.len(),
                        memory.events.len()
                    );
                }
                Err(error) => runtime.status = format!("导出记忆失败：{error}"),
            }
            true
        }
        RuntimeCommand::ImportMemory(path) => {
            match runtime.memory.import_persona(&runtime.persona.id, &path) {
                Ok((facts, events)) => {
                    runtime.status =
                        format!("已导入记忆：{facts} 条偏好 / {events} 条事件（覆盖当前人格）");
                }
                Err(error) => runtime.status = format!("导入记忆失败：{error}"),
            }
            true
        }
        RuntimeCommand::ClearMemory => {
            runtime.active_memory_learning = None;
            match runtime.memory.clear_persona(&runtime.persona.id) {
                Ok(()) => {
                    preserve_memory_learning_boundary(runtime);
                    runtime.status = "已清空当前人格的记忆".to_string();
                }
                Err(error) => runtime.status = format!("清空记忆失败：{error}"),
            }
            true
        }
        RuntimeCommand::SaveDeepSeekKey(key) => {
            // Credentials are per provider: switching to a custom endpoint must
            // not overwrite (or clear) the DeepSeek key (REQ-S16).
            let provider = runtime.config.deepseek.provider.clone();
            match petsona_core::deepseek::save_api_key(&provider, &key) {
                Ok(()) if key.trim().is_empty() => {
                    runtime.status = format!("已清除 {provider} 的凭据");
                }
                Ok(()) => {
                    runtime.status = format!("{provider} 密钥已保存到系统凭据库");
                }
                Err(error) => runtime.status = format!("保存密钥失败：{error}"),
            }
            runtime.key_configured = env_key_present(&runtime.config.deepseek);
            let request_id = next_key_presence_request_id(key_presence_request_id);
            schedule_key_presence_check(
                &runtime.config.deepseek,
                command_tx,
                key_presence,
                request_id,
            );
            true
        }
        RuntimeCommand::KeyPresenceResult {
            provider,
            request_id,
            present,
        } => {
            if key_presence_result_is_current(
                &runtime.config.deepseek.provider,
                *key_presence_request_id,
                &provider,
                request_id,
            ) {
                runtime.key_configured = present;
            }
            true
        }
        RuntimeCommand::ListModels => {
            if runtime.models_inflight {
                return true;
            }
            runtime.models_inflight = true;
            runtime.status = "正在拉取模型列表…".to_string();
            let persona = runtime.persona.clone();
            let config = runtime.config.deepseek.clone();
            let tx = command_tx.clone();
            thread::spawn(move || {
                let result = DeepSeekClient::new(config)
                    .and_then(|client| client.list_models())
                    .map_err(|error| {
                        let _ = &persona;
                        format!("{error:#}")
                    });
                let _ = tx.send(RuntimeCommand::ModelsResult(result));
            });
            true
        }
        RuntimeCommand::ModelsResult(result) => {
            runtime.models_inflight = false;
            match result {
                Ok(models) => {
                    runtime.models = models.clone();
                    runtime.status = if models.is_empty() {
                        "服务商没有返回任何模型；请手动填写模型名".to_string()
                    } else {
                        format!("已拉取 {} 个模型", models.len())
                    };
                }
                Err(error) => {
                    runtime.status = format!("拉取模型列表失败：{error}（可手动填写模型名）");
                }
            }
            true
        }
        RuntimeCommand::StartConversation(request) => {
            start_streaming_conversation(runtime, request, command_tx);
            true
        }
        RuntimeCommand::CancelConversation(request_id) => {
            let matches = runtime
                .active_conversation
                .as_ref()
                .is_some_and(|active| request_id.is_empty() || active.request_id == request_id);
            if matches {
                cancel_active_conversation(runtime, "");
            }
            true
        }
        RuntimeCommand::ClearConversationHistory(pet_id) => {
            if pet_id.is_empty() {
                runtime.conversation_error = "没有选中的宠物".to_string();
                return true;
            }
            if runtime
                .active_conversation
                .as_ref()
                .is_some_and(|active| active.pet_id == pet_id)
            {
                cancel_active_conversation(runtime, "");
            }
            if runtime.selected_pet.as_deref() == Some(pet_id.as_str()) {
                runtime.active_memory_learning = None;
            }
            match runtime.conversation_store.clear(&pet_id) {
                Ok(()) => {
                    if runtime.selected_pet.as_deref() == Some(pet_id.as_str()) {
                        runtime.conversation_history.clear();
                        runtime.conversation_context_start = 0;
                        runtime.conversation_page_start = 0;
                    }
                    runtime.conversation_error.clear();
                }
                Err(error) => {
                    runtime.conversation_error = format!("清除聊天记录失败：{error:#}");
                }
            }
            true
        }
        RuntimeCommand::LoadEarlierConversationHistory(pet_id) => {
            if runtime.selected_pet.as_deref() == Some(pet_id.as_str()) {
                runtime.conversation_page_start =
                    runtime.conversation_page_start.saturating_sub(50);
            }
            true
        }
        RuntimeCommand::ConversationStreamChunk {
            request_id,
            pet_id,
            persona_id,
            chunk,
        } => {
            let current_persona = serde_json::to_string(&runtime.persona).unwrap_or_default();
            let Some(active) = runtime.active_conversation.as_ref() else {
                return true;
            };
            if active.request_id != request_id
                || active.pet_id != pet_id
                || active.persona_id != persona_id
                || runtime.selected_pet.as_deref() != Some(pet_id.as_str())
                || active.persona_json != current_persona
            {
                return true;
            }
            let assistant_turn_id = active.assistant_turn_id.clone();
            let bubble = runtime
                .conversation_history
                .iter_mut()
                .find(|turn| turn.id == assistant_turn_id)
                .map(|turn| {
                    turn.text.push_str(&chunk);
                    conversation_bubble_preview(&turn.text)
                });
            if let Some(bubble) = bubble {
                renew_conversation_bubble(runtime, bubble, Duration::from_secs(30));
            }
            true
        }
        RuntimeCommand::ConversationStreamFinished {
            request_id,
            pet_id,
            persona_id,
            result,
        } => {
            let current_persona = serde_json::to_string(&runtime.persona).unwrap_or_default();
            let Some(active) = runtime.active_conversation.as_ref() else {
                return true;
            };
            if active.request_id != request_id
                || active.pet_id != pet_id
                || active.persona_id != persona_id
                || runtime.selected_pet.as_deref() != Some(pet_id.as_str())
                || active.persona_json != current_persona
            {
                return true;
            }
            let assistant_turn_id = active.assistant_turn_id.clone();
            runtime.active_conversation = None;
            runtime.conversation_inflight = false;
            let mut bubble_text = String::new();
            let mut completed = false;
            let mut memory_event = None;
            if let Some(turn) = runtime
                .conversation_history
                .iter_mut()
                .find(|turn| turn.id == assistant_turn_id)
            {
                match result {
                    Ok(()) => {
                        turn.status = "complete".to_string();
                        completed = true;
                        if runtime.config.memory.enabled {
                            memory_event = Some(turn.text.clone());
                        }
                        runtime.conversation_error.clear();
                    }
                    Err(error) => {
                        turn.status = if error == "已停止生成" {
                            "cancelled".to_string()
                        } else {
                            "failed".to_string()
                        };
                        runtime.conversation_error = error;
                        if turn.text.is_empty() {
                            turn.text = "没有生成回复".to_string();
                        }
                    }
                }
                bubble_text = conversation_bubble_preview(&turn.text);
            }
            if let Some(text) = memory_event {
                let _ = runtime.memory.record_event(
                    &runtime.persona.id,
                    EventKind::PetReaction,
                    Some(text),
                );
            }
            if completed {
                runtime.status.clear();
                if runtime.config.memory.enabled {
                    maybe_request_memory_learning(runtime, command_tx);
                }
            }
            renew_conversation_bubble(runtime, bubble_text, Duration::from_secs(8));
            persist_conversation_history(runtime);
            true
        }
        RuntimeCommand::ReviewMemoryCandidate(review) => {
            match runtime.memory.review_candidate(
                &runtime.persona.id,
                &review.candidate_id,
                review.accept,
            ) {
                Ok(Some(fact)) => {
                    compress_facts_if_enabled(runtime);
                    runtime.status = format!("已确认习惯：{}：{}", fact.key, fact.value);
                }
                Ok(None) if review.accept => {
                    runtime.status = "这个习惯候选已处理或不存在".to_string();
                }
                Ok(None) => runtime.status = "已忽略这条习惯候选".to_string(),
                Err(error) => runtime.status = format!("处理习惯候选失败：{error:#}"),
            }
            true
        }
        RuntimeCommand::MemoryLearningFinished {
            request_id,
            pet_id,
            persona_id,
            through_turn_id,
            result,
        } => {
            apply_memory_learning_result(
                runtime,
                &request_id,
                &pet_id,
                &persona_id,
                &through_turn_id,
                result,
            );
            true
        }
        RuntimeCommand::ParsePersonaSource(request) => {
            if request.path.is_some() == request.text.is_some() {
                runtime.persona_source_error =
                    "请选择一个 TXT/JSON 文件，或粘贴聊天文字。".to_string();
                return true;
            }
            runtime.active_persona_source_request = Some(request.request_id.clone());
            runtime.persona_source = None;
            runtime.persona_draft = None;
            runtime.persona_preview.clear();
            runtime.persona_source_error.clear();
            runtime.persona_draft_error.clear();
            let tx = command_tx.clone();
            let request_id = request.request_id;
            thread::spawn(move || {
                let result = if let Some(path) = request.path {
                    parse_persona_source_file(&path, &request.label, &request.format)
                } else {
                    parse_persona_source_text(
                        request.label,
                        &request.format,
                        request.text.unwrap_or_default(),
                    )
                };
                let _ = tx.send(RuntimeCommand::PersonaSourceParseFinished { request_id, result });
            });
            true
        }
        RuntimeCommand::PersonaSourceParseFinished { request_id, result } => {
            if runtime.active_persona_source_request.as_deref() != Some(request_id.as_str()) {
                return true;
            }
            runtime.active_persona_source_request = None;
            match result {
                Ok(source) => {
                    runtime.persona_source = Some(source);
                    runtime.persona_source_error.clear();
                    runtime.persona_preview.clear();
                }
                Err(error) => runtime.persona_source_error = error,
            }
            true
        }
        RuntimeCommand::GeneratePersonaProfile(request) => {
            start_persona_generation(runtime, request, command_tx);
            true
        }
        RuntimeCommand::PersonaProfileFinished {
            request_id,
            pet_id,
            persona_id,
            result,
        } => {
            let current_persona_json = serde_json::to_string(&runtime.persona).unwrap_or_default();
            let Some(active) = runtime.active_persona_generation.as_ref() else {
                return true;
            };
            if active.request_id != request_id
                || active.pet_id != pet_id
                || active.persona_id != persona_id
                || active.persona_json != current_persona_json
                || runtime.selected_pet.as_deref() != Some(pet_id.as_str())
            {
                return true;
            }
            let active = runtime.active_persona_generation.take().unwrap();
            match result {
                Ok(generated) if generated.needs_more_context => {
                    runtime.persona_draft = None;
                    runtime.persona_draft_error = if generated.clarification.is_empty() {
                        "模型资料不足，请补充人物介绍或更多聊天样本。".to_string()
                    } else {
                        generated.clarification
                    };
                }
                Ok(generated) => {
                    let source = PersonaSource {
                        kind: active.kind,
                        label: active.label.clone(),
                        description: active.description,
                        sample_count: active.sample_count,
                        target_speaker: (!active.target_speaker_label.is_empty())
                            .then(|| active.target_speaker_label.clone()),
                        generated_at: petsona_core::memory::now_ms(),
                    };
                    let name = if generated.name.trim().is_empty() {
                        active.label
                    } else {
                        generated.name.trim().to_string()
                    };
                    runtime.persona_draft = Some(PersonaDraft {
                        id: active.request_id,
                        name,
                        style: generated.style,
                        source,
                    });
                    runtime.persona_draft_error.clear();
                }
                Err(error) => runtime.persona_draft_error = error,
            }
            true
        }
        RuntimeCommand::ApplyPersonaDraft(request) => {
            let Some(draft) = runtime.persona_draft.clone() else {
                runtime.persona_draft_error = "人格草稿已过期，请重新生成。".to_string();
                return true;
            };
            let Some(pet_id) = runtime.selected_pet.clone() else {
                runtime.persona_draft_error = "请先导入并选择宠物。".to_string();
                return true;
            };
            if draft.id != request.draft_id || pet_id != request.pet_id {
                runtime.persona_draft_error = "人格草稿属于另一只宠物，请重新生成。".to_string();
                return true;
            }
            let mut updated = runtime.persona.clone();
            let stable_id = updated.id.clone();
            updated.id = stable_id.clone();
            updated.name = if request.name.trim().is_empty() {
                draft.name.clone()
            } else {
                request.name.trim().to_string()
            };
            updated.style_profile = Some(request.style);
            updated.source = Some(draft.source);
            updated.builtin = false;
            if let Err(error) = updated.validate() {
                runtime.persona_draft_error = format!("人格内容无效：{error}");
                return true;
            }
            cancel_active_conversation(runtime, "");
            match runtime.personas.save(&updated) {
                Ok(()) => {
                    runtime.persona = updated;
                    runtime.config.active_persona = Some(stable_id.clone());
                    runtime.config.persona_by_pet.insert(pet_id, stable_id);
                    if let Err(error) = runtime.save_config() {
                        runtime.persona_draft_error =
                            format!("人格已保存，但绑定配置失败：{error}");
                    } else {
                        runtime.persona_draft = None;
                        runtime.persona_draft_error.clear();
                        runtime.persona_preview.clear();
                        runtime.status = "说话方式已应用，宠物记忆保持不变".to_string();
                    }
                }
                Err(error) => runtime.persona_draft_error = format!("保存人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::ClearPersonaDraft => {
            runtime.active_persona_source_request = None;
            runtime.active_persona_generation = None;
            runtime.active_persona_preview = None;
            runtime.persona_source = None;
            runtime.persona_draft = None;
            runtime.persona_source_error.clear();
            runtime.persona_draft_error.clear();
            runtime.persona_preview.clear();
            runtime.persona_preview_error.clear();
            true
        }
        RuntimeCommand::PreviewPersonaDraftRequest(request) => {
            start_persona_preview(runtime, request, command_tx);
            true
        }
        RuntimeCommand::PersonaPreviewFinished {
            request_id,
            pet_id,
            draft_id,
            result,
        } => {
            let Some(active) = runtime.active_persona_preview.as_ref() else {
                return true;
            };
            if active.request_id != request_id
                || active.pet_id != pet_id
                || active.draft_id != draft_id
                || runtime.selected_pet.as_deref() != Some(pet_id.as_str())
                || runtime
                    .persona_draft
                    .as_ref()
                    .map(|draft| draft.id.as_str())
                    != Some(draft_id.as_str())
            {
                return true;
            }
            runtime.active_persona_preview = None;
            match result {
                Ok(text) => {
                    runtime.persona_preview = text;
                    runtime.persona_preview_error.clear();
                }
                Err(error) => runtime.persona_preview_error = error,
            }
            true
        }
        RuntimeCommand::SendConversation(text) => {
            if runtime.conversation_inflight || text.trim().is_empty() {
                return true;
            }
            runtime.conversation_inflight = true;
            runtime.last_user_action = Instant::now();
            if runtime.config.memory.enabled {
                let _ = runtime.memory.record_event(
                    &runtime.persona.id,
                    EventKind::UserMessage,
                    Some(text.clone()),
                );
                if let Some((key, value, confidence)) = extract_preference(&text) {
                    record_explicit_preference(runtime, &key, &value, confidence, &text);
                }
            }
            let user_index = runtime.conversation_history.len();
            let user_turn_id = uuid::Uuid::new_v4().to_string();
            runtime
                .conversation_history
                .push(ConversationTurn::user(user_turn_id.clone(), text.clone()));
            if !runtime.config.memory.enabled {
                let _ = runtime
                    .memory
                    .mark_learning_cursor(&runtime.persona.id, &user_turn_id);
            }
            let context = conversation_memory_context(runtime);
            let history = runtime
                .conversation_history
                .iter()
                .take(user_index)
                .skip(if runtime.config.conversation.save_history {
                    0
                } else {
                    runtime.conversation_context_start
                })
                .rev()
                .take(12)
                .rev()
                .map(|turn| {
                    if turn.user {
                        format!("用户：{}", turn.text)
                    } else {
                        format!("宠物：{}", turn.text)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let persona = runtime.persona.clone();
            let config = runtime.config.deepseek.clone();
            let pet_name = runtime
                .pet
                .as_ref()
                .map(|pet| pet.entry.display_name.clone());
            let pet_state = runtime
                .pet
                .as_ref()
                .map(|pet| pet.engine.current().name().to_string())
                .unwrap_or_else(|| "idle".to_string());
            let tx = command_tx.clone();
            thread::spawn(move || {
                let result = DeepSeekClient::new(config)
                    .and_then(|client| {
                        client.generate_reply(
                            &persona,
                            &context,
                            &history,
                            &text,
                            &crate::greeting::local_now_text(),
                            pet_name.as_deref(),
                            &pet_state,
                        )
                    })
                    .map_err(|error| format!("{error:#}"));
                let _ = tx.send(RuntimeCommand::ConversationResult(result));
            });
            true
        }
        RuntimeCommand::GreetingResult(result) => {
            runtime.greeting_inflight = false;
            let text = match result {
                Ok(text) => text,
                Err(error) => {
                    // No key / network: still say hello with the local line.
                    runtime.status = error;
                    crate::greeting::fallback_greeting(&runtime.persona)
                }
            };
            let text = text.trim().to_string();
            if !text.is_empty() {
                if runtime.config.memory.enabled {
                    let _ = runtime.memory.record_event(
                        &runtime.persona.id,
                        EventKind::PetGreeting,
                        Some(text.clone()),
                    );
                }
                runtime.show_bubble(text, Duration::from_secs(8));
                let now = Instant::now();
                if let Some(pet) = &mut runtime.pet {
                    let raised = pet.engine.raise(
                        PetState::Waving,
                        "greeting",
                        None,
                        Some(Duration::from_secs(4)),
                        now,
                    );
                    if raised.is_some() {
                        pet.anim_started = now;
                        pet.last_state = pet.engine.current();
                    }
                }
            }
            true
        }
        RuntimeCommand::ConversationResult(result) => {
            runtime.conversation_inflight = false;
            let reply = result.unwrap_or_else(|error| {
                runtime.status = error;
                crate::greeting::fallback_greeting(&runtime.persona)
            });
            runtime.conversation_history.push(ConversationTurn {
                id: uuid::Uuid::new_v4().to_string(),
                request_id: None,
                user: false,
                text: reply.clone(),
                status: "complete".to_string(),
                created_at: petsona_core::memory::now_ms(),
            });
            runtime.conversation_page_start = runtime.conversation_history.len().saturating_sub(50);
            if runtime.config.memory.enabled {
                let _ = runtime.memory.record_event(
                    &runtime.persona.id,
                    EventKind::PetReaction,
                    Some(reply.clone()),
                );
            }
            runtime.show_bubble(reply, Duration::from_secs(8));
            true
        }
        RuntimeCommand::Stop => false,
    }
}

fn save_macos_window_position(
    config_dir: &std::path::Path,
    position: &WindowPosition,
) -> Result<()> {
    std::fs::create_dir_all(config_dir)
        .with_context(|| format!("创建配置目录失败：{}", config_dir.display()))?;
    let path = config_dir.join("macos-window-position.json");
    let temporary = config_dir.join("macos-window-position.json.tmp");
    let bytes = serde_json::to_vec_pretty(position).context("编码 macOS 窗口位置失败")?;
    std::fs::write(&temporary, bytes)
        .with_context(|| format!("写入窗口位置临时文件失败：{}", temporary.display()))?;
    std::fs::rename(&temporary, &path)
        .with_context(|| format!("原子替换窗口位置失败：{}", path.display()))
}

fn import_persona_into_stable_identity(
    store: &petsona_core::persona::PersonaStore,
    path: &std::path::Path,
    stable_id: &str,
) -> Result<Persona> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("读取人格文件失败：{}", path.display()))?;
    let mut persona: Persona = serde_json::from_str(&text).context("人格文件不是有效的 JSON")?;
    // Keep the active pet's stable identity so its memory bucket and per-pet
    // binding survive replacing the persona contents.
    persona.id = stable_id.to_string();
    persona.builtin = false;
    persona.validate()?;
    store.save(&persona)?;
    Ok(persona)
}

fn retry_insertion_index(turns: &[ConversationTurn], user_index: usize) -> Option<usize> {
    if !turns.get(user_index)?.user {
        return None;
    }
    let mut insertion_index = user_index + 1;
    let mut last_attempt_status = None;
    while let Some(turn) = turns.get(insertion_index) {
        if turn.user {
            break;
        }
        last_attempt_status = Some(turn.status.as_str());
        insertion_index += 1;
    }
    matches!(
        last_attempt_status,
        Some("failed" | "cancelled" | "interrupted")
    )
    .then_some(insertion_index)
}

fn start_streaming_conversation(
    runtime: &mut PetsonaRuntime,
    request: ConversationRequest,
    command_tx: &Sender<RuntimeCommand>,
) {
    if runtime.active_conversation.is_some() || runtime.conversation_inflight {
        runtime.conversation_error = "已有一条回复正在生成".to_string();
        return;
    }
    let Some(pet_id) = runtime.selected_pet.clone() else {
        runtime.conversation_error = "请先导入宠物".to_string();
        return;
    };
    if request.pet_id != pet_id || runtime.pet.is_none() {
        runtime.conversation_error = "聊天请求对应的宠物已切换".to_string();
        return;
    }

    let request_id = if request.request_id.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        request.request_id
    };
    if runtime
        .conversation_history
        .iter()
        .any(|turn| turn.request_id.as_deref() == Some(request_id.as_str()))
    {
        runtime.conversation_error = "聊天请求编号重复".to_string();
        return;
    }

    let retry_target = match request.retry_turn_id.as_deref() {
        Some(turn_id) => match runtime
            .conversation_history
            .iter()
            .position(|turn| turn.id == turn_id && turn.user)
        {
            Some(index) => {
                let user_turn = &runtime.conversation_history[index];
                if request.text.trim() != user_turn.text.trim() {
                    runtime.conversation_error = "重试内容与原消息不一致".to_string();
                    return;
                }
                match retry_insertion_index(&runtime.conversation_history, index) {
                    Some(insertion_index) => Some((index, insertion_index)),
                    None => {
                        runtime.conversation_error = "这条回复当前不能重试".to_string();
                        return;
                    }
                }
            }
            None => {
                runtime.conversation_error = "要重试的聊天消息已不存在".to_string();
                return;
            }
        },
        None => None,
    };
    let user_turn_index = if let Some((index, _)) = retry_target {
        index
    } else {
        let text = request.text.trim();
        if text.is_empty() {
            return;
        }
        let user_turn_id = uuid::Uuid::new_v4().to_string();
        let user_turn = ConversationTurn::user(user_turn_id.clone(), text.to_string());
        runtime.conversation_history.push(user_turn);
        let index = runtime.conversation_history.len() - 1;
        if runtime.config.memory.enabled {
            let _ = runtime.memory.record_event(
                &runtime.persona.id,
                EventKind::UserMessage,
                Some(text.to_string()),
            );
            if let Some((key, value, confidence)) = extract_preference(text) {
                record_explicit_preference(runtime, &key, &value, confidence, text);
            }
        } else {
            let _ = runtime
                .memory
                .mark_learning_cursor(&runtime.persona.id, &user_turn_id);
        }
        index
    };

    let text = runtime.conversation_history[user_turn_index].text.clone();
    let insertion_index = retry_target
        .map(|(_, insertion_index)| insertion_index)
        .unwrap_or(user_turn_index + 1);
    let assistant_turn = ConversationTurn::assistant(request_id.clone());
    let assistant_turn_id = assistant_turn.id.clone();
    runtime
        .conversation_history
        .insert(insertion_index, assistant_turn);
    runtime.conversation_page_start = runtime.conversation_history.len().saturating_sub(50);

    let context_start = if runtime.config.conversation.save_history {
        0
    } else {
        runtime.conversation_context_start
    };
    let history = runtime.conversation_history[context_start..user_turn_index]
        .iter()
        .filter(|turn| turn.status == "complete")
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|turn| {
            if turn.user {
                format!("用户：{}", turn.text)
            } else {
                format!("宠物：{}", turn.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let memory = conversation_memory_context(runtime);
    let persona = runtime.persona.clone();
    let persona_json = serde_json::to_string(&persona).unwrap_or_default();
    let config = runtime.config.deepseek.clone();
    let pet_name = runtime
        .pet
        .as_ref()
        .map(|pet| pet.entry.display_name.clone());
    let pet_state = runtime
        .pet
        .as_ref()
        .map(|pet| pet.engine.current().name().to_string())
        .unwrap_or_else(|| "idle".to_string());
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    runtime.active_conversation = Some(ActiveConversation {
        request_id: request_id.clone(),
        pet_id: pet_id.clone(),
        persona_id: persona.id.clone(),
        persona_json,
        assistant_turn_id,
        cancel: Arc::clone(&cancelled),
    });
    runtime.conversation_inflight = true;
    runtime.conversation_error.clear();
    runtime.last_user_action = Instant::now();
    renew_conversation_bubble(runtime, "…".to_string(), Duration::from_secs(30));
    persist_conversation_history(runtime);

    let tx = command_tx.clone();
    let stream_request_id = request_id.clone();
    let stream_pet_id = pet_id.clone();
    let stream_persona_id = persona.id.clone();
    let _ = thread::Builder::new()
        .name("petsona-chat-stream".to_string())
        .spawn(move || {
            let result = DeepSeekClient::new(config.clone())
                .map_err(|error| format!("{error:#}"))
                .and_then(|client| {
                    let (system, user) = client.conversation_prompt(
                        &persona,
                        &memory,
                        &history,
                        &text,
                        &crate::greeting::local_now_text(),
                        pet_name.as_deref(),
                        &pet_state,
                    );
                    crate::conversation::stream_completion(
                        &client,
                        &config,
                        &system,
                        &user,
                        Arc::clone(&cancelled),
                        |chunk| {
                            tx.send(RuntimeCommand::ConversationStreamChunk {
                                request_id: stream_request_id.clone(),
                                pet_id: stream_pet_id.clone(),
                                persona_id: stream_persona_id.clone(),
                                chunk,
                            })
                            .is_ok()
                        },
                    )
                });
            let _ = tx.send(RuntimeCommand::ConversationStreamFinished {
                request_id,
                pet_id,
                persona_id: persona.id,
                result,
            });
        });
}

fn preserve_memory_learning_boundary(runtime: &PetsonaRuntime) {
    if let Some(turn_id) = runtime
        .conversation_history
        .iter()
        .rev()
        .find(|turn| turn.user && turn.status == "complete")
        .map(|turn| turn.id.clone())
    {
        let _ = runtime
            .memory
            .mark_learning_cursor(&runtime.persona.id, &turn_id);
    }
}

fn maybe_request_memory_learning(
    runtime: &mut PetsonaRuntime,
    command_tx: &Sender<RuntimeCommand>,
) {
    if !runtime.config.memory.enabled
        || !runtime.key_configured
        || runtime.active_memory_learning.is_some()
        || runtime.selected_pet.is_none()
    {
        return;
    }
    let Some(persona_id) = runtime.selected_pet.clone() else {
        return;
    };
    let cursor = runtime.memory.learning_cursor(&runtime.persona.id);
    let start = cursor
        .as_deref()
        .and_then(|id| {
            runtime
                .conversation_history
                .iter()
                .rposition(|turn| turn.id == id)
        })
        .map(|index| index + 1)
        .unwrap_or(runtime.conversation_context_start);
    if start >= runtime.conversation_history.len() {
        return;
    }
    let new_user_turns = runtime.conversation_history[start..]
        .iter()
        .filter(|turn| turn.user && turn.status == "complete")
        .collect::<Vec<_>>();
    if new_user_turns.len() < 3 {
        return;
    }
    let Some(through_turn_id) = new_user_turns.last().map(|turn| turn.id.clone()) else {
        return;
    };
    let transcript_turns = runtime.conversation_history[start..]
        .iter()
        .filter(|turn| turn.status == "complete")
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let evidence_turn_ids = transcript_turns
        .iter()
        .filter(|turn| turn.user)
        .map(|turn| turn.id.clone())
        .collect::<Vec<_>>();
    if evidence_turn_ids.len() < 3 {
        return;
    }
    let transcript = serde_json::json!(transcript_turns
        .iter()
        .map(|turn| serde_json::json!({
            "id": turn.id,
            "role": if turn.user { "user" } else { "pet" },
            "text": turn.text,
        }))
        .collect::<Vec<_>>())
    .to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let persona = runtime.persona.clone();
    let persona_json = serde_json::to_string(&persona).unwrap_or_default();
    let config = runtime.config.deepseek.clone();
    runtime.active_memory_learning = Some(ActiveMemoryLearning {
        request_id: request_id.clone(),
        pet_id: persona_id.clone(),
        persona_id: persona.id.clone(),
        persona_json,
        through_turn_id: through_turn_id.clone(),
        evidence_turn_ids: evidence_turn_ids.clone(),
    });
    let tx = command_tx.clone();
    thread::spawn(move || {
        let result = DeepSeekClient::new(config)
            .and_then(|client| client.generate_memory_suggestions(&transcript))
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(RuntimeCommand::MemoryLearningFinished {
            request_id,
            pet_id: persona_id,
            persona_id: persona.id,
            through_turn_id,
            result,
        });
    });
}

fn apply_memory_learning_result(
    runtime: &mut PetsonaRuntime,
    request_id: &str,
    pet_id: &str,
    persona_id: &str,
    through_turn_id: &str,
    result: Result<Vec<petsona_core::memory::MemorySuggestion>, String>,
) {
    let Some(active) = runtime.active_memory_learning.as_ref() else {
        return;
    };
    let persona_json = serde_json::to_string(&runtime.persona).unwrap_or_default();
    if active.request_id != request_id
        || active.pet_id != pet_id
        || active.persona_id != persona_id
        || active.through_turn_id != through_turn_id
        || active.persona_json != persona_json
        || runtime.selected_pet.as_deref() != Some(pet_id)
        || !runtime.config.memory.enabled
    {
        return;
    }
    let Some(active) = runtime.active_memory_learning.take() else {
        return;
    };
    let mut proposed = 0;
    match result {
        Ok(suggestions) => {
            for suggestion in suggestions {
                let mut evidence = Vec::new();
                for turn_id in &suggestion.evidence_turn_ids {
                    if !active.evidence_turn_ids.iter().any(|id| id == turn_id) {
                        continue;
                    }
                    if let Some(turn) = runtime
                        .conversation_history
                        .iter()
                        .find(|turn| turn.id == *turn_id && turn.user && turn.status == "complete")
                    {
                        evidence.push(format!("{}：{}", turn.id, turn.text));
                    }
                }
                if evidence.len() < 3 {
                    continue;
                }
                match runtime.memory.propose_candidate(
                    &runtime.persona.id,
                    &suggestion.key,
                    &suggestion.value,
                    suggestion.confidence,
                    &evidence,
                ) {
                    Ok(Some(_)) => proposed += 1,
                    Ok(None) => {}
                    Err(error) => {
                        runtime.status = format!("保存习惯候选失败：{error:#}");
                    }
                }
            }
            if let Err(error) = runtime
                .memory
                .mark_learning_cursor(&runtime.persona.id, through_turn_id)
            {
                runtime.status = format!("保存记忆分析位置失败：{error:#}");
            } else if proposed > 0 {
                runtime.status = format!("有 {proposed} 条新习惯等待确认");
            }
        }
        Err(error) => {
            runtime.status = format!("习惯分析失败：{error}");
            if let Err(save_error) = runtime
                .memory
                .mark_learning_cursor(&runtime.persona.id, through_turn_id)
            {
                runtime.status = format!("习惯分析失败且无法保存分析位置：{save_error:#}");
            }
        }
    }
}

const MAX_PERSONA_SOURCE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_PERSONA_SAMPLE_BYTES: usize = 50 * 1024;

fn parse_persona_source_text(
    label: String,
    format: &str,
    text: String,
) -> Result<ParsedPersonaSource, String> {
    if text.len() as u64 > MAX_PERSONA_SOURCE_BYTES {
        return Err("聊天记录超过 5 MB，请缩小文件或分段导入。".to_string());
    }
    let messages = match format.trim().to_ascii_lowercase().as_str() {
        "json" => petsona_core::persona_source::parse_json_chat(&text),
        "txt" | "text" => petsona_core::persona_source::parse_text_chat(&text),
        _ => return Err("只支持 UTF-8 TXT 或标准 JSON 聊天记录。".to_string()),
    }
    .map_err(|error| format!("{error:#}"))?;
    Ok(ParsedPersonaSource {
        id: uuid::Uuid::new_v4().to_string(),
        label: if label.trim().is_empty() {
            "粘贴的聊天记录".to_string()
        } else {
            label.trim().to_string()
        },
        format: format.trim().to_ascii_lowercase(),
        messages,
    })
}

fn parse_persona_source_file(
    path: &std::path::Path,
    label: &str,
    format: &str,
) -> Result<ParsedPersonaSource, String> {
    let metadata = std::fs::metadata(path).map_err(|error| format!("无法读取资料文件：{error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_PERSONA_SOURCE_BYTES {
        return Err("只支持 5 MB 以内的 TXT/JSON 文件。".to_string());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("资料文件必须是 UTF-8 文本：{error}"))?;
    let inferred_format = if format.trim().is_empty() || format == "auto" {
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
    } else {
        format.to_ascii_lowercase()
    };
    let label = if label.trim().is_empty() {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("聊天记录")
            .to_string()
    } else {
        label.to_string()
    };
    parse_persona_source_text(label, &inferred_format, content)
}

fn partition_persona_messages<'a>(
    messages: &'a [petsona_core::persona_source::PersonaChatMessage],
    target_speaker: &str,
) -> (
    Vec<&'a petsona_core::persona_source::PersonaChatMessage>,
    Vec<&'a petsona_core::persona_source::PersonaChatMessage>,
) {
    let target = messages
        .iter()
        .filter(|message| message.speaker == target_speaker)
        .collect::<Vec<_>>();
    let context = messages
        .iter()
        .filter(|message| message.speaker != target_speaker)
        .take(24)
        .collect::<Vec<_>>();
    (target, context)
}

fn start_persona_generation(
    runtime: &mut PetsonaRuntime,
    request: PersonaProfileRequest,
    command_tx: &Sender<RuntimeCommand>,
) {
    if runtime.active_persona_generation.is_some() {
        runtime.persona_draft_error = "人格资料仍在生成，请稍候。".to_string();
        return;
    }
    let Some(pet_id) = runtime.selected_pet.clone() else {
        runtime.persona_draft_error = "请先导入并选择一只宠物。".to_string();
        return;
    };
    if !runtime.key_configured {
        runtime.persona_draft_error = "请先在模型服务中配置 API Key。".to_string();
        return;
    }
    let (target_messages, context_messages) = if request.kind == "chat_import" {
        let Some(source) = runtime
            .persona_source
            .as_ref()
            .filter(|source| source.id == request.source_id)
        else {
            runtime.persona_draft_error = "聊天样本已失效，请重新导入。".to_string();
            return;
        };
        if request.start_index >= request.end_index || request.end_index > source.messages.len() {
            runtime.persona_draft_error = "聊天样本范围无效。".to_string();
            return;
        }
        let selected = &source.messages[request.start_index..request.end_index];
        let target_count = selected
            .iter()
            .filter(|message| message.speaker == request.target_speaker)
            .count();
        if request.target_speaker.trim().is_empty() || target_count < 3 {
            runtime.persona_draft_error =
                "请选择目标说话人并至少包含 3 条该说话人的消息。".to_string();
            return;
        }
        let (target, context) = partition_persona_messages(selected, &request.target_speaker);
        (target, context)
    } else if request.kind == "public_figure" {
        if request.label.trim().is_empty() {
            runtime.persona_draft_error = "请输入要参考的人物姓名。".to_string();
            return;
        }
        (Vec::new(), Vec::new())
    } else {
        runtime.persona_draft_error = "未知的人格来源。".to_string();
        return;
    };
    let sample_count = target_messages.len();
    let messages_json = serde_json::json!({
        "targetMessages": target_messages,
        "conversationContext": context_messages,
    })
    .to_string();
    if messages_json.len() > MAX_PERSONA_SAMPLE_BYTES || request.description.len() > 20_000 {
        runtime.persona_draft_error = "选择的材料过长，请缩小采样范围或精简介绍。".to_string();
        return;
    }
    let request_id = if request.request_id.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        request.request_id
    };
    let persona = runtime.persona.clone();
    let persona_json = serde_json::to_string(&persona).unwrap_or_default();
    let persona_id = persona.id.clone();
    let config = runtime.config.deepseek.clone();
    let target_speaker_label = request.target_speaker_label.trim().to_string();
    runtime.active_persona_generation = Some(ActivePersonaGeneration {
        request_id: request_id.clone(),
        pet_id: pet_id.clone(),
        persona_id: persona_id.clone(),
        persona_json,
        kind: request.kind,
        label: request.label,
        description: request.description,
        target_speaker_label: target_speaker_label.clone(),
        sample_count,
    });
    runtime.persona_draft = None;
    runtime.persona_draft_error.clear();
    runtime.persona_preview.clear();
    runtime.persona_preview_error.clear();
    let tx = command_tx.clone();
    let source_kind = runtime
        .active_persona_generation
        .as_ref()
        .unwrap()
        .kind
        .clone();
    let source_label = runtime
        .active_persona_generation
        .as_ref()
        .unwrap()
        .label
        .clone();
    let description = runtime
        .active_persona_generation
        .as_ref()
        .unwrap()
        .description
        .clone();
    let target_speaker = if target_speaker_label.is_empty() {
        request.target_speaker
    } else {
        target_speaker_label
    };
    thread::spawn(move || {
        let result = DeepSeekClient::new(config)
            .and_then(|client| {
                client.generate_persona_profile(
                    &source_kind,
                    &source_label,
                    &description,
                    &target_speaker,
                    &messages_json,
                )
            })
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(RuntimeCommand::PersonaProfileFinished {
            request_id,
            pet_id,
            persona_id,
            result,
        });
    });
}

fn start_persona_preview(
    runtime: &mut PetsonaRuntime,
    request: PersonaPreviewRequest,
    command_tx: &Sender<RuntimeCommand>,
) {
    if runtime.active_persona_preview.is_some() || request.prompt.trim().is_empty() {
        return;
    }
    let Some(pet_id) = runtime.selected_pet.clone() else {
        return;
    };
    let Some(draft) = runtime
        .persona_draft
        .as_ref()
        .filter(|draft| draft.id == request.draft_id)
    else {
        runtime.persona_preview_error = "人格草稿已过期，请重新生成。".to_string();
        return;
    };
    if request.pet_id != pet_id || request.prompt.chars().count() > 10_000 {
        runtime.persona_preview_error = "试聊内容过长或宠物已切换。".to_string();
        return;
    }
    if !runtime.key_configured {
        runtime.persona_preview_error = "请先在模型服务中配置 API Key。".to_string();
        return;
    }
    runtime.active_persona_preview = Some(ActivePersonaPreview {
        request_id: request.request_id.clone(),
        pet_id: pet_id.clone(),
        draft_id: draft.id.clone(),
    });
    runtime.persona_preview.clear();
    runtime.persona_preview_error.clear();
    let mut persona = runtime.persona.clone();
    persona.name = draft.name.clone();
    persona.style_profile = Some(draft.style.clone());
    persona.source = Some(draft.source.clone());
    let config = runtime.config.deepseek.clone();
    let pet_state = runtime
        .pet
        .as_ref()
        .map(|pet| pet.engine.current().name().to_string())
        .unwrap_or_else(|| "idle".to_string());
    let pet_name = runtime
        .pet
        .as_ref()
        .map(|pet| pet.entry.display_name.clone());
    let tx = command_tx.clone();
    let draft_id = request.draft_id;
    thread::spawn(move || {
        let result = DeepSeekClient::new(config)
            .and_then(|client| {
                client.generate_reply(
                    &persona,
                    &GreetingContext::default(),
                    "",
                    &request.prompt,
                    &crate::greeting::local_now_text(),
                    pet_name.as_deref(),
                    &pet_state,
                )
            })
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(RuntimeCommand::PersonaPreviewFinished {
            request_id: request.request_id,
            pet_id,
            draft_id,
            result,
        });
    });
}

fn conversation_memory_context(runtime: &PetsonaRuntime) -> GreetingContext {
    if runtime.config.memory.enabled {
        runtime.memory.build_greeting_context(
            &runtime.persona.id,
            runtime.config.memory.recent_events,
            runtime.config.memory.fact_limit,
        )
    } else {
        GreetingContext::default()
    }
}

fn record_explicit_preference(
    runtime: &mut PetsonaRuntime,
    key: &str,
    value: &str,
    confidence: f32,
    evidence: &str,
) {
    match runtime.memory.remember_explicit_preference(
        &runtime.persona.id,
        key,
        value,
        confidence,
        evidence,
    ) {
        Ok(true) => {
            runtime.status = "已从对话记录一条用户偏好".to_string();
            compress_facts_if_enabled(runtime);
        }
        Ok(false) => {
            let has_pending_correction = runtime
                .memory
                .persona_snapshot(&runtime.persona.id)
                .candidates
                .iter()
                .any(|candidate| {
                    candidate.status == "pending"
                        && candidate.key.eq_ignore_ascii_case(key)
                        && candidate.value.eq_ignore_ascii_case(value)
                });
            if has_pending_correction {
                runtime.status = "这条偏好与手动记录有冲突，等待你确认".to_string();
            }
        }
        Err(error) => runtime.status = format!("保存明确偏好失败：{error:#}"),
    }
}

fn conversation_bubble_preview(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 110;
    let mut preview = text.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    if text.chars().count() > MAX_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

fn renew_conversation_bubble(runtime: &mut PetsonaRuntime, text: String, ttl: Duration) {
    let now = Instant::now();
    if let Some(bubble) = runtime.bubble.as_mut() {
        bubble.text = text;
        bubble.total = ttl;
        bubble.until = now.checked_add(ttl).unwrap_or(now);
        if bubble.paused_remaining.is_some() {
            bubble.paused_remaining = Some(ttl);
        }
    } else {
        runtime.show_bubble(text, ttl);
    }
}

fn persist_conversation_history(runtime: &mut PetsonaRuntime) {
    if !runtime.config.conversation.save_history {
        return;
    }
    let Some(pet_id) = runtime.selected_pet.as_deref() else {
        return;
    };
    if let Err(error) = runtime
        .conversation_store
        .save(pet_id, &runtime.conversation_history)
    {
        runtime.conversation_error = format!("保存聊天记录失败：{error:#}");
    }
}

fn cancel_active_conversation(runtime: &mut PetsonaRuntime, reason: &str) {
    runtime.active_memory_learning = None;
    let Some(active) = runtime.active_conversation.take() else {
        return;
    };
    active
        .cancel
        .store(true, std::sync::atomic::Ordering::Release);
    runtime.conversation_inflight = false;
    let bubble = if let Some(turn) = runtime
        .conversation_history
        .iter_mut()
        .find(|turn| turn.id == active.assistant_turn_id)
    {
        turn.status = "cancelled".to_string();
        if turn.text.is_empty() {
            turn.text = "回复已停止".to_string();
        }
        Some(conversation_bubble_preview(&turn.text))
    } else {
        None
    };
    if let Some(bubble) = bubble {
        renew_conversation_bubble(runtime, bubble, Duration::from_secs(8));
    }
    runtime.conversation_error = reason.to_string();
    persist_conversation_history(runtime);
}

fn load_pet_conversation_history(runtime: &mut PetsonaRuntime, pet_id: &str) {
    runtime.active_memory_learning = None;
    runtime.conversation_history = match runtime.conversation_store.load(pet_id) {
        Ok(turns) => turns,
        Err(error) => {
            tracing::warn!(pet = %pet_id, %error, "cannot load conversation history");
            runtime.conversation_error = format!("读取聊天记录失败：{error:#}");
            Vec::new()
        }
    };
    let mut interrupted = false;
    for turn in &mut runtime.conversation_history {
        if turn.status == "streaming" {
            turn.status = "interrupted".to_string();
            interrupted = true;
        }
    }
    if interrupted {
        persist_conversation_history(runtime);
    }
    runtime.conversation_context_start = runtime.conversation_history.len();
    runtime.conversation_page_start = runtime.conversation_history.len().saturating_sub(50);
}

fn apply_persona_patch(persona: &mut Persona, patch: PersonaPatch) {
    if let Some(value) = patch.name {
        persona.name = value;
    }
    if let Some(value) = patch.tone {
        persona.traits.tone = value;
    }
    if let Some(value) = patch.verbosity {
        persona.traits.verbosity = value;
    }
    if let Some(value) = patch.emoji {
        persona.traits.emoji = value;
    }
    if let Some(value) = patch.greeting {
        persona.greeting = empty_to_none(value);
    }
    if let Some(value) = patch.system_prompt {
        persona.system_prompt = value;
    }
}

/// True when the idle greeting should fire: the feature is on, the user has
/// been quiet for `idleMinutes`, and the previous greeting is older than
/// `cooldownMinutes`. Pure so it can be unit-tested with synthetic instants.
fn greeting_due(
    greeting: &petsona_core::config::GreetingConfig,
    last_user_action: Instant,
    last_greeting_at: Option<Instant>,
    now: Instant,
) -> bool {
    if !greeting.enabled {
        return false;
    }
    let idle = now.saturating_duration_since(last_user_action);
    if idle < Duration::from_secs(u64::from(greeting.idle_minutes.max(1)) * 60) {
        return false;
    }
    if let Some(last) = last_greeting_at {
        let cooldown = u64::from(greeting.cooldown_minutes);
        if now.saturating_duration_since(last) < Duration::from_secs(cooldown * 60) {
            return false;
        }
    }
    true
}

/// Start a proactive idle greeting once the user has been quiet for
/// `greeting.idleMinutes` and the cooldown has elapsed (REQ-S03). The model
/// call runs on its own thread and answers through `GreetingResult`.
fn maybe_request_greeting(runtime: &mut PetsonaRuntime, command_tx: &Sender<RuntimeCommand>) {
    if runtime.greeting_inflight || runtime.conversation_inflight {
        return;
    }
    if runtime.pet.is_none() || !runtime.pet_visible {
        return;
    }
    if !greeting_due(
        &runtime.config.greeting,
        runtime.last_user_action,
        runtime.last_greeting_at,
        Instant::now(),
    ) {
        return;
    }

    if !runtime.key_configured {
        runtime.greeting_inflight = true;
        runtime.last_greeting_at = Some(Instant::now());
        let _ = command_tx.send(RuntimeCommand::GreetingResult(Err(
            "未配置模型密钥，使用本地问候".to_string(),
        )));
        return;
    }

    let context = conversation_memory_context(runtime);
    let persona = runtime.persona.clone();
    let config = runtime.config.deepseek.clone();
    let max_chars = runtime.config.greeting.max_chars.clamp(1, 200);
    let now_text = crate::greeting::local_now_text();
    let pet_name = runtime
        .pet
        .as_ref()
        .map(|pet| pet.entry.display_name.clone());
    let pet_state = runtime
        .pet
        .as_ref()
        .map(|pet| pet.engine.current().name().to_string())
        .unwrap_or_else(|| PetState::Idle.name().to_string());

    runtime.greeting_inflight = true;
    runtime.last_greeting_at = Some(Instant::now());
    let tx = command_tx.clone();
    thread::spawn(move || {
        let result = DeepSeekClient::new(config)
            .and_then(|client| {
                client.generate_greeting(
                    &persona,
                    &context,
                    "idle",
                    &now_text,
                    pet_name.as_deref(),
                    &pet_state,
                    max_chars,
                )
            })
            .map_err(|error| format!("{error:#}"));
        let _ = tx.send(RuntimeCommand::GreetingResult(result));
    });
}

fn empty_to_none(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn create_persona(
    store: &petsona_core::persona::PersonaStore,
    spec: PersonaCreate,
) -> anyhow::Result<Persona> {
    if store.get(&spec.id)?.is_some() {
        anyhow::bail!("人格 '{}' 已存在", spec.id);
    }
    let mut persona = spec
        .template
        .as_deref()
        .and_then(|id| templates().get(id).cloned())
        .unwrap_or_else(|| Persona::new(&spec.id, &spec.name));
    persona.id = spec.id;
    persona.name = spec.name;
    persona.builtin = false;
    store.save(&persona)?;
    Ok(persona)
}

fn sanitize_deepseek_config(mut config: DeepSeekConfig) -> DeepSeekConfig {
    config.provider = normalize_provider(&config.provider);
    if config.base_url.trim().is_empty() {
        config.base_url = DeepSeekConfig::default().base_url;
    }
    if config.model.trim().is_empty() {
        config.model = DeepSeekConfig::default().model;
    }
    if config.api_key_env.trim().is_empty() {
        config.api_key_env = DeepSeekConfig::default().api_key_env;
    }
    config.timeout_seconds = config.timeout_seconds.clamp(5, 120);
    config.max_tokens = config.max_tokens.clamp(16, 4000);
    config.conversation_max_tokens = config.conversation_max_tokens.clamp(64, 4000);
    config.temperature = config.temperature.clamp(0.0, 2.0);
    config
}

fn sanitize_greeting_config(mut config: GreetingConfig) -> GreetingConfig {
    config.idle_minutes = config.idle_minutes.clamp(1, 24 * 60);
    config.cooldown_minutes = config.cooldown_minutes.clamp(0, 24 * 60);
    config.max_chars = config.max_chars.clamp(1, 200);
    config
}

/// Only the two shipped providers exist; anything else (or an empty string in
/// a hand-edited config) falls back to DeepSeek.
/// REQ-P05: fold facts beyond `factLimit` into one 「画像」 fact when the user
/// left compression on. Replaces the old silent truncation.
fn compress_facts_if_enabled(runtime: &mut PetsonaRuntime) {
    if !runtime.config.memory.enabled || !runtime.config.memory.fact_compress {
        return;
    }
    let keep = runtime.config.memory.fact_limit.max(1);
    let _ = runtime.memory.compress_facts(&runtime.persona.id, keep);
}

fn normalize_provider(provider: &str) -> String {
    if provider
        .trim()
        .eq_ignore_ascii_case(petsona_core::config::PROVIDER_CUSTOM)
    {
        petsona_core::config::PROVIDER_CUSTOM.to_string()
    } else {
        petsona_core::config::PROVIDER_DEEPSEEK.to_string()
    }
}

fn sanitize_memory_config(mut config: MemoryConfig) -> MemoryConfig {
    config.recent_events = config.recent_events.clamp(1, 100);
    config.fact_limit = config.fact_limit.clamp(1, 50);
    config
}

fn tick_runtime(
    runtime: &mut PetsonaRuntime,
    command_tx: &Sender<RuntimeCommand>,
    projection: &Arc<SharedProjection>,
    revision: &mut u64,
) {
    runtime.poll_state_events();
    maybe_request_greeting(runtime, command_tx);
    let now = Instant::now();
    if let Some(pet) = &mut runtime.pet {
        if pet.engine.tick(now).is_some() {
            pet.anim_started = now;
            pet.last_state = pet.engine.current();
        }
        let elapsed_ms = pet.anim_started.elapsed().as_secs_f32() * 1000.0;
        pet.current_sprite(elapsed_ms);
    }
    if runtime
        .bubble
        .as_ref()
        .is_some_and(|bubble| bubble.is_expired(now))
    {
        runtime.bubble = None;
    }
    if let Some(active) = runtime.active_conversation.as_ref() {
        let renew = runtime
            .bubble
            .as_ref()
            .is_none_or(|bubble| bubble.remaining(now) <= Duration::from_secs(6));
        if renew {
            let assistant_turn_id = active.assistant_turn_id.as_str();
            let text = runtime
                .conversation_history
                .iter()
                .find(|turn| turn.id == assistant_turn_id)
                .map(|turn| {
                    if turn.text.is_empty() {
                        "…".to_string()
                    } else {
                        conversation_bubble_preview(&turn.text)
                    }
                })
                .unwrap_or_else(|| "…".to_string());
            renew_conversation_bubble(runtime, text, Duration::from_secs(30));
        }
    }
    // A TTL expiry or one-shot fallback can change the visible state without
    // receiving a new protocol event. Keep /health aligned with the immutable
    // projection after every worker tick.
    runtime.publish_health();
    *revision = revision.saturating_add(1);
    publish_projection(runtime, projection, *revision);
}

fn next_wait(runtime: &PetsonaRuntime) -> Duration {
    if !runtime.pet_visible {
        return IDLE_WORKER_WAIT;
    }
    runtime
        .pet
        .as_ref()
        .map(|pet| {
            pet.next_frame_after()
                .clamp(Duration::from_millis(1), IDLE_WORKER_WAIT)
        })
        .unwrap_or(IDLE_WORKER_WAIT)
}

fn publish_projection(runtime: &PetsonaRuntime, projection: &Arc<SharedProjection>, revision: u64) {
    let mut snapshot = RuntimeSnapshot {
        ready: true,
        revision,
        pet_visible: runtime.pet_visible,
        click_through: runtime.config.window.click_through,
        scale: runtime.config.window.scale,
        state_server_port: runtime.state_server_port,
        first_run: runtime.config.first_run,
        auto_walk: runtime.config.window.auto_walk.enabled,
        gravity_enabled: runtime.config.window.gravity_enabled,
        always_on_top: runtime.config.window.always_on_top,
        conversation_inflight: runtime.conversation_inflight
            || runtime.active_conversation.is_some(),
        conversation_history_len: runtime.conversation_history.len() as u32,
        ..RuntimeSnapshot::default()
    };
    let pets = serde_json::to_string(
        &runtime
            .pets
            .iter()
            .map(|pet| {
                serde_json::json!({
                    "id": pet.id,
                    "name": pet.display_name,
                    "v2": pet.sprite_version_number == Some(2) || pet.frame.rows >= 11,
                    "spritesheet": pet.spritesheet,
                    "cellWidth": pet.frame.width,
                    "cellHeight": pet.frame.height,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let codex_pets = serde_json::to_string(
        &runtime
            .codex_pets
            .iter()
            .map(|pet| {
                serde_json::json!({
                    "id": pet.id,
                    "name": pet.display_name,
                    "path": pet.dir,
                    "spritesheet": pet.spritesheet,
                    "cellWidth": pet.frame.width,
                    "cellHeight": pet.frame.height,
                    "columns": pet.frame.columns,
                    "v2": pet.sprite_version_number == Some(2) || pet.frame.rows >= 11,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let personas = serde_json::to_string(
        &runtime
            .personas
            .list()
            .unwrap_or_default()
            .iter()
            .map(|persona| {
                serde_json::json!({
                    "id": persona.id,
                    "name": persona.name,
                    "builtin": persona.builtin,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let models = serde_json::to_string(&runtime.models).unwrap_or_else(|_| "[]".to_string());
    let persona = serde_json::to_string(&runtime.persona).unwrap_or_else(|_| "{}".to_string());
    // The UI needs to show 已配置 / 未配置 without ever reading the secret.
    let mut deepseek_value =
        serde_json::to_value(&runtime.config.deepseek).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = deepseek_value.as_object_mut() {
        object.insert(
            "keyConfigured".to_string(),
            serde_json::json!(runtime.key_configured),
        );
    }
    let deepseek_config =
        serde_json::to_string(&deepseek_value).unwrap_or_else(|_| "{}".to_string());
    let memory_snapshot = runtime.memory.persona_snapshot(&runtime.persona.id);
    let memory = serde_json::to_string(&serde_json::json!({
        "config": runtime.config.memory,
        "greeting": runtime.config.greeting,
        "facts": memory_snapshot.facts,
        "archivedFacts": memory_snapshot.archived_facts,
        "candidates": memory_snapshot.candidates,
        "learning": runtime.active_memory_learning.is_some(),
        "events": memory_snapshot.events,
        "lastSeenAt": memory_snapshot.last_seen_at,
        "lastGreetingAt": memory_snapshot.last_greeting_at,
        "lastTrigger": memory_snapshot.last_trigger,
    }))
    .unwrap_or_else(|_| "{}".to_string());
    let import_conflict = runtime
        .import_conflict
        .as_ref()
        .map(ImportConflict::as_json)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let conversation = serde_json::json!({
        "petId": runtime.selected_pet,
        "saveHistory": runtime.config.conversation.save_history,
        "requestId": runtime.active_conversation.as_ref().map(|active| &active.request_id),
        "inFlight": runtime.active_conversation.is_some() || runtime.conversation_inflight,
        "error": runtime.conversation_error,
        "totalCount": runtime.conversation_history.len(),
        "hasEarlier": runtime.conversation_page_start > 0,
        "turns": runtime.conversation_history
            .iter()
            .skip(runtime.conversation_page_start)
            .collect::<Vec<_>>(),
    })
    .to_string();
    let persona_source = if let Some(source) = &runtime.persona_source {
        let mut speakers = std::collections::BTreeMap::<String, usize>::new();
        for message in &source.messages {
            *speakers.entry(message.speaker.clone()).or_default() += 1;
        }
        serde_json::json!({
            "id": source.id,
            "label": source.label,
            "format": source.format,
            "messageCount": source.messages.len(),
            "speakers": speakers.iter().map(|(name, count)| serde_json::json!({
                "name": name,
                "count": count,
            })).collect::<Vec<_>>(),
            "preview": source.messages.iter().take(12).collect::<Vec<_>>(),
            "error": runtime.persona_source_error,
            "parsing": runtime.active_persona_source_request.is_some(),
        })
    } else {
        serde_json::json!({
            "error": runtime.persona_source_error,
            "parsing": runtime.active_persona_source_request.is_some(),
        })
    }
    .to_string();
    let persona_draft = serde_json::json!({
        "draft": runtime.persona_draft,
        "generating": runtime.active_persona_generation.is_some(),
        "error": runtime.persona_draft_error,
    })
    .to_string();
    let persona_preview = serde_json::json!({
        "requestId": runtime.active_persona_preview.as_ref().map(|preview| &preview.request_id),
        "inFlight": runtime.active_persona_preview.is_some(),
        "text": runtime.persona_preview,
        "error": runtime.persona_preview_error,
    })
    .to_string();
    let settings = serde_json::json!({
        "activePet": runtime.config.active_pet,
        "activePersona": runtime.config.active_persona,
        "firstRun": runtime.config.first_run,
        "window": runtime.config.window,
        "greeting": runtime.config.greeting,
        "memory": runtime.config.memory,
        "conversation": runtime.config.conversation,
        "stateServer": runtime.config.state_server,
        "paths": {
            "dataDir": runtime.paths.config_dir,
            "petsDir": runtime.paths.pets_dir,
            "personasDir": runtime.paths.personas_dir,
            "logsDir": runtime.paths.logs_dir,
            "configFile": runtime.paths.config_file,
            "memoryFile": runtime.paths.memory_file,
        },
    })
    .to_string();
    let mut texts = RuntimeTexts {
        pets,
        codex_pets,
        persona,
        personas,
        deepseek_config,
        memory,
        import_conflict,
        models,
        conversation,
        persona_source,
        persona_draft,
        persona_preview,
        settings,
        persona_id: runtime.persona.id.clone(),
        persona_name: runtime.persona.name.clone(),
        ..RuntimeTexts::default()
    };
    if let Some(pet) = &runtime.pet {
        snapshot.has_pet = true;
        snapshot.sprite_index = pet.current_sprite_index();
        snapshot.atlas_width = pet.atlas.image.width();
        snapshot.atlas_height = pet.atlas.image.height();
        snapshot.cell_width = pet.atlas.frame.width;
        snapshot.cell_height = pet.atlas.frame.height;
        snapshot.next_frame_ms = pet
            .next_frame_after()
            .as_millis()
            .clamp(1, u32::MAX as u128) as u32;
        snapshot.pet_path = Some(pet.entry.dir.clone());
        texts.state = pet.engine.current().name().to_string();
        texts.pet_id = pet.entry.id.clone();
        texts.pet_name = pet.entry.display_name.clone();
        texts.atlas_path = pet.atlas.path.to_string_lossy().into_owned();
    } else {
        snapshot.next_frame_ms = IDLE_WORKER_WAIT.as_millis() as u32;
    }
    if let Some(bubble) = &runtime.bubble {
        texts.bubble = bubble.text.clone();
        let remaining = bubble.remaining(Instant::now());
        texts.bubble_timing = format!(
            "{},{},{}",
            remaining.as_millis().min(u64::MAX as u128),
            bubble.total.as_millis().min(u64::MAX as u128),
            bubble.generation
        );
    }
    if !runtime.status.is_empty() {
        texts.error = runtime.status.clone();
        texts.status = runtime.status.clone();
    }
    if let Some(position) = &runtime.config.window.start_position {
        texts.position = format!("{},{}", position.x, position.y);
    }
    if let Some(position) = &runtime.macos_window_position {
        texts.window_position =
            serde_json::to_string(&position).unwrap_or_else(|_| "{}".to_string());
    }
    if let Ok(mut target) = projection.snapshot.write() {
        *target = snapshot;
    }
    if let Ok(mut target) = projection.texts.write() {
        *target = texts;
    }
}

fn set_fault(projection: &Arc<SharedProjection>, message: String) {
    if let Ok(mut snapshot) = projection.snapshot.write() {
        snapshot.ready = true;
        snapshot.faulted = true;
        snapshot.next_frame_ms = IDLE_WORKER_WAIT.as_millis() as u32;
    }
    if let Ok(mut texts) = projection.texts.write() {
        texts.error = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petsona_core::config::{AppConfig, AppPaths, ConversationConfig};
    use std::sync::Once;
    use tempfile::TempDir;

    const TEST_API_KEY_ENV: &str = "PETSONA_RUNTIME_TEST_API_KEY";
    static SET_TEST_API_KEY: Once = Once::new();

    fn engine_home() -> TempDir {
        // Credential-presence checks must resolve from this dummy environment
        // variable, never from the developer's real OS keychain.
        SET_TEST_API_KEY.call_once(|| std::env::set_var(TEST_API_KEY_ENV, "test-only-placeholder"));
        let home = tempfile::tempdir().expect("isolated runtime home");
        let paths = AppPaths::resolve(home.path().to_path_buf());
        paths.ensure().expect("home directories");
        let mut config = AppConfig::default();
        config.state_server.enabled = false;
        config.deepseek.api_key_env = TEST_API_KEY_ENV.to_string();
        config.save(&paths.config_file).expect("isolated config");
        home
    }

    #[test]
    fn retry_appends_a_new_attempt_without_losing_the_partial_reply() {
        let original_reply = ConversationTurn {
            id: "reply-1".into(),
            request_id: Some("request-1".into()),
            user: false,
            text: "先前已经生成的片段".into(),
            status: "failed".into(),
            created_at: 1,
        };
        let mut turns = vec![
            ConversationTurn::user("user-1".into(), "继续回答".into()),
            original_reply,
        ];
        let insertion_index = retry_insertion_index(&turns, 0).unwrap();
        turns.insert(
            insertion_index,
            ConversationTurn::assistant("request-2".into()),
        );

        assert_eq!(insertion_index, 2);
        assert_eq!(turns[1].text, "先前已经生成的片段");
        assert_eq!(turns[2].request_id.as_deref(), Some("request-2"));
    }

    #[test]
    fn retry_is_only_available_for_the_latest_failed_attempt() {
        let turns = vec![
            ConversationTurn::user("user-1".into(), "请回答".into()),
            ConversationTurn {
                id: "reply-1".into(),
                request_id: Some("request-1".into()),
                user: false,
                text: "部分一".into(),
                status: "failed".into(),
                created_at: 1,
            },
            ConversationTurn {
                id: "reply-2".into(),
                request_id: Some("request-2".into()),
                user: false,
                text: "部分二".into(),
                status: "cancelled".into(),
                created_at: 2,
            },
        ];

        assert_eq!(retry_insertion_index(&turns, 0), Some(3));

        let mut completed = turns;
        completed[2].status = "complete".into();
        assert_eq!(retry_insertion_index(&completed, 0), None);
    }

    #[test]
    fn persona_generation_keeps_other_speakers_in_context_only() {
        use petsona_core::persona_source::PersonaChatMessage;

        let messages = [
            ("m-1", "甲", "我喜欢早起"),
            ("m-2", "乙", "为什么？"),
            ("m-3", "甲", "我通常六点起床"),
            ("m-4", "乙", "真早"),
            ("m-5", "甲", "起床后先喝水"),
        ]
        .map(|(id, speaker, text)| PersonaChatMessage {
            id: id.to_string(),
            speaker: speaker.to_string(),
            text: text.to_string(),
            timestamp: None,
        });

        let (target, context) = partition_persona_messages(&messages, "甲");

        assert_eq!(target.len(), 3);
        assert!(target.iter().all(|message| message.speaker == "甲"));
        assert_eq!(context.len(), 2);
        assert!(context.iter().all(|message| message.speaker == "乙"));
    }

    #[test]
    fn macos_display_position_replaces_sidecar_atomically() {
        let home = tempfile::tempdir().expect("isolated home");
        let first = WindowPosition {
            x: 120.0,
            y: 240.0,
            display_id: Some("display-a".to_string()),
            backing_scale: 2.0,
        };
        let second = WindowPosition {
            x: 44.0,
            y: 88.0,
            display_id: Some("display-b".to_string()),
            backing_scale: 1.0,
        };

        save_macos_window_position(home.path(), &first).expect("save initial position");
        save_macos_window_position(home.path(), &second).expect("replace position");

        let saved: WindowPosition = serde_json::from_slice(
            &std::fs::read(home.path().join("macos-window-position.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved, second);
        assert!(!home.path().join("macos-window-position.json.tmp").exists());
    }

    #[test]
    fn importing_persona_contents_preserves_the_stable_identity() {
        let home = tempfile::tempdir().expect("isolated home");
        let store = petsona_core::persona::PersonaStore::new(home.path().join("personas"));
        store.ensure().expect("persona store");
        let incoming = Persona::new("imported-identity", "导入的人格");
        let source = home.path().join("source.json");
        std::fs::write(&source, serde_json::to_vec(&incoming).unwrap()).unwrap();

        let applied = import_persona_into_stable_identity(
            &store,
            &source,
            petsona_core::persona::DEFAULT_PERSONA_ID,
        )
        .expect("apply import");

        assert_eq!(applied.id, petsona_core::persona::DEFAULT_PERSONA_ID);
        assert_eq!(applied.name, "导入的人格");
        assert!(store.get("imported-identity").unwrap().is_none());
        assert_eq!(store.get(&applied.id).unwrap().unwrap(), applied);
    }

    #[test]
    fn stale_key_presence_result_is_ignored_even_for_the_same_provider() {
        assert!(!key_presence_result_is_current(
            "deepseek", 2, "deepseek", 1
        ));
        assert!(key_presence_result_is_current("deepseek", 2, "deepseek", 2));
        assert!(!key_presence_result_is_current("custom", 2, "deepseek", 2));
    }

    /// Same isolated home plus the self-authored V2 fixture pet, used by the
    /// animation-cadence regression test.
    fn engine_home_with_pet() -> TempDir {
        let home = engine_home();
        add_test_pet(&home, "v2-test-pet", "TestPet", "test_fixture_v2");
        home
    }

    fn add_test_pet(home: &TempDir, directory: &str, name: &str, id: &str) {
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../petsona-core/testdata/v2-test-pet");
        let destination = paths.pets_dir.join(directory);
        std::fs::create_dir_all(&destination).expect("fixture pet directory");
        let manifest = std::fs::read_to_string(fixture.join("pet.json")).expect("fixture manifest");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&manifest).expect("fixture manifest JSON");
        manifest["id"] = serde_json::Value::String(id.to_string());
        manifest["displayName"] = serde_json::Value::String(name.to_string());
        std::fs::write(
            destination.join("pet.json"),
            serde_json::to_string_pretty(&manifest).expect("serialize fixture manifest"),
        )
        .expect("write fixture manifest");
        std::fs::copy(
            fixture.join("spritesheet.png"),
            destination.join("spritesheet.png"),
        )
        .expect("fixture spritesheet");
    }

    #[test]
    fn settings_projection_contains_paths_and_editable_config() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..50 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let settings: serde_json::Value =
            serde_json::from_str(&engine.text(RuntimeTextField::Settings)).expect("settings JSON");
        assert!(settings["paths"]["dataDir"].as_str().is_some());
        assert!(settings["paths"]["logsDir"].as_str().is_some());
        assert_eq!(settings["window"]["scale"], 1.0);
        assert_eq!(settings["greeting"]["enabled"], true);
        engine.stop();
    }

    #[test]
    fn spawn_does_not_load_the_home_on_the_caller_thread() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..50 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let snapshot = engine.snapshot();
        assert!(snapshot.ready);
        assert!(
            !snapshot.faulted,
            "{}",
            engine.text(RuntimeTextField::Error)
        );
        engine.stop();
    }

    #[test]
    fn empty_home_has_a_slow_idle_deadline_and_stops_cleanly() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..50 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let snapshot = engine.snapshot();
        assert!(!snapshot.has_pet);
        assert!(snapshot.next_frame_ms >= 500);
        engine.stop();
        assert!(engine.send(RuntimeCommand::Tick).is_err());
    }

    #[test]
    fn native_settings_commands_round_trip_through_projection() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..50 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        engine
            .send(RuntimeCommand::SetScale(1.1))
            .expect("scale command");
        engine
            .send(RuntimeCommand::SetWindowPosition(WindowPosition {
                x: 240.0,
                y: 360.0,
                display_id: Some("retina-test".to_string()),
                backing_scale: 2.0,
            }))
            .expect("display-relative position command");
        engine
            .send(RuntimeCommand::UpdateDeepSeekConfig(DeepSeekConfig {
                model: "test-model".to_string(),
                api_key_env: TEST_API_KEY_ENV.to_string(),
                ..DeepSeekConfig::default()
            }))
            .expect("DeepSeek command");
        engine
            .send(RuntimeCommand::UpdateConversationConfig(
                ConversationConfig {
                    save_history: false,
                },
            ))
            .expect("conversation history config");
        engine
            .send(RuntimeCommand::CreatePersona(PersonaCreate {
                id: "tester".to_string(),
                name: "测试人格".to_string(),
                template: None,
            }))
            .expect("persona command");
        engine
            .send(RuntimeCommand::RememberFact(MemoryFactInput {
                key: "喜欢".to_string(),
                value: "安静音乐".to_string(),
                confidence: Some(0.9),
            }))
            .expect("memory command");

        for _ in 0..100 {
            let _ = engine.send(RuntimeCommand::Tick);
            if engine.text(RuntimeTextField::PersonaId) == "tester"
                && engine
                    .text(RuntimeTextField::DeepSeekConfig)
                    .contains("test-model")
                && engine.text(RuntimeTextField::Memory).contains("安静音乐")
                && engine
                    .text(RuntimeTextField::WindowPosition)
                    .contains("retina-test")
                && engine
                    .text(RuntimeTextField::Conversation)
                    .contains("\"saveHistory\":false")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(engine.snapshot().scale, 1.0);
        let position: WindowPosition =
            serde_json::from_str(&engine.text(RuntimeTextField::WindowPosition))
                .expect("display-relative position projection");
        assert_eq!(position.x, 240.0);
        assert_eq!(position.y, 360.0);
        assert_eq!(position.display_id.as_deref(), Some("retina-test"));
        assert_eq!(position.backing_scale, 2.0);
        assert!(engine
            .text(RuntimeTextField::Conversation)
            .contains("\"saveHistory\":false"));
        assert_eq!(engine.text(RuntimeTextField::PersonaId), "tester");
        assert!(engine.text(RuntimeTextField::Personas).contains("测试人格"));
        assert!(engine
            .text(RuntimeTextField::DeepSeekConfig)
            .contains("test-model"));
        assert!(engine
            .text(RuntimeTextField::DeepSeekConfig)
            .contains("\"keyConfigured\":true"));
        assert!(engine.text(RuntimeTextField::Memory).contains("安静音乐"));
        engine.stop();
    }

    #[test]
    fn conversation_start_without_a_pet_reports_the_real_precondition() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..50 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        engine
            .send(RuntimeCommand::StartConversation(ConversationRequest {
                request_id: "no-pet".to_string(),
                pet_id: String::new(),
                text: "你好".to_string(),
                retry_turn_id: None,
            }))
            .unwrap();
        for _ in 0..30 {
            let _ = engine.send(RuntimeCommand::Tick);
            if engine
                .text(RuntimeTextField::Conversation)
                .contains("请先导入宠物")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(engine
            .text(RuntimeTextField::Conversation)
            .contains("请先导入宠物"));
        assert!(!engine.snapshot().conversation_inflight);
        engine.stop();
    }

    #[test]
    fn repeated_drag_state_does_not_rewind_the_animation() {
        use petsona_core::pet::PetState;

        let home = engine_home_with_pet();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        let mut ready = false;
        for _ in 0..200 {
            let _ = engine.send(RuntimeCommand::Tick);
            let snapshot = engine.snapshot();
            if snapshot.ready && snapshot.has_pet {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready, "fixture pet did not load");

        let state = PetState::RunningRight;
        engine
            .send(RuntimeCommand::SetState {
                state,
                ttl: Some(Duration::from_secs(2)),
            })
            .expect("first drag state");

        let mut first_frame = None;
        for _ in 0..200 {
            let _ = engine.send(RuntimeCommand::Tick);
            if engine.text(RuntimeTextField::State) == state.name() {
                let sprite = engine.snapshot().sprite_index;
                first_frame = Some(sprite);
                if sprite != 0 {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let first_frame = first_frame.expect("running-right became visible");

        // 150 ms is past the fixture's first 120 ms running frame. A second
        // SetState with the same state simulates the 80 ms drag resend; it must
        // keep the clock rather than returning to the first frame.
        std::thread::sleep(Duration::from_millis(150));
        engine
            .send(RuntimeCommand::SetState {
                state,
                ttl: Some(Duration::from_secs(2)),
            })
            .expect("drag resend");
        std::thread::sleep(Duration::from_millis(60));

        assert_ne!(
            engine.snapshot().sprite_index,
            first_frame,
            "a repeated drag state must not rewind the running animation"
        );
        engine.stop();
    }

    fn post_state(port: u16, body: &str) -> String {
        use std::io::{Read as _, Write as _};

        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let request = format!(
            "POST /state HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn wait_for_text(engine: &RuntimeEngine, field: RuntimeTextField, expected: &str) -> bool {
        for _ in 0..300 {
            if engine.text(field) == expected {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn protocol_clear_retracts_a_sticky_source_override() {
        use petsona_core::pet::PetState;

        let home = engine_home_with_pet();
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("isolated config");
        config.state_server.enabled = true;
        config.state_server.port = 0; // ephemeral; the snapshot reports the bound port
        config
            .save(&paths.config_file)
            .expect("config with protocol");

        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        let mut port = 0u16;
        for _ in 0..300 {
            let snapshot = engine.snapshot();
            if snapshot.ready && snapshot.has_pet && snapshot.state_server_port != 0 {
                port = snapshot.state_server_port;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_ne!(port, 0, "the state protocol did not start");

        let base = engine.text(RuntimeTextField::State);
        assert_ne!(base, "running");

        let raised = post_state(
            port,
            r#"{"source":"win-verify","state":"running","ttlMs":0}"#,
        );
        assert!(raised.starts_with("HTTP/1.1 202"), "{raised}");
        assert!(
            wait_for_text(&engine, RuntimeTextField::State, "running"),
            "hook state did not raise, now {}",
            engine.text(RuntimeTextField::State)
        );

        // A click cannot retract a higher-priority hook override.
        engine
            .send(RuntimeCommand::SetState {
                state: PetState::Waving,
                ttl: None,
            })
            .expect("native click state");
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            engine.text(RuntimeTextField::State),
            "running",
            "native input must not steal a higher-priority hook state"
        );

        let cleared = post_state(port, r#"{"source":"win-verify","action":"clear"}"#);
        assert!(cleared.starts_with("HTTP/1.1 202"), "{cleared}");
        assert!(
            wait_for_text(&engine, RuntimeTextField::State, &base),
            "clear did not restore {base}, now {}",
            engine.text(RuntimeTextField::State)
        );
        engine.stop();
    }

    /// REQ-P01: upgrading an existing install binds the persona that was active
    /// to the pet that was active, and does it once.
    #[test]
    fn first_load_binds_the_active_persona_to_the_active_pet() {
        let home = engine_home_with_pet();
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("config");
        config.active_pet = Some("test_fixture_v2".to_string());
        config.active_persona = Some("default".to_string());
        config.save(&paths.config_file).expect("config with a pet");

        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..200 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        engine.stop();

        let saved = AppConfig::load(&paths.config_file).expect("saved config");
        assert_eq!(
            saved.persona_by_pet.get("test_fixture_v2"),
            Some(&"default".to_string()),
            "the upgrade must bind the active persona to the active pet"
        );
    }

    #[test]
    fn copying_a_persona_to_another_pet_creates_an_independent_binding() {
        let home = engine_home();
        add_test_pet(&home, "first-pet", "FirstPet", "first");
        add_test_pet(&home, "second-pet", "SecondPet", "second");
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("config");
        config.active_pet = Some("first".to_string());
        config.active_persona = Some("default".to_string());
        config
            .save(&paths.config_file)
            .expect("config with two pets");

        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..300 {
            if engine.snapshot().ready && engine.text(RuntimeTextField::PetId) == "first" {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let source_persona = engine.text(RuntimeTextField::PersonaId);

        engine
            .send(RuntimeCommand::CopyPersonaToPet("second".to_string()))
            .expect("copy persona");
        for _ in 0..300 {
            if engine.text(RuntimeTextField::Status).contains("复制到") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        engine
            .send(RuntimeCommand::SelectPet("second".to_string()))
            .expect("select second pet");
        for _ in 0..300 {
            if engine.text(RuntimeTextField::PetId) == "second" {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let copied_persona = engine.text(RuntimeTextField::PersonaId);
        assert_ne!(source_persona, copied_persona);
        assert!(copied_persona.ends_with("--second"));

        engine
            .send(RuntimeCommand::SelectPet("first".to_string()))
            .expect("select first pet again");
        for _ in 0..300 {
            if engine.text(RuntimeTextField::PetId) == "first" {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(engine.text(RuntimeTextField::PersonaId), source_persona);
        engine.stop();
    }

    /// REQ-P02: choosing a speaking style binds it to the current pet, and a
    /// restart restores that binding instead of the last-used persona.
    #[test]
    fn speaking_style_is_bound_to_the_pet_and_restored_after_restart() {
        let home = engine_home_with_pet();
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("config");
        config.active_pet = Some("test_fixture_v2".to_string());
        config.save(&paths.config_file).expect("config with a pet");

        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..200 {
            if engine.snapshot().ready && engine.snapshot().has_pet {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        engine
            .send(RuntimeCommand::CreatePersona(PersonaCreate {
                id: "stylish".to_string(),
                name: "有型".to_string(),
                template: None,
            }))
            .expect("create persona");
        engine
            .send(RuntimeCommand::SelectPersona("stylish".to_string()))
            .expect("select persona");
        for _ in 0..200 {
            if engine.text(RuntimeTextField::PersonaId) == "stylish" {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        engine.stop();

        let saved = AppConfig::load(&paths.config_file).expect("saved config");
        assert_eq!(
            saved.persona_by_pet.get("test_fixture_v2"),
            Some(&"stylish".to_string()),
            "the chosen speaking style must be bound to the current pet"
        );

        // Restart: the pet's binding wins over any stale activePersona.
        let mut config = AppConfig::load(&paths.config_file).expect("config");
        config.active_persona = Some("default".to_string());
        config
            .save(&paths.config_file)
            .expect("stale active persona");
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        let mut restored = false;
        for _ in 0..200 {
            if engine.text(RuntimeTextField::PersonaId) == "stylish" {
                restored = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(restored, "the pet binding must be used after a restart");
        engine.stop();
    }

    #[test]
    fn memory_edit_scoped_clear_and_export_round_trip() {
        let home = engine_home();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..200 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        engine
            .send(RuntimeCommand::RememberFact(MemoryFactInput {
                key: "咖啡".to_string(),
                value: "美式".to_string(),
                confidence: None,
            }))
            .expect("remember fact");
        assert!(
            wait_for_text_contains(&engine, RuntimeTextField::Memory, "美式"),
            "fact did not appear: {}",
            engine.text(RuntimeTextField::Memory)
        );

        let memory: serde_json::Value =
            serde_json::from_str(&engine.text(RuntimeTextField::Memory)).unwrap();
        let fact_id = memory["facts"][0]["id"].as_str().unwrap().to_string();

        engine
            .send(RuntimeCommand::UpdateFact(MemoryFactUpdate {
                id: fact_id,
                key: "咖啡".to_string(),
                value: "拿铁".to_string(),
                confidence: None,
            }))
            .expect("update fact");
        assert!(
            wait_for_text_contains(&engine, RuntimeTextField::Memory, "拿铁"),
            "edited fact did not appear: {}",
            engine.text(RuntimeTextField::Memory)
        );

        let export = home.path().join("memory-export.json");
        engine
            .send(RuntimeCommand::ExportMemory(export.clone()))
            .expect("export memory");
        let mut exported = false;
        for _ in 0..200 {
            if export.exists() {
                exported = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(exported, "export file was not written");

        engine
            .send(RuntimeCommand::ClearMemoryScope(MemoryScope::Facts))
            .expect("clear facts");
        let mut cleared = false;
        for _ in 0..200 {
            let memory: serde_json::Value =
                serde_json::from_str(&engine.text(RuntimeTextField::Memory)).unwrap();
            if memory["facts"]
                .as_array()
                .is_some_and(|facts| facts.is_empty())
            {
                cleared = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(cleared, "scoped clear left facts behind");

        engine
            .send(RuntimeCommand::ImportMemory(export))
            .expect("import memory");
        assert!(
            wait_for_text_contains(&engine, RuntimeTextField::Memory, "拿铁"),
            "imported fact did not come back: {}",
            engine.text(RuntimeTextField::Memory)
        );
        engine.stop();
    }

    /// REQ-S16b: `ListModels` goes out over HTTP and lands in the projection.
    /// A stub listener stands in for the provider so the test never touches the
    /// network (and proves the request goes to `{base}/models`).
    #[test]
    fn list_models_publishes_the_provider_catalog() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            // Generous deadline: the worker may still be warming up on a cold,
            // loaded machine (this used to flake at 5 s).
            let deadline = Instant::now() + Duration::from_secs(30);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0u8; 1024];
                        let _ = stream.read(&mut buffer);
                        let body = r#"{"object":"list","data":[{"id":"stub-model-b"},{"id":"stub-model-a"}]}"#;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        return;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        });

        let home = engine_home();
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("isolated config");
        config.deepseek.provider = "custom".to_string();
        config.deepseek.base_url = format!("http://127.0.0.1:{port}/v1");
        config.deepseek.api_key_env = "PETSONA_TEST_MODELS_KEY".to_string();
        config
            .save(&paths.config_file)
            .expect("config with stub provider");
        std::env::set_var("PETSONA_TEST_MODELS_KEY", "sk-test");

        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        for _ in 0..200 {
            if engine.snapshot().ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        engine
            .send(RuntimeCommand::ListModels)
            .expect("list models");
        assert!(
            wait_for_text_contains(&engine, RuntimeTextField::Models, "stub-model-a"),
            "model list did not arrive: {}",
            engine.text(RuntimeTextField::Models)
        );
        assert!(engine
            .text(RuntimeTextField::Models)
            .contains("stub-model-b"));
        engine.stop();
        let _ = server.join();
    }

    fn wait_for_text_contains(
        engine: &RuntimeEngine,
        field: RuntimeTextField,
        needle: &str,
    ) -> bool {
        for _ in 0..300 {
            if engine.text(field).contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn greeting_due_respects_the_switch_idle_window_and_cooldown() {
        use petsona_core::config::GreetingConfig;

        let now = Instant::now();
        let mut greeting = GreetingConfig {
            enabled: true,
            idle_minutes: 30,
            cooldown_minutes: 120,
            max_chars: 40,
        };
        let quiet = now - Duration::from_secs(31 * 60);

        // Just used the app: nothing to greet.
        assert!(!greeting_due(&greeting, now, None, now));
        // Quiet past the threshold and never greeted: due.
        assert!(greeting_due(&greeting, quiet, None, now));
        // Greeted 10 minutes ago: the cooldown suppresses it.
        assert!(!greeting_due(
            &greeting,
            quiet,
            Some(now - Duration::from_secs(10 * 60)),
            now
        ));
        // Last greeting three hours ago: due again.
        assert!(greeting_due(
            &greeting,
            quiet,
            Some(now - Duration::from_secs(3 * 60 * 60)),
            now
        ));
        // Disabled: never due.
        greeting.enabled = false;
        assert!(!greeting_due(&greeting, quiet, None, now));
    }

    #[test]
    fn greeting_result_shows_a_bubble_and_records_memory() {
        let home = engine_home_with_pet();
        let mut engine = RuntimeEngine::spawn(Some(home.path().to_path_buf()), || {}).unwrap();
        let mut ready = false;
        for _ in 0..300 {
            let snapshot = engine.snapshot();
            if snapshot.ready && snapshot.has_pet {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready, "fixture pet did not load");

        engine
            .send(RuntimeCommand::GreetingResult(Ok("早上好呀。".to_string())))
            .expect("greeting result");
        assert!(
            wait_for_text(&engine, RuntimeTextField::Bubble, "早上好呀。"),
            "greeting must show a bubble, now {:?}",
            engine.text(RuntimeTextField::Bubble)
        );
        assert!(
            engine.text(RuntimeTextField::Memory).contains("早上好呀。"),
            "greeting must be recorded in memory: {}",
            engine.text(RuntimeTextField::Memory)
        );

        // No key / network error falls back to the persona line instead of
        // swallowing the greeting.
        engine
            .send(RuntimeCommand::GreetingResult(Err(
                "no api key configured".to_string()
            )))
            .expect("failed greeting result");
        let expected =
            crate::greeting::fallback_greeting(&petsona_core::persona::Persona::default());
        assert!(
            wait_for_text(&engine, RuntimeTextField::Bubble, &expected),
            "fallback greeting must surface, now {:?}",
            engine.text(RuntimeTextField::Bubble)
        );
        engine.stop();
    }

    #[test]
    fn missing_key_projects_unconfigured_and_uses_the_local_greeting() {
        let home = engine_home_with_pet();
        let mut engine = RuntimeEngine::spawn_with_key_presence(
            Some(home.path().to_path_buf()),
            || {},
            |_| false,
        )
        .unwrap();
        let mut ready = false;
        for _ in 0..300 {
            if engine.snapshot().ready && engine.snapshot().has_pet {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready, "no-key runtime fixture should become ready");
        assert!(engine
            .text(RuntimeTextField::DeepSeekConfig)
            .contains("\"keyConfigured\":false"));
        engine.stop();

        let paths = AppPaths::resolve(home.path().to_path_buf());
        let mut config = AppConfig::load(&paths.config_file).expect("isolated config");
        config.greeting.enabled = true;
        config.greeting.idle_minutes = 1;
        config.greeting.cooldown_minutes = 0;
        config
            .save(&paths.config_file)
            .expect("config with idle greeting");

        let mut runtime = PetsonaRuntime::load(paths, config).expect("runtime with fixture pet");
        runtime.key_configured = false;
        runtime.last_user_action = Instant::now() - Duration::from_secs(120);
        runtime.persona.greeting = Some("本地固定问候".to_string());

        let (command_tx, command_rx) = mpsc::channel();
        maybe_request_greeting(&mut runtime, &command_tx);
        let result = command_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("missing-key path should enqueue a local fallback immediately");
        assert!(matches!(
            &result,
            RuntimeCommand::GreetingResult(Err(message)) if message.contains("未配置模型密钥")
        ));
        let mut key_presence_request_id = 0;
        assert!(apply_command(
            result,
            &mut runtime,
            &command_tx,
            &|_| false,
            &mut key_presence_request_id,
        ));
        assert_eq!(
            runtime.bubble.as_ref().map(|bubble| bubble.text.as_str()),
            Some("本地固定问候")
        );
        assert!(!runtime.greeting_inflight);

        let projection = Arc::new(SharedProjection {
            snapshot: RwLock::new(RuntimeSnapshot::default()),
            texts: RwLock::new(RuntimeTexts::default()),
        });
        publish_projection(&runtime, &projection, 1);
        assert!(projection
            .texts
            .read()
            .unwrap()
            .deepseek_config
            .contains("\"keyConfigured\":false"));
    }
}
