use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use eframe::egui;
use petsona_core::config::{AppConfig, AppPaths};
use petsona_core::deepseek::{save_api_key, DeepSeekClient};
use petsona_core::memory::{EventKind, PetMemory};
use petsona_core::persona::{Persona, PersonaStore};
use petsona_core::pet::state::{PetEngine, PetState};
use petsona_core::pet::{PetAtlas, PetEntry, PetLibrary};
use petsona_core::state_server::{Health, StateEvent, StateServer};

#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

use crate::greeting;
#[cfg(feature = "test-hooks")]
use crate::test_hooks::{TestActionRequest, TestHookServer, TestStatus};

const PET_WINDOW_MIN_WIDTH: f32 = 220.0;
const BUBBLE_TITLE: &str = "Petsona 气泡";
const BUBBLE_WINDOW_SIZE: egui::Vec2 = egui::vec2(360.0, 220.0);
const BUBBLE_GAP: f32 = 8.0;
const BUBBLE_TAIL: f32 = 7.0;
const BUBBLE_BOTTOM_PADDING: f32 = 2.0;
const MENU_TITLE: &str = "Petsona 菜单";
const MENU_WIDTH: f32 = 176.0;
const MENU_ROW: f32 = 30.0;
const MENU_PAD: f32 = 6.0;
const MENU_ROWS: f32 = 4.0;
const MENU_SIZE: egui::Vec2 = egui::vec2(MENU_WIDTH, MENU_ROWS * MENU_ROW + MENU_PAD * 2.0);
const ACTIVE_REPAINT: Duration = Duration::from_millis(16);
const EVENT_POLL_REPAINT: Duration = Duration::from_millis(100);
const IDLE_REPAINT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
const NATIVE_MENU_OPEN_SETTINGS_ID: &str = "petsona.open-settings";
#[cfg(target_os = "macos")]
const NATIVE_MENU_SELECT_PET_PREFIX: &str = "petsona.select-pet:";
#[cfg(target_os = "macos")]
const NATIVE_MENU_TOGGLE_PET_ID: &str = "petsona.toggle-pet";
#[cfg(target_os = "macos")]
const NATIVE_MENU_QUIT_ID: &str = "petsona.quit";
/// How far the cursor may travel before a press becomes a drag.
const CLICK_MOVE_TOLERANCE: f32 = 4.0;
/// How long a press may last and still count as a click.
const CLICK_MAX_HOLD: Duration = Duration::from_millis(700);

pub struct PetsonaApp {
    paths: AppPaths,
    config: AppConfig,
    personas: PersonaStore,
    persona: Persona,
    memory: PetMemory,
    pet: Option<PetRuntime>,
    bubble: Option<Bubble>,
    bubble_window_created: bool,
    bubble_styled: bool,
    settings_open: bool,
    /// A settings command requests a real activation once the viewport exists.
    /// `with_active` only applies when egui creates the child window, while the
    /// same viewport can be reused after it was hidden.
    settings_focus_pending: bool,
    status: String,
    greeting_draft: String,
    api_key_draft: String,
    greeting_rx: Option<Receiver<std::result::Result<String, String>>>,
    greeting_inflight: bool,
    last_greeting_at: Option<Instant>,
    fonts_installed: bool,
    pet_visible: bool,
    last_passthrough: Option<bool>,
    walk_direction: f32,
    next_walk_at: Instant,
    walk_until: Option<Instant>,
    walk_origin_x: Option<f32>,
    walk_position_x: Option<f32>,
    last_user_action: Instant,
    last_walk_tick: Instant,
    last_click_at: Option<Instant>,
    pending_single_click: bool,
    menu_open: bool,
    /// Global (monitor space) position of the open context menu.
    menu_anchor: Option<egui::Pos2>,
    /// Where the menu window actually ended up (clamped to the monitor).
    menu_window_pos: Option<egui::Pos2>,
    /// The menu window exists after the first frame it is requested.
    menu_created: bool,
    menu_styled: bool,
    menu_button_was_down: bool,
    menu_right_button_was_down: bool,
    /// Pet library (Codex / UniPet / local roots) and its current contents.
    library: PetLibrary,
    pets: Vec<PetEntry>,
    selected_pet: Option<String>,
    pet_preview: Option<(String, egui::TextureHandle)>,
    /// Which side the cursor was on when the pet last glanced (-1/0/1).
    glance_side: i8,
    last_glance_at: Option<Instant>,
    /// The pet is being moved by the user right now.
    pet_dragged: bool,
    pointer_left_down: bool,
    pointer_right_down: bool,
    /// Where the current press started (window-local) and when.
    press_origin: Option<egui::Pos2>,
    press_started_at: Option<Instant>,
    press_moved: bool,
    /// Offset from the window origin to the cursor when the drag started.
    drag_grab: Option<egui::Vec2>,
    last_window_pos: Option<egui::Vec2>,
    tray: Option<tray_icon::TrayIcon>,
    tray_events: Option<Receiver<tray_icon::TrayIconEvent>>,
    #[cfg(target_os = "windows")]
    windows_menu: Option<crate::windows_menu::WindowsMenu>,
    #[cfg(target_os = "macos")]
    native_tray_menu: Option<NativeMenu>,
    #[cfg(target_os = "macos")]
    native_menu_events: Option<Receiver<tray_icon::menu::MenuEvent>>,
    settings_pos: Option<egui::Pos2>,
    /// Local state protocol (Codex hooks -> pet).
    state_server: Option<StateServer>,
    state_events: Option<Receiver<StateEvent>>,
    state_server_port: u16,
    #[cfg(feature = "test-hooks")]
    test_hooks: Option<TestHookServer>,
    #[cfg(feature = "test-hooks")]
    test_hook_events: Option<Receiver<TestActionRequest>>,
    #[cfg(feature = "test-hooks")]
    test_status: Arc<Mutex<TestStatus>>,
    #[cfg(feature = "test-hooks")]
    test_logic_count: u64,
    #[cfg(feature = "test-hooks")]
    test_ui_count: u64,
    #[cfg(feature = "test-hooks")]
    test_state_event_count: u64,
    #[cfg(feature = "test-hooks")]
    test_style_reapply_count: u64,
    #[cfg(feature = "test-hooks")]
    test_last_repaint_ms: u64,
    #[cfg(feature = "test-hooks")]
    test_animation_repaint_ms: u64,
    #[cfg(feature = "test-hooks")]
    test_repaint_fast: u64,
    #[cfg(feature = "test-hooks")]
    test_repaint_medium: u64,
    #[cfg(feature = "test-hooks")]
    test_repaint_slow: u64,
    #[cfg(feature = "test-hooks")]
    test_glance_side: Option<i8>,
    repaint_context: Arc<Mutex<Option<egui::Context>>>,
    pointer: crate::platform::PointerSnapshot,
    last_pointer_refresh: Option<Instant>,
    last_health_at: Instant,
    /// Pet import / export state for the settings window.
    import_draft: String,
    pending_overwrite: Option<PathBuf>,
    pending_delete: Option<String>,
    /// Cached viewport commands. Re-sending them every frame made Windows
    /// redraw the non-client frame, which showed up as a flashing border.
    applied_window_size: Option<egui::Vec2>,
    applied_always_on_top: Option<bool>,
    #[cfg(target_os = "windows")]
    window_chrome_ready: bool,
    /// When the window was last resized; the Windows chrome is re-applied once
    /// the resize settles instead of on every step of a drag.
    resize_settled_at: Option<Instant>,
    /// Icon built from the active pet, cached by pet id.
    pet_icon: Option<(String, std::sync::Arc<egui::IconData>)>,
    applied_window_icon: Option<String>,
}

struct PetRuntime {
    entry: PetEntry,
    atlas: PetAtlas,
    engine: PetEngine,
    textures: Vec<Option<egui::TextureHandle>>,
    cell_size: egui::Vec2,
    anim_started: Instant,
    last_state: PetState,
    last_sprite: u32,
}

struct Bubble {
    text: String,
    until: Instant,
}

#[cfg(target_os = "macos")]
struct NativeMenu {
    menu: tray_icon::menu::Menu,
    pet_menu: tray_icon::menu::Submenu,
    toggle_pet: tray_icon::menu::MenuItem,
}

/// What the context menu should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Dismiss,
    OpenSettings,
    TogglePet,
    Quit,
}

