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
use petsona_core::pet::state::{GazePhase, PetState};
use petsona_core::pet::{PetAtlas, PetEntry};
use petsona_runtime::pet::{PetSession, IDLE_REPAINT};
use petsona_runtime::session::{Bubble, ConversationTurn, PetsonaRuntime};

use crate::platform::{MenuCommand, PhysicalRect, PlatformHost, PlatformMenu, PointerSnapshot};

mod bubble;
mod conversation;
mod geometry;
mod interaction;
mod menus;
mod pets;
mod settings;
mod shadow;
#[cfg(feature = "test-hooks")]
mod test_hooks;

use bubble::BUBBLE_ENTRY;
use conversation::{CONVERSATION_ENTRY, CONVERSATION_EXIT};
#[cfg(any(feature = "test-hooks", test))]
use geometry::rect_within_work_area;
use geometry::{
    bottom_center_anchor, clamp_rect_to_work_area, ease_out, position_for_bottom_center,
};
use menus::NativeMenu;

#[cfg(feature = "test-hooks")]
use crate::test_hooks::{TestActionRequest, TestHookServer, TestStatus};
use petsona_runtime::greeting;

const PET_WINDOW_MIN_WIDTH: f32 = 220.0;
/// Scale presets ease into place instead of jumping: the window is
/// resized once, the sprite animates inside it.
const SCALE_TRANSITION: Duration = Duration::from_millis(180);
const ACTIVE_REPAINT: Duration = Duration::from_millis(16);
const EVENT_POLL_REPAINT: Duration = Duration::from_millis(100);
const GAZE_POINTER_POLL_REPAINT: Duration = Duration::from_millis(40);
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
    shadow_window_created: bool,
    shadow_window_warmed: bool,
    shadow_hovered: bool,
    shadow_hover_progress: f32,
    shadow_last_tick: Instant,
    conversation_open: bool,
    conversation_window_created: bool,
    /// Same warm-up as the bubble, for the conversation ("input box") window.
    conversation_window_warmed: bool,
    /// When the conversation window became visible; drives its entry animation.
    conversation_shown_at: Option<Instant>,
    /// When the close animation began. The viewport stays visible until it ends.
    conversation_closing_at: Option<Instant>,
    conversation_focus_pending: bool,
    conversation_input_focus_pending: bool,
    conversation_draft: String,
    conversation_cursor: Option<egui::Pos2>,
    /// Primary-button edge detector used to close the composer when the
    /// user clicks anywhere outside the input pill.
    conversation_primary_was_down: bool,
    /// Ignore pet clicks while a native menu owns the pointer: the click
    /// that dismisses the menu must not also wave the pet.
    click_guard_until: Option<Instant>,
    /// Set by the "立即活动" menu item so the next auto-walk tick starts a
    /// reminder walk regardless of the interval.
    activity_now_requested: bool,
    /// Set while a click-triggered model greeting is in flight. If the
    /// reply takes too long the UI shows a local line instead of leaving
    /// the click without feedback.
    click_greeting_pending_at: Option<Instant>,
    click_greeting_fallback_shown: bool,
    settings_open: bool,
    /// A settings command requests a real activation once the viewport exists.
    /// `with_active` only applies when egui creates the child window, while the
    /// same viewport can be reused after it was hidden.
    settings_focus_pending: bool,
    /// Do not fight the user forever if the OS refuses to hand us the
    /// foreground window; settings itself is still usable after this deadline.
    settings_focus_deadline: Option<Instant>,
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
    /// Scale the sprite is currently drawn at; eases towards the preset
    /// selected in the settings.
    scale_display: f32,
    scale_anim_from: f32,
    scale_anim_started: Instant,
    scale_anim_active: bool,
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

