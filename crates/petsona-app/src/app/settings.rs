use super::geometry::monitor_for_rect;
use super::*;

impl PetsonaApp {
    pub(super) fn show_settings_viewport(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let viewport_id = egui::ViewportId::from_hash_of("petsona-settings");
        let size = egui::vec2(640.0, 720.0);
        // Place the settings window next to the pet, clamped to the monitor
        // that currently holds it, the first time it opens. After that the
        // user owns the position.
        let position = match self.settings_pos {
            Some(position) => position,
            None => {
                let position = self.default_settings_position(ctx, frame, size);
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
            if self
                .settings_focus_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                // Never keep stealing focus if Windows refuses the activation
                // request (for example while another app is in the foreground).
                self.settings_focus_pending = false;
                self.settings_focus_deadline = None;
            } else {
                // This also works when the settings viewport already existed
                // but had been closed/hidden. Windows retries until the actual
                // foreground window is ours; this heals the menu-focus race.
                ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
                if self.platform.confirm_settings_focus("Petsona 设置") {
                    self.settings_focus_pending = false;
                    self.settings_focus_deadline = None;
                }
            }
        }
    }

    pub(super) fn open_settings(&mut self) {
        self.settings_open = true;
        self.settings_pos = None;
        self.settings_focus_pending = true;
        self.settings_focus_deadline = Some(Instant::now() + Duration::from_millis(1000));
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
    fn default_settings_position(
        &self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        size: egui::Vec2,
    ) -> egui::Pos2 {
        // `monitor_size` alone has no origin: on a secondary monitor whose
        // origin is not zero the old clamp always pushed the settings
        // window back onto the primary display. Go through the physical
        // monitor that actually contains the pet instead.
        if let Some(window) = frame.winit_window() {
            let (Ok(position), size_physical) = (window.outer_position(), window.outer_size())
            else {
                return self.fallback_settings_position(ctx, size);
            };
            let pet_rect = PhysicalRect::new(
                position.x,
                position.y,
                size_physical.width.max(1) as i32,
                size_physical.height.max(1) as i32,
            );
            if let Some(monitor) = monitor_for_rect(&self.physical_monitors(window), pet_rect) {
                let scale = monitor.scale_factor.max(0.1) as f32;
                let to_logical = |rect: PhysicalRect| {
                    egui::Rect::from_min_size(
                        egui::pos2(rect.x as f32 / scale, rect.y as f32 / scale),
                        egui::vec2(rect.width as f32 / scale, rect.height as f32 / scale),
                    )
                };
                return settings_position_for_pet(
                    to_logical(pet_rect),
                    size,
                    to_logical(monitor.work_area),
                );
            }
        }
        self.fallback_settings_position(ctx, size)
    }

    /// Portable fallback: winit's `monitor_size`, which has no origin.
    fn fallback_settings_position(&self, ctx: &egui::Context, size: egui::Vec2) -> egui::Pos2 {
        let (monitor, pet_rect) =
            ctx.input(|input| (input.viewport().monitor_size, input.viewport().outer_rect));
        let monitor = monitor.unwrap_or(egui::vec2(1280.0, 800.0));
        let pet_rect =
            pet_rect.unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, size));
        settings_position_for_pet(
            pet_rect,
            size,
            egui::Rect::from_min_size(egui::Pos2::ZERO, monitor),
        )
    }

    fn draw_settings(&mut self, root_ui: &mut egui::Ui) {
        self.handle_dropped_files(root_ui.ctx());
        if root_ui
            .ctx()
            .input(|input| input.viewport().close_requested())
        {
            self.settings_open = false;
            self.settings_focus_pending = false;
            self.settings_focus_deadline = None;
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
                            (self.target_scale() - *value).abs() < f32::EPSILON,
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
                    .on_hover_text("登录后在后台启动 Petsona")
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
                ui.label("当前平台不支持在设置里配置开机自启动。");
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
                self.settings_focus_pending = false;
                self.settings_focus_deadline = None;
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
    pub(super) fn sync_state_server(&mut self) {
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
}

/// Place the settings window next to the pet, inside `work` (same logical
/// space). Prefer the right side and flip to the left when it does not fit.
fn settings_position_for_pet(pet: egui::Rect, size: egui::Vec2, work: egui::Rect) -> egui::Pos2 {
    const GAP: f32 = 12.0;
    let right = pet.right() + GAP;
    let left = pet.left() - size.x - GAP;
    let x = if right + size.x <= work.right() {
        right
    } else {
        left
    };
    let max_x = (work.right() - size.x).max(work.left());
    let max_y = (work.bottom() - size.y).max(work.top());
    egui::pos2(
        x.clamp(work.left(), max_x),
        (pet.bottom() - size.y).clamp(work.top(), max_y),
    )
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

#[cfg(test)]
mod tests {
    use super::settings_position_for_pet;

    fn secondary_work_area() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(1920.0, 0.0), egui::vec2(1920.0, 1080.0))
    }

    #[test]
    fn settings_open_on_the_pets_secondary_monitor() {
        let pet = egui::Rect::from_min_size(egui::pos2(2200.0, 300.0), egui::vec2(192.0, 208.0));
        let position =
            settings_position_for_pet(pet, egui::vec2(640.0, 720.0), secondary_work_area());
        assert!(
            position.x >= 1920.0,
            "settings must stay on the pet monitor"
        );
        assert!(position.x + 640.0 <= 3840.0);
    }

    #[test]
    fn settings_flip_to_the_left_when_the_right_edge_is_full() {
        let pet = egui::Rect::from_min_size(egui::pos2(3600.0, 100.0), egui::vec2(192.0, 208.0));
        let position =
            settings_position_for_pet(pet, egui::vec2(640.0, 720.0), secondary_work_area());
        assert!(position.x < pet.left());
        assert!(position.x >= 1920.0);
    }
}
