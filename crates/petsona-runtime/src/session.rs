use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use petsona_core::config::{AppConfig, AppPaths};
use petsona_core::memory::{EventKind, PetMemory};
use petsona_core::persona::{Persona, PersonaStore};
use petsona_core::pet::{default_pet, PetEntry, PetLibrary};
use petsona_core::state_server::{Health, StateEvent, StateServer};

use crate::pet::PetSession;

/// Shared text bubble state. The platform shell decides how to render it.
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub user: bool,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Bubble {
    pub text: String,
    pub until: Instant,
}

/// Platform-neutral runtime state shared by all UI shells.
pub struct PetsonaRuntime {
    pub paths: AppPaths,
    pub config: AppConfig,
    pub personas: PersonaStore,
    pub persona: Persona,
    pub memory: PetMemory,
    pub library: PetLibrary,
    pub pets: Vec<PetEntry>,
    pub selected_pet: Option<String>,
    pub pet: Option<PetSession>,
    pub state_server: Option<StateServer>,
    pub state_events: Option<Receiver<StateEvent>>,
    pub state_server_port: u16,
    pub last_health_at: Instant,
    pub bubble: Option<Bubble>,
    pub pet_visible: bool,
    pub status: String,
    pub walk_direction: f32,
    pub next_walk_at: Instant,
    pub walk_until: Option<Instant>,
    pub walk_origin_x: Option<f32>,
    pub walk_position_x: Option<f32>,
    pub last_user_action: Instant,
    pub last_walk_tick: Instant,
    pub last_click_at: Option<Instant>,
    pub pending_single_click: bool,
    pub glance_side: i8,
    pub last_glance_at: Option<Instant>,
    pub greeting_rx: Option<Receiver<Result<String, String>>>,
    pub greeting_inflight: bool,
    pub last_greeting_at: Option<Instant>,
    pub conversation_history: Vec<ConversationTurn>,
    pub conversation_rx: Option<Receiver<Result<String, String>>>,
    pub conversation_inflight: bool,
}

impl PetsonaRuntime {
    pub fn load(paths: AppPaths, mut config: AppConfig) -> Result<Self> {
        paths.ensure()?;

        let personas = PersonaStore::new(paths.personas_dir.clone());
        personas.ensure()?;
        let persona = config
            .active_persona
            .as_ref()
            .and_then(|id| personas.get(id).ok().flatten())
            .or_else(|| {
                personas
                    .list()
                    .ok()
                    .and_then(|list| list.into_iter().next())
            })
            .unwrap_or_default();
        if config.active_persona.as_deref() != Some(persona.id.as_str()) {
            config.active_persona = Some(persona.id.clone());
        }

        let library = PetLibrary::discover(paths.pets_dir.clone());
        if let Some(installed) =
            default_pet::ensure_installed(&library, config.bundled_pet_removed)?
        {
            tracing::info!(pet = %installed.id, "installed the bundled pet");
            if config.active_pet.is_none() {
                config.active_pet = Some(installed.id);
            }
        }
        let pets = library.list();
        let memory = PetMemory::open(&paths.memory_file)?;

        let entry = config
            .active_pet
            .as_ref()
            .and_then(|id| pets.iter().find(|pet| &pet.id == id).cloned())
            .or_else(|| pets.first().cloned());
        let pet = match entry {
            Some(entry) => {
                config.active_pet = Some(entry.id.clone());
                Some(PetSession::load(entry)?)
            }
            None => None,
        };
        if let Some(pet) = &pet {
            memory.record_event(
                &persona.id,
                EventKind::AppStart,
                Some(format!("Petsona 启动：{}", pet.entry.display_name)),
            )?;
        }

        let selected_pet = config.active_pet.clone();
        config.save(&paths.config_file)?;

        let now = Instant::now();
        Ok(Self {
            paths,
            config,
            personas,
            persona,
            memory,
            library,
            pets,
            selected_pet,
            pet,
            state_server: None,
            state_events: None,
            state_server_port: 0,
            last_health_at: now,
            bubble: None,
            pet_visible: true,
            status: String::new(),
            walk_direction: 1.0,
            next_walk_at: now + Duration::from_secs(45 * 60),
            walk_until: None,
            walk_origin_x: None,
            walk_position_x: None,
            last_user_action: now,
            last_walk_tick: now,
            last_click_at: None,
            pending_single_click: false,
            glance_side: 0,
            last_glance_at: None,
            greeting_rx: None,
            greeting_inflight: false,
            last_greeting_at: None,
            conversation_history: Vec::new(),
            conversation_rx: None,
            conversation_inflight: false,
        })
    }

