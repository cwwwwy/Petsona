use super::geometry::lerp;
use super::*;

const SHADOW_TITLE: &str = "Petsona 影子";
pub(super) const SHADOW_CENTER_GAP: f32 = 22.0;
const SHADOW_TRANSITION: Duration = Duration::from_millis(180);
pub(super) const SHADOW_BUTTON_SIZE: f32 = 34.0;
pub(super) const SHADOW_HIT_SIZE: f32 = 40.0;
const SHADOW_WINDOW_SIZE: egui::Vec2 = egui::vec2(SHADOW_HIT_SIZE, SHADOW_HIT_SIZE);

pub(super) fn shadow_center(pet_window: egui::Vec2) -> egui::Pos2 {
    egui::pos2(pet_window.x * 0.5, pet_window.y + SHADOW_CENTER_GAP)
}

pub(super) fn shadow_hit_rect(pet_window: egui::Vec2) -> egui::Rect {
    egui::Rect::from_center_size(
        shadow_center(pet_window),
        egui::vec2(SHADOW_HIT_SIZE, SHADOW_HIT_SIZE),
    )
}

impl PetsonaApp {
    /// A small, always-present shadow below the pet. Hovering morphs it into
    /// an edit button. Once the composer is warmed, it takes over the same
    /// anchor so the button and composer never compete for the same pixels.
    pub(super) fn show_shadow_viewport(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let shadow_id = egui::ViewportId::from_hash_of("petsona-shadow");
        if !self.pet_visible {
            self.shadow_hovered = false;
            self.shadow_hover_progress = 0.0;
            if self.shadow_window_created {
                ctx.send_viewport_cmd_to(shadow_id, egui::ViewportCommand::Visible(false));
                self.shadow_window_created = false;
            }
            return;
        }
        if self.conversation_window_warmed
            && (self.conversation_open || self.conversation_closing_at.is_some())
        {
            self.shadow_hovered = false;
            if self.shadow_window_created {
                ctx.send_viewport_cmd_to(shadow_id, egui::ViewportCommand::Visible(false));
                self.shadow_window_created = false;
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
        let center = parent + shadow_center(pet_window).to_vec2();
        let window_position = center - SHADOW_WINDOW_SIZE * 0.5;

        if !self.shadow_window_warmed {
            let builder = egui::ViewportBuilder::default()
                .with_title(SHADOW_TITLE)
                .with_inner_size([SHADOW_WINDOW_SIZE.x, SHADOW_WINDOW_SIZE.y])
                .with_position([window_position.x, window_position.y])
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_resizable(false)
                .with_active(false)
                .with_visible(false);
            ctx.show_viewport_immediate(shadow_id, builder, |_ui, _class| {});
            self.shadow_window_warmed = true;
            let _ = self.platform.set_no_activate_for_title(SHADOW_TITLE);
            return;
        }

        let pointer = self
            .pointer
            .position
            .map(|(x, y)| egui::pos2(x as f32 / scale, y as f32 / scale));
        let hover_rect =
            egui::Rect::from_center_size(center, egui::vec2(SHADOW_HIT_SIZE, SHADOW_HIT_SIZE));
        self.shadow_hovered = pointer.is_some_and(|point| hover_rect.contains(point));

        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.shadow_last_tick)
            .as_secs_f32()
            .min(0.05);
        self.shadow_last_tick = now;
        let target = if self.shadow_hovered { 1.0 } else { 0.0 };
        let delta = target - self.shadow_hover_progress;
        if delta.abs() > f32::EPSILON {
            let step = dt / SHADOW_TRANSITION.as_secs_f32();
            self.shadow_hover_progress += delta.signum() * step.min(delta.abs());
            self.shadow_hover_progress = self.shadow_hover_progress.clamp(0.0, 1.0);
        }

        let builder = egui::ViewportBuilder::default()
            .with_title(SHADOW_TITLE)
            .with_inner_size([SHADOW_WINDOW_SIZE.x, SHADOW_WINDOW_SIZE.y])
            .with_position([window_position.x, window_position.y])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_mouse_passthrough(!self.shadow_hovered)
            .with_visible(true);
        let mut open_conversation = false;
        ctx.show_viewport_immediate(shadow_id, builder, |ui, _class| {
            let rect = ui.max_rect();
            draw_shadow_button(ui.painter(), rect.center(), self.shadow_hover_progress);
            let sense = if self.shadow_hover_progress >= 1.0 {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let response = ui.interact(
                egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(SHADOW_BUTTON_SIZE, SHADOW_BUTTON_SIZE),
                ),
                ui.id().with("shadow-edit"),
                sense,
            );
            if response.clicked() {
                open_conversation = true;
            }
        });
        if open_conversation {
            self.open_conversation();
        }
        self.shadow_window_created = true;
        // Re-assert this every visible frame: winit can restore the decorated
        // style after the first show or after a mouse-passthrough update.
        let _ = self.platform.set_no_activate_for_title(SHADOW_TITLE);
    }
}

fn draw_shadow_button(painter: &egui::Painter, center: egui::Pos2, progress: f32) {
    let progress = progress.clamp(0.0, 1.0);
    let radius = egui::vec2(
        lerp(10.0, SHADOW_BUTTON_SIZE * 0.5, progress),
        lerp(3.5, SHADOW_BUTTON_SIZE * 0.5, progress),
    );
    let target_fill = edit_button_background();
    let fill = egui::Color32::from_rgba_unmultiplied(
        lerp(0.0, target_fill.r() as f32, progress) as u8,
        lerp(0.0, target_fill.g() as f32, progress) as u8,
        lerp(0.0, target_fill.b() as f32, progress) as u8,
        lerp(78.0, target_fill.a() as f32, progress) as u8,
    );
    painter.add(egui::Shape::ellipse_filled(center, radius, fill));
    if progress > 0.08 {
        let stroke = edit_button_stroke();
        let stroke = egui::Stroke::new(
            stroke.width,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, (20.0 * progress) as u8),
        );
        painter.add(egui::Shape::ellipse_stroke(center, radius, stroke));
    }
    if progress > 0.08 {
        draw_edit_icon(
            painter,
            center,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * progress) as u8),
        );
    }
}

