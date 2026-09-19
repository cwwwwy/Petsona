use super::geometry::lerp;
use super::*;

const SHADOW_TITLE: &str = "Petsona 影子";
pub(super) const SHADOW_CENTER_GAP: f32 = 22.0;
const SHADOW_TRANSITION: Duration = Duration::from_millis(180);
pub(super) const SHADOW_BUTTON_SIZE: f32 = 34.0;
pub(super) const SHADOW_HIT_SIZE: f32 = 40.0;
/// How far below the overlay window's top edge the resting shadow sits. The
/// window itself has to stay clear of the pet (its 40pt hit target must never
/// cover the sprite), so the drawn shadow moves up inside the window instead.
const SHADOW_IDLE_DROP: f32 = 6.0;
const SHADOW_IDLE_RADIUS_Y: f32 = 3.5;
const SHADOW_MAX_RADIUS_X: f32 = 76.0;

/// The resting shadow follows the pet's width so it reads as the pet's shadow
/// at every scale. The hover button keeps its fixed size.
pub(super) fn shadow_idle_radius(pet_window: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        (pet_window.x * 0.19).clamp(10.0, SHADOW_MAX_RADIUS_X),
        SHADOW_IDLE_RADIUS_Y,
    )
}

/// The overlay is created once at its maximum size (widest resting shadow
/// plus padding) because winit ignores resize requests for non-resizable
/// windows: a per-frame size would silently never apply, and the shadow would
/// look constant across pet sizes. The drawn ellipse scales inside.
pub(super) const SHADOW_WINDOW_SIZE: egui::Vec2 =
    egui::vec2(SHADOW_MAX_RADIUS_X * 2.0 + 8.0, SHADOW_HIT_SIZE);

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
        // The overlay is created once and rendered every frame from then on:
        // stopping the render made egui destroy the viewport, and the next
        // show re-created the window (an occasional visible "restart").
        let composer_active = self.conversation_window_warmed
            && (self.conversation_open || self.conversation_closing_at.is_some());
        let showing = self.pet.is_some() && self.pet_visible && !composer_active;
        if !showing {
            self.shadow_hovered = false;
            self.shadow_hover_progress = 0.0;
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
        // The window keeps a minimum width so the pet's click target stays
        // usable, but the shadow has to follow the *sprite*, otherwise the
        // smaller presets all render the same size.
        let pet_size = self.pet_size();
        let window_size = SHADOW_WINDOW_SIZE;
        let center = parent + shadow_center(pet_window).to_vec2();
        let window_position = center - window_size * 0.5;

        if !self.shadow_window_warmed {
            let builder = egui::ViewportBuilder::default()
                .with_title(SHADOW_TITLE)
                .with_inner_size([window_size.x, window_size.y])
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
        self.shadow_hovered = showing && pointer.is_some_and(|point| hover_rect.contains(point));

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
            .with_inner_size([window_size.x, window_size.y])
            .with_position([window_position.x, window_position.y])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_mouse_passthrough(!self.shadow_hovered)
            .with_visible(self.pet_visible);
        let mut open_conversation = false;
        ctx.show_viewport_immediate(shadow_id, builder, |ui, _class| {
            if !showing {
                return;
            }
            let rect = ui.max_rect();
            let idle_center = egui::pos2(rect.center().x, rect.top() + SHADOW_IDLE_DROP);
            draw_shadow_button(
                ui.painter(),
                idle_center,
                rect.center(),
                shadow_idle_radius(pet_size),
                self.shadow_hover_progress,
            );
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
        self.shadow_window_created = showing;
        // Re-assert this every visible frame: winit can restore the decorated
        // style after the first show or after a mouse-passthrough update.
        let _ = self.platform.set_no_activate_for_title(SHADOW_TITLE);
    }
}

fn draw_shadow_button(
    painter: &egui::Painter,
    idle_center: egui::Pos2,
    button_center: egui::Pos2,
    idle_radius: egui::Vec2,
    progress: f32,
) {
    let progress = progress.clamp(0.0, 1.0);
    let center = egui::pos2(
        lerp(idle_center.x, button_center.x, progress),
        lerp(idle_center.y, button_center.y, progress),
    );
    let radius = egui::vec2(
        lerp(idle_radius.x, SHADOW_BUTTON_SIZE * 0.5, progress),
        lerp(idle_radius.y, SHADOW_BUTTON_SIZE * 0.5, progress),
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
        shadow_hit_rect, shadow_idle_radius, SHADOW_BUTTON_SIZE, SHADOW_CENTER_GAP,
        SHADOW_HIT_SIZE, SHADOW_IDLE_DROP, SHADOW_WINDOW_SIZE,
    };

    #[test]
    fn shadow_button_and_hit_target_stay_below_the_pet_window() {
        let pet_window = egui::vec2(220.0, 318.0);
        let overlay_size = SHADOW_WINDOW_SIZE;
        let hit = shadow_hit_rect(pet_window);
        let button_top = pet_window.y + SHADOW_CENTER_GAP - SHADOW_BUTTON_SIZE * 0.5;
        let overlay_top = pet_window.y + SHADOW_CENTER_GAP - overlay_size.y * 0.5;
        let idle_top = overlay_top + SHADOW_IDLE_DROP - shadow_idle_radius(pet_window).y;

        assert!(button_top >= pet_window.y);
        assert!(overlay_top >= pet_window.y);
        assert!(idle_top >= pet_window.y);
        assert!(hit.top() >= pet_window.y);
        assert_eq!(hit.width(), SHADOW_HIT_SIZE);
        // The resting shadow follows the pet width instead of a fixed 20pt.
        assert!(shadow_idle_radius(pet_window).x > 10.0);
        assert!(shadow_idle_radius(egui::vec2(384.0, 416.0)).x > shadow_idle_radius(pet_window).x);
        // The pet window has a minimum width, so the shadow has to be driven
        // by the sprite size: 96pt (mini) must be visibly narrower than 192pt.
        assert!(
            shadow_idle_radius(egui::vec2(96.0, 104.0)).x
                < shadow_idle_radius(egui::vec2(192.0, 208.0)).x
        );
    }
}
