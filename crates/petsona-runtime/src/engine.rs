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
use petsona_core::memory::{extract_preference, EventKind};
use petsona_core::persona::{templates, Persona};
use petsona_core::pet::PetLibrary;
use petsona_core::pet::PetState;

use crate::commands::{
    ImportConflict, MemoryFactInput, MemoryFactUpdate, MemoryScope, PersonaCreate, PersonaPatch,
    RuntimeCommand,
};
use crate::events::RuntimeWaker;
use crate::instance_lock::InstanceLock;
use crate::logging;
use crate::session::PetsonaRuntime;
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

fn run_worker(
    home: Option<PathBuf>,
    command_rx: Receiver<RuntimeCommand>,
    command_tx: Sender<RuntimeCommand>,
    projection: Arc<SharedProjection>,
    external_wake: Arc<dyn Fn() + Send + Sync>,
) {
    let result = load_runtime(home, &command_tx, &external_wake);
    let (mut runtime, _instance_lock) = match result {
        Ok(value) => value,
        Err(error) => {
            set_fault(&projection, format!("{error:#}"));
            return;
        }
    };

    let mut revision = 1u64;
    tick_runtime(&mut runtime, &command_tx, &projection, &mut revision);

    loop {
        let wait = next_wait(&runtime);
        match command_rx.recv_timeout(wait) {
            Ok(RuntimeCommand::Stop) => break,
            Ok(command) => {
                if apply_command(command, &mut runtime, &command_tx) {
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
    runtime.key_configured = petsona_core::deepseek::api_key_present(&runtime.config.deepseek);
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

fn apply_command(
    command: RuntimeCommand,
    runtime: &mut PetsonaRuntime,
    command_tx: &Sender<RuntimeCommand>,
) -> bool {
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
            runtime.config.window.start_position = Some(WindowPosition { x, y });
            if let Err(error) = runtime.save_config() {
                runtime.status = format!("保存位置失败：{error}");
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
                    runtime.config.active_pet = Some(entry.id.clone());
                    runtime.config.first_run = false;
                    match runtime.save_config() {
                        Ok(()) => match crate::pet::PetSession::load(entry.clone()) {
                            Ok(pet) => {
                                runtime.pet = Some(pet);
                                runtime.selected_pet = Some(entry.id.clone());
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
            match runtime.pets.iter().find(|pet| pet.id == id).cloned() {
                Some(entry) => match crate::pet::PetSession::load(entry.clone()) {
                    Ok(pet) => {
                        runtime.pet = Some(pet);
                        runtime.selected_pet = Some(id.clone());
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
                        } else {
                            runtime.config.active_pet = None;
                            runtime.pet = None;
                            runtime.selected_pet = None;
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
        RuntimeCommand::ResetPersona => {
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
        RuntimeCommand::ExportPersona { id, path } => {
            match runtime.personas.export_file(&id, &path) {
                Ok(()) => runtime.status = format!("已导出人格：{}", path.display()),
                Err(error) => runtime.status = format!("导出人格失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateDeepSeekConfig(config) => {
            runtime.config.deepseek = sanitize_deepseek_config(config);
            runtime.key_configured =
                petsona_core::deepseek::api_key_present(&runtime.config.deepseek);
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
            runtime.config.memory = sanitize_memory_config(config);
            let retention = runtime.config.memory.event_retention_days;
            let _ = runtime.memory.prune_events(&runtime.persona.id, retention);
            compress_facts_if_enabled(runtime);
            match runtime.save_config() {
                Ok(()) => runtime.status = "记忆设置已保存".to_string(),
                Err(error) => runtime.status = format!("保存记忆设置失败：{error}"),
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
            runtime.status = match result {
                Ok(message) => message,
                Err(error) => format!("清空记忆失败：{error}"),
            };
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
            match runtime.memory.clear_persona(&runtime.persona.id) {
                Ok(()) => runtime.status = "已清空当前人格的记忆".to_string(),
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
            runtime.key_configured =
                petsona_core::deepseek::api_key_present(&runtime.config.deepseek);
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
        RuntimeCommand::SendConversation(text) => {
            if runtime.conversation_inflight || text.trim().is_empty() {
                return true;
            }
            runtime.conversation_inflight = true;
            runtime.last_user_action = Instant::now();
            let _ = runtime.memory.record_event(
                &runtime.persona.id,
                EventKind::UserMessage,
                Some(text.clone()),
            );
            if runtime.config.memory.enabled {
                if let Some((key, value, confidence)) = extract_preference(&text) {
                    let _ = runtime.memory.remember_fact_from(
                        &runtime.persona.id,
                        &key,
                        &value,
                        confidence,
                        petsona_core::memory::FACT_SOURCE_CONVERSATION,
                    );
                    runtime.status = "已从对话记录一条用户偏好".to_string();
                    compress_facts_if_enabled(runtime);
                }
            }
            runtime
                .conversation_history
                .push(crate::session::ConversationTurn {
                    user: true,
                    text: text.clone(),
                });
            let context = runtime.memory.build_greeting_context(
                &runtime.persona.id,
                runtime.config.memory.recent_events,
                runtime.config.memory.fact_limit,
            );
            let history = runtime
                .conversation_history
                .iter()
                .rev()
                .take(6)
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
                let _ = runtime.memory.record_event(
                    &runtime.persona.id,
                    EventKind::PetGreeting,
                    Some(text.clone()),
                );
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
            runtime
                .conversation_history
                .push(crate::session::ConversationTurn {
                    user: false,
                    text: reply.clone(),
                });
            let _ = runtime.memory.record_event(
                &runtime.persona.id,
                EventKind::PetReaction,
                Some(reply.clone()),
            );
            runtime.show_bubble(reply, Duration::from_secs(8));
            true
        }
        RuntimeCommand::Stop => false,
    }
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

    let context = runtime.memory.build_greeting_context(
        &runtime.persona.id,
        runtime.config.memory.recent_events,
        runtime.config.memory.fact_limit,
    );
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
        conversation_inflight: runtime.conversation_inflight,
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
    let mut texts = RuntimeTexts {
        pets,
        codex_pets,
        persona,
        personas,
        deepseek_config,
        memory,
        import_conflict,
        models,
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
    if let Some(position) = runtime.config.window.start_position {
        texts.position = format!("{},{}", position.x, position.y);
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
    use petsona_core::config::{AppConfig, AppPaths};
    use tempfile::TempDir;

    fn engine_home() -> TempDir {
        let home = tempfile::tempdir().expect("isolated runtime home");
        let paths = AppPaths::resolve(home.path().to_path_buf());
        paths.ensure().expect("home directories");
        let mut config = AppConfig::default();
        config.state_server.enabled = false;
        config.save(&paths.config_file).expect("isolated config");
        home
    }

    /// Same isolated home plus the self-authored V2 fixture pet, used by the
    /// animation-cadence regression test.
    fn engine_home_with_pet() -> TempDir {
        let home = engine_home();
        let paths = AppPaths::resolve(home.path().to_path_buf());
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../petsona-core/testdata/v2-test-pet");
        let destination = paths.pets_dir.join("v2-test-pet");
        std::fs::create_dir_all(&destination).expect("fixture pet directory");
        std::fs::copy(fixture.join("pet.json"), destination.join("pet.json"))
            .expect("fixture manifest");
        std::fs::copy(
            fixture.join("spritesheet.png"),
            destination.join("spritesheet.png"),
        )
        .expect("fixture spritesheet");
        home
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
            .send(RuntimeCommand::UpdateDeepSeekConfig(DeepSeekConfig {
                model: "test-model".to_string(),
                ..DeepSeekConfig::default()
            }))
            .expect("DeepSeek command");
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
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(engine.snapshot().scale, 1.0);
        assert_eq!(engine.text(RuntimeTextField::PersonaId), "tester");
        assert!(engine.text(RuntimeTextField::Personas).contains("测试人格"));
        assert!(engine
            .text(RuntimeTextField::DeepSeekConfig)
            .contains("test-model"));
        assert!(engine.text(RuntimeTextField::Memory).contains("安静音乐"));
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
}
