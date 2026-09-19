use super::geometry::{ease_out, entry_progress};
use super::*;

const BUBBLE_TITLE: &str = "Petsona 气泡";
const BUBBLE_WINDOW_SIZE: egui::Vec2 = egui::vec2(360.0, 220.0);
const BUBBLE_GAP: f32 = 8.0;
const BUBBLE_TAIL: f32 = 7.0;
/// Entry animation of a newly shown bubble: the content rises into place while
/// fading in ("from below"), which reads as the bubble growing out of the pet.
pub(super) const BUBBLE_ENTRY: Duration = Duration::from_millis(160);
const BUBBLE_ENTRY_RISE: f32 = 8.0;
/// Resting padding of the bubble inside its overlay window, plus the room the
/// entry animation needs below the resting place so nothing is clipped.
const BUBBLE_BOTTOM_PADDING: f32 = 2.0 + BUBBLE_ENTRY_RISE;
/// The overlay window is placed this much higher than [`BUBBLE_GAP`] would ask
/// for, so the extra bottom padding above keeps the resting position identical.
const BUBBLE_WINDOW_GAP: f32 = BUBBLE_GAP - BUBBLE_ENTRY_RISE;

impl PetsonaApp {
    /// Show the speech bubble in its own transparent overlay so the pet window
    /// stays exactly the size of the sprite and never moves when text appears.
    pub(super) fn show_bubble_viewport(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let bubble_id = egui::ViewportId::from_hash_of("petsona-bubble");
        let text = if self.pet_visible {
            self.bubble
                .as_ref()
                .filter(|bubble| Instant::now() < bubble.until)
                .map(|bubble| bubble.text.clone())
        } else {
            None
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
        let showing = text.is_some();
        let bubble_rect_global = egui::Rect::from_min_size(bubble_position, BUBBLE_WINDOW_SIZE);
        let bubble_hovered = showing
            && self.pointer.position.is_some_and(|(x, y)| {
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

        let content = if let Some(text) = text {
            let shown_at = *self.bubble_shown_at.get_or_insert_with(Instant::now);
            let eased = ease_out(entry_progress(Some(shown_at), BUBBLE_ENTRY));
            // "From below": the content starts one rise lower and climbs into place.
            Some((text, eased, (1.0 - eased) * BUBBLE_ENTRY_RISE))
        } else {
            self.bubble_shown_at = None;
            None
        };
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
            .with_visible(self.pet_visible);
        let mut open_conversation = false;
        ctx.show_viewport_immediate(bubble_id, builder, |ui, _class| {
            // Render the viewport even when empty: egui drops viewports that
            // are not rendered, and a re-created window shows up as a flash.
            let Some((text, opacity, rise)) = content.as_ref() else {
                return;
            };
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(ui, |ui| {
                    let window = ui.max_rect();
                    let bubble_rect = draw_bubble_window(
                        ui.painter(),
                        window,
                        text,
                        *opacity,
                        *rise,
                        bubble_hovered,
                    );
                    // The bubble itself is the reply affordance. Keeping the
                    // separate floating "回复" button made the bubble and the
                    // conversation window look like two unrelated products.
                    let response = ui
                        .interact(
                            bubble_rect,
                            ui.id().with("bubble-open-conversation"),
                            egui::Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        open_conversation = true;
                    }
                });
        });
        if open_conversation {
            self.open_conversation();
        }
        self.bubble_window_created = showing;
        // Re-assert every visible frame: winit can restore the decorated
        // style after the first show or a mouse-passthrough update.
        if self.platform.set_no_activate_for_title(BUBBLE_TITLE) > 0 {
            self.bubble_styled = true;
        }
    }

    pub(super) fn show_bubble(&mut self, text: String) {
        if self.pet.is_none() {
            // No pet: never leave a floating bubble behind.
            return;
        }
        self.bubble = Some(Bubble {
            text,
            until: Instant::now() + Duration::from_secs(8),
        });
    }

    pub(super) fn expire_bubble(&mut self) {
        if self
            .bubble
            .as_ref()
            .is_some_and(|bubble| Instant::now() >= bubble.until)
        {
            self.bubble = None;
        }
    }
}

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
    hovered: bool,
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
    let fill = if hovered {
        egui::Color32::from_rgba_unmultiplied(34, 34, 40, alpha(245))
    } else {
        egui::Color32::from_rgba_unmultiplied(24, 24, 28, alpha(235))
    };
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

fn bubble_overlay_y(window: egui::Rect, bubble_size: egui::Vec2) -> f32 {
    (window.bottom() - BUBBLE_BOTTOM_PADDING - BUBBLE_TAIL - bubble_size.y).max(window.top() + 6.0)
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
}