impl PetsonaApp {
    pub fn new(paths: AppPaths, mut config: AppConfig) -> Result<Self> {
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
        // The bundled pet lives in the local library next to the user's own
        // pets. Deleting it in the settings opts out for good.
        if let Some(installed) =
            petsona_core::pet::default_pet::ensure_installed(&library, config.bundled_pet_removed)?
        {
            tracing::info!(pet = %installed.id, "installed the bundled pet");
            if config.active_pet.is_none() {
                config.active_pet = Some(installed.id);
            }
        }
        let pets = library.list();
        let entry = config
            .active_pet
            .as_ref()
            .and_then(|id| pets.iter().find(|pet| &pet.id == id).cloned())
            .or_else(|| pets.first().cloned());
        let pet = match entry {
            Some(entry) => {
                config.active_pet = Some(entry.id.clone());
                Some(PetRuntime::load(entry)?)
            }
            None => None,
        };

        let memory = PetMemory::open(&paths.memory_file)?;
        if let Some(pet) = &pet {
            memory.record_event(
                &persona.id,
                EventKind::AppStart,
                Some(format!("Petsona 启动：{}", pet.entry.display_name)),
            )?;
        }
        config.save(&paths.config_file)?;

        let greeting_draft = persona.greeting.clone().unwrap_or_default();
        let fallback = greeting::fallback_greeting(&persona);
        let selected_pet = config.active_pet.clone();
        let state_port = config.state_server.port;
        let repaint_context: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));

        #[cfg(feature = "test-hooks")]
        let (test_hook_sender, test_hook_receiver) = mpsc::channel();
        #[cfg(feature = "test-hooks")]
        let test_hooks = {
            let wake_context = Arc::clone(&repaint_context);
            TestHookServer::start_from_env(test_hook_sender, move || {
                if let Some(ctx) = wake_context
                    .lock()
                    .ok()
                    .and_then(|repaint_context| repaint_context.clone())
                {
                    ctx.request_repaint();
                }
            })?
        };
        #[cfg(feature = "test-hooks")]
        let test_hook_events = test_hooks.as_ref().map(|_| test_hook_receiver);
        #[cfg(feature = "test-hooks")]
        let test_status = test_hooks
            .as_ref()
            .map(|server| server.status())
            .unwrap_or_else(|| Arc::new(Mutex::new(TestStatus::default())));
        let mut app = Self {
            paths,
            config,
            personas,
            persona,
            memory,
            pet,
            bubble: Some(Bubble {
                text: fallback,
                until: Instant::now() + Duration::from_secs(8),
            }),
            bubble_window_created: false,
            bubble_styled: false,
            settings_open: false,
            settings_focus_pending: false,
            status: String::new(),
            greeting_draft,
            api_key_draft: String::new(),
            greeting_rx: None,
            greeting_inflight: false,
            last_greeting_at: None,
            fonts_installed: false,
            pet_visible: true,
            last_passthrough: None,
            walk_direction: 1.0,
            next_walk_at: Instant::now() + Duration::from_secs(45 * 60),
            walk_until: None,
            walk_origin_x: None,
            walk_position_x: None,
            last_user_action: Instant::now(),
            last_walk_tick: Instant::now(),
            last_click_at: None,
            pending_single_click: false,
            menu_open: false,
            menu_anchor: None,
            menu_window_pos: None,
            menu_created: false,
            menu_styled: false,
            menu_button_was_down: false,
            menu_right_button_was_down: false,
            selected_pet,
            pet_preview: None,
            glance_side: 0,
            last_glance_at: None,
            pet_dragged: false,
            pointer_left_down: false,
            pointer_right_down: false,
            press_origin: None,
            press_started_at: None,
            press_moved: false,
            drag_grab: None,
            last_window_pos: None,
            library,
            pets,
            tray: None,
            tray_events: None,
            #[cfg(target_os = "windows")]
            windows_menu: None,
            #[cfg(target_os = "macos")]
            native_tray_menu: None,
            #[cfg(target_os = "macos")]
            native_menu_events: None,
            settings_pos: None,
            state_server: None,
            state_events: None,
            state_server_port: state_port,
            #[cfg(feature = "test-hooks")]
            test_hooks,
            #[cfg(feature = "test-hooks")]
            test_hook_events,
            #[cfg(feature = "test-hooks")]
            test_status,
            #[cfg(feature = "test-hooks")]
            test_logic_count: 0,
            #[cfg(feature = "test-hooks")]
            test_ui_count: 0,
            #[cfg(feature = "test-hooks")]
            test_state_event_count: 0,
            #[cfg(feature = "test-hooks")]
            test_style_reapply_count: 0,
            #[cfg(feature = "test-hooks")]
            test_last_repaint_ms: 0,
            #[cfg(feature = "test-hooks")]
            test_animation_repaint_ms: 0,
            #[cfg(feature = "test-hooks")]
            test_repaint_fast: 0,
            #[cfg(feature = "test-hooks")]
            test_repaint_medium: 0,
            #[cfg(feature = "test-hooks")]
            test_repaint_slow: 0,
            #[cfg(feature = "test-hooks")]
            test_glance_side: None,
            repaint_context,
            pointer: crate::platform::PointerSnapshot::default(),
            last_pointer_refresh: None,
            last_health_at: Instant::now(),
            import_draft: String::new(),
            pending_overwrite: None,
            pending_delete: None,
            applied_window_size: None,
            applied_always_on_top: None,
            #[cfg(target_os = "windows")]
            window_chrome_ready: false,
            resize_settled_at: None,
            pet_icon: None,
            applied_window_icon: None,
        };
        let _ = app.trigger_greeting("startup", true);
        app.sync_state_server();
        Ok(app)
    }

    pub fn initialize(&mut self, creation_context: &eframe::CreationContext<'_>) {
        if let Ok(mut repaint_context) = self.repaint_context.lock() {
            *repaint_context = Some(creation_context.egui_ctx.clone());
        }
        #[cfg(target_os = "windows")]
        {
            let installed = crate::platform::install_mouse_waker(&creation_context.egui_ctx);
            tracing::info!(installed, "event-driven mouse waker");
            match crate::windows_menu::WindowsMenu::start(&creation_context.egui_ctx) {
                Ok(menu) => {
                    self.windows_menu = Some(menu);
                    tracing::info!("Win32 native menu thread started");
                }
                Err(error) => {
                    tracing::warn!(%error, "Win32 native menu unavailable; using egui fallback");
                }
            }
        }
        self.install_tray(creation_context.egui_ctx.clone());
    }

    fn refresh_pointer(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        {
            let pointer_event = ctx.input(|input| {
                input.pointer.delta() != egui::Vec2::ZERO
                    || input.pointer.primary_pressed()
                    || input.pointer.primary_released()
                    || input.pointer.secondary_pressed()
                    || input.pointer.secondary_released()
            });
            let low_frequency_due = self
                .last_pointer_refresh
                .is_none_or(|last| last.elapsed() >= EVENT_POLL_REPAINT);
            if !(pointer_event || low_frequency_due || self.pet_dragged || self.menu_open) {
                return;
            }
        }
        self.pointer = crate::platform::pointer_snapshot();
        self.last_pointer_refresh = Some(Instant::now());
    }

    fn pet_size(&self) -> egui::Vec2 {
        self.pet
            .as_ref()
            .map(|pet| pet.cell_size * self.config.window.scale.clamp(0.5, 3.0))
            .unwrap_or_else(|| egui::vec2(192.0, 208.0))
    }

    fn pet_window_size(&self) -> egui::Vec2 {
        let cell = self.pet_cell_size();
        // The window is sized in quarter steps of the scale slider. The sprite
        // itself still follows the scale continuously, but the native window
        // only resizes when a quarter step is crossed - resizing it on every
        // frame of a drag is what made the pet flicker.
        let stage = self.stage_scale();
        let stage_size = cell * stage;
        egui::vec2(stage_size.x.max(PET_WINDOW_MIN_WIDTH), stage_size.y)
    }

    /// Unscaled atlas cell size of the active pet.
    fn pet_cell_size(&self) -> egui::Vec2 {
        self.pet
            .as_ref()
            .map(|pet| pet.cell_size)
            .unwrap_or_else(|| egui::vec2(192.0, 208.0))
    }

    /// Window scale rounded up to the next quarter step.
    fn stage_scale(&self) -> f32 {
        let scale = self.config.window.scale.clamp(0.5, 3.0);
        ((scale * 4.0).ceil() / 4.0).max(0.5)
    }

    fn apply_viewport(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        window_size: egui::Vec2,
    ) {
        let level = self.config.window.always_on_top;
        if self.applied_always_on_top != Some(level) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if level {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
            self.applied_always_on_top = Some(level);
        }
        let target = egui::vec2(window_size.x.round(), window_size.y.round());
        let previous = self.applied_window_size;
        if previous != Some(target) {
            let mut positioned = false;
            if let Some(window) = frame.winit_window() {
                if let Ok(position) = window.outer_position() {
                    let scale = window.scale_factor().max(0.1) as f32;
                    let current = egui::pos2(position.x as f32 / scale, position.y as f32 / scale);
                    let actual_size = window.outer_size();
                    let actual_size = egui::vec2(
                        actual_size.width as f32 / scale,
                        actual_size.height as f32 / scale,
                    );
                    let anchor = bottom_center_anchor(current, actual_size);
                    let next = position_for_bottom_center(anchor, target);
                    crate::platform::set_window_geometry(
                        window,
                        next.x as f64,
                        next.y as f64,
                        target.x as f64,
                        target.y as f64,
                    );
                    positioned = true;
                }
            }
            if !positioned {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
            }
            self.applied_window_size = Some(target);
            self.resize_settled_at = Some(Instant::now());
        }
    }

    fn draw_pet(&mut self, root_ui: &mut egui::Ui, window_size: egui::Vec2) {
        if self.pet.is_none() {
            self.draw_missing_pet(root_ui);
            return;
        }

        // Compute the sprite rectangle before borrowing the pet mutably.
        let pet_rect = self.pet_rect(window_size);
        let scale = self.config.window.scale.clamp(0.5, 3.0);
        let total = window_size;
        let pet = self.pet.as_mut().expect("checked above");
        let cell = pet.cell_size * scale;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(root_ui, |ui| {
                // Input is handled from the system cursor and button state (the
                // window is permanently click-through), so the pet itself only
                // needs a passive area to draw into.
                let (rect, _response) = ui.allocate_exact_size(total, egui::Sense::hover());
                let pet_rect = pet_rect.translate(rect.min.to_vec2());

                let elapsed_ms = pet.anim_started.elapsed().as_secs_f32() * 1000.0;
                let sprite = pet.current_sprite(elapsed_ms);
                if let Some(texture_id) = pet.texture_for(ui.ctx(), sprite) {
                    // SizedTexture uses `ImageFit::Exact`, so hand it the scaled
                    // size or the 大小 setting would be ignored.
                    let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, cell));
                    ui.put(pet_rect, image);
                }
            });
    }

    /// Show the speech bubble in its own transparent overlay so the pet window
    /// stays exactly the size of the sprite and never moves when text appears.
    fn show_bubble_viewport(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let bubble_id = egui::ViewportId::from_hash_of("petsona-bubble");
        let text = if self.pet_visible {
            self.bubble
                .as_ref()
                .filter(|bubble| Instant::now() < bubble.until)
                .map(|bubble| bubble.text.clone())
        } else {
            None
        };
        let Some(text) = text else {
            if self.bubble_window_created {
                ctx.send_viewport_cmd_to(bubble_id, egui::ViewportCommand::Visible(false));
                self.bubble_window_created = false;
                self.bubble_styled = false;
            }
            return;
        };

        let Some(window) = frame.winit_window() else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor().max(0.1);
        let parent_position = egui::pos2(
            position.x as f32 / scale as f32,
            position.y as f32 / scale as f32,
        );
        let pet_rect = self.pet_rect(self.pet_window_size());
        let pet_center = parent_position + pet_rect.center().to_vec2();
        let pet_top = parent_position.y + pet_rect.top();
        let bubble_position = egui::pos2(
            pet_center.x - BUBBLE_WINDOW_SIZE.x * 0.5,
            pet_top - BUBBLE_WINDOW_SIZE.y - BUBBLE_GAP,
        );
        let builder = egui::ViewportBuilder::default()
            .with_title(BUBBLE_TITLE)
            .with_inner_size([BUBBLE_WINDOW_SIZE.x, BUBBLE_WINDOW_SIZE.y])
            .with_position([bubble_position.x, bubble_position.y])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_mouse_passthrough(true)
            .with_visible(true);
        ctx.show_viewport_immediate(bubble_id, builder, |ui, _class| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(ui, |ui| {
                    let window = ui.max_rect();
                    draw_bubble_window(ui.painter(), window, &text);
                });
        });
        self.bubble_window_created = true;
        if !self.bubble_styled {
            self.bubble_styled = crate::platform::set_no_activate_for_title(BUBBLE_TITLE) > 0;
        }
    }

    fn draw_missing_pet(&mut self, root_ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(root_ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("没有找到宠物\n右键打开设置");
                });
            });
    }

    /// Real popup menu: its own native window, sized to the items, so it can
    /// open at the cursor and is never clipped by the pet window.
    fn show_context_menu(&mut self, ctx: &egui::Context) {
        let size = MENU_SIZE;
        let anchor = self.menu_anchor.unwrap_or(egui::Pos2::ZERO);
        let target = clamp_to_monitor(ctx, anchor, size);
        // The window is created one frame early as a tiny transparent square
        // under the cursor, then resized into place. It is on screen (so the
        // backend really paints it) but invisible, which avoids the flash of a
        // window being shown before its first paint.
        let first_frame = !self.menu_created;
        let (position, size) = if first_frame {
            (anchor, egui::vec2(8.0, 8.0))
        } else {
            (target, MENU_SIZE)
        };
        let builder = egui::ViewportBuilder::default()
            .with_title(MENU_TITLE)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_inner_size([size.x, size.y])
            .with_position([position.x, position.y]);
        let builder = match self.pet_icon() {
            Some(icon) => builder.with_icon(icon),
            None => builder,
        };

        let mut action: Option<MenuAction> = None;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("petsona-menu"),
            builder,
            |ui, _class| {
                if first_frame {
                    // Nothing may be drawn yet: this frame only exists to create
                    // and paint the window. Drawing here is what showed up as a
                    // small dark dot before the menu appeared.
                    return;
                }
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(MENU_PAD as i8))
                    .show(ui, |ui| {
                        ui.set_min_width(MENU_WIDTH - MENU_PAD * 2.0);
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                        let toggle = if self.pet_visible {
                            "隐藏宠物"
                        } else {
                            "显示宠物"
                        };
                        let entries = [
                            ("打开设置", MenuAction::OpenSettings),
                            ("更换宠物", MenuAction::OpenSettings),
                            (toggle, MenuAction::TogglePet),
                            ("退出", MenuAction::Quit),
                        ];
                        for (label, candidate) in entries {
                            let button = egui::Button::new(label)
                                .min_size(egui::vec2(MENU_WIDTH - MENU_PAD * 2.0, MENU_ROW - 2.0));
                            if ui.add(button).clicked() {
                                action = Some(candidate);
                            }
                        }
                    });
                if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
                    action = Some(MenuAction::Dismiss);
                }
            },
        );
        self.menu_created = true;
        self.menu_window_pos = Some(target);
        if !self.menu_styled {
            // Menus never activate, so they cannot steal focus (or repaint a
            // frame) either. Retry until the window really exists.
            self.menu_styled = crate::platform::set_no_activate_for_title(MENU_TITLE) > 0;
        }
        let Some(action) = action else {
            return;
        };
        self.dismiss_menu();
        match action {
            MenuAction::Dismiss => {}
            MenuAction::OpenSettings => self.open_settings(),
            MenuAction::TogglePet => {
                let visible = !self.pet_visible;
                self.set_pet_visible(ctx, visible);
            }
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn dismiss_menu(&mut self) {
        self.menu_open = false;
        self.menu_anchor = None;
        self.menu_window_pos = None;
        self.menu_created = false;
        self.menu_styled = false;
        self.menu_button_was_down = false;
        self.menu_right_button_was_down = false;
    }

    /// Menus do not take focus, so "clicked somewhere else" and Escape are read
    /// from the system instead of from focus events.
    fn poll_menu(&mut self, ctx: &egui::Context) {
        if !self.menu_open {
            self.menu_button_was_down = false;
            self.menu_right_button_was_down = false;
            return;
        }
        if crate::platform::escape_pressed() {
            self.dismiss_menu();
            return;
        }
        let button_down = self
            .pointer
            .primary_down
            .unwrap_or_else(|| ctx.input(|input| input.pointer.any_down()));
        let right_down = self
            .pointer
            .secondary_down
            .unwrap_or_else(|| ctx.input(|input| input.pointer.secondary_down()));
        let (left_event_pressed, right_event_pressed) = ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.secondary_pressed(),
            )
        });
        let pressed_left =
            button_pressed_edge(button_down, self.menu_button_was_down, left_event_pressed);
        let pressed_right = button_pressed_edge(
            right_down,
            self.menu_right_button_was_down,
            right_event_pressed,
        );
        self.menu_button_was_down = button_down;
        self.menu_right_button_was_down = right_down;
        if !(pressed_left || pressed_right) || !self.menu_created {
            return;
        }
        let (Some(menu_pos), Some((cursor_x, cursor_y))) =
            (self.menu_window_pos, self.pointer.position)
        else {
            return;
        };
        let scale = ctx
            .input(|input| input.viewport().native_pixels_per_point)
            .unwrap_or(1.0) as f64;
        let menu_rect = egui::Rect::from_min_size(menu_pos, MENU_SIZE);
        let cursor = egui::pos2(
            cursor_x as f32 / scale as f32,
            cursor_y as f32 / scale as f32,
        );
        if !menu_rect.contains(cursor) {
            self.dismiss_menu();
        }
    }

    fn show_settings_viewport(&mut self, ctx: &egui::Context) {
        let viewport_id = egui::ViewportId::from_hash_of("petsona-settings");
        let size = egui::vec2(640.0, 720.0);
        // Place the settings window next to the pet, clamped to the monitor,
        // the first time it opens. After that the user owns the position.
        let position = match self.settings_pos {
            Some(position) => position,
            None => {
                let position = self.default_settings_position(ctx, size);
                self.settings_pos = Some(position);
                position
            }
        };
        let builder = egui::ViewportBuilder::default()
            .with_title("Petsona 设置")
            .with_inner_size([size.x, size.y])
            .with_min_inner_size([420.0, 480.0])
            .with_decorations(true)
            .with_transparent(false)
            .with_taskbar(true)
            .with_position([position.x, position.y])
            .with_resizable(true)
            .with_active(true)
            .with_visible(true);
        let builder = match self.pet_icon() {
            Some(icon) => builder.with_icon(icon),
            None => builder,
        };
        ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
            self.draw_settings(ui);
        });
        if self.settings_open && self.settings_focus_pending {
            // This also works when the settings viewport already existed but
            // had been closed/hidden. On macOS the explicit AppKit activation
            // is needed because the pet itself is deliberately non-activating.
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
            #[cfg(target_os = "macos")]
            {
                if crate::platform::focus_window_for_title("Petsona 设置") > 0 {
                    self.settings_focus_pending = false;
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                self.settings_focus_pending = false;
            }
        }
    }

    fn open_settings(&mut self) {
        self.settings_open = true;
        self.settings_pos = None;
        self.settings_focus_pending = true;
    }

    /// Bottom-right of the pet when there is room, otherwise the closest spot
    /// that still fits on the monitor.
    fn default_settings_position(&self, ctx: &egui::Context, size: egui::Vec2) -> egui::Pos2 {
        let (monitor, pet_rect) =
            ctx.input(|input| (input.viewport().monitor_size, input.viewport().outer_rect));
        let monitor = monitor.unwrap_or(egui::vec2(1280.0, 800.0));
        let pet_rect =
            pet_rect.unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, size));
        let gap = 12.0;
        let right = pet_rect.right() + gap;
        let left = pet_rect.left() - size.x - gap;
        let x = if right + size.x <= monitor.x {
            right
        } else {
            left.max(0.0)
        };
        let y = (pet_rect.bottom() - size.y).clamp(0.0, (monitor.y - size.y).max(0.0));
        egui::pos2(x.clamp(0.0, (monitor.x - size.x).max(0.0)), y)
    }

    fn draw_settings(&mut self, root_ui: &mut egui::Ui) {
        self.handle_dropped_files(root_ui.ctx());
        if root_ui
            .ctx()
            .input(|input| input.viewport().close_requested())
        {
            self.settings_open = false;
            self.settings_pos = None;
            return;
        }
        egui::CentralPanel::default().show(root_ui, |ui| {
            // The window has native decorations now, so it scrolls as a whole
            // instead of pretending to be a title bar.
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    self.draw_settings_body(ui);
                });
        });
    }

    fn draw_settings_body(&mut self, ui: &mut egui::Ui) {
        let mut switch_to: Option<String> = None;
        let mut import_request: Option<(PathBuf, bool)> = None;
        let mut export_id: Option<String> = None;
        let mut delete_id: Option<String> = None;
        ui.collapsing("宠物", |ui| {
            let pets: Vec<(String, String, bool)> = self
                .pets
                .iter()
                .map(|pet| {
                    let local = pet.root == petsona_core::pet::RootKind::AppData;
                    let mut label = format!(
                        "{}  ·  {}  ({})",
                        pet.display_name,
                        pet.id,
                        pet.root.label()
                    );
                    if local && pet.id == petsona_core::pet::DEFAULT_PET_ID {
                        label.push_str("  ·  内置");
                    }
                    if pet.sprite_version_number == Some(2) || pet.frame.rows >= 11 {
                        label.push_str("  ·  V2（支持持续注视）");
                    }
                    (pet.id.clone(), label, local)
                })
                .collect();
            ui.horizontal(|ui| {
                ui.label(format!("当前：{}", self.active_pet_name()));
                ui.label(format!(
                    "共 {} 个（本地库 {} 个，其余来自 ~/.codex/pets、~/.unipet/pets）",
                    pets.len(),
                    pets.iter().filter(|(_, _, local)| *local).count()
                ));
            });
            ui.horizontal(|ui| {
                ui.label("导入");
                ui.add(
                    egui::TextEdit::singleline(&mut self.import_draft)
                        .hint_text("宠物文件夹或 .zip 的路径")
                        .desired_width(240.0),
                );
                #[cfg(target_os = "macos")]
                if ui.button("选择文件…").clicked() {
                    if let Some(path) = crate::platform::choose_pet_import_path() {
                        import_request = Some((path, false));
                    }
                }
                if ui
                    .button("导入")
                    .on_hover_text("把文件夹或 .zip 拖到窗口里也可以导入")
                    .clicked()
                {
                    let draft = self.import_draft.trim().to_string();
                    if !draft.is_empty() {
                        import_request = Some((PathBuf::from(draft), false));
                    }
                }
                if ui.button("打开宠物库目录").clicked() {
                    if crate::platform::open_in_file_manager(&self.paths.pets_dir) {
                        self.status = format!("宠物库：{}", self.paths.pets_dir.display());
                    } else {
                        self.status = format!("宠物库目录：{}", self.paths.pets_dir.display());
                    }
                }
                if ui.button("重新扫描").clicked() {
                    self.refresh_pets();
                    self.status = format!("发现 {} 个宠物", self.pets.len());
                }
            });
            if let Some(path) = self.pending_overwrite.clone() {
                ui.horizontal(|ui| {
                    ui.label(format!("{} 与本地库里的宠物同 id。", path.display()));
                    if ui.button("覆盖导入").clicked() {
                        import_request = Some((path.clone(), true));
                    }
                    if ui.button("取消").clicked() {
                        self.pending_overwrite = None;
                    }
                });
            }
            if pets.is_empty() {
                ui.label("没有找到宠物包。用上面的「导入」或直接拖进窗口即可。");
            }
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("pet-list")
                .show(ui, |ui| {
                    for (id, label, _local) in &pets {
                        let selected = self.selected_pet.as_deref() == Some(id.as_str());
                        if ui.radio(selected, label).clicked() {
                            self.selected_pet = Some(id.clone());
                        }
                    }
                });

            if let Some(selected) = self.selected_pet.clone() {
                let dirty =
                    self.pet_preview.as_ref().map(|(id, _)| id.clone()) != Some(selected.clone());
                if dirty {
                    self.pet_preview =
                        self.pets
                            .iter()
                            .find(|pet| pet.id == selected)
                            .and_then(|pet| {
                                pet_preview_texture(ui.ctx(), pet)
                                    .map(|texture| (pet.id.clone(), texture))
                            });
                }
                // Read everything the row needs before opening the nested
                // closures, so they never borrow `self` while it is used.
                let texture_id = self
                    .pet_preview
                    .as_ref()
                    .filter(|(id, _)| id == &selected)
                    .map(|(_, texture)| texture.id());
                let frame = self
                    .pets
                    .iter()
                    .find(|pet| pet.id == selected)
                    .map(|pet| pet.frame);
                let local = self
                    .pets
                    .iter()
                    .find(|pet| pet.id == selected)
                    .is_some_and(|pet| pet.root == petsona_core::pet::RootKind::AppData);
                let is_active = selected == self.active_pet_id();
                let confirm_delete = self.pending_delete.as_deref() == Some(selected.as_str());
                if let Some(texture_id) = texture_id {
                    ui.horizontal(|ui| {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                            texture_id,
                            egui::vec2(96.0, 104.0),
                        )));
                        ui.vertical(|ui| {
                            if let Some(frame) = frame {
                                ui.label(format!(
                                    "{} 列 × {} 行，单元格 {}×{}",
                                    frame.columns, frame.rows, frame.width, frame.height
                                ));
                            }
                            if is_active {
                                ui.label("已经是当前宠物");
                            } else if ui.button("切换到这个宠物").clicked() {
                                switch_to = Some(selected.clone());
                            }
                            ui.horizontal(|ui| {
                                if ui.button("导出为 zip").clicked() {
                                    export_id = Some(selected.clone());
                                }
                                if !local {
                                    ui.label("（来自 Codex/UniPet，只读引用）");
                                } else if confirm_delete {
                                    if ui.button("确认删除").clicked() {
                                        delete_id = Some(selected.clone());
                                    }
                                    if ui.button("取消").clicked() {
                                        self.pending_delete = None;
                                    }
                                } else if ui.button("从本地库删除").clicked() {
                                    self.pending_delete = Some(selected.clone());
                                }
                            });
                        });
                    });
                }
            }
            if Self::files_are_hovering(ui.ctx()) {
                ui.label("松手即可把宠物导入本地库");
            }
        });
        if let Some(id) = switch_to {
            self.switch_pet(&id);
        }
        if let Some((path, overwrite)) = import_request {
            self.import_path(&path, overwrite);
        }
        if let Some(id) = export_id {
            self.export_pet_zip(&id);
        }
        if let Some(id) = delete_id {
            self.remove_local_pet(&id);
        }

        ui.collapsing("人格", |ui| {
            egui::Grid::new("persona-grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("名字");
                    ui.text_edit_singleline(&mut self.persona.name);
                    ui.end_row();

                    ui.label("语气");
                    ui.text_edit_singleline(&mut self.persona.traits.tone);
                    ui.end_row();

                    ui.label("语言");
                    ui.text_edit_singleline(&mut self.persona.traits.language);
                    ui.end_row();

                    ui.label("固定问候");
                    ui.text_edit_singleline(&mut self.greeting_draft);
                    ui.end_row();
                });
            ui.label("系统提示");
            ui.add(
                egui::TextEdit::multiline(&mut self.persona.system_prompt)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
        });

        ui.collapsing("宠物行为", |ui| {
            ui.horizontal(|ui| {
                ui.label("大小");
                ui.add(
                    egui::Slider::new(&mut self.config.window.scale, 0.5..=2.0).fixed_decimals(2),
                );
            });
            ui.checkbox(&mut self.config.window.auto_walk.enabled, "启用活动提醒");
            egui::Grid::new("auto-walk-grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("提醒间隔（分钟）");
                    ui.add(
                        egui::DragValue::new(&mut self.config.window.auto_walk.interval_minutes)
                            .range(5..=240),
                    );
                    ui.end_row();

                    ui.label("单次活动时间（秒）");
                    ui.add(
                        egui::DragValue::new(&mut self.config.window.auto_walk.walk_seconds)
                            .range(1.0..=60.0),
                    );
                    ui.end_row();

                    ui.label("移动速度");
                    ui.add(
                        egui::DragValue::new(&mut self.config.window.auto_walk.speed_px_s)
                            .range(5.0..=120.0),
                    );
                    ui.end_row();

                    ui.label("活动范围（像素）");
                    ui.add(
                        egui::DragValue::new(&mut self.config.window.auto_walk.range_px)
                            .range(20.0..=400.0),
                    );
                    ui.end_row();

                    ui.label("交互后静默（秒）");
                    ui.add(
                        egui::DragValue::new(&mut self.config.window.auto_walk.user_grace_seconds)
                            .range(0.0..=300.0),
                    );
                    ui.end_row();
                });
            ui.checkbox(&mut self.config.window.click_through, "像素级点击穿透");
        });

        ui.collapsing("状态协议", |ui| {
                ui.checkbox(
                    &mut self.config.state_server.enabled,
                    "允许本地程序驱动宠物（Codex hooks）",
                );
                ui.horizontal(|ui| {
                    ui.label("端口");
                    ui.add(
                        egui::DragValue::new(&mut self.config.state_server.port)
                            .range(1024..=65535),
                    );
                    let running = match &self.state_server {
                        Some(server) => format!("监听中 127.0.0.1:{}", server.port()),
                        None => "未运行".to_string(),
                    };
                    ui.label(running);
                });
                if let Some(server) = &self.state_server {
                    ui.label(format!(
                        "curl -XPOST http://127.0.0.1:{}/state -H \"content-type: application/json\" -d \"{{\\\"source\\\":\\\"codex\\\",\\\"state\\\":\\\"running\\\",\\\"message\\\":\\\"跑测试中\\\"}}\"",
                        server.port()
                    ));
                }
                ui.label(
                    "状态名：idle / running / waiting / failed / review / waving / jumping / running-left / running-right",
                );
                ui.label("改完端口后点「保存」生效。");
            });

        ui.collapsing("DeepSeek", |ui| {
            egui::Grid::new("deepseek-grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Base URL");
                    ui.text_edit_singleline(&mut self.config.deepseek.base_url);
                    ui.end_row();

                    ui.label("模型");
                    ui.text_edit_singleline(&mut self.config.deepseek.model);
                    ui.end_row();

                    ui.label("API Key 环境变量");
                    ui.text_edit_singleline(&mut self.config.deepseek.api_key_env);
                    ui.end_row();

                    ui.label("最大 tokens");
                    ui.add(
                        egui::DragValue::new(&mut self.config.deepseek.max_tokens).range(16..=400),
                    );
                    ui.end_row();

                    ui.label("temperature");
                    ui.add(egui::Slider::new(
                        &mut self.config.deepseek.temperature,
                        0.0..=2.0,
                    ));
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                ui.label("写入钥匙串");
                ui.add(
                    egui::TextEdit::singleline(&mut self.api_key_draft)
                        .password(true)
                        .hint_text("sk-..."),
                );
                if ui.button("保存 Key").clicked() {
                    match save_api_key(&self.api_key_draft) {
                        Ok(()) => {
                            self.api_key_draft.clear();
                            self.status = "API Key 已保存到系统钥匙串".to_string();
                        }
                        Err(error) => self.status = format!("保存 Key 失败：{error}"),
                    }
                }
            });
        });

        ui.collapsing("记忆", |ui| {
            let facts = self.memory.list_facts(&self.persona.id);
            if facts.is_empty() {
                ui.label("还没有记住任何事实。");
            } else {
                for fact in facts {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}：{}", fact.key, fact.value));
                        if ui.small_button("删除").clicked() {
                            if let Err(error) = self.memory.forget_fact(&self.persona.id, &fact.id)
                            {
                                self.status = format!("删除失败：{error}");
                            }
                        }
                    });
                }
            }
            if ui.button("清空这个人格的记忆").clicked() {
                match self.memory.clear_persona(&self.persona.id) {
                    Ok(()) => self.status = "记忆已清空".to_string(),
                    Err(error) => self.status = format!("清空失败：{error}"),
                }
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("保存").clicked() {
                self.save_all();
            }
            if ui.button("测试问候").clicked() {
                let _ = self.trigger_greeting("manual", true);
            }
            if ui.button("关闭设置").clicked() {
                self.settings_open = false;
            }
        });
        if !self.status.is_empty() {
            ui.separator();
            ui.label(&self.status);
        }
    }

    fn save_all(&mut self) {
        self.persona.greeting = if self.greeting_draft.trim().is_empty() {
            None
        } else {
            Some(self.greeting_draft.trim().to_string())
        };
        if let Err(error) = self.personas.save(&self.persona) {
            self.status = format!("保存人格失败：{error}");
            return;
        }
        if let Err(error) = self.config.save(&self.paths.config_file) {
            self.status = format!("保存配置失败：{error}");
            return;
        }
        self.sync_state_server();
        self.publish_health();
        self.status = "已保存".to_string();
    }

    /// Start, stop or restart the local state protocol to match the config.
    fn sync_state_server(&mut self) {
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
        let repaint_context = Arc::clone(&self.repaint_context);
        let wake = move || {
            let ctx = repaint_context
                .lock()
                .ok()
                .and_then(|repaint_context| repaint_context.clone());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        };
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
    fn publish_health(&self) {
        let Some(server) = &self.state_server else {
            return;
        };
        let (pet, pet_path) = self
            .pet
            .as_ref()
            .map(|pet| (pet.entry.display_name.clone(), Some(pet.entry.dir.clone())))
            .unwrap_or_else(|| ("".to_string(), None));
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

    #[cfg(feature = "test-hooks")]
    fn poll_test_hooks(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let mut actions = Vec::new();
        if let Some(receiver) = &self.test_hook_events {
            while let Ok(action) = receiver.try_recv() {
                actions.push(action);
            }
        }
        for action in actions {
            self.apply_test_action(ctx, frame, action);
        }
    }

    #[cfg(feature = "test-hooks")]
    fn apply_test_action(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        action: TestActionRequest,
    ) {
        match action.action.as_str() {
            "open-menu" => {
                let anchor = self.test_menu_anchor(frame);
                self.open_menu_at(anchor);
            }
            "close-menu" => self.dismiss_menu(),
            "open-settings" => self.open_settings(),
            "close-settings" => {
                self.settings_open = false;
                self.settings_pos = None;
            }
            "hide-pet" => self.set_pet_visible(ctx, false),
            "show-pet" => self.set_pet_visible(ctx, true),
            "toggle-pet" => {
                let visible = !self.pet_visible;
                self.set_pet_visible(ctx, visible);
            }
            "set-scale" => {
                if let Some(value) = action.value {
                    self.config.window.scale = (value as f32).clamp(0.5, 2.0);
                }
            }
            "set-click-through" => {
                if let Some(enabled) = action.enabled {
                    self.config.window.click_through = enabled;
                    self.last_passthrough = None;
                }
            }
            "set-always-on-top" => {
                if let Some(enabled) = action.enabled {
                    self.config.window.always_on_top = enabled;
                    self.applied_always_on_top = None;
                }
            }
            "set-auto-walk-enabled" => {
                if let Some(enabled) = action.enabled {
                    self.config.window.auto_walk.enabled = enabled;
                }
            }
            "show-bubble" => {
                let text = action.text.unwrap_or_else(|| "test bubble".to_string());
                let ttl = Duration::from_millis(action.ttl_ms.unwrap_or(8_000).max(1));
                self.bubble = Some(Bubble {
                    text,
                    until: Instant::now().checked_add(ttl).unwrap_or_else(Instant::now),
                });
            }
            "clear-bubble" => self.bubble = None,
            "trigger-auto-walk" => {
                let now = Instant::now();
                self.config.window.auto_walk.enabled = true;
                self.next_walk_at = now;
                self.walk_until = None;
                self.walk_origin_x = None;
                self.walk_position_x = None;
                self.walk_direction = 1.0;
                if let Some(pet) = &mut self.pet {
                    let _ = pet.engine.clear_all();
                    let _ = pet.engine.set_base(PetState::Idle);
                    pet.anim_started = now;
                    pet.last_state = pet.engine.current();
                }
                let grace = self.config.window.auto_walk.user_grace_seconds.max(0.0);
                self.last_user_action = now
                    .checked_sub(Duration::from_secs_f32(grace + 1.0))
                    .unwrap_or(now);
                self.last_walk_tick = now.checked_sub(Duration::from_millis(100)).unwrap_or(now);
            }
            "stop-auto-walk" => {
                self.walk_until = None;
                self.walk_origin_x = None;
                self.walk_position_x = None;
                self.next_walk_at = Instant::now() + Duration::from_secs(3600);
                self.set_walk_state(PetState::Idle);
            }
            "start-gaze" => {
                let dx = action.value.unwrap_or(1.0) as f32;
                let now = Instant::now();
                let raised = self
                    .pet
                    .as_mut()
                    .is_some_and(|pet| pet.engine.glance(dx, now).is_some());
                if raised {
                    self.glance_side = if dx < 0.0 { -1 } else { 1 };
                    self.test_glance_side = Some(self.glance_side);
                    self.last_glance_at = Some(now);
                    if let Some(pet) = &mut self.pet {
                        pet.anim_started = now;
                        pet.last_state = pet.engine.current();
                    }
                }
            }
            "set-glance-side" => {
                self.test_glance_side =
                    Some(action.value.unwrap_or(0.0).round().clamp(-1.0, 1.0) as i8);
            }
            "clear-glance-side" => self.test_glance_side = None,
            "release-gaze" => self.release_glance(Instant::now()),
            "cancel-gaze" => self.cancel_glance(),
            "click-pet" => self.on_pet_click(),
            "double-click-pet" => self.on_double_click(),
            "save-config" => {
                if let Err(error) = self.config.save(&self.paths.config_file) {
                    tracing::warn!(%error, "test hook could not save config");
                }
            }
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            other => tracing::warn!(action = other, "unknown test hook action"),
        }
        ctx.request_repaint();
    }

    #[cfg(feature = "test-hooks")]
    fn test_menu_anchor(&self, frame: &eframe::Frame) -> egui::Pos2 {
        if let Some(window) = frame.winit_window() {
            let scale = window.scale_factor().max(0.1) as f32;
            if let Ok(position) = window.outer_position() {
                return egui::pos2(
                    position.x as f32 / scale + 32.0,
                    position.y as f32 / scale + 32.0,
                );
            }
        }
        egui::pos2(100.0, 100.0)
    }

    #[cfg(feature = "test-hooks")]
    fn test_click_point(&self, frame: &eframe::Frame) -> Option<(i32, i32)> {
        let pet = self.pet.as_ref()?;
        let window = frame.winit_window()?;
        let mask = &pet.atlas.mask;
        let idle_sprites = pet
            .engine
            .animation(PetState::Idle)
            .map(|animation| animation.sprites.clone())
            .unwrap_or_else(|| vec![pet.last_sprite]);

        let mut found = None;
        for my in 0..mask.mask_height {
            for mx in 0..mask.mask_width {
                let x = (mx * mask.scale + mask.scale / 2) as f32;
                let y = (my * mask.scale + mask.scale / 2) as f32;
                if idle_sprites
                    .iter()
                    .all(|sprite| mask.opaque_at_cell(*sprite, x, y))
                {
                    found = Some((x, y));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let (x, y) = found?;

        let pet_rect = self.pet_rect(self.pet_window_size());
        let logical_x = pet_rect.min.x + x * self.config.window.scale;
        let logical_y = pet_rect.min.y + y * self.config.window.scale;
        let position = window.outer_position().ok()?;
        let scale = window.scale_factor().max(0.1) as f32;
        Some((
            (position.x as f32 + logical_x * scale).round() as i32,
            (position.y as f32 + logical_y * scale).round() as i32,
        ))
    }

    #[cfg(feature = "test-hooks")]
    fn test_click_hits(&self, frame: &eframe::Frame, x: i32, y: i32) -> bool {
        let Some(window) = frame.winit_window() else {
            return false;
        };
        let Ok(position) = window.outer_position() else {
            return false;
        };
        let scale = window.scale_factor().max(0.1) as f32;
        let local = egui::pos2(
            (x as f32 - position.x as f32) / scale,
            (y as f32 - position.y as f32) / scale,
        );
        self.cursor_over_pet(self.pet_window_size(), local)
    }

    #[cfg(feature = "test-hooks")]
    fn publish_test_status(&mut self, frame: &eframe::Frame) {
        let (state, base_state, sprite_index) = match &self.pet {
            Some(pet) => (
                pet.engine.current().name().to_string(),
                pet.engine.base().name().to_string(),
                pet.last_sprite,
            ),
            None => ("idle".to_string(), "idle".to_string(), 0),
        };
        let now = Instant::now();
        let bubble_text = self
            .bubble
            .as_ref()
            .filter(|bubble| bubble.until > now)
            .map(|bubble| bubble.text.clone());
        let window = frame.winit_window();
        let position = window.and_then(|window| window.outer_position().ok());
        let size = window.map(|window| window.outer_size());
        let hooks_port = self
            .test_hooks
            .as_ref()
            .map(|server| server.port())
            .unwrap_or(0);
        let click_point = self.test_click_point(frame);
        let click_hits = click_point.is_some_and(|(x, y)| self.test_click_hits(frame, x, y));

        if let Ok(mut status) = self.test_status.lock() {
            status.ok = true;
            status.version = env!("CARGO_PKG_VERSION").to_string();
            status.process_id = std::process::id();
            status.pet_visible = self.pet_visible;
            status.settings_open = self.settings_open;
            #[cfg(target_os = "macos")]
            {
                status.settings_key_window =
                    crate::platform::is_window_key_for_title("Petsona 设置");
            }
            status.menu_open = self.menu_open;
            status.click_through = self.config.window.click_through;
            status.passthrough = self.last_passthrough.unwrap_or(false);
            status.pointer_left_down = self.pointer_left_down;
            status.always_on_top = self.config.window.always_on_top;
            status.scale = self.config.window.scale;
            status.state = state;
            status.base_state = base_state;
            status.sprite_index = sprite_index;
            status.bubble_text = bubble_text;
            status.bubble_window_created = self.bubble_window_created;
            #[cfg(target_os = "windows")]
            {
                status.native_menu_ready = self.windows_menu.is_some();
            }
            #[cfg(target_os = "macos")]
            {
                status.native_menu_ready = self.native_tray_menu.is_some();
            }
            status.gaze_side = self.glance_side;
            status.gaze_phase = self
                .pet
                .as_ref()
                .and_then(|pet| pet.engine.gaze_phase())
                .map(|phase| phase.name().to_string());
            status.pet_dragged = self.pet_dragged;
            status.window_x = position.map(|position| position.x);
            status.window_y = position.map(|position| position.y);
            status.window_width = size.map(|size| size.width);
            status.window_height = size.map(|size| size.height);
            status.pet_click_x = click_point.map(|point| point.0);
            status.pet_click_y = click_point.map(|point| point.1);
            status.pet_click_hits = click_hits;
            status.logic_count = self.test_logic_count;
            status.ui_count = self.test_ui_count;
            status.state_event_count = self.test_state_event_count;
            status.cursor_poll_count = crate::platform::cursor_poll_count();
            status.mouse_events = crate::platform::event_driven_mouse();
            status.mouse_position_valid = crate::platform::mouse_position_valid();
            status.style_reapply_count = self.test_style_reapply_count;
            status.last_repaint_ms = self.test_last_repaint_ms;
            status.animation_repaint_ms = self.test_animation_repaint_ms;
            status.repaint_fast = self.test_repaint_fast;
            status.repaint_medium = self.test_repaint_medium;
            status.repaint_slow = self.test_repaint_slow;
            status.hooks_port = hooks_port;
        }
    }

    /// Apply state events pushed by hooks.
    fn poll_state_events(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.state_events else {
            return;
        };
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        if events.is_empty() {
            return;
        }
        for event in events {
            #[cfg(feature = "test-hooks")]
            {
                self.test_state_event_count = self.test_state_event_count.wrapping_add(1);
            }
            let Some(state) = event.pet_state() else {
                continue;
            };
            tracing::info!(source = %event.source, state = state.name(), "state event");
            let message = event.message_clipped();
            if let Some(text) = &message {
                self.show_bubble(text.clone());
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
            ctx.request_repaint();
        }
        self.publish_health();
    }

    fn active_pet_id(&self) -> String {
        self.pet
            .as_ref()
            .map(|pet| pet.entry.id.clone())
            .unwrap_or_default()
    }

    /// Re-scan the local library and the linked Codex / UniPet roots.
    fn refresh_pets(&mut self) {
        self.pets = self.library.list();
        self.pet_preview = None;
        #[cfg(target_os = "macos")]
        self.refresh_native_tray_menu();
        self.publish_health();
    }

    /// Icon of the active pet (its idle pose), built once per pet and reused by
    /// the settings window, the menu and the tray.
    fn pet_icon(&mut self) -> Option<std::sync::Arc<egui::IconData>> {
        const SIZE: u32 = 128;
        let id = self.pet.as_ref()?.entry.id.clone();
        if let Some((cached, icon)) = &self.pet_icon {
            if cached == &id {
                return Some(std::sync::Arc::clone(icon));
            }
        }
        let rgba = {
            let pet = self.pet.as_ref()?;
            let sprite = pet.current_sprite_index();
            pet.atlas.icon_rgba(sprite, SIZE)?
        };
        let icon = std::sync::Arc::new(egui::IconData {
            rgba,
            width: SIZE,
            height: SIZE,
        });
        self.pet_icon = Some((id, std::sync::Arc::clone(&icon)));
        Some(icon)
    }

    /// Redraw the tray icon from the active pet.
    fn refresh_tray_icon(&mut self) {
        const SIZE: u32 = 64;
        let Some(tray) = &self.tray else {
            return;
        };
        let Some(pet) = self.pet.as_ref() else {
            return;
        };
        let sprite = pet.current_sprite_index();
        let Some(rgba) = pet.atlas.icon_rgba(sprite, SIZE) else {
            return;
        };
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, SIZE, SIZE) {
            let _ = tray.set_icon(Some(icon));
        }
    }

    /// Import a pet folder or `.zip` into the app-local library.
    ///
    /// Refuses invalid packages (the library validates manifest, geometry,
    /// decode and path safety first) and asks for confirmation before
    /// overwriting an existing id.
    fn import_path(&mut self, path: &Path, overwrite: bool) {
        if !path.exists() {
            self.status = format!(
                "找不到 {}：可以把宠物文件夹或 .zip 拖到窗口里",
                path.display()
            );
            return;
        }
        let is_zip = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
        if !path.is_dir() && !is_zip {
            self.status = "只支持宠物文件夹或 .zip 压缩包".to_string();
            return;
        }
        let result = if path.is_dir() {
            self.library.import_dir(path, overwrite)
        } else {
            self.library.import_zip(path, overwrite)
        };
        match result {
            Ok(entry) => {
                tracing::info!(pet = %entry.id, "imported pet");
                self.pending_overwrite = None;
                self.selected_pet = Some(entry.id.clone());
                self.refresh_pets();
                self.switch_pet(&entry.id);
                self.status = format!("已导入并切换到 {}（本地库）", entry.display_name);
            }
            Err(error) => {
                let text = format!("{error:#}");
                if text.contains("already exists") {
                    self.pending_overwrite = Some(path.to_path_buf());
                    self.status = format!("本地库已有同名宠物，点「覆盖导入」确认覆盖：{text}");
                } else {
                    self.pending_overwrite = None;
                    self.status = format!("导入失败：{text}");
                }
            }
        }
    }

    /// Take pet packages dropped onto a window (folder or `.zip`).
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            // A single drop of several files keeps the first pet's id; the
            // remaining ones import on their own because the ids differ.
            let overwrite = self.pending_overwrite.as_deref() == Some(path.as_path());
            self.import_path(&path, overwrite);
        }
    }

    /// True while the user is dragging files over the window.
    fn files_are_hovering(ctx: &egui::Context) -> bool {
        ctx.input(|input| !input.raw.hovered_files.is_empty())
    }

    /// Write the Codex upload format next to the config directory.
    fn export_pet_zip(&mut self, id: &str) {
        let exports = self.paths.config_dir.join("exports");
        #[cfg(target_os = "macos")]
        let (out, reveal_dir) = {
            let Some(out) = crate::platform::choose_pet_export_path(&exports, &format!("{id}.zip"))
            else {
                return;
            };
            let reveal_dir = out
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| exports.clone());
            (out, reveal_dir)
        };
        #[cfg(not(target_os = "macos"))]
        let (out, reveal_dir) = (exports.join(format!("{id}.zip")), exports.clone());
        match self.library.export_zip(id, &out) {
            Ok(()) => {
                self.status = format!("已导出 {}", out.display());
                if crate::platform::open_in_file_manager(&reveal_dir) {
                    self.status.push_str("（已打开导出目录）");
                }
            }
            Err(error) => self.status = format!("导出失败：{error:#}"),
        }
    }

    /// Delete a pet from the app-local library (linked Codex pets are never
    /// touched). Requires the confirmation stored in `pending_delete`.
    fn remove_local_pet(&mut self, id: &str) {
        match self.library.remove_local(id) {
            Ok(()) => {
                self.pending_delete = None;
                if id == petsona_core::pet::DEFAULT_PET_ID {
                    // Do not resurrect a pet the user deleted on purpose.
                    self.config.bundled_pet_removed = true;
                    let _ = self.config.save(&self.paths.config_file);
                }
                if self.active_pet_id() == id {
                    self.refresh_pets();
                    if let Some(next) = self.pets.first().map(|pet| pet.id.clone()) {
                        self.switch_pet(&next);
                    } else {
                        self.pet = None;
                    }
                } else {
                    self.refresh_pets();
                }
                self.status = format!("已从本地库删除 {id}");
            }
            Err(error) => self.status = format!("删除失败：{error:#}"),
        }
    }

    fn active_pet_name(&self) -> String {
        self.pet
            .as_ref()
            .map(|pet| pet.entry.display_name.clone())
            .unwrap_or_else(|| "（无）".to_string())
    }

    #[cfg(target_os = "macos")]
    fn fill_native_pet_menu(
        pet_menu: &tray_icon::menu::Submenu,
        pets: &[PetEntry],
        active_pet: &str,
    ) -> bool {
        use tray_icon::menu::CheckMenuItem;

        for item in pet_menu.items() {
            let Some(item) = item.as_check_menuitem() else {
                return false;
            };
            if pet_menu.remove(item).is_err() {
                return false;
            }
        }
        pet_menu.set_enabled(!pets.is_empty());
        for pet in pets {
            let mut label = pet.display_name.clone();
            if pet.sprite_version_number == Some(2) || pet.frame.rows >= 11 {
                label.push_str("  ·  V2");
            }
            let item = CheckMenuItem::with_id(
                format!("{NATIVE_MENU_SELECT_PET_PREFIX}{}", pet.id),
                label,
                true,
                pet.id == active_pet,
                None,
            );
            if pet_menu.append(&item).is_err() {
                return false;
            }
        }
        true
    }

    #[cfg(target_os = "macos")]
    fn build_native_menu(&self) -> Option<NativeMenu> {
        use tray_icon::menu::{IsMenuItem, Menu, MenuItem, Submenu};

        let menu = Menu::new();
        let open_settings = MenuItem::with_id(NATIVE_MENU_OPEN_SETTINGS_ID, "打开设置", true, None);
        let pets = Submenu::with_id("petsona.select-pet", "选择宠物", !self.pets.is_empty());
        if !Self::fill_native_pet_menu(&pets, &self.pets, &self.active_pet_id()) {
            return None;
        }
        let toggle_pet = MenuItem::with_id(
            NATIVE_MENU_TOGGLE_PET_ID,
            if self.pet_visible {
                "隐藏宠物"
            } else {
                "显示宠物"
            },
            true,
            None,
        );
        let quit = MenuItem::with_id(NATIVE_MENU_QUIT_ID, "退出", true, None);

        let items: [&dyn IsMenuItem; 4] = [&open_settings, &pets, &toggle_pet, &quit];
        for item in items {
            if let Err(error) = menu.append(item) {
                tracing::warn!(%error, "cannot build native macOS menu");
                return None;
            }
        }

        Some(NativeMenu {
            menu,
            pet_menu: pets,
            toggle_pet,
        })
    }

    #[cfg(target_os = "macos")]
    fn refresh_native_tray_menu(&self) {
        let Some(native_menu) = &self.native_tray_menu else {
            return;
        };
        native_menu.toggle_pet.set_text(if self.pet_visible {
            "隐藏宠物"
        } else {
            "显示宠物"
        });
        if !Self::fill_native_pet_menu(&native_menu.pet_menu, &self.pets, &self.active_pet_id()) {
            tracing::warn!("cannot refresh native macOS pet menu");
        }
    }

    #[cfg(target_os = "windows")]
    fn poll_windows_menu(&mut self, ctx: &egui::Context) {
        let commands = self
            .windows_menu
            .as_ref()
            .map(|menu| menu.poll_commands().collect::<Vec<_>>())
            .unwrap_or_default();
        for command in commands {
            match command {
                crate::windows_menu::COMMAND_OPEN_SETTINGS
                | crate::windows_menu::COMMAND_CHANGE_PET => {
                    self.open_settings();
                }
                crate::windows_menu::COMMAND_TOGGLE_PET => {
                    self.set_pet_visible(ctx, !self.pet_visible);
                }
                crate::windows_menu::COMMAND_QUIT => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }
    }

    /// Load another pet from the library and swap it in without restarting.
    fn switch_pet(&mut self, id: &str) {
        let Some(entry) = self.pets.iter().find(|pet| pet.id == id).cloned() else {
            self.status = format!("找不到宠物 {id}");
            return;
        };
        match PetRuntime::load(entry) {
            Ok(runtime) => {
                tracing::info!(pet = %id, "switching pet");
                self.pet = Some(runtime);
                self.config.active_pet = Some(id.to_string());
                self.selected_pet = Some(id.to_string());
                self.pet_preview = None;
                self.pet_icon = None;
                self.refresh_tray_icon();
                #[cfg(target_os = "macos")]
                self.refresh_native_tray_menu();
                self.walk_until = None;
                self.walk_origin_x = None;
                self.walk_position_x = None;
                if let Err(error) = self.config.save(&self.paths.config_file) {
                    self.status = format!("已切换，但保存配置失败：{error}");
                } else {
                    self.status = format!("已切换到 {id}");
                }
                let _ = self.memory.record_event(
                    &self.persona.id,
                    EventKind::PetChanged,
                    Some(id.to_string()),
                );
            }
            Err(error) => self.status = format!("切换宠物失败：{error:#}"),
        }
    }

    fn install_tray(&mut self, ctx: egui::Context) {
        // Windows keeps the custom egui menu because TrackPopupMenu enters a
        // modal loop on the event-loop thread. macOS uses AppKit's native menu
        // so the status-item menu is positioned and dismissed by the system.
        #[cfg(target_os = "macos")]
        let native_tray_menu = self.build_native_menu();

        let mut builder = tray_icon::TrayIconBuilder::new()
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(false)
            .with_tooltip("Petsona");
        #[cfg(target_os = "macos")]
        if let Some(native_menu) = native_tray_menu.as_ref() {
            builder = builder
                .with_menu(Box::new(native_menu.menu.clone()))
                .with_menu_on_left_click(true)
                .with_menu_on_right_click(true);
        }
        tracing::info!("creating tray icon");
        if let Ok(icon) = tray_icon::Icon::from_rgba(tray_icon_rgba(), 32, 32) {
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(tray) => {
                self.tray = Some(tray);
                tracing::info!("tray icon created");
                self.refresh_tray_icon();
            }
            Err(error) => tracing::warn!(%error, "cannot create tray icon"),
        }

        // Click events arrive on the message thread, so hand them to the UI
        // through a channel we own and wake the event loop.
        let (sender, receiver) = mpsc::channel();
        let tray_ctx = ctx.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(
            move |event: tray_icon::TrayIconEvent| {
                let _ = sender.send(event);
                tray_ctx.request_repaint();
            },
        ));
        self.tray_events = Some(receiver);

        #[cfg(target_os = "macos")]
        {
            self.native_tray_menu = native_tray_menu;
            let (sender, receiver) = mpsc::channel();
            let menu_ctx = ctx.clone();
            tray_icon::menu::MenuEvent::set_event_handler(Some(
                move |event: tray_icon::menu::MenuEvent| {
                    let _ = sender.send(event);
                    menu_ctx.request_repaint();
                },
            ));
            self.native_menu_events = Some(receiver);
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "windows")]
        self.poll_windows_menu(ctx);

        #[cfg(target_os = "macos")]
        if self.native_tray_menu.is_some() {
            // AppKit already opened and positioned the status-item menu.
            if let Some(receiver) = &self.tray_events {
                while receiver.try_recv().is_ok() {}
            }
            return;
        }

        let Some(receiver) = &self.tray_events else {
            return;
        };
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        let scale = ctx
            .input(|input| input.viewport().native_pixels_per_point)
            .unwrap_or(1.0);
        for event in events {
            let tray_icon::TrayIconEvent::Click {
                button_state,
                position,
                ..
            } = event
            else {
                continue;
            };
            if button_state != tray_icon::MouseButtonState::Down {
                continue;
            }
            tracing::info!(?position, "tray click");
            #[cfg(target_os = "windows")]
            if let Some(menu) = &self.windows_menu {
                menu.show(
                    position.x.round() as i32,
                    position.y.round() as i32,
                    self.pet_visible,
                );
                continue;
            }
            self.open_menu_at(egui::pos2(
                position.x as f32 / scale,
                position.y as f32 / scale,
            ));
            ctx.request_repaint();
        }
    }

    #[cfg(target_os = "macos")]
    fn poll_native_menu(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.native_menu_events else {
            return;
        };
        let mut ids = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            ids.push(event.id);
        }

        for id in ids {
            let id = id.as_ref();
            if let Some(pet_id) = id.strip_prefix(NATIVE_MENU_SELECT_PET_PREFIX) {
                self.switch_pet(pet_id);
                continue;
            }
            match id {
                NATIVE_MENU_OPEN_SETTINGS_ID => {
                    self.open_settings();
                }
                NATIVE_MENU_TOGGLE_PET_ID => {
                    self.set_pet_visible(ctx, !self.pet_visible);
                }
                NATIVE_MENU_QUIT_ID => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }
    }

    /// Open the shared popup menu with its top-left at `anchor`.
    fn open_menu_at(&mut self, anchor: egui::Pos2) {
        self.menu_open = true;
        self.menu_created = false;
        self.menu_styled = false;
        self.menu_button_was_down = false;
        self.menu_right_button_was_down = true;
        self.menu_anchor = Some(anchor);
    }

    fn set_pet_visible(&mut self, ctx: &egui::Context, visible: bool) {
        tracing::info!(visible, "set pet visible");
        self.pet_visible = visible;
        #[cfg(target_os = "macos")]
        self.refresh_native_tray_menu();
        self.walk_position_x = None;
        self.last_passthrough = None;
        self.press_origin = None;
        self.pet_dragged = false;
        self.drag_grab = None;
        ctx.request_repaint();
    }

    fn set_walk_state(&mut self, state: PetState) {
        if let Some(pet) = &mut self.pet {
            if pet.engine.set_base(state) {
                pet.anim_started = Instant::now();
                pet.last_state = pet.engine.current();
            }
        }
    }

    fn update_auto_walk(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.settings_open || !self.pet_visible || !self.config.window.auto_walk.enabled {
            self.walk_position_x = None;
            return;
        }
        // The user is holding the pet; the reminder can wait.
        if self.pet_dragged {
            self.walk_position_x = None;
            return;
        }
        // A hook state, greeting or click animation owns the pet; the walk
        // reminder waits until the pet is back to its base animation.
        if self
            .pet
            .as_ref()
            .is_some_and(|pet| pet.engine.current() != pet.engine.base())
        {
            self.walk_position_x = None;
            return;
        }
        let cfg = self.config.window.auto_walk.clone();
        if self.last_user_action.elapsed()
            < Duration::from_secs_f32(cfg.user_grace_seconds.max(0.0))
        {
            return;
        }
        let Some(window) = frame.winit_window() else {
            return;
        };
        let now = Instant::now();
        let dt = (now - self.last_walk_tick).as_secs_f32().clamp(0.0, 0.1);
        self.last_walk_tick = now;

        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor() as f32;
        let current_x = position.x as f32 / scale;

        if self.walk_until.is_none() {
            if now < self.next_walk_at {
                self.set_walk_state(PetState::Idle);
                return;
            }
            self.walk_until = Some(now + Duration::from_secs_f32(cfg.walk_seconds.max(1.0)));
            self.walk_origin_x = Some(current_x);
            self.walk_position_x = Some(current_x);
            self.show_bubble("坐久了，起来活动一下吧。".to_string());
        }

        let Some(until) = self.walk_until else {
            return;
        };
        if now >= until {
            self.walk_until = None;
            self.walk_origin_x = None;
            self.walk_position_x = None;
            self.next_walk_at = now + Duration::from_secs(cfg.interval_minutes.max(1) as u64 * 60);
            self.set_walk_state(PetState::Idle);
            return;
        }

        let origin = self.walk_origin_x.unwrap_or(current_x);
        let current_x = self.walk_position_x.unwrap_or(current_x);
        let half_range = cfg.range_px.max(20.0) * 0.5;
        let min_x = origin - half_range;
        let max_x = origin + half_range;
        let step = cfg.speed_px_s.max(1.0) * dt * self.walk_direction;
        let mut next_x = current_x + step;
        if next_x <= min_x {
            next_x = min_x;
            self.walk_direction = 1.0;
        } else if next_x >= max_x {
            next_x = max_x;
            self.walk_direction = -1.0;
        }
        #[cfg(feature = "test-hooks")]
        tracing::debug!(
            current_x,
            next_x,
            physical_x = position.x,
            scale,
            "test auto-walk move"
        );
        self.walk_position_x = Some(next_x);
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            next_x,
            position.y as f32 / scale,
        )));
        self.set_walk_state(if self.walk_direction >= 0.0 {
            PetState::RunningRight
        } else {
            PetState::RunningLeft
        });
    }

    /// Expire timed states (click wave, greeting, hook TTLs) so the pet always
    /// returns to its base animation.
    fn update_pet_timers(&mut self) {
        let now = Instant::now();
        let Some(pet) = &mut self.pet else {
            return;
        };
        if pet.engine.tick(now).is_some() {
            pet.anim_started = now;
            pet.last_state = pet.engine.current();
        }
    }

    /// Play the locomotion row that matches how the pet is being carried.
    ///
    /// The state is raised with a short TTL and refreshed while the window
    /// keeps moving, so it always falls back to the base animation on its own -
    /// no "drag ended" bookkeeping can get stuck.
    fn raise_motion(&mut self, state: PetState) {
        const MOTION_TTL: Duration = Duration::from_millis(300);
        let now = Instant::now();
        let Some(pet) = &mut self.pet else {
            return;
        };
        let already_running =
            pet.engine.current() == state && pet.engine.source() == Some("motion");
        let raised = pet
            .engine
            .raise(state, "motion", None, Some(MOTION_TTL), now)
            .is_some();
        if raised && !already_running {
            pet.anim_started = now;
            pet.last_state = state;
        }
    }

    /// Move the pet with the cursor ourselves.
    ///
    /// Handing the drag to the OS (`ViewportCommand::StartDrag`) enters a modal
    /// move loop: it blocks the event loop for the whole gesture, so the pet
    /// cannot animate, and asking for it again on every frame re-entered that
    /// loop and flashed the window frame. Instead the window is positioned from
    /// the cursor each frame, which also lets the locomotion row play.
    fn drag_pet(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            return;
        };
        let button_down = self
            .pointer
            .primary_down
            .unwrap_or_else(|| ctx.input(|input| input.pointer.any_down()));
        if self.pet_dragged && !button_down {
            self.pet_dragged = false;
            self.drag_grab = None;
        }

        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor().max(0.1);
        let current = egui::vec2(
            position.x as f32 / scale as f32,
            position.y as f32 / scale as f32,
        );
        let previous = self.last_window_pos.replace(current);

        if self.pet_dragged {
            let cursor = self.pointer.position;
            if self.drag_grab.is_none() {
                if let Some((cursor_x, cursor_y)) = cursor {
                    self.drag_grab = Some(egui::vec2(
                        cursor_x as f32 / scale as f32 - current.x,
                        cursor_y as f32 / scale as f32 - current.y,
                    ));
                }
            }
            if let (Some(grab), Some((cursor_x, cursor_y))) = (self.drag_grab, cursor) {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    cursor_x as f32 / scale as f32 - grab.x,
                    cursor_y as f32 / scale as f32 - grab.y,
                )));
            }
        }

        let dx = if self.pet_dragged {
            previous.map_or(0.0, |previous| current.x - previous.x)
        } else {
            0.0
        };
        if dx.abs() >= 0.5 {
            let state = if dx > 0.0 {
                PetState::RunningRight
            } else {
                PetState::RunningLeft
            };
            self.raise_motion(state);
        }
    }

    fn cancel_glance(&mut self) {
        self.glance_side = 0;
        if let Some(pet) = &mut self.pet {
            pet.engine.cancel_gaze();
        }
    }

    fn release_glance(&mut self, now: Instant) {
        self.glance_side = 0;
        if let Some(pet) = &mut self.pet {
            let visible = pet.engine.gaze_visible();
            let released = pet.engine.release_gaze(now);
            if released && visible {
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
            }
        }
    }

    #[cfg(feature = "test-hooks")]
    fn update_test_glance(&mut self, side: i8) {
        if side == 0 {
            self.release_glance(Instant::now());
            return;
        }

        let existing_direction = self
            .pet
            .as_ref()
            .and_then(|pet| pet.engine.gaze_direction());
        if existing_direction == Some(side) {
            self.glance_side = side;
            return;
        }
        if existing_direction.is_some() {
            self.release_glance(Instant::now());
            return;
        }
        if self
            .last_glance_at
            .is_some_and(|last| last.elapsed() < Duration::from_millis(900))
        {
            self.glance_side = 0;
            return;
        }

        let now = Instant::now();
        let dx = if side < 0 { -1.0 } else { 1.0 };
        let raised = self
            .pet
            .as_mut()
            .is_some_and(|pet| pet.engine.glance(dx, now).is_some());
        if raised {
            self.glance_side = side;
            self.last_glance_at = Some(now);
            if let Some(pet) = &mut self.pet {
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
            }
        }
    }

    /// Let the V2 look rows follow the cursor. The turn stops on the strongest
    /// side-facing frame while the cursor remains in the trigger region; once
    /// it leaves, the row's return segment plays back to the base pose.
    fn update_glance(&mut self, frame: &eframe::Frame) {
        if self.settings_open || !self.pet_visible || self.pet_dragged {
            self.cancel_glance();
            return;
        }
        #[cfg(feature = "test-hooks")]
        if let Some(side) = self.test_glance_side {
            self.update_test_glance(side);
            return;
        }
        let Some(pet) = &self.pet else {
            return;
        };
        if pet.engine.base().is_locomotion() || pet.engine.animation(PetState::LookRow9).is_none() {
            self.cancel_glance();
            return;
        }
        let Some(window) = frame.winit_window() else {
            return;
        };
        let (Some((cursor_x, cursor_y)), Ok(position), size) = (
            self.pointer.position,
            window.outer_position(),
            window.outer_size(),
        ) else {
            return;
        };
        let scale = window.scale_factor().max(0.1);
        let cursor_x = cursor_x / scale;
        let cursor_y = cursor_y / scale;
        let pet_size = self.pet_size();
        let window_width = size.width as f64 / scale;
        let window_height = size.height as f64 / scale;
        let pet_left =
            position.x as f64 / scale + (window_width - pet_size.x as f64).max(0.0) * 0.5;
        let pet_top = position.y as f64 / scale + (window_height - pet_size.y as f64).max(0.0);
        let dx = cursor_x - (pet_left + pet_size.x as f64 * 0.5);
        let dy = cursor_y - (pet_top + pet_size.y as f64 * 0.5);
        // Only glance when the cursor is actually near the pet.
        let reach = pet_size.x.max(pet_size.y) as f64 * 2.5;
        if dx.abs() > reach || dy.abs() > reach {
            self.release_glance(Instant::now());
            return;
        }
        let dead_zone = pet_size.x as f64 * 0.35;
        let side = if dx > dead_zone {
            1
        } else if dx < -dead_zone {
            -1
        } else {
            0
        };
        if side == 0 {
            self.release_glance(Instant::now());
            return;
        }

        let existing_direction = self
            .pet
            .as_ref()
            .and_then(|pet| pet.engine.gaze_direction());
        if existing_direction == Some(side) {
            self.glance_side = side;
            return;
        }
        if existing_direction.is_some() {
            // If the cursor crosses directly from one trigger region to the
            // other, finish the old return segment before starting a new turn.
            self.release_glance(Instant::now());
            return;
        }
        if self
            .last_glance_at
            .is_some_and(|last| last.elapsed() < Duration::from_millis(900))
        {
            self.glance_side = 0;
            return;
        }
        let now = Instant::now();
        let raised = self
            .pet
            .as_mut()
            .is_some_and(|pet| pet.engine.glance(dx as f32, now).is_some());
        if raised {
            if let Some(pet) = &mut self.pet {
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
            }
            self.glance_side = side;
            self.last_glance_at = Some(now);
        }
    }

    /// Rectangle of the scaled pet sprite inside the window.
    fn pet_rect(&self, window_size: egui::Vec2) -> egui::Rect {
        let size = self.pet_size();
        egui::Rect::from_min_size(
            egui::pos2(
                (window_size.x - size.x).max(0.0) * 0.5,
                (window_size.y - size.y).max(0.0),
            ),
            size,
        )
    }

    /// Is the cursor on a drawn pixel of the pet?
    ///
    /// The window is permanently click-through, so this - plus the Win32 button
    /// state - is what decides whether a press belongs to the pet.
    fn cursor_over_pet(&self, window_size: egui::Vec2, local: egui::Pos2) -> bool {
        let Some(pet) = &self.pet else {
            return false;
        };
        let rect = self.pet_rect(window_size);
        if !rect.contains(local) {
            return false;
        }
        // With the "pixel-level click-through" option off, the whole sprite
        // rectangle reacts; with it on, only drawn pixels do.
        if !self.config.window.click_through {
            return true;
        }
        let scale = self.config.window.scale.clamp(0.5, 3.0);
        pet.atlas.mask.opaque_at_cell_dilated(
            pet.last_sprite,
            (local.x - rect.min.x) / scale,
            (local.y - rect.min.y) / scale,
            1,
        )
    }

    /// Keep the window click-through on exactly the pixels the pet does not
    /// draw, so the desktop below stays usable while the sprite still reacts.
    /// The window carries no frame styles, so changing this style can no longer
    /// make Windows paint a border.
    fn update_passthrough(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.pet_dragged {
            return;
        }
        let ignore = if !self.pet_visible {
            true
        } else if !self.config.window.click_through {
            false
        } else {
            let Some(window) = frame.winit_window() else {
                return;
            };
            let (cursor, position) = (self.pointer.position, window.outer_position());
            let (Some((cursor_x, cursor_y)), Ok(position)) = (cursor, position) else {
                return;
            };
            let scale = window.scale_factor().max(0.1) as f32;
            let local = egui::pos2(
                (cursor_x - position.x as f64) as f32 / scale,
                (cursor_y - position.y as f64) as f32 / scale,
            );
            !self.cursor_over_pet(self.pet_window_size(), local)
        };
        if self.last_passthrough != Some(ignore) {
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(ignore));
            self.last_passthrough = Some(ignore);
        }
    }

    #[cfg(target_os = "macos")]
    fn show_native_pet_menu(&self, window: &winit::window::Window) {
        use tray_icon::menu::ContextMenu as _;

        let Some(native_menu) = self.build_native_menu() else {
            return;
        };
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };

        // Passing None asks AppKit to use the current mouse location in
        // screen coordinates, so it handles menu-bar offsets, Retina scaling,
        // and multi-monitor placement itself.
        unsafe {
            let _ = native_menu
                .menu
                .show_context_menu_for_nsview(handle.ns_view.as_ptr(), None);
        }
    }

    /// Read the cursor and the mouse buttons and turn them into pet input.
    ///
    /// Using the system state instead of window events keeps the per-pixel
    /// click-through exact and never changes a window style at runtime, which
    /// is what used to make Windows flash the frame.
    fn update_pointer(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            return;
        };
        let (left_event_pressed, left_event_released, right_event_pressed, event_position) = ctx
            .input(|input| {
                (
                    input.pointer.primary_pressed(),
                    input.pointer.primary_released(),
                    input.pointer.secondary_pressed(),
                    input.pointer.hover_pos(),
                )
            });
        let left_down = self.pointer.primary_down.unwrap_or(false);
        let right_down = self.pointer.secondary_down.unwrap_or(false);
        let left_pressed =
            button_pressed_edge(left_down, self.pointer_left_down, left_event_pressed);
        let left_released = left_event_released || (!left_down && self.pointer_left_down);
        let right_pressed =
            button_pressed_edge(right_down, self.pointer_right_down, right_event_pressed);
        self.pointer_left_down = left_down;
        self.pointer_right_down = right_down;

        if !self.pet_visible {
            self.press_origin = None;
            return;
        }
        let Some((cursor_x, cursor_y)) = self.pointer.position else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor().max(0.1) as f32;
        let polled_local = egui::pos2(
            (cursor_x - position.x as f64) as f32 / scale,
            (cursor_y - position.y as f64) as f32 / scale,
        );
        let window_size = self.pet_window_size();
        // A trackpad secondary click can be shorter than the 100ms global
        // pointer poll. When egui saw the actual button event, its local
        // position is the authoritative hit-test coordinate (and avoids any
        // Retina/mixed-display conversion drift in the fallback poll).
        let local = match (left_event_pressed || right_event_pressed, event_position) {
            (true, Some(position)) => position,
            _ => polled_local,
        };
        let over_pet = self.cursor_over_pet(window_size, local);
        let now = Instant::now();

        if left_pressed && over_pet {
            self.press_origin = Some(local);
            self.press_started_at = Some(now);
            self.press_moved = false;
        }
        if left_down {
            if let Some(origin) = self.press_origin {
                if (local - origin).length() > CLICK_MOVE_TOLERANCE {
                    self.press_moved = true;
                    self.pet_dragged = true;
                }
            }
        }
        if left_released {
            let clicked = self.press_origin.is_some()
                && !self.press_moved
                && self
                    .press_started_at
                    .is_some_and(|at| at.elapsed() <= CLICK_MAX_HOLD);
            self.press_origin = None;
            self.press_started_at = None;
            self.press_moved = false;
            self.pet_dragged = false;
            self.drag_grab = None;
            if clicked {
                self.register_click();
            }
        }
        if right_pressed && over_pet {
            #[cfg(target_os = "macos")]
            self.show_native_pet_menu(window);
            #[cfg(target_os = "windows")]
            if let Some(menu) = &self.windows_menu {
                menu.show(
                    cursor_x.round() as i32,
                    cursor_y.round() as i32,
                    self.pet_visible,
                );
            } else {
                self.open_menu_at(egui::pos2(cursor_x as f32 / scale, cursor_y as f32 / scale));
            }
            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
            self.open_menu_at(egui::pos2(cursor_x as f32 / scale, cursor_y as f32 / scale));
        }
    }

    /// A press that neither dragged nor out-lasted a click.
    fn register_click(&mut self) {
        let now = Instant::now();
        let is_double = self
            .last_click_at
            .is_some_and(|last| now.duration_since(last) <= Duration::from_millis(320));
        if is_double {
            self.last_click_at = None;
            self.pending_single_click = false;
            self.on_double_click();
        } else {
            self.last_click_at = Some(now);
            self.pending_single_click = true;
        }
    }

    fn on_pet_click(&mut self) {
        self.last_user_action = Instant::now();
        let _ = self
            .memory
            .record_event(&self.persona.id, EventKind::UserClick, None);
        if let Some(pet) = &mut self.pet {
            pet.engine.raise(
                PetState::Waving,
                "click",
                None,
                Some(Duration::from_secs(2)),
                Instant::now(),
            );
            pet.anim_started = Instant::now();
            pet.last_state = pet.engine.current();
        }
        if !self.trigger_greeting("click", true) {
            self.show_bubble(greeting::fallback_greeting(&self.persona));
        }
    }

    fn on_double_click(&mut self) {
        self.last_user_action = Instant::now();
        let _ =
            self.memory
                .record_event(&self.persona.id, EventKind::UserClick, Some("双击".into()));
        self.show_bubble("嘿！".to_string());
        if let Some(pet) = &mut self.pet {
            pet.engine.raise(
                PetState::Jumping,
                "double-click",
                None,
                Some(Duration::from_secs(2)),
                Instant::now(),
            );
            pet.anim_started = Instant::now();
            pet.last_state = pet.engine.current();
        }
    }

    fn trigger_greeting(&mut self, trigger: &str, force: bool) -> bool {
        if !force && !self.config.greeting.enabled {
            return false;
        }
        if !force {
            if let Some(last) = self.last_greeting_at {
                let cooldown =
                    Duration::from_secs(self.config.greeting.cooldown_minutes as u64 * 60);
                if last.elapsed() < cooldown {
                    return false;
                }
            }
        }
        if self.greeting_inflight {
            return false;
        }

        let context = self.memory.build_greeting_context(
            &self.persona.id,
            self.config.memory.recent_events,
            self.config.memory.fact_limit,
        );
        let persona = self.persona.clone();
        let config = self.config.deepseek.clone();
        let trigger = trigger.to_string();
        let now_text = greeting::local_now_text();
        let pet_name = self.pet.as_ref().map(|pet| pet.entry.display_name.clone());
        let pet_state = self
            .pet
            .as_ref()
            .map(|pet| pet.engine.current().name().to_string())
            .unwrap_or_else(|| PetState::Idle.name().to_string());

        let (sender, receiver) = mpsc::channel();
        self.greeting_rx = Some(receiver);
        self.greeting_inflight = true;
        let repaint_context = Arc::clone(&self.repaint_context);
        thread::spawn(move || {
            let result = DeepSeekClient::new(config)
                .and_then(|client| {
                    client.generate_greeting(
                        &persona,
                        &context,
                        &trigger,
                        &now_text,
                        pet_name.as_deref(),
                        &pet_state,
                    )
                })
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
            let ctx = repaint_context
                .lock()
                .ok()
                .and_then(|repaint_context| repaint_context.clone());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        true
    }

    fn poll_greeting(&mut self) {
        let received = self
            .greeting_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());
        match received {
            Some(Ok(result)) => {
                self.greeting_rx = None;
                self.greeting_inflight = false;
                match result {
                    Ok(text) => {
                        self.show_bubble(text.clone());
                        if let Some(pet) = &mut self.pet {
                            pet.engine.raise(
                                PetState::Waving,
                                "greeting",
                                Some(text.clone()),
                                Some(Duration::from_secs(5)),
                                Instant::now(),
                            );
                            pet.anim_started = Instant::now();
                            pet.last_state = pet.engine.current();
                        }
                        let _ = self.memory.mark_greeted(&self.persona.id, "api", &text);
                        self.last_greeting_at = Some(Instant::now());
                    }
                    Err(error) => {
                        self.show_bubble(greeting::fallback_greeting(&self.persona));
                        self.status = error;
                    }
                }
            }
            Some(Err(TryRecvError::Empty)) => {}
            Some(Err(TryRecvError::Disconnected)) => {
                self.greeting_rx = None;
                self.greeting_inflight = false;
            }
            None => {}
        }
    }

    fn show_bubble(&mut self, text: String) {
        self.bubble = Some(Bubble {
            text,
            until: Instant::now() + Duration::from_secs(8),
        });
    }

    /// Schedule only the next state change that can make this viewport stale.
    ///
    /// Input events and background callbacks request an immediate repaint on
    /// their own. The fallback poll keeps global mouse state and the local
    /// state protocol responsive, while animation frames are scheduled at
    /// their actual durations instead of forcing a 60 FPS redraw loop.
    fn schedule_repaint(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let mut after = IDLE_REPAINT;
        let mut sooner = |candidate: Duration| {
            if candidate < after {
                after = candidate;
            }
        };

        if self.pet_dragged || self.walk_until.is_some() || self.menu_open {
            sooner(ACTIVE_REPAINT);
        }

        if self.pet_visible {
            if let Some(pet) = &self.pet {
                let animation_after = pet.next_frame_after();
                #[cfg(feature = "test-hooks")]
                {
                    self.test_animation_repaint_ms = animation_after.as_millis() as u64;
                }
                sooner(animation_after);
            }
        }

        if self.pet_visible && !self.settings_open && !crate::platform::event_driven_mouse() {
            // macOS still needs a low-frequency global pointer poll. Windows
            // uses the low-level mouse hook and wakes only on real input.
            sooner(EVENT_POLL_REPAINT);
        }
        if self.greeting_inflight {
            sooner(EVENT_POLL_REPAINT);
        }

        if self.pending_single_click {
            if let Some(last_click) = self.last_click_at {
                sooner(last_click.checked_add(Duration::from_millis(320)).map_or(
                    Duration::from_millis(1),
                    |deadline| {
                        deadline
                            .saturating_duration_since(now)
                            .max(Duration::from_millis(1))
                    },
                ));
            } else {
                sooner(Duration::from_millis(1));
            }
        }

        if self.pet_visible {
            if let Some(bubble) = &self.bubble {
                sooner(
                    bubble
                        .until
                        .saturating_duration_since(now)
                        .max(Duration::from_millis(1)),
                );
            }
        }

        sooner(
            self.last_health_at
                .checked_add(Duration::from_secs(1))
                .map_or(Duration::from_millis(1), |deadline| {
                    deadline
                        .saturating_duration_since(now)
                        .max(Duration::from_millis(1))
                }),
        );

        #[cfg(feature = "test-hooks")]
        {
            self.test_last_repaint_ms = after.as_millis() as u64;
            if after < Duration::from_millis(50) {
                self.test_repaint_fast = self.test_repaint_fast.wrapping_add(1);
            } else if after < Duration::from_millis(150) {
                self.test_repaint_medium = self.test_repaint_medium.wrapping_add(1);
            } else {
                self.test_repaint_slow = self.test_repaint_slow.wrapping_add(1);
            }
        }
        ctx.request_repaint_after(after);
    }
}

