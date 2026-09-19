use super::geometry::clamp_to_monitor;
use super::*;

const MENU_TITLE: &str = "Petsona 菜单";
const MENU_WIDTH: f32 = 176.0;
const MENU_ROW: f32 = 30.0;
const MENU_PAD: f32 = 6.0;
const MENU_ROWS: f32 = 5.0;
const MENU_SIZE: egui::Vec2 = egui::vec2(MENU_WIDTH, MENU_ROWS * MENU_ROW + MENU_PAD * 2.0);
/// Identifiers of the tray-icon menu entries. Shells that hand the menu to the
/// operating system (macOS) build it from these.
const NATIVE_MENU_OPEN_SETTINGS_ID: &str = "petsona.open-settings";
const NATIVE_MENU_SELECT_PET_PREFIX: &str = "petsona.select-pet:";
const NATIVE_MENU_SCALE_PREFIX: &str = "petsona.scale:";
const NATIVE_MENU_TRIGGER_ACTIVITY_ID: &str = "petsona.trigger-activity";
const NATIVE_MENU_TOGGLE_PET_ID: &str = "petsona.toggle-pet";
const NATIVE_MENU_QUIT_ID: &str = "petsona.quit";

/// Tray menu handed to AppKit, kept so the check marks can be refreshed.
pub(super) struct NativeMenu {
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
    TriggerActivity,
    TogglePet,
    Quit,
}

impl PetsonaApp {
    /// Real popup menu: its own native window, sized to the items, so it can
    /// open at the cursor and is never clipped by the pet window.
    pub(super) fn show_context_menu(&mut self, ctx: &egui::Context) {
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
                            ("立即活动", MenuAction::TriggerActivity),
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
            MenuAction::TriggerActivity => self.request_activity_now(),
            MenuAction::TogglePet => {
                let visible = !self.pet_visible;
                self.set_pet_visible(ctx, visible);
            }
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    pub(super) fn dismiss_menu(&mut self) {
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
    pub(super) fn poll_menu(&mut self, ctx: &egui::Context) {
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
        if !Self::fill_native_scale_menu(&scale_menu, self.target_scale()) {
            return None;
        }
        let trigger_activity =
            MenuItem::with_id(NATIVE_MENU_TRIGGER_ACTIVITY_ID, "立即活动", true, None);
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

        let items: [&dyn IsMenuItem; 6] = [
            &open_settings,
            &pets,
            &scale_menu,
            &trigger_activity,
            &toggle_pet,
            &quit,
        ];
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
    pub(super) fn refresh_native_tray_menu(&self) {
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
        if !Self::fill_native_scale_menu(&native_menu.scale_menu, self.target_scale()) {
            tracing::warn!("cannot refresh native macOS scale menu");
        }
    }

    #[cfg(feature = "test-hooks")]
    pub(super) fn native_checked_pet_id(&self) -> Option<String> {
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
                MenuCommand::TriggerActivity => {
                    self.request_activity_now();
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
    pub(super) fn switch_pet(&mut self, id: &str) {
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

    pub(super) fn install_tray(&mut self, ctx: egui::Context) {
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

    pub(super) fn poll_tray(&mut self, ctx: &egui::Context) {
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
    pub(super) fn poll_native_menu(&mut self, ctx: &egui::Context) {
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
                NATIVE_MENU_TRIGGER_ACTIVITY_ID => {
                    self.request_activity_now();
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
    pub(super) fn open_menu_at(&mut self, anchor: egui::Pos2) {
        self.menu_open = true;
        self.menu_created = false;
        self.menu_styled = false;
        self.menu_button_was_down = false;
        self.menu_right_button_was_down = true;
        self.menu_anchor = Some(anchor);
    }

    /// Show the context menu through the shell.
    ///
    /// Windows owns a Win32 popup-menu thread; macOS hands a freshly built
    /// `tray-icon` menu to AppKit, which places it at the cursor. Returns false
    /// when the caller has to fall back to the in-window egui menu.
    pub(super) fn show_platform_menu(
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
