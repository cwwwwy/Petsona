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
use petsona_core::config::{AppConfig, AppPaths, DeepSeekConfig, MemoryConfig, WindowPosition};
use petsona_core::deepseek::DeepSeekClient;
use petsona_core::memory::{extract_preference, EventKind};
use petsona_core::persona::{templates, ModelRef, Persona};
use petsona_core::pet::PetLibrary;

use crate::commands::{
    ImportConflict, MemoryFactInput, PersonaCreate, PersonaPatch, RuntimeCommand,
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
    tick_runtime(&mut runtime, &projection, &mut revision);

    loop {
        let wait = next_wait(&runtime);
        match command_rx.recv_timeout(wait) {
            Ok(RuntimeCommand::Stop) => break,
            Ok(command) => {
                if apply_command(command, &mut runtime, &command_tx) {
                    tick_runtime(&mut runtime, &projection, &mut revision);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                tick_runtime(&mut runtime, &projection, &mut revision);
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
                        let _ = runtime.memory.record_event(
                            &runtime.persona.id,
                            EventKind::PetChanged,
                            Some(id),
                        );
                        if let Err(error) = runtime.save_config() {
                            runtime.status = format!("保存宠物选择失败：{error}");
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
        RuntimeCommand::SelectPersona(id) => {
            match runtime.personas.get(&id) {
                Ok(Some(persona)) => {
                    runtime.persona = persona;
                    runtime.config.active_persona = Some(id.clone());
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
            match runtime.save_config() {
                Ok(()) => runtime.status = "DeepSeek 配置已保存".to_string(),
                Err(error) => runtime.status = format!("保存 DeepSeek 配置失败：{error}"),
            }
            true
        }
        RuntimeCommand::UpdateMemoryConfig(config) => {
            runtime.config.memory = sanitize_memory_config(config);
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
            match runtime.memory.remember_fact(
                &runtime.persona.id,
                key.trim(),
                value.trim(),
                confidence.unwrap_or(0.8).clamp(0.0, 1.0),
            ) {
                Ok(_) => runtime.status = "已保存一条用户偏好".to_string(),
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
        RuntimeCommand::ClearMemory => {
            match runtime.memory.clear_persona(&runtime.persona.id) {
                Ok(()) => runtime.status = "已清空当前人格的记忆".to_string(),
                Err(error) => runtime.status = format!("清空记忆失败：{error}"),
            }
            true
        }
        RuntimeCommand::SaveDeepSeekKey(key) => {
            match petsona_core::deepseek::save_api_key(&key) {
                Ok(()) => runtime.status = "DeepSeek 密钥已保存到系统凭据库".to_string(),
                Err(error) => runtime.status = format!("保存密钥失败：{error}"),
            }
            true
        }
        RuntimeCommand::SendConversation(text) => {
            if runtime.conversation_inflight || text.trim().is_empty() {
                return true;
            }
            runtime.conversation_inflight = true;
            let _ = runtime.memory.record_event(
                &runtime.persona.id,
                EventKind::UserMessage,
                Some(text.clone()),
            );
            if runtime.config.memory.enabled && runtime.persona.memory.enabled {
                if let Some((key, value, confidence)) = extract_preference(&text) {
                    let _ =
                        runtime
                            .memory
                            .remember_fact(&runtime.persona.id, &key, &value, confidence);
                    runtime.status = "已从对话记录一条用户偏好".to_string();
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
    if let Some(value) = patch.description {
        persona.description = empty_to_none(value);
    }
    if let Some(value) = patch.avatar_pet {
        persona.avatar_pet = empty_to_none(value);
    }
    if let Some(value) = patch.name {
        persona.name = value;
    }
    if let Some(value) = patch.tone {
        persona.traits.tone = value;
    }
    if let Some(value) = patch.verbosity {
        persona.traits.verbosity = value;
    }
    if let Some(value) = patch.language {
        persona.traits.language = value;
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
    if let Some(value) = patch.temperature {
        persona.sampling.temperature = value.clamp(0.0, 2.0);
    }
    if let Some(value) = patch.max_tokens {
        persona.sampling.max_tokens = value.clamp(16, 4000);
    }
    if let Some(provider) = patch.model_provider {
        let provider = provider.trim().to_string();
        let model = patch.model.and_then(empty_to_none);
        persona.model = if provider.is_empty() {
            None
        } else {
            Some(ModelRef { provider, model })
        };
    } else if let Some(model) = patch.model {
        if let Some(binding) = &mut persona.model {
            binding.model = empty_to_none(model);
        }
    }
    if let Some(value) = patch.memory_enabled {
        persona.memory.enabled = value;
    }
    if let Some(value) = patch.memory_window_turns {
        persona.memory.window_turns = value.clamp(1, 100);
    }
    if let Some(value) = patch.memory_long_term {
        persona.memory.long_term = value;
    }
    if let Some(value) = patch.memory_summarize_after_turns {
        persona.memory.summarize_after_turns = value.clamp(1, 200);
    }
    if let Some(value) = patch.tts_enabled {
        persona.tts.enabled = value;
    }
    if let Some(value) = patch.tts_voice {
        persona.tts.voice = empty_to_none(value);
    }
    if let Some(value) = patch.tts_rate {
        persona.tts.rate = value.clamp(0.25, 4.0);
    }
    if let Some(value) = patch.proactive_enabled {
        persona.proactive.enabled = value;
    }
    if let Some(value) = patch.proactive_idle_minutes {
        persona.proactive.idle_minutes = value.clamp(1, 24 * 60);
    }
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

fn sanitize_memory_config(mut config: MemoryConfig) -> MemoryConfig {
    config.recent_events = config.recent_events.clamp(1, 100);
    config.retention_days = config.retention_days.clamp(1, 3650);
    config.fact_limit = config.fact_limit.clamp(1, 50);
    config
}

fn tick_runtime(
    runtime: &mut PetsonaRuntime,
    projection: &Arc<SharedProjection>,
    revision: &mut u64,
) {
    runtime.poll_state_events();
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
        .is_some_and(|bubble| bubble.until <= now)
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
                    "description": persona.description,
                    "builtin": persona.builtin,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let persona = serde_json::to_string(&runtime.persona).unwrap_or_else(|_| "{}".to_string());
    let deepseek_config =
        serde_json::to_string(&runtime.config.deepseek).unwrap_or_else(|_| "{}".to_string());
    let memory_snapshot = runtime.memory.persona_snapshot(&runtime.persona.id);
    let memory = serde_json::to_string(&serde_json::json!({
        "config": runtime.config.memory,
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
}