impl PetRuntime {
    /// Sprite that was drawn last, used for the window / tray icon.
    fn current_sprite_index(&self) -> u32 {
        self.last_sprite
    }

    /// Time until the current animation can display a different frame.
    fn next_frame_after(&self) -> Duration {
        let elapsed_ms = self.anim_started.elapsed().as_secs_f32() * 1000.0;
        if let Some(after) = self.engine.gaze_next_frame_after(elapsed_ms) {
            return after;
        }
        let Some(animation) = self.engine.current_animation() else {
            return IDLE_REPAINT;
        };
        if animation.total_ms <= 0.0 || animation.durations_ms.is_empty() {
            return IDLE_REPAINT;
        }

        let time_ms = if animation.loop_anim {
            elapsed_ms % animation.total_ms
        } else {
            elapsed_ms
        };
        if !animation.loop_anim && time_ms >= animation.total_ms {
            return Duration::from_millis(1);
        }

        let mut frame_end = 0.0;
        for duration in &animation.durations_ms {
            frame_end += duration.max(1.0);
            if time_ms < frame_end {
                let remaining_ms = (frame_end - time_ms).clamp(1.0, 60_000.0);
                return Duration::from_millis(remaining_ms.ceil() as u64);
            }
        }
        Duration::from_millis(1)
    }

