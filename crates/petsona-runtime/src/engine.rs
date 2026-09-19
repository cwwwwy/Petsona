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
use petsona_core::config::{AppConfig, AppPaths, WindowPosition};
use petsona_core::deepseek::DeepSeekClient;
use petsona_core::memory::EventKind;
use petsona_core::pet::PetLibrary;

use crate::commands::RuntimeCommand;
use crate::events::RuntimeWaker;
use crate::instance_lock::InstanceLock;
use crate::logging;
use crate::session::PetsonaRuntime;
use crate::snapshot::{RuntimeSnapshot, RuntimeTextField, RuntimeTexts};

const INITIAL_SNAPSHOT_DELAY: Duration = Duration::from_secs(1);
const IDLE_WORKER_WAIT: Duration = Duration::from_secs(1);

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
            runtime.config.window.scale = scale.clamp(0.5, 2.0);
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
                pet.engine.raise(state, "native", None, ttl, now);
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
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
            if let Some(value) = patch.name {
                runtime.persona.name = value;
            }
            if let Some(value) = patch.tone {
                runtime.persona.traits.tone = value;
            }
            if let Some(value) = patch.language {
                runtime.persona.traits.language = value;
            }
            if let Some(value) = patch.greeting {
                runtime.persona.greeting = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
            }
            if let Some(value) = patch.system_prompt {
                runtime.persona.system_prompt = value;
            }
            true
        }
        RuntimeCommand::SavePersona => {
            match runtime.personas.save(&runtime.persona) {
                Ok(()) => runtime.status = "人格已保存".to_string(),
                Err(error) => runtime.status = format!("保存人格失败：{error}"),
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
    let mut texts = RuntimeTexts {
        pets,
        codex_pets,
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
}
