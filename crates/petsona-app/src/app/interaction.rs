use super::geometry::{
    clamp_rect_to_work_area, gaze_range_allows, monitor_for_point, monitor_for_rect,
    PhysicalMonitor,
};
use super::shadow::shadow_hit_rect;
use super::*;

impl PetsonaApp {
    pub(super) fn set_pet_visible(&mut self, ctx: &egui::Context, visible: bool) {
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

    /// "立即活动" from the context menu: start a reminder walk on the next
    /// tick even if the interval and the user grace period have not elapsed.
    pub(super) fn request_activity_now(&mut self) {
        self.activity_now_requested = true;
    }

    pub(super) fn set_walk_state(&mut self, state: PetState) {
        if let Some(pet) = &mut self.pet {
            if pet.engine.set_base(state) {
                pet.anim_started = Instant::now();
                pet.last_state = pet.engine.current();
            }
        }
    }

    pub(super) fn update_auto_walk(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
        let forced = std::mem::take(&mut self.activity_now_requested);
        if !forced
            && self.last_user_action.elapsed()
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
            if !forced && now < self.next_walk_at {
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
    pub(super) fn update_pet_timers(&mut self) {
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
    pub(super) fn raise_motion(&mut self, state: PetState) {
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
    pub(super) fn drag_pet(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
                let desired = egui::pos2(
                    cursor_x as f32 / scale as f32 - grab.x,
                    cursor_y as f32 / scale as f32 - grab.y,
                );
                if self.config.window.gravity_enabled {
                    // Gravity needs a floor: dragging the pet below the
                    // work area used to strand it off-screen, where the
                    // fall loop would declare it grounded.
                    let size = window.outer_size();
                    let rect = PhysicalRect::new(
                        (desired.x as f64 * scale).round() as i32,
                        (desired.y as f64 * scale).round() as i32,
                        size.width.max(1) as i32,
                        size.height.max(1) as i32,
                    );
                    if let Some(monitor) = monitor_for_rect(&self.physical_monitors(window), rect) {
                        self.platform.set_window_geometry_physical(
                            window,
                            clamp_rect_to_work_area(rect, monitor.work_area),
                        );
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(desired));
                }
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
    pub(super) fn physical_monitors(&self, window: &winit::window::Window) -> Vec<PhysicalMonitor> {
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
    pub(super) fn saved_monitor(
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
    pub(super) fn remember_window_position(&mut self, window: &winit::window::Window) {
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
    pub(super) fn update_gravity(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
            if rect.y > floor {
                // Heal a pet that was left below the floor (an older build
                // allowed dragging past the boundary).
                self.platform.set_window_geometry_physical(
                    window,
                    PhysicalRect::new(rect.x, floor, rect.width, rect.height),
                );
            }
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
    pub(super) fn play_gravity_landing(&mut self, now: Instant) {
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

    pub(super) fn cancel_glance(&mut self) {
        self.glance_side = 0;
        if let Some(pet) = &mut self.pet {
            pet.engine.cancel_gaze();
        }
    }

    pub(super) fn release_glance(&mut self, now: Instant) {
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
    #[cfg(feature = "test-hooks")]
    pub(super) fn update_test_glance(&mut self, side: i8) {
        if side == 0 {
            self.release_glance(Instant::now());
            return;
        }

        let now = Instant::now();
        let dx = if side < 0 { -1.0 } else { 1.0 };
        let changed = self.pet.as_mut().is_some_and(|pet| {
            if pet.engine.gaze_direction().is_some() {
                pet.engine.retarget_gaze(dx, 0.0)
            } else {
                pet.engine.glance(dx, now).is_some()
            }
        });
        if changed {
            if let Some(pet) = &mut self.pet {
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
            }
        }
        self.glance_side = side;
        if changed {
            self.last_glance_at = Some(now);
        }
    }

    /// Let the V2 look rows follow the cursor. The pose table is addressed
    /// directly; moving between poses animates along the row, and releasing
    /// returns to the neutral middle frame before falling back to the base
    /// animation.
    pub(super) fn update_glance(&mut self, frame: &eframe::Frame) {
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
        let gaze_active = pet.engine.gaze_direction().is_some();
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
        let caret_target = self.conversation_cursor.is_some();
        if !gaze_range_allows(caret_target, dx, dy, pet_size, gaze_active) {
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
        let now = Instant::now();
        let changed = self.pet.as_mut().is_some_and(|pet| {
            if pet.engine.gaze_direction().is_some() {
                pet.engine.retarget_gaze(dx as f32, dy as f32)
            } else {
                pet.engine
                    .glance_towards(dx as f32, dy as f32, now)
                    .is_some()
            }
        });
        if changed {
            if let Some(pet) = &mut self.pet {
                pet.anim_started = now;
                pet.last_state = pet.engine.current();
            }
            self.last_glance_at = Some(now);
        }
        self.glance_side = side;
    }

    /// Rectangle of the scaled pet sprite inside the window.
    pub(super) fn pet_rect(&self, window_size: egui::Vec2) -> egui::Rect {
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
    pub(super) fn cursor_over_pet(&self, window_size: egui::Vec2, local: egui::Pos2) -> bool {
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
    pub(super) fn update_passthrough(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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

    /// Read the cursor and the mouse buttons and turn them into pet input.
    ///
    /// Using the system state instead of window events keeps the per-pixel
    /// click-through exact and never changes a window style at runtime, which
    /// is what used to make Windows flash the frame.
    pub(super) fn update_pointer(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
        // The shadow is an interactive edit button. Do not let its clicks or
        // drags leak into the pet underneath it.
        if shadow_hit_rect(window_size).contains(local) {
            self.press_origin = None;
            self.press_started_at = None;
            self.press_moved = false;
            self.pet_dragged = false;
            self.drag_grab = None;
            return;
        }
        let over_pet = self.cursor_over_pet(window_size, local);
        let now = Instant::now();

        // A native menu owns the pointer while it is open; the click that
        // dismisses it arrives through the global button poll as well, and
        // used to wave the pet ("right click makes the pet reply").
        let menu_owns_pointer = self
            .platform_menu
            .as_ref()
            .is_some_and(|menu| menu.is_open())
            || self.click_guard_until.is_some_and(|until| now < until);
        if menu_owns_pointer {
            self.press_origin = None;
            self.press_started_at = None;
            self.press_moved = false;
            self.pet_dragged = false;
            self.drag_grab = None;
        }

        if left_pressed && over_pet && !menu_owns_pointer {
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
            if clicked && !menu_owns_pointer {
                self.register_click();
            }
        }
        // The shell owns the native context menu; only shells without one
        // (and a failed Win32 menu thread) fall back to the egui menu.
        if right_pressed && over_pet {
            // Guard the dismissal click even when the shell falls back to the
            // egui menu.
            self.click_guard_until = Some(now + Duration::from_millis(250));
            if !self.show_platform_menu(window, cursor_x, cursor_y) {
                self.open_menu_at(egui::pos2(cursor_x as f32 / scale, cursor_y as f32 / scale));
            }
        }
    }

    /// A press that neither dragged nor out-lasted a click.
    pub(super) fn register_click(&mut self) {
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

    pub(super) fn on_pet_click(&mut self) {
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
        if self.trigger_greeting("click", true) {
            // The pet reacts immediately; only the bubble waits briefly for
            // the model line, then falls back to a local one.
            self.click_greeting_pending_at = Some(Instant::now());
            self.click_greeting_fallback_shown = false;
        } else {
            self.show_bubble(greeting::fallback_greeting(&self.persona));
        }
    }

    pub(super) fn on_double_click(&mut self) {
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

    pub(super) fn trigger_greeting(&mut self, trigger: &str, force: bool) -> bool {
        // No pet on screen: a floating greeting would have nothing to anchor to.
        if self.pet.is_none() {
            return false;
        }
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

    /// Show the local line if a click greeting waits too long for the model.
    fn poll_click_greeting_fallback(&mut self, ctx: &egui::Context) {
        const FALLBACK_AFTER: Duration = Duration::from_millis(700);

        let Some(requested_at) = self.click_greeting_pending_at else {
            return;
        };
        if self.click_greeting_fallback_shown {
            return;
        }
        let elapsed = requested_at.elapsed();
        if elapsed >= FALLBACK_AFTER {
            self.click_greeting_fallback_shown = true;
            self.show_bubble(greeting::fallback_greeting(&self.persona));
        } else {
            ctx.request_repaint_after(FALLBACK_AFTER - elapsed);
        }
    }

    pub(super) fn poll_greeting(&mut self, ctx: &egui::Context) {
        self.poll_click_greeting_fallback(ctx);
        let received = self
            .greeting_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());
        match received {
            Some(Ok(result)) => {
                self.greeting_rx = None;
                self.greeting_inflight = false;
                self.click_greeting_pending_at = None;
                self.click_greeting_fallback_shown = false;
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
                self.click_greeting_pending_at = None;
                self.click_greeting_fallback_shown = false;
            }
            None => {}
        }
    }
}