    fn load(entry: PetEntry) -> Result<Self> {
        let (atlas, warnings) = PetAtlas::open(&entry.dir, &entry.manifest)
            .with_context(|| format!("cannot open pet '{}'", entry.id))?;
        for warning in warnings {
            tracing::warn!(pet = %entry.id, %warning, "pet atlas warning");
        }
        let frame = atlas.frame;
        // `from_atlas` follows the frames this pet actually drew instead of the
        // frame count of the reference sheet.
        let engine = PetEngine::from_atlas(&atlas, &entry.manifest);
        Ok(Self {
            entry,
            atlas,
            engine,
            textures: Vec::new(),
            cell_size: egui::vec2(frame.width as f32, frame.height as f32),
            anim_started: Instant::now(),
            last_state: PetState::Idle,
            last_sprite: 0,
        })
    }

    fn texture_for(&mut self, ctx: &egui::Context, sprite_index: u32) -> Option<egui::TextureId> {
        let index = sprite_index as usize;
        if index >= self.textures.len() {
            self.textures.resize_with(index + 1, || None);
        }
        if self.textures[index].is_none() {
            let frame = self.atlas.frame;
            let row = sprite_index / frame.columns.max(1);
            if row >= frame.rows {
                return None;
            }
            let col = sprite_index % frame.columns.max(1);
            let cell_width = frame.width as usize;
            let cell_height = frame.height as usize;
            let source_x = col * frame.width;
            let source_y = row * frame.height;
            let image = &self.atlas.image;
            let mut pixels = Vec::with_capacity(cell_width * cell_height * 4);
            for y in 0..frame.height {
                for x in 0..frame.width {
                    let pixel = image.get_pixel(source_x + x, source_y + y);
                    pixels.extend_from_slice(&pixel.0);
                }
            }
            let color =
                egui::ColorImage::from_rgba_unmultiplied([cell_width, cell_height], &pixels);
            let texture = ctx.load_texture(
                format!("pet-{}-cell-{}", self.entry.id, sprite_index),
                color,
                egui::TextureOptions::NEAREST,
            );
            self.textures[index] = Some(texture);
        }
        self.textures[index].as_ref().map(|texture| texture.id())
    }

