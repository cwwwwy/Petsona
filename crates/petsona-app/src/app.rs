use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui;
use petsona_core::config::{AppConfig, AppPaths, WindowPosition};
use petsona_core::deepseek::{save_api_key, DeepSeekClient};
use petsona_core::memory::EventKind;
use petsona_core::pet::state::PetState;
use petsona_core::pet::{PetAtlas, PetEntry};
use petsona_runtime::pet::{PetSession, IDLE_REPAINT};
use petsona_runtime::session::{Bubble, ConversationTurn, PetsonaRuntime};

use crate::platform::{MenuCommand, PhysicalRect, PlatformHost, PlatformMenu, PointerSnapshot};

#[cfg(feature = "test-hooks")]
use crate::test_hooks::{TestActionRequest, TestHookServer, TestStatus};
use petsona_runtime::greeting;

const PET_WINDOW_MIN_WIDTH: f32 = 220.0;
const BUBBLE_TITLE: &str = "Petsona 气泡";
const BUBBLE_WINDOW_SIZE: egui::Vec2 = egui::vec2(360.0, 220.0);
const BUBBLE_GAP: f32 = 8.0;
const BUBBLE_TAIL: f32 = 7.0;
/// Entry animation of a newly shown bubble: the content rises into place while
/// fading in ("from below"), which reads as the bubble growing out of the pet.
const BUBBLE_ENTRY: Duration = Duration::from_millis(160);
const BUBBLE_ENTRY_RISE: f32 = 8.0;
/// Resting padding of the bubble inside its overlay window, plus the room the
/// entry animation needs below the resting place so nothing is clipped.
const BUBBLE_BOTTOM_PADDING: f32 = 2.0 + BUBBLE_ENTRY_RISE;
/// The overlay window is placed this much higher than [`BUBBLE_GAP`] would ask
/// for, so the extra bottom padding above keeps the resting position identical.
const BUBBLE_WINDOW_GAP: f32 = BUBBLE_GAP - BUBBLE_ENTRY_RISE;
const CONVERSATION_TITLE: &str = "Petsona 对话";
const CONVERSATION_WINDOW_SIZE: egui::Vec2 = egui::vec2(440.0, 220.0);
const CONVERSATION_GAP: f32 = 10.0;
/// Entry animation of the conversation window (the input box): it drops in from
/// above while fading in.
const CONVERSATION_ENTRY: Duration = Duration::from_millis(180);
const CONVERSATION_ENTRY_DROP: f32 = 8.0;
const MENU_TITLE: &str = "Petsona 菜单";
const MENU_WIDTH: f32 = 176.0;
const MENU_ROW: f32 = 30.0;
const MENU_PAD: f32 = 6.0;
const MENU_ROWS: f32 = 4.0;
const MENU_SIZE: egui::Vec2 = egui::vec2(MENU_WIDTH, MENU_ROWS * MENU_ROW + MENU_PAD * 2.0);
const ACTIVE_REPAINT: Duration = Duration::from_millis(16);
const EVENT_POLL_REPAINT: Duration = Duration::from_millis(100);
const SCALE_PRESETS: &[(&str, f32)] = &[
    ("迷你", 0.5),
    ("小", 0.75),
    ("标准", 1.0),
    ("大", 1.25),
    ("特大", 1.5),
    ("超大", 2.0),
];

fn nearest_scale(value: f32) -> f32 {
    let value = value.clamp(0.5, 2.0);
    SCALE_PRESETS
        .iter()
        .min_by(|(_, left), (_, right)| {
            (value - *left)
                .abs()
                .partial_cmp(&(value - *right).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, value)| *value)
        .unwrap_or(1.0)
}
/// Identifiers of the tray-icon menu entries. Shells that hand the menu to the
/// operating system (macOS) build it from these.
const NATIVE_MENU_OPEN_SETTINGS_ID: &str = "petsona.open-settings";
const NATIVE_MENU_SELECT_PET_PREFIX: &str = "petsona.select-pet:";
const NATIVE_MENU_SCALE_PREFIX: &str = "petsona.scale:";
const NATIVE_MENU_TOGGLE_PET_ID: &str = "petsona.toggle-pet";
const NATIVE_MENU_QUIT_ID: &str = "petsona.quit";
/// How far the cursor may travel before a press becomes a drag.
const CLICK_MOVE_TOLERANCE: f32 = 4.0;
/// How long a press may last and still count as a click.
const CLICK_MAX_HOLD: Duration = Duration::from_millis(700);

/// Gravity accelerates the pet in physical pixels per second squared; the
/// terminal velocity keeps a long fall from looking like a teleport.
const GRAVITY_PX_S2: f64 = 2600.0;
const GRAVITY_MAX_SPEED_PX_S: f64 = 1800.0;
/// How long the landing animation plays (raised once per touchdown).
const GRAVITY_LANDING_TTL: Duration = Duration::from_millis(1200);
/// A single physics step never covers more than this much time, so a stalled
/// event loop cannot fling the pet through the floor.
const GRAVITY_MAX_STEP_S: f64 = 0.05;

pub struct PetsonaApp {
    /// Native backend supplied by the shell.
    platform: Arc<dyn PlatformHost>,
    runtime: PetsonaRuntime,
    pet_textures: PetTextures,
    bubble_window_created: bool,
    /// The bubble's OS window already exists (created hidden). Windows gets a
    /// show transition when a window is first shown, so the window is created
    /// hidden once and the shell disables that transition before it is ever
    /// visible.
    bubble_window_warmed: bool,
    /// When the current bubble text first appeared; drives the entry animation.
    bubble_shown_at: Option<Instant>,
    bubble_styled: bool,
    conversation_open: bool,
    conversation_window_created: bool,
    /// Same warm-up as the bubble, for the conversation ("input box") window.
    conversation_window_warmed: bool,
    /// When the conversation window became visible; drives its entry animation.
    conversation_shown_at: Option<Instant>,
    conversation_focus_pending: bool,
    conversation_draft: String,
    conversation_cursor: Option<egui::Pos2>,
    settings_open: bool,
    /// A settings command requests a real activation once the viewport exists.
    /// `with_active` only applies when egui creates the child window, while the
    /// same viewport can be reused after it was hidden.
    settings_focus_pending: bool,
    greeting_draft: String,
    api_key_draft: String,
    fonts_installed: bool,
    last_passthrough: Option<bool>,
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
    /// Pet library (the app-local writable folder) and its current contents.
    pet_preview: Option<(String, egui::TextureHandle)>,
    /// Pets found in `~/.codex/pets`, listed by the explicit import panel.
    /// Nothing there is loaded until the user imports it.
    codex_pets: Vec<PetEntry>,
    codex_pets_scanned: bool,
    /// Which side the cursor was on when the pet last glanced (-1/0/1).
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
    start_position_applied: bool,
    tray: Option<tray_icon::TrayIcon>,
    tray_events: Option<Receiver<tray_icon::TrayIconEvent>>,
    /// Native context menu owned by the shell (Win32 popup-menu thread).
    platform_menu: Option<Box<dyn PlatformMenu>>,
    /// `tray-icon` menu used by shells that let the system show it (macOS).
    native_tray_menu: Option<NativeMenu>,
    native_menu_events: Option<Receiver<tray_icon::menu::MenuEvent>>,
    settings_pos: Option<egui::Pos2>,
    /// Local state protocol (Codex hooks -> pet).
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
    pointer: PointerSnapshot,
    last_pointer_refresh: Option<Instant>,
    /// Pet import / export state for the settings window.
    import_draft: String,
    pending_overwrite: Option<PathBuf>,
    pending_delete: Option<String>,
    /// Cached viewport commands. Re-sending them every frame made Windows
    /// redraw the non-client frame, which showed up as a flashing border.
    applied_window_size: Option<egui::Vec2>,
    applied_always_on_top: Option<bool>,
    /// Icon built from the active pet, cached by pet id.
    pet_icon: Option<(String, std::sync::Arc<egui::IconData>)>,
    applied_window_icon: Option<String>,
    /// Login-item state for the settings checkbox. Read from the OS (the
    /// registry on Windows) at startup so an external change is never masked
    /// by a stale config copy.
    autostart_enabled: bool,
    /// Gravity physics, in physical pixels: current downward velocity, whether
    /// the pet is in free fall, whether it is resting on the floor, and the
    /// last physics tick.
    gravity_velocity: f64,
    gravity_falling: bool,
    gravity_grounded: bool,
    last_gravity_tick: Instant,
    #[cfg(feature = "test-hooks")]
    gravity_landings: u64,
}

impl std::ops::Deref for PetsonaApp {
    type Target = PetsonaRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

impl std::ops::DerefMut for PetsonaApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runtime
    }
}
#[derive(Default)]
struct PetTextures {
    textures: Vec<Option<egui::TextureHandle>>,
}