    pub fn sync_state_server<F>(&mut self, wake: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let wanted = self.config.state_server.enabled;
        let port = self.config.state_server.port;
        let running = self.state_server.is_some();
        if running && (!wanted || port != self.state_server_port) {
            self.state_server = None;
            self.state_events = None;
        }
        if !wanted || self.state_server.is_some() {
            if !wanted {
                self.status = "状态服务已关闭".to_string();
            }
            return;
        }

        let (sender, receiver) = mpsc::channel();
        match StateServer::start_with_waker(port, sender, wake) {
            Ok(server) => {
                tracing::info!(port = server.port(), "state protocol listening");
                self.state_server_port = server.port();
                self.state_server = Some(server);
                self.state_events = Some(receiver);
                self.status = format!(
                    "状态协议已监听 http://127.0.0.1:{}/state",
                    self.state_server_port
                );
                self.publish_health();
            }
            Err(error) => {
                tracing::warn!(%error, "cannot start the state protocol");
                self.state_server = None;
                self.state_events = None;
                self.status = format!("状态协议启动失败：{error}");
            }
        }
    }

    /// Publish the current pet / persona / state snapshot for `GET /health`.
    pub fn publish_health(&self) {
        let Some(server) = &self.state_server else {
            return;
        };
        let (pet, pet_path) = self
            .pet
            .as_ref()
            .map(|pet| (pet.entry.display_name.clone(), Some(pet.entry.dir.clone())))
            .unwrap_or_else(|| (String::new(), None));
        let state = self
            .pet
            .as_ref()
            .map(|pet| pet.engine.current().name().to_string())
            .unwrap_or_default();
        server.set_health(Health {
            ok: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
            pet,
            pet_path,
            persona: self.persona.id.clone(),
            state,
            pets: self.pets.iter().map(|pet| pet.id.clone()).collect(),
            sources: Vec::new(),
        });
    }

    /// Apply state events pushed by hooks and return how many were handled.
    pub fn poll_state_events(&mut self) -> usize {
        let Some(receiver) = &self.state_events else {
            return 0;
        };
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        if events.is_empty() {
            return 0;
        }

        let mut processed = 0usize;
        for event in events {
            let Some(state) = event.pet_state() else {
                continue;
            };
            processed += 1;
            tracing::info!(source = %event.source, state = state.name(), "state event");
            let message = event.message_clipped();
            if let Some(text) = &message {
                self.show_bubble(text.clone(), Duration::from_secs(8));
            }
            if let Some(pet) = &mut self.pet {
                let source = format!("hook:{}", event.source);
                pet.engine
                    .raise(state, &source, message.clone(), event.ttl(), Instant::now());
                pet.anim_started = Instant::now();
                pet.last_state = pet.engine.current();
            }
            let _ = self.memory.record_event(
                &self.persona.id,
                EventKind::CodexStatus,
                Some(match &message {
                    Some(text) => format!("{}：{}", state.name(), text),
                    None => state.name().to_string(),
                }),
            );
            self.last_user_action = Instant::now();
        }
        self.publish_health();
        processed
    }

    pub fn show_bubble(&mut self, text: String, ttl: Duration) {
        self.bubble = Some(Bubble {
            text,
            until: Instant::now().checked_add(ttl).unwrap_or_else(Instant::now),
        });
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save(&self.paths.config_file)?;
        Ok(())
    }

    pub fn active_pet_entry(&self) -> Option<PetEntry> {
        self.config
            .active_pet
            .as_ref()
            .and_then(|id| self.pets.iter().find(|pet| &pet.id == id).cloned())
            .or_else(|| self.pets.first().cloned())
    }
}