    fn current_sprite(&mut self, elapsed_ms: f32) -> u32 {
        let state = self.engine.current();
        if state != self.last_state {
            self.last_state = state;
            self.anim_started = Instant::now();
        }

        if self.engine.gaze_visible() {
            if let Some(sprite) = self.engine.gaze_sprite_at(elapsed_ms) {
                self.last_sprite = sprite;
                return sprite;
            }
            // The return segment completed. Start the base/event animation
            // from its first frame instead of reusing the gaze elapsed time.
            self.anim_started = Instant::now();
            self.last_state = self.engine.current();
            let sprite = self
                .engine
                .current_animation()
                .and_then(|animation| animation.sprite_at(0.0))
                .unwrap_or(0);
            self.last_sprite = sprite;
            return sprite;
        }

        let (sprite, finished) = {
            let Some(animation) = self.engine.current_animation() else {
                return 0;
            };
            (
                animation.sprite_at(elapsed_ms).unwrap_or(0),
                elapsed_ms >= animation.total_ms && animation.total_ms > 0.0,
            )
        };
        if finished && self.engine.current().is_one_shot() {
            self.engine.on_one_shot_finished();
            self.anim_started = Instant::now();
            self.last_state = self.engine.current();
        }
        self.last_sprite = sprite;
        sprite
    }
}