impl PetTextures {
    fn texture_for(
        &mut self,
        ctx: &egui::Context,
        session: &PetSession,
        sprite_index: u32,
    ) -> Option<egui::TextureId> {
        let index = sprite_index as usize;
        if index >= self.textures.len() {
            self.textures.resize_with(index + 1, || None);
        }
        if self.textures[index].is_none() {
            let frame = session.atlas.frame;
            let row = sprite_index / frame.columns.max(1);
            if row >= frame.rows {
                return None;
            }
            let col = sprite_index % frame.columns.max(1);
            let cell_width = frame.width as usize;
            let cell_height = frame.height as usize;
            let source_x = col * frame.width;
            let source_y = row * frame.height;
            let image = &session.atlas.image;
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
                format!("pet-{}-cell-{}", session.entry.id, sprite_index),
                color,
                egui::TextureOptions::NEAREST,
            );
            self.textures[index] = Some(texture);
        }
        self.textures[index].as_ref().map(|texture| texture.id())
    }
}

/// Tray menu handed to AppKit, kept so the check marks can be refreshed.
struct NativeMenu {
    menu: tray_icon::menu::Menu,
    pet_menu: tray_icon::menu::Submenu,
    scale_menu: tray_icon::menu::Submenu,
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
    pub fn new(
        paths: AppPaths,
        mut config: AppConfig,
        platform: Arc<dyn PlatformHost>,
    ) -> Result<Self> {
        config.window.scale = nearest_scale(config.window.scale);
        let mut runtime = PetsonaRuntime::load(paths, config)?;

        let greeting_draft = runtime.persona.greeting.clone().unwrap_or_default();
        let fallback = greeting::fallback_greeting(&runtime.persona);
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

        runtime.bubble = Some(Bubble {
            text: fallback,
            until: Instant::now() + Duration::from_secs(8),
        });

        // The login item lives in the OS, not in the config file, so read it
        // once here and re-read it whenever the settings toggle changes it.
        let autostart_enabled = platform.autostart_enabled();

        let mut app = Self {
            platform,
            runtime,
            pet_textures: PetTextures::default(),
            bubble_window_created: false,
            bubble_window_warmed: false,
            bubble_shown_at: None,
            bubble_styled: false,
            conversation_open: false,
            conversation_window_created: false,
            conversation_window_warmed: false,
            conversation_shown_at: None,
            conversation_focus_pending: false,
            conversation_draft: String::new(),
            conversation_cursor: None,
            settings_open: false,
            settings_focus_pending: false,
            greeting_draft,
            api_key_draft: String::new(),
            fonts_installed: false,
            last_passthrough: None,
            menu_open: false,
            menu_anchor: None,
            menu_window_pos: None,
            menu_created: false,
            menu_styled: false,
            menu_button_was_down: false,
            menu_right_button_was_down: false,
            pet_preview: None,
            codex_pets: Vec::new(),
            codex_pets_scanned: false,
            pet_dragged: false,
            pointer_left_down: false,
            pointer_right_down: false,
            press_origin: None,
            press_started_at: None,
            press_moved: false,
            drag_grab: None,
            last_window_pos: None,
            start_position_applied: false,
            tray: None,
            tray_events: None,
            platform_menu: None,
            native_tray_menu: None,
            native_menu_events: None,
            settings_pos: None,
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
            pointer: PointerSnapshot::default(),
            last_pointer_refresh: None,
            import_draft: String::new(),
            pending_overwrite: None,
            pending_delete: None,
            applied_window_size: None,
            applied_always_on_top: None,
            pet_icon: None,
            applied_window_icon: None,
            autostart_enabled,
            gravity_velocity: 0.0,
            gravity_falling: false,
            gravity_grounded: false,
            last_gravity_tick: Instant::now(),
            #[cfg(feature = "test-hooks")]
            gravity_landings: 0,
        };
        let _ = app.trigger_greeting("startup", true);
        app.sync_state_server();
        Ok(app)
    }

    pub fn initialize(&mut self, creation_context: &eframe::CreationContext<'_>) {
        if let Ok(mut repaint_context) = self.repaint_context.lock() {
            *repaint_context = Some(creation_context.egui_ctx.clone());
        }
        let installed = self
            .platform
            .install_event_waker(&creation_context.egui_ctx);
        tracing::info!(installed, "event-driven mouse waker");
        self.platform_menu = self.platform.create_menu(&creation_context.egui_ctx);
        self.install_tray(creation_context.egui_ctx.clone());
    }

    /// Sampling the global pointer is cheap on Windows (the low-level hook
    /// caches the position) but expensive on macOS, so only shells that ask for
    /// it get edge-triggered sampling.
    fn refresh_pointer(&mut self, ctx: &egui::Context) {
        if self.platform.throttle_pointer_sampling() {
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
        self.pointer = self.platform.pointer_snapshot();
        self.last_pointer_refresh = Some(Instant::now());
    }

    fn pet_size(&self) -> egui::Vec2 {
        self.pet
            .as_ref()
            .map(|pet| egui::vec2(pet.cell_width, pet.cell_height) * self.effective_scale())
            .unwrap_or_else(|| egui::vec2(192.0, 208.0))
    }

    fn effective_scale(&self) -> f32 {
        nearest_scale(self.config.window.scale)
    }

    fn scale_label(&self) -> &'static str {
        SCALE_PRESETS
            .iter()
            .find(|(_, value)| (*value - self.effective_scale()).abs() < f32::EPSILON)
            .map(|(label, _)| *label)
            .unwrap_or("标准")
    }

    fn set_scale_preset(&mut self, value: f32) {
        let snapped = nearest_scale(value);
        if (self.config.window.scale - snapped).abs() < f32::EPSILON {
            return;
        }
        self.config.window.scale = snapped;
        if let Err(error) = self.config.save(&self.paths.config_file) {
            tracing::warn!(%error, "cannot save pet scale preset");
        }
        self.refresh_native_tray_menu();
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
            .map(|pet| egui::vec2(pet.cell_width, pet.cell_height))
            .unwrap_or_else(|| egui::vec2(192.0, 208.0))
    }

