use super::*;

impl PetsonaApp {
    pub(super) fn active_pet_id(&self) -> String {
        self.pet
            .as_ref()
            .map(|pet| pet.entry.id.clone())
            .unwrap_or_default()
    }

    /// Re-scan the app-local library. Codex pets are only read when the user
    /// imports them from the settings window.
    pub(super) fn refresh_pets(&mut self) {
        self.pets = self.library.list();
        self.pet_preview = None;
        self.refresh_native_tray_menu();
        self.publish_health();
    }

    /// Icon of the active pet (its idle pose), built once per pet and reused by
    /// the settings window, the menu and the tray.
    pub(super) fn pet_icon(&mut self) -> Option<std::sync::Arc<egui::IconData>> {
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
    pub(super) fn refresh_tray_icon(&mut self) {
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
    pub(super) fn import_path(&mut self, path: &Path, overwrite: bool) {
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
    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
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
    pub(super) fn files_are_hovering(ctx: &egui::Context) -> bool {
        ctx.input(|input| !input.raw.hovered_files.is_empty())
    }

    /// Write the Codex upload format next to the config directory.
    pub(super) fn export_pet_zip(&mut self, id: &str) {
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
    pub(super) fn remove_local_pet(&mut self, id: &str) {
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

    pub(super) fn active_pet_name(&self) -> String {
        self.pet
            .as_ref()
            .map(|pet| pet.entry.display_name.clone())
            .unwrap_or_else(|| "（无）".to_string())
    }
}