impl eframe::App for PetsonaApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(feature = "test-hooks")]
        {
            self.test_logic_count = self.test_logic_count.wrapping_add(1);
            self.poll_test_hooks(ctx, frame);
        }
        self.refresh_pointer(ctx);
        self.poll_tray(ctx);
        #[cfg(target_os = "macos")]
        self.poll_native_menu(ctx);
        self.poll_menu(ctx);
        self.poll_state_events(ctx);
        self.poll_greeting();
        self.update_pet_timers();
        self.update_auto_walk(ctx, frame);
        self.drag_pet(ctx, frame);
        self.update_glance(frame);
        self.update_passthrough(ctx, frame);
        self.update_pointer(ctx, frame);
        // `logic` also runs while the pet window is hidden or occluded, so the
        // `GET /health` snapshot stays fresh even when nothing is painted.
        if self.last_health_at.elapsed() >= Duration::from_secs(1) {
            self.publish_health();
            self.last_health_at = Instant::now();
        }
        self.schedule_repaint(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if let Some(window) = _frame.winit_window() {
            // AppKit needs the non-activating panel mask; winit's
            // `with_active(false)` only affects initial creation.
            crate::platform::set_no_activate(window);
        }

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::{CornerPreference, WindowExtWindows as _};
            if let Some(window) = _frame.winit_window() {
                // Apply the chrome exactly once (and again after a resize):
                // touching these attributes every frame made Windows repaint
                // the non-client frame, which is the border that flashed.
                let chrome_due = self
                    .resize_settled_at
                    .is_some_and(|at| at.elapsed() >= Duration::from_millis(200));
                if !self.window_chrome_ready {
                    window.set_undecorated_shadow(false);
                    window.set_border_color(None);
                    window.set_corner_preference(CornerPreference::DoNotRound);
                    let changed = crate::platform::strip_frame_styles(window);
                    if changed {
                        crate::platform::clear_dwm_frame(window);
                        crate::platform::enable_transparency(window);
                    }
                    // Never activate: an activation repaints the frame state and
                    // would also steal focus from the user's editor.
                    crate::platform::set_no_activate(window);
                    #[cfg(feature = "test-hooks")]
                    if changed {
                        self.test_style_reapply_count =
                            self.test_style_reapply_count.wrapping_add(1);
                    }
                    self.window_chrome_ready = true;
                    self.resize_settled_at = None;
                } else if chrome_due {
                    // winit only needs a frame rebuild if Windows actually
                    // restored the decorated style after a resize. Reapplying
                    // DWM state unconditionally was the remaining flash risk.
                    let changed = crate::platform::strip_frame_styles(window);
                    if changed {
                        crate::platform::clear_dwm_frame(window);
                        crate::platform::enable_transparency(window);
                        crate::platform::set_no_activate(window);
                        #[cfg(feature = "test-hooks")]
                        {
                            self.test_style_reapply_count =
                                self.test_style_reapply_count.wrapping_add(1);
                        }
                    }
                    self.resize_settled_at = None;
                }
            }
        }

        if !self.fonts_installed {
            crate::fonts::install_cjk_font(ui.ctx());
            self.fonts_installed = true;
            ui.ctx().request_repaint();
        }
        // Dropping a pet folder or .zip on the pet itself imports it too.
        self.handle_dropped_files(ui.ctx());
        // Keep the window icon in step with the active pet.
        if let Some(pet_id) = self.pet.as_ref().map(|pet| pet.entry.id.clone()) {
            if self.applied_window_icon.as_deref() != Some(pet_id.as_str()) {
                if let Some(icon) = self.pet_icon() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
                    self.applied_window_icon = Some(pet_id);
                }
            }
        }
        if self.pending_single_click {
            if let Some(last_click) = self.last_click_at {
                if last_click.elapsed() >= Duration::from_millis(320) {
                    self.pending_single_click = false;
                    self.last_click_at = None;
                    self.on_pet_click();
                }
            } else {
                self.pending_single_click = false;
            }
        }

        let window_size = self.pet_window_size();
        self.apply_viewport(ui.ctx(), _frame, window_size);
        if self.pet_visible {
            self.draw_pet(ui, window_size);
        }
        self.show_bubble_viewport(ui.ctx(), _frame);

        if self.settings_open {
            self.show_settings_viewport(ui.ctx());
        }
        if self.menu_open {
            self.show_context_menu(ui.ctx());
        }
        #[cfg(feature = "test-hooks")]
        {
            self.test_ui_count = self.test_ui_count.wrapping_add(1);
            self.publish_test_status(_frame);
        }
    }
}