    /// Window scale rounded up to the next quarter step.
    fn stage_scale(&self) -> f32 {
        self.effective_scale()
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
            // Physical pixels, as written by `remember_window_position`.
            let restore_position = if !self.start_position_applied {
                self.start_position_applied = true;
                self.config
                    .window
                    .start_position
                    .filter(|position| position.x.is_finite() && position.y.is_finite())
            } else {
                None
            };
            let mut positioned = false;
            if let Some(window) = frame.winit_window() {
                if let Some(saved) = restore_position {
                    // Restore in physical pixels and clamp into the work area
                    // of the monitor the saved origin falls on. If that
                    // monitor is gone (undocked laptop, new resolution), the
                    // nearest visible work area wins instead of reopening
                    // off-screen.
                    if let Some(monitor) = self.saved_monitor(window, saved) {
                        let width = (target.x as f64 * monitor.scale_factor).round() as i32;
                        let height = (target.y as f64 * monitor.scale_factor).round() as i32;
                        let wanted = PhysicalRect::new(
                            saved.x.round() as i32,
                            saved.y.round() as i32,
                            width.max(1),
                            height.max(1),
                        );
                        let rect = clamp_rect_to_work_area(wanted, monitor.work_area);
                        self.platform.set_window_geometry_physical(window, rect);
                        positioned = true;
                    }
                }
                if !positioned {
                    if let Ok(position) = window.outer_position() {
                        let scale = window.scale_factor().max(0.1) as f32;
                        let current =
                            egui::pos2(position.x as f32 / scale, position.y as f32 / scale);
                        let actual_size = window.outer_size();
                        let actual_size = egui::vec2(
                            actual_size.width as f32 / scale,
                            actual_size.height as f32 / scale,
                        );
                        let anchor = bottom_center_anchor(current, actual_size);
                        let next = position_for_bottom_center(anchor, target);
                        self.platform.set_window_geometry(
                            window,
                            next.x as f64,
                            next.y as f64,
                            target.x as f64,
                            target.y as f64,
                        );
                        positioned = true;
                    }
                }
            }
            if !positioned {
                if let Some(position) = restore_position {
                    // Portable fallback: winit only takes logical points, so
                    // convert with the current window scale. The Windows shell
                    // uses `set_window_geometry_physical` above instead.
                    let scale = frame
                        .winit_window()
                        .map(|window| window.scale_factor().max(0.1) as f32)
                        .unwrap_or(1.0);
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                        position.x / scale,
                        position.y / scale,
                    )));
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
            }
            self.applied_window_size = Some(target);
            self.platform.notify_window_resize();
        }
    }

    fn draw_pet(&mut self, root_ui: &mut egui::Ui, window_size: egui::Vec2) {
        if self.pet.is_none() {
            self.draw_missing_pet(root_ui);
            return;
        }

        // Compute the sprite rectangle before borrowing the pet mutably.
        let pet_rect = self.pet_rect(window_size);
        let scale = self.effective_scale();
        let total = window_size;
        let pet = self.runtime.pet.as_mut().expect("checked above");
        let cell = egui::vec2(pet.cell_width, pet.cell_height) * scale;

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
                if let Some(texture_id) = self.pet_textures.texture_for(ui.ctx(), pet, sprite) {
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
            self.bubble_shown_at = None;
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
            pet_top - BUBBLE_WINDOW_SIZE.y - BUBBLE_WINDOW_GAP,
        );
        let bubble_rect_global = egui::Rect::from_min_size(bubble_position, BUBBLE_WINDOW_SIZE);
        let bubble_hovered = self.pointer.position.is_some_and(|(x, y)| {
            bubble_rect_global
                .contains(egui::pos2(x as f32 / scale as f32, y as f32 / scale as f32))
        });
        // First appearance: create the overlay window hidden and let the shell
        // disable the system's show/hide transition for it. Showing it later is
        // then instant and only our own entry animation is visible.
        if !self.bubble_window_warmed {
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
                .with_visible(false);
            ctx.show_viewport_immediate(bubble_id, builder, |_ui, _class| {});
            self.bubble_window_warmed = true;
            self.bubble_styled = self.platform.set_no_activate_for_title(BUBBLE_TITLE) > 0;
            return;
        }

        let shown_at = *self.bubble_shown_at.get_or_insert_with(Instant::now);
        let eased = ease_out(entry_progress(Some(shown_at), BUBBLE_ENTRY));
        let opacity = eased;
        // "From below": the content starts one rise lower and climbs into place.
        let rise = (1.0 - eased) * BUBBLE_ENTRY_RISE;
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
            .with_mouse_passthrough(!bubble_hovered)
            .with_visible(true);
        let mut open_conversation = false;
        ctx.show_viewport_immediate(bubble_id, builder, |ui, _class| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(ui, |ui| {
                    let window = ui.max_rect();
                    let bubble_rect =
                        draw_bubble_window(ui.painter(), window, &text, opacity, rise);
                    if bubble_hovered {
                        let reply_rect = egui::Rect::from_min_size(
                            egui::pos2(bubble_rect.right() - 66.0, bubble_rect.top() + 8.0),
                            egui::vec2(56.0, 26.0),
                        );
                        if ui.put(reply_rect, egui::Button::new("回复")).clicked() {
                            open_conversation = true;
                        }
                    }
                });
        });
        if open_conversation {
            self.open_conversation();
        }
        self.bubble_window_created = true;
        if !self.bubble_styled {
            self.bubble_styled = self.platform.set_no_activate_for_title(BUBBLE_TITLE) > 0;
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
            self.menu_styled = self.platform.set_no_activate_for_title(MENU_TITLE) > 0;
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
        if self.platform.escape_pressed() {
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
            if self.platform.confirm_settings_focus("Petsona 设置") {
                self.settings_focus_pending = false;
            }
        }
    }

    fn open_settings(&mut self) {
        self.settings_open = true;
        self.settings_pos = None;
        self.settings_focus_pending = true;
        // The Codex import list is scanned on demand, not polled.
        self.codex_pets_scanned = false;
    }

    /// List what is available in `~/.codex/pets` for the explicit import panel.
    fn scan_codex_pets(&mut self) {
        self.codex_pets = petsona_core::pet::codex_pets_dir()
            .map(|dir| {
                petsona_core::pet::PetLibrary::scan_dir(&dir, petsona_core::pet::RootKind::Codex)
            })
            .unwrap_or_default();
        self.codex_pets_scanned = true;
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
        let mut codex_imports: Vec<PathBuf> = Vec::new();
        let mut export_id: Option<String> = None;
        let mut delete_id: Option<String> = None;
        ui.collapsing("宠物", |ui| {
            let pets: Vec<(String, String)> = self
                .pets
                .iter()
                .map(|pet| {
                    // Every listed pet lives in the app-local library; Codex
                    // pets are imported explicitly (see the panel below).
                    let mut label = format!(
                        "{}  ·  {}  ({})",
                        pet.display_name,
                        pet.id,
                        pet.root.label()
                    );
                    if pet.id == petsona_core::pet::DEFAULT_PET_ID {
                        label.push_str("  ·  内置");
                    }
                    if pet.sprite_version_number == Some(2) || pet.frame.rows >= 11 {
                        label.push_str("  ·  V2（支持持续注视）");
                    }
                    (pet.id.clone(), label)
                })
                .collect();
            ui.horizontal(|ui| {
                ui.label(format!("当前：{}", self.active_pet_name()));
                // Only the app-local library is listed now; Codex pets are
                // imported explicitly below.
                ui.label(format!("共 {} 个（都在本地库）", pets.len()));
            });
            ui.horizontal(|ui| {
                ui.label("导入");
                ui.add(
                    egui::TextEdit::singleline(&mut self.import_draft)
                        .hint_text("宠物文件夹或 .zip 的路径")
                        .desired_width(240.0),
                );
                if self.platform.supports_native_file_dialogs() && ui.button("选择文件…").clicked()
                {
                    if let Some(path) = self.platform.choose_pet_import_path() {
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
                    if self.platform.open_in_file_manager(&self.paths.pets_dir) {
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
            ui.collapsing("从 Codex 导入", |ui| {
                ui.label(
                    "Petsona 不会自动加载 ~/.codex/pets。这里列出的宠物只有在你点「导入」后\
                     才会复制进本地库。",
                );
                if !self.codex_pets_scanned {
                    self.scan_codex_pets();
                }
                let local_ids: Vec<String> = self.pets.iter().map(|pet| pet.id.clone()).collect();
                let candidates: Vec<(String, String, PathBuf, bool)> = self
                    .codex_pets
                    .iter()
                    .map(|pet| {
                        (
                            pet.display_name.clone(),
                            pet.id.clone(),
                            pet.dir.clone(),
                            local_ids.contains(&pet.id),
                        )
                    })
                    .collect();

                if candidates.is_empty() {
                    ui.label("在 ~/.codex/pets 里没有发现宠物包。");
                } else {
                    ui.label(format!("发现 {} 个：", candidates.len()));
                    for (display_name, id, dir, already_local) in &candidates {
                        ui.horizontal(|ui| {
                            let suffix = if *already_local {
                                "（已在本地库）"
                            } else {
                                ""
                            };
                            ui.label(format!("{display_name}  ·  {id}{suffix}"));
                            if ui.button("导入").clicked() {
                                codex_imports.push(dir.clone());
                            }
                        });
                    }
                    if ui
                        .button("全部导入")
                        .on_hover_text("导入所有还不在本地库里的宠物")
                        .clicked()
                    {
                        for (_, _, dir, already_local) in &candidates {
                            if !*already_local {
                                codex_imports.push(dir.clone());
                            }
                        }
                    }
                }
                if ui.button("重新扫描").clicked() {
                    self.codex_pets_scanned = false;
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
                    for (id, label) in &pets {
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
                                if confirm_delete {
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
        for path in codex_imports {
            self.import_path(&path, false);
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
                ui.label(format!("大小（{}）", self.scale_label()));
                for (label, value) in SCALE_PRESETS {
                    if ui
                        .selectable_label(
                            (self.effective_scale() - *value).abs() < f32::EPSILON,
                            *label,
                        )
                        .clicked()
                    {
                        self.set_scale_preset(*value);
                    }
                }
            });
            ui.checkbox(&mut self.config.window.auto_walk.enabled, "启用活动提醒");
            ui.checkbox(
                &mut self.config.window.gravity_enabled,
                "重力（松手后掉到工作区底部）",
            )
            .on_hover_text(
                "开启后可以把宠物拖到半空松手，它会落到当前显示器工作区底部并播放一次跳跃。\
                 拖动期间和活动提醒行走期间不生效。",
            );
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

        ui.collapsing("启动", |ui| {
            if self.platform.autostart_supported() {
                let mut enabled = self.autostart_enabled;
                if ui
                    .checkbox(&mut enabled, "开机自启动")
                    .on_hover_text("登录后在后台启动 Petsona（Windows 注册表 Run 项）")
                    .changed()
                {
                    match self.platform.set_autostart(enabled) {
                        Ok(()) => {
                            self.autostart_enabled = enabled;
                            self.status = if enabled {
                                "已开启开机自启动".to_string()
                            } else {
                                "已关闭开机自启动".to_string()
                            };
                        }
                        Err(error) => {
                            // Re-read the real OS state so the checkbox can
                            // never show something that is not registered.
                            self.autostart_enabled = self.platform.autostart_enabled();
                            self.status = format!("设置开机自启动失败：{error}");
                        }
                    }
                }
            } else {
                ui.label("当前平台不支持在设置里配置开机自启动（macOS 使用 LaunchAgent 脚本）。");
            }
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
    /// Start, stop or restart the local state protocol to match the config.
    fn sync_state_server(&mut self) {
        let repaint_context = Arc::clone(&self.repaint_context);
        self.runtime.sync_state_server(move || {
            let ctx = repaint_context
                .lock()
                .ok()
                .and_then(|repaint_context| repaint_context.clone());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
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
            // Show/dismiss the shell's own context menu (Win32 popup menu on
            // Windows) so the smoke test can cover it without SendInput.
            "native-menu" => {
                if let Some(menu) = &self.platform_menu {
                    let (x, y) = self.test_native_menu_anchor(frame);
                    menu.show(x, y, self.pet_visible);
                }
            }
            "close-native-menu" => {
                if let Some(menu) = &self.platform_menu {
                    menu.dismiss();
                }
            }
            "open-settings" => self.open_settings(),
            "close-settings" => {
                self.settings_open = false;
                self.settings_pos = None;
            }
            "open-conversation" => self.open_conversation(),
            "close-conversation" => self.close_conversation(),
            "set-conversation-text" => {
                self.conversation_draft = action.text.unwrap_or_default();
            }
            "send-conversation" => self.send_conversation(),
            "hide-pet" => self.set_pet_visible(ctx, false),
            "show-pet" => self.set_pet_visible(ctx, true),
            "toggle-pet" => {
                let visible = !self.pet_visible;
                self.set_pet_visible(ctx, visible);
            }
            "set-scale" => {
                if let Some(value) = action.value {
                    self.set_scale_preset(value as f32);
                }
            }
            "set-window-position" => {
                if let (Some(x), Some(y)) = (action.x, action.y) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                        x as f32, y as f32,
                    )));
                }
            }
            "set-window-position-physical" => {
                if let (Some(x), Some(y)) = (action.x, action.y) {
                    if let Some(window) = frame.winit_window() {
                        let size = window.outer_size();
                        self.platform.set_window_geometry_physical(
                            window,
                            PhysicalRect::new(
                                x.round() as i32,
                                y.round() as i32,
                                size.width.max(1) as i32,
                                size.height.max(1) as i32,
                            ),
                        );
                    }
                }
            }
            "save-window-position" => {
                if let Some(window) = frame.winit_window() {
                    self.remember_window_position(window);
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
            "set-gravity" => {
                if let Some(enabled) = action.enabled {
                    self.config.window.gravity_enabled = enabled;
                    self.gravity_velocity = 0.0;
                    self.gravity_falling = false;
                    self.gravity_grounded = false;
                    self.last_gravity_tick = Instant::now();
                }
            }
            "set-autostart" => {
                if let Some(enabled) = action.enabled {
                    match self.platform.set_autostart(enabled) {
                        Ok(()) => self.autostart_enabled = enabled,
                        Err(error) => {
                            self.autostart_enabled = self.platform.autostart_enabled();
                            tracing::warn!(%error, "test hook could not change autostart");
                        }
                    }
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

    /// Physical screen position at the centre of the pet window, used to pop
    /// the shell's native menu from the smoke test.
    #[cfg(feature = "test-hooks")]
    fn test_native_menu_anchor(&self, frame: &eframe::Frame) -> (f64, f64) {
        frame
            .winit_window()
            .and_then(|window| {
                let position = window.outer_position().ok()?;
                let size = window.outer_size();
                Some((
                    position.x as f64 + size.width as f64 * 0.5,
                    position.y as f64 + size.height as f64 * 0.5,
                ))
            })
            .unwrap_or((100.0, 100.0))
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
        let scale = self.effective_scale();
        let logical_x = pet_rect.min.x + x * scale;
        let logical_y = pet_rect.min.y + y * scale;
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
    fn publish_test_status(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
            status.conversation_open = self.conversation_open;
            status.conversation_inflight = self.conversation_inflight;
            status.conversation_window_created = self.conversation_window_created;
            status.conversation_history_len = self.conversation_history.len();
            status.settings_key_window = self.platform.is_window_key_for_title("Petsona 设置");
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
            status.native_menu_ready =
                self.platform_menu.is_some() || self.native_tray_menu.is_some();
            status.native_menu_checked_pet = self.native_checked_pet_id();
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
            status.cursor_poll_count = self.platform.cursor_poll_count();
            status.mouse_events = self.platform.event_driven_mouse();
            status.mouse_event_count = self.platform.mouse_event_count();
            status.mouse_position_valid = self.platform.mouse_position_valid();
            status.bubble_transitions_disabled = self.platform.popup_transitions_disabled();
            status.style_reapply_count = self.test_style_reapply_count;
            status.last_repaint_ms = self.test_last_repaint_ms;
            status.animation_repaint_ms = self.test_animation_repaint_ms;
            status.repaint_fast = self.test_repaint_fast;
            status.repaint_medium = self.test_repaint_medium;
            status.repaint_slow = self.test_repaint_slow;
            status.repaint_causes = ctx
                .repaint_causes()
                .into_iter()
                .map(|cause| cause.to_string())
                .collect();
            status.autostart_supported = self.platform.autostart_supported();
            status.autostart_enabled = self.autostart_enabled;
            status.gravity_enabled = self.config.window.gravity_enabled;
            status.gravity_falling = self.gravity_falling;
            status.gravity_grounded = self.gravity_grounded;
            status.gravity_velocity = self.gravity_velocity;
            status.gravity_landings = self.gravity_landings;
            let monitors = window
                .map(|window| self.physical_monitors(window))
                .unwrap_or_default();
            status.monitor_count = monitors.len() as u32;
            // A window is "on screen" when it fits inside the work area of the
            // monitor nearest to its centre; that is exactly what the restore
            // path guarantees after a monitor disappears.
            status.window_within_work_area = if monitors.is_empty() {
                true
            } else {
                position
                    .zip(size)
                    .map(|(position, size)| {
                        let rect = PhysicalRect::new(
                            position.x,
                            position.y,
                            size.width as i32,
                            size.height as i32,
                        );
                        monitor_for_rect(&monitors, rect)
                            .map(|monitor| rect_within_work_area(rect, monitor.work_area))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            };
            status.hooks_port = hooks_port;
        }
    }

    /// Apply state events pushed by hooks.
    /// Apply state protocol events and wake the UI when something changed.
    fn poll_state_events(&mut self, ctx: &egui::Context) {
        let processed = self.runtime.poll_state_events();
        if processed == 0 {
            return;
        }
        #[cfg(feature = "test-hooks")]
        {
            self.test_state_event_count =
                self.test_state_event_count.wrapping_add(processed as u64);
        }
        ctx.request_repaint();
    }

    fn active_pet_id(&self) -> String {
        self.pet
            .as_ref()
            .map(|pet| pet.entry.id.clone())
            .unwrap_or_default()
    }

    /// Re-scan the app-local library. Codex pets are only read when the user
    /// imports them from the settings window.
    fn refresh_pets(&mut self) {
        self.pets = self.library.list();
        self.pet_preview = None;
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
        let (out, reveal_dir) = if self.platform.supports_native_file_dialogs() {
            let Some(out) = self
                .platform
                .choose_pet_export_path(&exports, &format!("{id}.zip"))
            else {
                return;
            };
            let reveal_dir = out
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| exports.clone());
            (out, reveal_dir)
        } else {
            (exports.join(format!("{id}.zip")), exports.clone())
        };
        match self.library.export_zip(id, &out) {
            Ok(()) => {
                self.status = format!("已导出 {}", out.display());
                if self.platform.open_in_file_manager(&reveal_dir) {
                    self.status.push_str("（已打开导出目录）");
                }
            }
            Err(error) => self.status = format!("导出失败：{error:#}"),
        }
    }

    /// Delete a pet from the app-local library. Requires the confirmation
    /// stored in `pending_delete`.
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

    /// Fill the "select pet" submenu of the tray-icon menu.
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

    /// Fill the "pet size" submenu of the tray-icon menu.
    fn fill_native_scale_menu(scale_menu: &tray_icon::menu::Submenu, active_scale: f32) -> bool {
        use tray_icon::menu::CheckMenuItem;

        for item in scale_menu.items() {
            let Some(item) = item.as_check_menuitem() else {
                return false;
            };
            if scale_menu.remove(item).is_err() {
                return false;
            }
        }
        for (label, value) in SCALE_PRESETS {
            let item = CheckMenuItem::with_id(
                format!("{NATIVE_MENU_SCALE_PREFIX}{value:.2}"),
                *label,
                true,
                (*value - active_scale).abs() < f32::EPSILON,
                None,
            );
            if scale_menu.append(&item).is_err() {
                return false;
            }
        }
        true
    }

    /// Build the `tray-icon` menu used by shells that let the system show it.
    fn build_native_menu(&self) -> Option<NativeMenu> {
        use tray_icon::menu::{IsMenuItem, Menu, MenuItem, Submenu};

        let menu = Menu::new();
        let open_settings = MenuItem::with_id(NATIVE_MENU_OPEN_SETTINGS_ID, "打开设置", true, None);
        let pets = Submenu::with_id("petsona.select-pet", "选择宠物", !self.pets.is_empty());
        if !Self::fill_native_pet_menu(&pets, &self.pets, &self.active_pet_id()) {
            return None;
        }
        let scale_menu = Submenu::with_id("petsona.scale", "宠物大小", true);
        if !Self::fill_native_scale_menu(&scale_menu, self.effective_scale()) {
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

        let items: [&dyn IsMenuItem; 5] = [&open_settings, &pets, &scale_menu, &toggle_pet, &quit];
        for item in items {
            if let Err(error) = menu.append(item) {
                tracing::warn!(%error, "cannot build native macOS menu");
                return None;
            }
        }

        Some(NativeMenu {
            menu,
            pet_menu: pets,
            scale_menu,
            toggle_pet,
        })
    }

    /// Keep the native tray menu in sync; a no-op when the shell has none.
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
        if !Self::fill_native_scale_menu(&native_menu.scale_menu, self.effective_scale()) {
            tracing::warn!("cannot refresh native macOS scale menu");
        }
    }

    #[cfg(feature = "test-hooks")]
    fn native_checked_pet_id(&self) -> Option<String> {
        let native_menu = self.native_tray_menu.as_ref()?;
        native_menu.pet_menu.items().iter().find_map(|item| {
            let item = item.as_check_menuitem()?;
            if !item.is_checked() {
                return None;
            }
            item.id()
                .as_ref()
                .strip_prefix(NATIVE_MENU_SELECT_PET_PREFIX)
                .map(str::to_string)
        })
    }

    /// Apply the commands selected in the shell's native context menu.
    fn poll_platform_menu(&mut self, ctx: &egui::Context) {
        let commands = self
            .platform_menu
            .as_ref()
            .map(|menu| menu.poll())
            .unwrap_or_default();
        for command in commands {
            match command {
                MenuCommand::OpenSettings | MenuCommand::ChangePet => {
                    self.open_settings();
                }
                MenuCommand::TogglePet => {
                    self.set_pet_visible(ctx, !self.pet_visible);
                }
                MenuCommand::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Load another pet from the library and swap it in without restarting.
    fn switch_pet(&mut self, id: &str) {
        let Some(entry) = self.pets.iter().find(|pet| pet.id == id).cloned() else {
            self.status = format!("找不到宠物 {id}");
            return;
        };
        match PetSession::load(entry) {
            Ok(session) => {
                tracing::info!(pet = %id, "switching pet");
                self.runtime.pet = Some(session);
                self.config.active_pet = Some(id.to_string());
                self.selected_pet = Some(id.to_string());
                self.pet_textures = PetTextures::default();
                self.pet_preview = None;
                self.pet_icon = None;
                self.refresh_tray_icon();
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
        let native_tray_menu = if self.platform.uses_native_tray_menu() {
            self.build_native_menu()
        } else {
            None
        };

        let mut builder = tray_icon::TrayIconBuilder::new()
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(false)
            .with_tooltip("Petsona");
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

        self.native_tray_menu = native_tray_menu;
        if self.native_tray_menu.is_some() {
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
        self.poll_platform_menu(ctx);

        if self.native_tray_menu.is_some() {
            // The system already opened and positioned the status-item menu.
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
            if let Some(menu) = &self.platform_menu {
                menu.show(position.x, position.y, self.pet_visible);
                continue;
            }
            self.open_menu_at(egui::pos2(
                position.x as f32 / scale,
                position.y as f32 / scale,
            ));
            ctx.request_repaint();
        }
    }

    /// Apply the tray-icon menu events on shells that let the system show it.
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
            if let Some(scale) = id.strip_prefix(NATIVE_MENU_SCALE_PREFIX) {
                if let Ok(scale) = scale.parse::<f32>() {
                    self.set_scale_preset(scale);
                }
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
        // Do not start a reminder walk while the pet is still falling: the
        // landing animation owns the state and the physics owns the position.
        if self.config.window.gravity_enabled && !self.gravity_grounded {
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
            self.remember_window_position(window);
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

    /// Monitors as physical rectangles, carrying the shell's real work area
    /// (taskbar excluded on Windows) and winit's scale factor.
    fn physical_monitors(&self, window: &winit::window::Window) -> Vec<PhysicalMonitor> {
        window
            .available_monitors()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                let bounds = PhysicalRect::new(
                    position.x,
                    position.y,
                    size.width as i32,
                    size.height as i32,
                );
                let work_area = self
                    .platform
                    .monitor_work_area(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
                    .unwrap_or(bounds);
                PhysicalMonitor {
                    bounds,
                    work_area,
                    scale_factor: monitor.scale_factor().max(0.1),
                }
            })
            .collect()
    }

    /// The monitor a saved physical origin should be restored onto.
    fn saved_monitor(
        &self,
        window: &winit::window::Window,
        saved: WindowPosition,
    ) -> Option<PhysicalMonitor> {
        let monitors = self.physical_monitors(window);
        monitor_for_point(&monitors, saved.x.round() as i32, saved.y.round() as i32)
    }

    /// Persist the physical outer position after a user drag.
    ///
    /// Physical pixels are what Windows uses for `GetWindowRect`; storing
    /// logical points would drift on mixed-DPI desktops. The saved origin is
    /// clamped into the work area of the monitor it is on, so a pet dragged
    /// mostly off-screen cannot be remembered off-screen. The restore path
    /// clamps again when that monitor no longer exists.
    fn remember_window_position(&mut self, window: &winit::window::Window) {
        let Ok(position) = window.outer_position() else {
            return;
        };
        let size = window.outer_size();
        let rect = PhysicalRect::new(
            position.x,
            position.y,
            size.width.max(1) as i32,
            size.height.max(1) as i32,
        );
        let monitors = self.physical_monitors(window);
        let rect = monitor_for_rect(&monitors, rect)
            .map(|monitor| clamp_rect_to_work_area(rect, monitor.work_area))
            .unwrap_or(rect);
        let saved = WindowPosition {
            x: rect.x as f32,
            y: rect.y as f32,
        };
        if self.config.window.start_position == Some(saved) {
            return;
        }
        self.config.window.start_position = Some(saved);
        if let Err(error) = self.config.save(&self.paths.config_file) {
            tracing::warn!(%error, "cannot save pet window position");
        }
    }

    /// Drop the pet to the bottom of the current monitor's work area.
    ///
    /// Only a real fall plays the landing animation; a pet that is already at
    /// the floor stays put. Dragging and the auto-walk reminder suspend the
    /// physics, so the user always owns the pet while holding it.
    fn update_gravity(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        self.gravity_falling = false;
        if !self.config.window.gravity_enabled || !self.pet_visible {
            self.gravity_velocity = 0.0;
            self.gravity_grounded = false;
            self.last_gravity_tick = Instant::now();
            return;
        }
        if self.pet_dragged || self.walk_until.is_some() {
            // Hold position while the user carries the pet or the reminder
            // walks it sideways; gravity resumes from wherever it was left.
            self.gravity_velocity = 0.0;
            self.last_gravity_tick = Instant::now();
            return;
        }
        let Some(window) = frame.winit_window() else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let size = window.outer_size();
        let rect = PhysicalRect::new(
            position.x,
            position.y,
            size.width.max(1) as i32,
            size.height.max(1) as i32,
        );
        let monitors = self.physical_monitors(window);
        let Some(monitor) = monitor_for_rect(&monitors, rect) else {
            return;
        };
        // Rest on the bottom edge of the work area (above the taskbar).
        let floor = monitor.work_area.bottom() - rect.height;
        if rect.y >= floor {
            self.gravity_velocity = 0.0;
            self.gravity_grounded = true;
            self.last_gravity_tick = Instant::now();
            return;
        }

        let now = Instant::now();
        let dt = (now - self.last_gravity_tick)
            .as_secs_f64()
            .clamp(0.0, GRAVITY_MAX_STEP_S);
        self.last_gravity_tick = now;
        self.gravity_velocity =
            (self.gravity_velocity + GRAVITY_PX_S2 * dt).min(GRAVITY_MAX_SPEED_PX_S);
        let next_y = (rect.y as f64 + self.gravity_velocity * dt).round() as i32;
        let landed = next_y >= floor;
        let next = PhysicalRect::new(rect.x, next_y.min(floor), rect.width, rect.height);
        self.platform.set_window_geometry_physical(window, next);
        if landed {
            self.gravity_velocity = 0.0;
            self.gravity_grounded = true;
            self.play_gravity_landing(now);
        } else {
            self.gravity_grounded = false;
            self.gravity_falling = true;
            ctx.request_repaint();
        }
    }

    /// One `jumping` animation per touchdown.
    fn play_gravity_landing(&mut self, now: Instant) {
        #[cfg(feature = "test-hooks")]
        {
            self.gravity_landings = self.gravity_landings.wrapping_add(1);
        }
        let Some(pet) = &mut self.pet else {
            return;
        };
        let raised = pet
            .engine
            .raise(
                PetState::Jumping,
                "gravity",
                None,
                Some(GRAVITY_LANDING_TTL),
                now,
            )
            .is_some();
        if raised {
            pet.anim_started = now;
            pet.last_state = pet.engine.current();
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
            let had_gaze = pet.engine.gaze_direction().is_some();
            let released = pet.engine.release_gaze(now);
            if released && had_gaze {
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
        let (Ok(position), size) = (window.outer_position(), window.outer_size()) else {
            return;
        };
        let scale = window.scale_factor().max(0.1);
        let (cursor_x, cursor_y) = if let Some(cursor) = self.conversation_cursor {
            (cursor.x as f64, cursor.y as f64)
        } else if let Some((cursor_x, cursor_y)) = self.pointer.position {
            (cursor_x / scale, cursor_y / scale)
        } else {
            return;
        };
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
        let dead_zone = pet_size.x.min(pet_size.y) as f64 * 0.22;
        if dx.hypot(dy) <= dead_zone {
            self.release_glance(Instant::now());
            return;
        }

        let Some((target_state, _)) = self
            .pet
            .as_ref()
            .and_then(|pet| pet.engine.gaze_target(dx as f32, dy as f32))
        else {
            self.release_glance(Instant::now());
            return;
        };
        let side = if target_state.look_towards_right() {
            1
        } else {
            -1
        };

        let existing_direction = self
            .pet
            .as_ref()
            .and_then(|pet| pet.engine.gaze_direction());
        if existing_direction == Some(side) {
            let retargeted = self
                .pet
                .as_mut()
                .is_some_and(|pet| pet.engine.retarget_gaze(dx as f32, dy as f32));
            if retargeted {
                let now = Instant::now();
                if let Some(pet) = &mut self.pet {
                    pet.anim_started = now;
                    pet.last_state = pet.engine.current();
                }
            }
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
        let raised = self.pet.as_mut().is_some_and(|pet| {
            pet.engine
                .glance_towards(dx as f32, dy as f32, now)
                .is_some()
        });
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
        let scale = self.effective_scale();
        let local_x = (local.x - rect.min.x) / scale;
        let local_y = (local.y - rect.min.y) / scale;
        if pet
            .atlas
            .mask
            .opaque_at_cell_dilated(pet.last_sprite, local_x, local_y, 1)
        {
            return true;
        }
        // Transient poses move pixels: the V2 gaze rows turn the head, a wave
        // lifts an arm, a jump stretches the body. If the hit test only looked
        // at the current cell, moving the cursor onto the pet would make the
        // pet look at the cursor and then become click-through at that exact
        // spot, so it could no longer be grabbed. Fall back to the resting
        // (idle) body mask to keep the pet interactive under the cursor.
        pet.engine
            .animation(PetState::Idle)
            .is_some_and(|animation| {
                animation.sprites.iter().any(|sprite| {
                    pet.atlas
                        .mask
                        .opaque_at_cell_dilated(*sprite, local_x, local_y, 1)
                })
            })
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

    /// Show the context menu through the shell.
    ///
    /// Windows owns a Win32 popup-menu thread; macOS hands a freshly built
    /// `tray-icon` menu to AppKit, which places it at the cursor. Returns false
    /// when the caller has to fall back to the in-window egui menu.
    fn show_platform_menu(
        &self,
        window: &winit::window::Window,
        cursor_x: f64,
        cursor_y: f64,
    ) -> bool {
        if let Some(menu) = &self.platform_menu {
            menu.show(cursor_x, cursor_y, self.pet_visible);
            return true;
        }
        if self.platform.uses_native_tray_menu() {
            if let Some(native_menu) = self.build_native_menu() {
                self.platform
                    .show_context_menu_for_window(window, &native_menu.menu);
            }
            return true;
        }
        false
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
        // The shell owns the native context menu; only shells without one
        // (and a failed Win32 menu thread) fall back to the egui menu.
        if right_pressed && over_pet && !self.show_platform_menu(window, cursor_x, cursor_y) {
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
        self.open_conversation();
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

    fn open_conversation(&mut self) {
        self.conversation_open = true;
        self.conversation_focus_pending = true;
        self.conversation_window_created = false;
    }

    fn close_conversation(&mut self) {
        self.conversation_open = false;
        self.conversation_focus_pending = false;
        self.conversation_cursor = None;
    }

    fn conversation_history_text(&self) -> String {
        self.conversation_history
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
            .join("\n")
    }

    fn remember_user_preferences(&self, text: &str) {
        let trimmed = text.trim();
        let preference = [
            "我喜欢",
            "我喜歡",
            "我偏好",
            "我习惯",
            "我習慣",
            "我不喜欢",
            "我不喜歡",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix));
        if preference && trimmed.chars().count() >= 4 {
            let _ = self
                .memory
                .remember_fact(&self.persona.id, "用户偏好", trimmed, 0.75);
        }
    }

    fn send_conversation(&mut self) {
        let text = self.conversation_draft.trim().to_string();
        if text.is_empty() || self.conversation_inflight {
            return;
        }
        self.conversation_draft.clear();
        let _ =
            self.memory
                .record_event(&self.persona.id, EventKind::UserMessage, Some(text.clone()));
        self.remember_user_preferences(&text);
        self.conversation_history.push(ConversationTurn {
            user: true,
            text: text.clone(),
        });
        let context = self.memory.build_greeting_context(
            &self.persona.id,
            self.config.memory.recent_events,
            self.config.memory.fact_limit,
        );
        let history = self.conversation_history_text();
        let persona = self.persona.clone();
        let config = self.config.deepseek.clone();
        let now_text = greeting::local_now_text();
        let pet_name = self.pet.as_ref().map(|pet| pet.entry.display_name.clone());
        let pet_state = self
            .pet
            .as_ref()
            .map(|pet| pet.engine.current().name().to_string())
            .unwrap_or_else(|| PetState::Idle.name().to_string());
        let (sender, receiver) = mpsc::channel();
        self.conversation_rx = Some(receiver);
        self.conversation_inflight = true;
        let repaint_context = Arc::clone(&self.repaint_context);
        thread::spawn(move || {
            let result = DeepSeekClient::new(config)
                .and_then(|client| {
                    client.generate_reply(
                        &persona,
                        &context,
                        &history,
                        &text,
                        &now_text,
                        pet_name.as_deref(),
                        &pet_state,
                    )
                })
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
            if let Some(ctx) = repaint_context
                .lock()
                .ok()
                .and_then(|context| context.clone())
            {
                ctx.request_repaint();
            }
        });
    }

    fn poll_conversation(&mut self) {
        let received = self
            .conversation_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());
        match received {
            Some(Ok(Ok(reply))) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
                self.conversation_history.push(ConversationTurn {
                    user: false,
                    text: reply.clone(),
                });
                let _ = self.memory.record_event(
                    &self.persona.id,
                    EventKind::PetReaction,
                    Some(reply.clone()),
                );
                self.show_bubble(reply);
            }
            Some(Ok(Err(error))) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
                self.status = error;
                let fallback = greeting::fallback_greeting(&self.persona);
                self.conversation_history.push(ConversationTurn {
                    user: false,
                    text: fallback.clone(),
                });
                let _ = self.memory.record_event(
                    &self.persona.id,
                    EventKind::PetReaction,
                    Some(fallback.clone()),
                );
                self.show_bubble(fallback);
            }
            Some(Err(TryRecvError::Empty)) | None => {}
            Some(Err(TryRecvError::Disconnected)) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
            }
        }
    }

    fn show_conversation_viewport(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let conversation_id = egui::ViewportId::from_hash_of("petsona-conversation");
        if !self.conversation_open {
            self.conversation_shown_at = None;
            if self.conversation_window_created {
                ctx.send_viewport_cmd_to(conversation_id, egui::ViewportCommand::Visible(false));
                self.conversation_window_created = false;
            }
            return;
        }
        let Some(window) = frame.winit_window() else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor().max(0.1) as f32;
        let parent = egui::pos2(position.x as f32 / scale, position.y as f32 / scale);
        let pet_window = self.pet_window_size();
        let conversation_position = egui::pos2(
            parent.x + pet_window.x * 0.5 - CONVERSATION_WINDOW_SIZE.x * 0.5,
            parent.y + pet_window.y + CONVERSATION_GAP,
        );
        // First appearance: create the window hidden and let the shell disable
        // the system's show/hide transition before it is ever visible, so the
        // only motion is our own entry animation.
        if !self.conversation_window_warmed {
            let builder = egui::ViewportBuilder::default()
                .with_title(CONVERSATION_TITLE)
                .with_inner_size([CONVERSATION_WINDOW_SIZE.x, CONVERSATION_WINDOW_SIZE.y])
                .with_position([conversation_position.x, conversation_position.y])
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_resizable(false)
                .with_active(true)
                .with_visible(false);
            ctx.show_viewport_immediate(conversation_id, builder, |_ui, _class| {});
            self.conversation_window_warmed = true;
            let _ = self
                .platform
                .disable_window_animation_for_title(CONVERSATION_TITLE);
            return;
        }

        let shown_at = *self.conversation_shown_at.get_or_insert_with(Instant::now);
        let eased = ease_out(entry_progress(Some(shown_at), CONVERSATION_ENTRY));
        // "From above": the content starts one drop higher and settles down.
        let drop = (1.0 - eased) * CONVERSATION_ENTRY_DROP;
        let builder = egui::ViewportBuilder::default()
            .with_title(CONVERSATION_TITLE)
            .with_inner_size([CONVERSATION_WINDOW_SIZE.x, CONVERSATION_WINDOW_SIZE.y])
            .with_position([conversation_position.x, conversation_position.y])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(true)
            .with_mouse_passthrough(false)
            .with_visible(true);
        let mut send = false;
        let mut close = false;
        ctx.show_viewport_immediate(conversation_id, builder, |ui, _class| {
            let content_rect = ui.max_rect().translate(egui::vec2(0.0, drop));
            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                ui.set_opacity(eased);
                egui::CentralPanel::default()
                    .frame(egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(12)))
                    .show(ui, |ui| {
                        ui.heading("和宠物聊聊");
                        if !self.conversation_history.is_empty() {
                            egui::ScrollArea::vertical()
                                .max_height(72.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for turn in self.conversation_history.iter().rev().take(4).rev()
                                    {
                                        ui.label(if turn.user {
                                            format!("你：{}", turn.text)
                                        } else {
                                            format!("宠物：{}", turn.text)
                                        });
                                    }
                                });
                        }
                        let output = egui::TextEdit::multiline(&mut self.conversation_draft)
                            .id_salt("conversation-input")
                            .desired_rows(3)
                            .hint_text("输入消息…")
                            .show(ui);
                        if let Some(cursor_range) = output.cursor_range {
                            let cursor = output.galley.pos_from_cursor(cursor_range.primary);
                            self.conversation_cursor = Some(
                                conversation_position
                                    + output.galley_pos.to_vec2()
                                    + cursor.min.to_vec2(),
                            );
                        } else {
                            self.conversation_cursor = Some(
                                conversation_position
                                    + output.response.response.rect.center().to_vec2(),
                            );
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(!self.conversation_inflight, egui::Button::new("发送"))
                                .clicked()
                            {
                                send = true;
                            }
                            if ui.button("关闭").clicked() {
                                close = true;
                            }
                            if self.conversation_inflight {
                                ui.spinner();
                            }
                        });
                        if output.response.response.has_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                        {
                            send = true;
                        }
                    });
            });
        });
        self.conversation_window_created = true;
        if self.conversation_focus_pending {
            ctx.send_viewport_cmd_to(conversation_id, egui::ViewportCommand::Focus);
            self.conversation_focus_pending = false;
        }
        if send {
            self.send_conversation();
        }
        if close {
            self.close_conversation();
        }
    }

    fn expire_bubble(&mut self) {
        if self
            .bubble
            .as_ref()
            .is_some_and(|bubble| Instant::now() >= bubble.until)
        {
            self.bubble = None;
        }
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

        if self.pet_dragged || self.walk_until.is_some() || self.menu_open || self.conversation_open
        {
            sooner(ACTIVE_REPAINT);
        }
        if self.gravity_falling {
            // Keep the fall smooth instead of stepping once per idle tick.
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

        if self.pet_visible && !self.settings_open && !self.platform.event_driven_mouse() {
            // macOS still needs a low-frequency global pointer poll. Windows
            // uses the low-level mouse hook and wakes only on real input.
            sooner(EVENT_POLL_REPAINT);
        }
        if self.greeting_inflight {
            sooner(EVENT_POLL_REPAINT);
        }

        if self
            .conversation_shown_at
            .is_some_and(|shown_at| shown_at.elapsed() < CONVERSATION_ENTRY)
        {
            sooner(ACTIVE_REPAINT);
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
            if self
                .bubble_shown_at
                .is_some_and(|shown_at| shown_at.elapsed() < BUBBLE_ENTRY)
            {
                // Keep the entry animation smooth instead of jumping in one step.
                sooner(ACTIVE_REPAINT);
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
        // egui subtracts the predicted frame time before scheduling the
        // callback. Add it back so a frame boundary close to the current
        // frame does not collapse into a burst of immediate repaints.
        let predicted_dt = ctx.input(|input| input.predicted_dt.max(0.0));
        let predicted_dt = Duration::try_from_secs_f32(predicted_dt).unwrap_or(Duration::ZERO);
        ctx.request_repaint_after(after.saturating_add(predicted_dt));
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
        self.poll_native_menu(ctx);
        self.poll_menu(ctx);
        self.poll_state_events(ctx);
        self.poll_greeting();
        self.poll_conversation();
        self.expire_bubble();
        self.update_pet_timers();
        self.update_auto_walk(ctx, frame);
        self.drag_pet(ctx, frame);
        self.update_gravity(ctx, frame);
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
        if let Some(window) = _frame.winit_window() {
            // The shell owns the native window chrome: Win32 keeps the
            // frameless `WS_POPUP` style and the non-activating flag in place,
            // macOS re-asserts its non-activating panel mask, and a portable
            // host does nothing at all.
            let reapplied = self.platform.present_window(window);
            #[cfg(feature = "test-hooks")]
            if reapplied {
                self.test_style_reapply_count = self.test_style_reapply_count.wrapping_add(1);
            }
            #[cfg(not(feature = "test-hooks"))]
            let _ = reapplied;
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
        self.show_conversation_viewport(ui.ctx(), _frame);

        if self.settings_open {
            self.show_settings_viewport(ui.ctx());
        }
        if self.menu_open {
            self.show_context_menu(ui.ctx());
        }
        #[cfg(feature = "test-hooks")]
        {
            self.test_ui_count = self.test_ui_count.wrapping_add(1);
            self.publish_test_status(ui.ctx(), _frame);
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

/// A monitor in physical pixels: winit knows the bounds and scale factor, the
/// shell supplies the real work area (taskbar excluded) when it can.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysicalMonitor {
    bounds: PhysicalRect,
    work_area: PhysicalRect,
    scale_factor: f64,
}

/// Clamp `rect` so it stays inside `work`; a rect larger than the work area is
/// pinned to its top-left corner instead of being pushed outside.
fn clamp_rect_to_work_area(rect: PhysicalRect, work: PhysicalRect) -> PhysicalRect {
    let max_x = work.right().saturating_sub(rect.width).max(work.x);
    let max_y = work.bottom().saturating_sub(rect.height).max(work.y);
    PhysicalRect::new(
        rect.x.clamp(work.x, max_x),
        rect.y.clamp(work.y, max_y),
        rect.width,
        rect.height,
    )
}

/// Monitor whose bounds contain the point, else the nearest one by centre
/// distance. An unplugged monitor therefore falls back to the closest visible
/// one instead of leaving the pet off-screen.
fn monitor_for_point(monitors: &[PhysicalMonitor], x: i32, y: i32) -> Option<PhysicalMonitor> {
    if let Some(monitor) = monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(x, y))
    {
        return Some(*monitor);
    }
    monitors
        .iter()
        .min_by_key(|monitor| {
            let (center_x, center_y) = monitor.bounds.center();
            let dx = (center_x as i64) - (x as i64);
            let dy = (center_y as i64) - (y as i64);
            dx * dx + dy * dy
        })
        .copied()
}

fn monitor_for_rect(monitors: &[PhysicalMonitor], rect: PhysicalRect) -> Option<PhysicalMonitor> {
    let (x, y) = rect.center();
    monitor_for_point(monitors, x, y)
}

/// Does the window rectangle fit inside the work area (with a couple of pixels
/// of slack for DPI rounding)?
///
/// Only the smoke status and the unit tests need this; production clamps with
/// `clamp_rect_to_work_area` instead.
#[cfg(any(feature = "test-hooks", test))]
fn rect_within_work_area(rect: PhysicalRect, work: PhysicalRect) -> bool {
    const TOLERANCE: i32 = 2;
    rect.x >= work.x - TOLERANCE
        && rect.y >= work.y - TOLERANCE
        && rect.right() <= work.right() + TOLERANCE
        && rect.bottom() <= work.bottom() + TOLERANCE
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
fn draw_bubble_window(
    painter: &egui::Painter,
    window: egui::Rect,
    text: &str,
    opacity: f32,
    dy: f32,
) -> egui::Rect {
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
    let y = bubble_overlay_y(window, size) + dy;
    let rect = egui::Rect::from_min_size(egui::pos2(x, y), size);
    let opacity = opacity.clamp(0.0, 1.0);
    let alpha = |value: u8| (value as f32 * opacity).round() as u8;
    let fill = egui::Color32::from_rgba_unmultiplied(24, 24, 28, alpha(235));
    painter.rect_filled(rect, egui::CornerRadius::same(10), fill);
    // Keep the bubble free of a bright outline. On Windows the old
    // semi-transparent white stroke read as a visible line along the top edge.
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
    painter.galley(
        rect.min + padding,
        galley,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha(255)),
    );
    rect
}

/// Progress (0..=1) of a viewport entry animation, easing not applied.
fn entry_progress(started: Option<Instant>, duration: Duration) -> f32 {
    let Some(started) = started else {
        return 1.0;
    };
    (started.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

/// Ease-out cubic: fast start, soft landing.
fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
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
        PetsonaApp::new(
            paths,
            AppConfig::default(),
            Arc::new(crate::platform::PortableHost),
        )
        .expect("app starts")
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
    fn a_look_pose_still_keeps_the_resting_body_clickable() {
        let mut app = test_app("gaze-hit");
        let pet = app.pet.as_ref().expect("bundled pet loads");
        let idle = pet
            .engine
            .animation(PetState::Idle)
            .expect("idle animation")
            .sprites
            .clone();
        let gaze_sprite = pet
            .engine
            .animation(PetState::LookRow9)
            .expect("V2 look row")
            .sprites[0];
        let mask = &pet.atlas.mask;
        // The smoke harness clicks the first pixel that is opaque in every
        // idle frame; the same rule is used here.
        let mut found = None;
        'outer: for my in 0..mask.mask_height {
            for mx in 0..mask.mask_width {
                let x = (mx * mask.scale + mask.scale / 2) as f32;
                let y = (my * mask.scale + mask.scale / 2) as f32;
                if idle.iter().all(|sprite| mask.opaque_at_cell(*sprite, x, y)) {
                    found = Some(egui::vec2(x, y));
                    break 'outer;
                }
            }
        }
        let point = found.expect("the bundled pet has a resting body pixel");
        let look_sprites: Vec<u32> = [PetState::LookRow9, PetState::LookRow10]
            .into_iter()
            .flat_map(|state| {
                pet.engine
                    .animation(state)
                    .map(|animation| animation.sprites.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert!(
            look_sprites
                .iter()
                .any(|sprite| !mask.opaque_at_cell_dilated(*sprite, point.x, point.y, 1)),
            "expected at least one look pose to move the body pixel ({} sprites checked, start {gaze_sprite})",
            look_sprites.len()
        );

        let window = app.pet_window_size();
        let rect = app.pet_rect(window);
        for sprite in look_sprites {
            app.pet.as_mut().expect("pet").last_sprite = sprite;
            assert!(
                app.cursor_over_pet(window, rect.min + point * app.effective_scale()),
                "sprite {sprite} made the resting body click-through while the pet looked at the cursor"
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
            (0.51, 0.5),
            (1.0, 1.0),
            (1.26, 1.25),
            (2.0, 2.0),
        ] {
            app.config.window.scale = scale;
            assert!(
                (app.stage_scale() - expected).abs() < f32::EPSILON,
                "scale {scale} -> {} (expected {expected})",
                app.stage_scale()
            );
        }
        // The sprite and window use the same fixed preset scale.
        app.config.window.scale = 1.1;
        assert!((app.pet_size().x - app.pet_cell_size().x * 1.0).abs() < 0.01);
        assert!(app.pet_window_size().x >= app.pet_size().x);
    }

    fn monitor(x: i32, y: i32, width: i32, height: i32, scale: f64) -> PhysicalMonitor {
        let bounds = PhysicalRect::new(x, y, width, height);
        PhysicalMonitor {
            bounds,
            work_area: PhysicalRect::new(x, y + 40, width, height - 40),
            scale_factor: scale,
        }
    }

    #[test]
    fn saved_position_is_clamped_into_the_work_area() {
        let work = PhysicalRect::new(0, 0, 1920, 1040);
        let wanted = PhysicalRect::new(1850, 1000, 220, 208);
        let clamped = clamp_rect_to_work_area(wanted, work);
        assert_eq!(clamped.x, 1700);
        assert_eq!(clamped.y, 832);
        assert!(rect_within_work_area(clamped, work));
        // A window that already fits is not moved.
        let fitting = PhysicalRect::new(100, 100, 220, 208);
        assert_eq!(clamp_rect_to_work_area(fitting, work), fitting);
    }

    #[test]
    fn an_unplugged_monitor_falls_back_to_the_nearest_one() {
        let monitors = [
            monitor(0, 0, 1920, 1080, 1.0),
            monitor(1920, 0, 2560, 1440, 1.5),
        ];
        // The right monitor is still attached: the point lands on it.
        let right = monitor_for_point(&monitors, 2200, 300).expect("right monitor");
        assert_eq!(right.bounds.x, 1920);
        // The saved position was on a monitor that is gone; the nearest
        // remaining monitor wins and the origin is clamped back inside it.
        let gone = monitor_for_point(&monitors, -4000, 200).expect("nearest monitor");
        assert_eq!(gone.bounds.x, 0);
        let clamped =
            clamp_rect_to_work_area(PhysicalRect::new(-4000, 200, 220, 208), gone.work_area);
        assert!(rect_within_work_area(clamped, gone.work_area));
    }

    #[test]
    fn gravity_floor_uses_the_physical_work_area() {
        let monitor = monitor(0, 0, 1920, 1080, 1.25);
        let work = monitor.work_area;
        let rect = PhysicalRect::new(400, 100, 220, 208);
        let floor = work.bottom() - rect.height;
        assert_eq!(floor, 872);
        assert!(!rect_within_work_area(
            PhysicalRect::new(400, floor + 10, 220, 208),
            work
        ));
    }
}