impl PetsonaApp {
    pub fn new(
        paths: AppPaths,
        mut config: AppConfig,
        platform: Arc<dyn PlatformHost>,
    ) -> Result<Self> {
        config.window.scale = nearest_scale(config.window.scale);
        let initial_scale = config.window.scale;
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

        if runtime.pet.is_some() {
            // Startup greeting; with no pet on screen there is nothing to
            // anchor a bubble to (the settings window opens instead).
            runtime.bubble = Some(Bubble {
                text: fallback,
                until: Instant::now() + Duration::from_secs(8),
            });
        }

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
            shadow_window_created: false,
            shadow_window_warmed: false,
            shadow_hovered: false,
            shadow_hover_progress: 0.0,
            shadow_last_tick: Instant::now(),
            conversation_open: false,
            conversation_window_created: false,
            conversation_window_warmed: false,
            conversation_shown_at: None,
            conversation_closing_at: None,
            conversation_focus_pending: false,
            conversation_input_focus_pending: false,
            conversation_draft: String::new(),
            conversation_cursor: None,
            conversation_primary_was_down: false,
            click_guard_until: None,
            activity_now_requested: false,
            click_greeting_pending_at: None,
            click_greeting_fallback_shown: false,
            settings_open: false,
            settings_focus_pending: false,
            settings_focus_deadline: None,
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
            scale_display: initial_scale,
            scale_anim_from: initial_scale,
            scale_anim_started: Instant::now(),
            scale_anim_active: false,
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
        if app.pet.is_none() {
            // First run, or the library is empty: send the user straight to
            // the settings window to pick or import a pet.
            app.open_settings();
        }
        if app.config.first_run {
            app.config.first_run = false;
            if let Err(error) = app.config.save(&app.paths.config_file) {
                tracing::warn!(%error, "cannot persist the first-run flag");
            }
        }
        if app.pet.is_some() {
            let _ = app.trigger_greeting("startup", true);
        }
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
            let gaze_active = self
                .pet
                .as_ref()
                .is_some_and(|pet| pet.engine.gaze_direction().is_some());
            let sampling_interval = if gaze_active {
                GAZE_POINTER_POLL_REPAINT
            } else {
                EVENT_POLL_REPAINT
            };
            let pointer_event = ctx.input(|input| {
                input.pointer.delta() != egui::Vec2::ZERO
                    || input.pointer.primary_pressed()
                    || input.pointer.primary_released()
                    || input.pointer.secondary_pressed()
                    || input.pointer.secondary_released()
            });
            let low_frequency_due = self
                .last_pointer_refresh
                .is_none_or(|last| last.elapsed() >= sampling_interval);
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

    /// Preset the user selected; what the window geometry and menus use.
    fn target_scale(&self) -> f32 {
        nearest_scale(self.config.window.scale)
    }

    /// Scale the sprite is drawn at; equals the preset except during the
    /// transition animation.
    fn effective_scale(&self) -> f32 {
        if self.scale_anim_active {
            self.scale_display
        } else {
            self.target_scale()
        }
    }

    /// Advance the scale transition. Called once per frame before drawing.
    fn update_scale_animation(&mut self, ctx: &egui::Context) {
        if !self.scale_anim_active {
            return;
        }
        let progress = (self.scale_anim_started.elapsed().as_secs_f32()
            / SCALE_TRANSITION.as_secs_f32())
        .clamp(0.0, 1.0);
        let target = self.target_scale();
        self.scale_display =
            self.scale_anim_from + (target - self.scale_anim_from) * ease_out(progress);
        if progress >= 1.0 {
            self.scale_display = target;
            self.scale_anim_active = false;
        }
        ctx.request_repaint();
    }

    fn scale_label(&self) -> &'static str {
        SCALE_PRESETS
            .iter()
            .find(|(_, value)| (*value - self.target_scale()).abs() < f32::EPSILON)
            .map(|(label, _)| *label)
            .unwrap_or("标准")
    }