/// A global button poll can miss a short press when no repaint happens during
/// the gesture. The egui event edge is therefore an equally valid press
/// signal, while the polled edge remains the fallback for click-through and
/// drag handling.
fn button_pressed_edge(current_down: bool, previous_down: bool, event_pressed: bool) -> bool {
    event_pressed || (current_down && !previous_down)
}

fn bottom_center_anchor(position: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
    position + egui::vec2(size.x * 0.5, size.y)
}

fn position_for_bottom_center(anchor: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
    anchor - egui::vec2(size.x * 0.5, size.y)
}

/// Keep a popup inside the monitor it was opened on.
fn clamp_to_monitor(ctx: &egui::Context, anchor: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
    let monitor = ctx
        .input(|input| input.viewport().monitor_size)
        .unwrap_or(egui::vec2(1280.0, 800.0));
    egui::pos2(
        anchor.x.clamp(0.0, (monitor.x - size.x).max(0.0)),
        anchor.y.clamp(0.0, (monitor.y - size.y).max(0.0)),
    )
}

/// Speech bubble sized to its text and anchored just above the pet, with a
/// tail pointing at it. The pet window itself remains the exact sprite size.
#[cfg(test)]
fn draw_bubble(painter: &egui::Painter, window: egui::Rect, pet: egui::Rect, text: &str) {
    let galley = painter.layout(
        text.to_owned(),
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
        (window.width() - 32.0).max(96.0),
    );
    let padding = egui::vec2(10.0, 8.0);
    let size = galley.size() + padding * 2.0;
    let tail = BUBBLE_TAIL;
    let max_x = (window.right() - size.x - 6.0).max(window.left() + 6.0);
    let x = (pet.center().x - size.x * 0.5).clamp(window.left() + 6.0, max_x);
    let y = (pet.top() - tail - 8.0 - size.y).max(window.top() + 6.0);
    let rect = egui::Rect::from_min_size(egui::pos2(x, y), size);
    let fill = egui::Color32::from_rgba_unmultiplied(24, 24, 28, 235);
    painter.rect_filled(rect, egui::CornerRadius::same(10), fill);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(10),
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 26),
        ),
        egui::StrokeKind::Inside,
    );
    let tip_x = pet
        .center()
        .x
        .clamp(rect.left() + 14.0, rect.right() - 14.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(tip_x - tail * 0.8, rect.bottom() - 1.0),
            egui::pos2(tip_x + tail * 0.8, rect.bottom() - 1.0),
            egui::pos2(tip_x, rect.bottom() + tail),
        ],
        fill,
        egui::Stroke::NONE,
    ));
    painter.galley(rect.min + padding, galley, egui::Color32::WHITE);
}