pub(super) fn edit_button_background() -> egui::Color32 {
    egui::Color32::from_rgb(28, 28, 32)
}

pub(super) fn edit_button_stroke() -> egui::Stroke {
    egui::Stroke::new(
        1.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
    )
}

pub(super) fn draw_edit_icon(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    let tail = center + egui::vec2(-6.5, 6.5);
    let head = center + egui::vec2(3.5, -3.5);
    painter.line_segment([tail, head], egui::Stroke::new(3.0, color));
    painter.line_segment(
        [tail + egui::vec2(-2.0, 2.0), tail + egui::vec2(0.5, -0.5)],
        egui::Stroke::new(1.5, color),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        shadow_hit_rect, SHADOW_BUTTON_SIZE, SHADOW_CENTER_GAP, SHADOW_HIT_SIZE, SHADOW_WINDOW_SIZE,
    };

    #[test]
    fn shadow_button_and_hit_target_stay_below_the_pet_window() {
        let pet_window = egui::vec2(220.0, 318.0);
        let hit = shadow_hit_rect(pet_window);
        let button_top = pet_window.y + SHADOW_CENTER_GAP - SHADOW_BUTTON_SIZE * 0.5;
        let overlay_top = pet_window.y + SHADOW_CENTER_GAP - SHADOW_WINDOW_SIZE.y * 0.5;

        assert!(button_top >= pet_window.y);
        assert!(overlay_top >= pet_window.y);
        assert!(hit.top() >= pet_window.y);
        assert_eq!(hit.width(), SHADOW_HIT_SIZE);
    }
}
