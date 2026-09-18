use std::time::{Duration, Instant};

use crate::platform::PhysicalRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PhysicalMonitor {
    pub(super) bounds: PhysicalRect,
    pub(super) work_area: PhysicalRect,
    pub(super) scale_factor: f64,
}

pub(super) fn bottom_center_anchor(position: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
    position + egui::vec2(size.x * 0.5, size.y)
}

pub(super) fn position_for_bottom_center(anchor: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
    anchor - egui::vec2(size.x * 0.5, size.y)
}

/// Keep gaze local to the pet while adding a small release margin so a cursor
/// near the trigger edge does not make the pose flicker on and off.
pub(super) fn cursor_within_gaze_range(
    dx: f64,
    dy: f64,
    pet_size: egui::Vec2,
    already_gazing: bool,
) -> bool {
    const ENTER_MARGIN: f64 = 0.25;
    const EXIT_MARGIN: f64 = 0.35;
    let margin = pet_size.x.min(pet_size.y).max(1.0) as f64
        * if already_gazing {
            EXIT_MARGIN
        } else {
            ENTER_MARGIN
        };
    let radius_x = pet_size.x.max(1.0) as f64 * 0.5 + margin;
    let radius_y = pet_size.y.max(1.0) as f64 * 0.5 + margin;
    let normalized_x = dx / radius_x;
    let normalized_y = dy / radius_y;
    normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
}

/// Clamp `rect` so it stays inside `work`; a rect larger than the work area is
/// pinned to its top-left corner instead of being pushed outside.
pub(super) fn clamp_rect_to_work_area(rect: PhysicalRect, work: PhysicalRect) -> PhysicalRect {
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
pub(super) fn monitor_for_point(
    monitors: &[PhysicalMonitor],
    x: i32,
    y: i32,
) -> Option<PhysicalMonitor> {
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

pub(super) fn monitor_for_rect(
    monitors: &[PhysicalMonitor],
    rect: PhysicalRect,
) -> Option<PhysicalMonitor> {
    let (x, y) = rect.center();
    monitor_for_point(monitors, x, y)
}

/// Does the window rectangle fit inside the work area (with a couple of pixels
/// of slack for DPI rounding)?
#[cfg(any(feature = "test-hooks", test))]
pub(super) fn rect_within_work_area(rect: PhysicalRect, work: PhysicalRect) -> bool {
    const TOLERANCE: i32 = 2;
    rect.x >= work.x - TOLERANCE
        && rect.y >= work.y - TOLERANCE
        && rect.right() <= work.right() + TOLERANCE
        && rect.bottom() <= work.bottom() + TOLERANCE
}

/// Keep a popup inside the monitor it was opened on.
pub(super) fn clamp_to_monitor(
    ctx: &egui::Context,
    anchor: egui::Pos2,
    size: egui::Vec2,
) -> egui::Pos2 {
    let monitor = ctx
        .input(|input| input.viewport().monitor_size)
        .unwrap_or(egui::vec2(1280.0, 800.0));
    egui::pos2(
        anchor.x.clamp(0.0, (monitor.x - size.x).max(0.0)),
        anchor.y.clamp(0.0, (monitor.y - size.y).max(0.0)),
    )
}

/// Progress (0..=1) of a viewport entry animation, easing not applied.
pub(super) fn entry_progress(started: Option<Instant>, duration: Duration) -> f32 {
    let Some(started) = started else {
        return 1.0;
    };
    (started.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

pub(super) fn lerp(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}

/// Smooth start and stop, used for the collapse back into the pet shadow.
pub(super) fn smoothstep(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

/// Ease-out cubic: fast start, soft landing.
pub(super) fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(test)]
mod tests {
    use super::cursor_within_gaze_range;

    #[test]
    fn gaze_trigger_is_near_the_pet_with_release_hysteresis() {
        let pet = egui::vec2(220.0, 318.0);

        assert!(cursor_within_gaze_range(110.0, 159.0, pet, false));
        assert!(!cursor_within_gaze_range(170.0, 0.0, pet, false));
        assert!(cursor_within_gaze_range(170.0, 0.0, pet, true));
        assert!(!cursor_within_gaze_range(190.0, 0.0, pet, true));
        assert!(!cursor_within_gaze_range(0.0, 220.0, pet, false));
        assert!(cursor_within_gaze_range(0.0, 220.0, pet, true));
    }
}