/// Draw a bubble in the fixed overlay viewport above the pet.
fn draw_bubble_window(painter: &egui::Painter, window: egui::Rect, text: &str) {
    let galley = painter.layout(
        text.to_owned(),
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
        (window.width() - 32.0).max(96.0),
    );
    let padding = egui::vec2(10.0, 8.0);
    let size = galley.size() + padding * 2.0;
    let tail = BUBBLE_TAIL;
    let max_x = (window.right() - size.x - 6.0).max(window.left() + 6.0);
    let x = (window.center().x - size.x * 0.5).clamp(window.left() + 6.0, max_x);
    let y = bubble_overlay_y(window, size);
    let rect = egui::Rect::from_min_size(egui::pos2(x, y), size);
    let fill = egui::Color32::from_rgba_unmultiplied(24, 24, 28, 235);
    painter.rect_filled(rect, egui::CornerRadius::same(10), fill);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(10),
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 26),
        ),
        egui::StrokeKind::Inside,
    );
    let tip_x = window
        .center()
        .x
        .clamp(rect.left() + 14.0, rect.right() - 14.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(tip_x - tail * 0.8, rect.bottom() - 1.0),
            egui::pos2(tip_x + tail * 0.8, rect.bottom() - 1.0),
            egui::pos2(tip_x, rect.bottom() + tail),
        ],
        fill,
        egui::Stroke::NONE,
    ));
    painter.galley(rect.min + padding, galley, egui::Color32::WHITE);
}

fn bubble_overlay_y(window: egui::Rect, bubble_size: egui::Vec2) -> f32 {
    (window.bottom() - BUBBLE_BOTTOM_PADDING - BUBBLE_TAIL - bubble_size.y).max(window.top() + 6.0)
}

/// Decode the idle frame of a pet into a texture for the settings preview.
fn pet_preview_texture(ctx: &egui::Context, entry: &PetEntry) -> Option<egui::TextureHandle> {
    let (atlas, warnings) = PetAtlas::open(&entry.dir, &entry.manifest).ok()?;
    for warning in warnings {
        tracing::debug!(pet = %entry.id, %warning, "preview atlas warning");
    }
    let frame = atlas.frame;
    let mut pixels = Vec::with_capacity((frame.width * frame.height * 4) as usize);
    for y in 0..frame.height {
        for x in 0..frame.width {
            pixels.extend_from_slice(&atlas.image.get_pixel(x, y).0);
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &pixels,
    );
    Some(ctx.load_texture(
        format!("pet-preview-{}", entry.id),
        image,
        egui::TextureOptions::NEAREST,
    ))
}

fn tray_icon_rgba() -> Vec<u8> {
    const SIZE: i32 = 32;
    let mut pixels = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x - SIZE / 2;
            let dy = y - SIZE / 2;
            let inside = dx * dx + dy * dy <= (SIZE / 2 - 2) * (SIZE / 2 - 2);
            let eye = (11..=13).contains(&x) && (11..=13).contains(&y)
                || (18..=20).contains(&x) && (11..=13).contains(&y);
            let color = if eye {
                [24, 24, 28, 255]
            } else if inside {
                [255, 170, 64, 255]
            } else {
                [0, 0, 0, 0]
            };
            let index = ((y * SIZE + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lay out a bubble in a realistically sized pet window.
    fn bubble_geometry(text: &str, width: f32, height: f32) -> egui::Rect {
        let ctx = egui::Context::default();
        let mut painted = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
            ui.set_clip_rect(window);
            let pet = egui::Rect::from_min_size(
                egui::pos2(width * 0.5 - 96.0, height - 208.0),
                egui::vec2(192.0, 208.0),
            );
            draw_bubble(ui.painter(), window, pet, text);
            painted = window;
        });
        // Nothing uploads the font atlas in a headless pass, so drop it.
        output.textures_delta.clear();
        painted
    }

    #[test]
    fn bubbles_render_for_short_and_long_text() {
        for text in [
            "嗨！",
            "早上好呀，今天也一起加油吧。",
            "这句特别长的问候会被自动换行，并且必须完全待在宠物窗口内部，不允许溢出。",
        ] {
            assert!(bubble_geometry(text, 220.0, 318.0).width() > 0.0);
        }
    }

    #[test]
    fn overlay_bubble_tail_stays_near_the_pet_edge() {
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, BUBBLE_WINDOW_SIZE);
        let bubble_size = egui::vec2(180.0, 40.0);
        let y = bubble_overlay_y(window, bubble_size);
        let tail_tip = y + bubble_size.y + BUBBLE_TAIL;
        assert!((tail_tip - (window.bottom() - BUBBLE_BOTTOM_PADDING)).abs() < 0.01);
    }

    #[test]
    fn menu_position_is_clamped_to_the_monitor() {
        let ctx = egui::Context::default();
        // No monitor info yet: the helper falls back to a 1280x800 desktop.
        let position = clamp_to_monitor(&ctx, egui::pos2(5000.0, 5000.0), egui::vec2(176.0, 126.0));
        assert!(position.x <= 1280.0 - 176.0);
        assert!(position.y <= 800.0 - 126.0);
        let position = clamp_to_monitor(&ctx, egui::pos2(-40.0, -40.0), egui::vec2(176.0, 126.0));
        assert_eq!(position, egui::Pos2::ZERO);
    }

    fn test_app(name: &str) -> PetsonaApp {
        let paths = AppPaths::resolve(std::env::temp_dir().join(format!("petsona-test-{name}")));
        let _ = std::fs::remove_dir_all(&paths.config_dir);
        PetsonaApp::new(paths, AppConfig::default()).expect("app starts")
    }

    fn first_opaque_cell_point(app: &PetsonaApp) -> egui::Vec2 {
        let pet = app.pet.as_ref().expect("bundled pet loads");
        let mask = &pet.atlas.mask;
        for my in 0..mask.mask_height {
            for mx in 0..mask.mask_width {
                let x = (mx * mask.scale + mask.scale / 2) as f32;
                let y = (my * mask.scale + mask.scale / 2) as f32;
                if mask.opaque_at_cell(pet.last_sprite, x, y) {
                    return egui::vec2(x, y);
                }
            }
        }
        panic!("the bundled pet draws something");
    }

    #[test]
    fn clicks_land_on_drawn_pixels_only_at_any_scale() {
        let mut app = test_app("pointer");
        let point = first_opaque_cell_point(&app);

        for scale in [1.0, 1.5, 0.75] {
            app.config.window.scale = scale;
            let window = app.pet_window_size();
            let rect = app.pet_rect(window);
            // The sprite is centered horizontally and pinned to the window bottom.
            assert!((rect.center().x - window.x * 0.5).abs() < 0.5);
            assert!((rect.bottom() - window.y).abs() < 0.5);
            // The corner of a cell is transparent at every scale.
            assert!(!app.cursor_over_pet(window, rect.min + egui::vec2(2.0, 2.0) * scale));

            let hit = rect.min + point * scale;
            assert!(
                app.cursor_over_pet(window, hit),
                "opaque mask point missed at scale {scale}"
            );
        }
    }

    #[test]
    fn resizing_keeps_the_bottom_center_anchor() {
        let old_position = egui::pos2(100.0, 100.0);
        let old_size = egui::vec2(220.0, 318.0);
        let new_size = egui::vec2(268.0, 357.0);
        let moved =
            position_for_bottom_center(bottom_center_anchor(old_position, old_size), new_size);

        let old_anchor = bottom_center_anchor(old_position, old_size);
        let new_anchor = bottom_center_anchor(moved, new_size);
        assert!((old_anchor.x - new_anchor.x).abs() < 0.01);
        assert!((old_anchor.y - new_anchor.y).abs() < 0.01);
    }

    #[test]
    fn double_click_replaces_the_single_click() {
        let mut app = test_app("click");
        app.register_click();
        assert!(app.pending_single_click, "first click waits for a second");
        assert!(app.last_click_at.is_some());
        app.register_click();
        assert!(!app.pending_single_click, "the pair is a double click");
        assert!(app.last_click_at.is_none());
    }

    #[test]
    fn pointer_event_edge_recovers_a_short_press_between_polls() {
        assert!(button_pressed_edge(false, false, true));
        assert!(button_pressed_edge(true, false, false));
        assert!(!button_pressed_edge(false, false, false));
        assert!(!button_pressed_edge(true, true, false));
    }

    #[test]
    fn stage_scale_rounds_up_to_quarter_steps() {
        let mut app = test_app("stage");
        for (scale, expected) in [
            (0.5, 0.5),
            (0.51, 0.75),
            (1.0, 1.0),
            (1.26, 1.5),
            (2.0, 2.0),
        ] {
            app.config.window.scale = scale;
            assert!(
                (app.stage_scale() - expected).abs() < f32::EPSILON,
                "scale {scale} -> {} (expected {expected})",
                app.stage_scale()
            );
        }
        // The sprite keeps following the raw scale while the stage snaps.
        app.config.window.scale = 1.1;
        assert!((app.pet_size().x - app.pet_cell_size().x * 1.1).abs() < 0.01);
        assert!(app.pet_window_size().x >= app.pet_size().x);
    }
}