    fn set_scale_preset(&mut self, value: f32) {
        let snapped = nearest_scale(value);
        if (self.config.window.scale - snapped).abs() < f32::EPSILON {
            return;
        }
        // Ease from whatever is on screen right now, so a quick second change
        // does not snap back to the previous preset.
        self.scale_anim_from = self.effective_scale();
        self.scale_anim_started = Instant::now();
        self.scale_anim_active = true;
        self.scale_display = self.scale_anim_from;
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

    /// Window scale. While a transition runs the window keeps the larger of
    /// the two presets so a shrinking sprite is never clipped; the extra
    /// transparent area is invisible and the final resize happens once the
    /// animation has finished.
    fn stage_scale(&self) -> f32 {
        if self.scale_anim_active {
            self.scale_anim_from.max(self.target_scale())
        } else {
            self.target_scale()
        }
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

    fn draw_pet(&mut self, root_ui: &mut egui::Ui, frame: &eframe::Frame, window_size: egui::Vec2) {
        if self.pet.is_none() {
            // No pet configured: nothing is drawn at all. The window stays
            // transparent and click-through until a pet is imported.
            return;
        }

        // Anchor the sprite in the window that actually exists right now.
        // During a scale transition the requested size and the OS window size
        // differ for a frame; using the target size made the sprite jump.
        let window_size = frame_actual_size(frame).unwrap_or(window_size);
        // Compute the sprite rectangle before borrowing the pet mutably.
        let pet_rect = self.pet_rect(window_size);
        let scale = self.effective_scale();
        let total = window_size;
        let Some(pet) = self.runtime.pet.as_mut() else {
            return;
        };
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

        let conversation_animating = self
            .conversation_shown_at
            .is_some_and(|shown_at| shown_at.elapsed() < CONVERSATION_ENTRY)
            || self
                .conversation_closing_at
                .is_some_and(|closing_at| closing_at.elapsed() < CONVERSATION_EXIT);
        if self.pet_dragged
            || self.walk_until.is_some()
            || self.menu_open
            || conversation_animating
            || self.scale_anim_active
        {
            sooner(ACTIVE_REPAINT);
        }
        if self.gravity_falling {
            // Keep the fall smooth instead of stepping once per idle tick.
            sooner(ACTIVE_REPAINT);
        }
        if self.shadow_hover_progress > 0.0 && self.shadow_hover_progress < 1.0 {
            sooner(ACTIVE_REPAINT);
        }

        if self.pet_visible {
            if let Some(pet) = &self.pet {
                // A gaze transition steps one direction pose per
                // `GAZE_FRAME_MS`; without a steady frame clock the pose only
                // advanced on mouse events, which read as dropped frames.
                if matches!(
                    pet.engine.gaze_phase(),
                    Some(GazePhase::Turning | GazePhase::Returning)
                ) {
                    sooner(ACTIVE_REPAINT);
                }
                let animation_after = pet.next_frame_after();
                #[cfg(feature = "test-hooks")]
                {
                    self.test_animation_repaint_ms = animation_after.as_millis() as u64;
                }
                sooner(animation_after);
            }
        }

        if self.pet_visible && !self.settings_open && !self.platform.event_driven_mouse() {
            // macOS uses a cheap low-frequency fallback while idle, then
            // samples faster while a gaze is active so the pose follows motion.
            // Windows uses its low-level mouse hook and wakes on real input.
            let gaze_active = self
                .pet
                .as_ref()
                .is_some_and(|pet| pet.engine.gaze_direction().is_some());
            sooner(if gaze_active {
                GAZE_POINTER_POLL_REPAINT
            } else {
                EVENT_POLL_REPAINT
            });
        }
        if self.greeting_inflight {
            sooner(EVENT_POLL_REPAINT);
        }

        if conversation_animating || self.settings_focus_pending {
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
        self.update_scale_animation(ctx);
        self.poll_tray(ctx);
        self.poll_native_menu(ctx);
        self.poll_menu(ctx);
        self.poll_state_events(ctx);
        self.poll_greeting(ctx);
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
            crate::fonts::install_cjk_font(ui.ctx(), &self.platform.cjk_font_candidates());
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
            self.draw_pet(ui, _frame, window_size);
        }
        self.show_bubble_viewport(ui.ctx(), _frame);
        self.show_shadow_viewport(ui.ctx(), _frame);
        self.show_conversation_viewport(ui.ctx(), _frame);

        if self.settings_open {
            self.show_settings_viewport(ui.ctx(), _frame);
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

/// Logical size of the pet window as the OS currently has it.
fn frame_actual_size(frame: &eframe::Frame) -> Option<egui::Vec2> {
    let window = frame.winit_window()?;
    let scale = window.scale_factor().max(0.1) as f32;
    let size = window.outer_size();
    Some(egui::vec2(
        size.width as f32 / scale,
        size.height as f32 / scale,
    ))
}

/// Speech bubble sized to its text and anchored just above the pet, with a
/// tail pointing at it. The pet window itself remains the exact sprite size.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::geometry::{clamp_to_monitor, monitor_for_point, PhysicalMonitor};

    #[test]
    fn conversation_window_is_small_and_wraps_long_text() {
        let size = conversation::conversation_window_size();
        assert!(size.x <= 340.0);
        assert_eq!(
            conversation::CONVERSATION_MIN_INPUT_HEIGHT,
            shadow::SHADOW_BUTTON_SIZE,
            "single-line composer and edit button must share a height"
        );
        assert_eq!(
            conversation::conversation_input_rows("", 240.0),
            1,
            "empty input keeps one row"
        );
        assert!(
            conversation::conversation_input_rows(&"很长".repeat(120), 240.0) > 1,
            "long text grows the input"
        );
    }

    #[test]
    fn gaze_range_stays_near_the_pet_and_has_exit_hysteresis() {
        let size = egui::vec2(220.0, 318.0);
        assert!(geometry::cursor_within_gaze_range(
            110.0, 159.0, size, false
        ));
        assert!(geometry::cursor_within_gaze_range(200.0, 0.0, size, false));
        assert!(!geometry::cursor_within_gaze_range(250.0, 0.0, size, false));
        assert!(geometry::cursor_within_gaze_range(240.0, 0.0, size, true));
        assert!(!geometry::cursor_within_gaze_range(300.0, 0.0, size, true));
    }

    #[test]
    fn conversation_close_keeps_the_viewport_for_the_exit_animation() {
        let mut app = test_app("conversation-animation");
        app.conversation_window_created = true;
        app.open_conversation();
        assert!(app.conversation_open);
        assert!(app.conversation_shown_at.is_some());

        app.close_conversation();
        assert!(!app.conversation_open);
        assert!(app.conversation_closing_at.is_some());
        assert!(app.conversation_window_created);
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

    /// Synthetic V2 pet used by tests after the bundled pet was removed.
    const TEST_PET_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../petsona-core/testdata/v2-test-pet"
    );

    fn install_test_pet(paths: &AppPaths) {
        let target = paths.pets_dir.join("test_fixture_v2");
        std::fs::create_dir_all(&target).expect("create test pet dir");
        for file in ["pet.json", "spritesheet.png"] {
            std::fs::copy(format!("{TEST_PET_DIR}/{file}"), target.join(file))
                .expect("copy test pet file");
        }
    }

    fn test_app(name: &str) -> PetsonaApp {
        let paths = AppPaths::resolve(std::env::temp_dir().join(format!("petsona-test-{name}")));
        let _ = std::fs::remove_dir_all(&paths.config_dir);
        install_test_pet(&paths);
        PetsonaApp::new(
            paths,
            AppConfig::default(),
            Arc::new(crate::platform::PortableHost),
        )
        .expect("app starts")
    }

    #[test]
    fn an_empty_library_opens_settings_once() {
        let paths = AppPaths::resolve(std::env::temp_dir().join("petsona-test-first-run"));
        let _ = std::fs::remove_dir_all(&paths.config_dir);
        let app = PetsonaApp::new(
            paths,
            AppConfig::default(),
            Arc::new(crate::platform::PortableHost),
        )
        .expect("app starts");

        assert!(app.pet.is_none());
        assert!(
            app.settings_open,
            "an empty library should send the user to the settings window"
        );
        assert!(
            !app.config.first_run,
            "the first-run flag should be consumed"
        );
    }

    #[test]
    fn a_pet_in_the_library_starts_without_settings() {
        let app = test_app("no-first-run");
        assert!(app.pet.is_some());
        assert!(!app.settings_open);
    }

    fn first_opaque_cell_point(app: &PetsonaApp) -> egui::Vec2 {
        let pet = app.pet.as_ref().expect("the test pet loads");
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
        panic!("the test pet draws something");
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
        let pet = app.pet.as_ref().expect("the test pet loads");
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
        let point = found.expect("the test pet has a resting body pixel");
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
    fn scale_transition_keeps_the_window_at_the_larger_preset() {
        let mut app = test_app("scale-anim");
        app.config.window.scale = 0.5;
        app.scale_anim_from = 2.0;
        app.scale_display = 2.0;
        app.scale_anim_started = Instant::now();
        app.scale_anim_active = true;

        // Shrinking: the window keeps the larger preset so the sprite is not
        // clipped while it animates down.
        assert!((app.stage_scale() - 2.0).abs() < f32::EPSILON);
        assert!((app.effective_scale() - 2.0).abs() < f32::EPSILON);

        app.scale_anim_started = Instant::now() - SCALE_TRANSITION - Duration::from_millis(1);
        let ctx = egui::Context::default();
        app.update_scale_animation(&ctx);
        assert!(!app.scale_anim_active);
        assert!((app.stage_scale() - 0.5).abs() < f32::EPSILON);
        assert!((app.effective_scale() - 0.5).abs() < f32::EPSILON);
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
