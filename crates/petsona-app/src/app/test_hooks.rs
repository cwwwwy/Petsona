use super::*;

impl PetsonaApp {
    pub(super) fn poll_test_hooks(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
                self.settings_focus_pending = false;
                self.settings_focus_deadline = None;
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
    pub(super) fn publish_test_status(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
                        geometry::monitor_for_rect(&monitors, rect)
                            .map(|monitor| rect_within_work_area(rect, monitor.work_area))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            };
            status.hooks_port = hooks_port;
        }
    }
}
