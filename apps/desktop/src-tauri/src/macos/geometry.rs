//! Physical pixels relative to one display. AppKit global points exist only at
//! these conversion boundaries, so mixed-DPI screens never share a fake scale.
use petsona_core::config::WindowPosition;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LogicalRect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Debug)]
pub struct Display {
    pub id: String,
    pub bounds: LogicalRect,
    pub backing: f64,
    pub work: PhysicalRect,
}

impl Display {
    pub fn new(id: String, bounds: LogicalRect, visible: LogicalRect, backing: f64) -> Self {
        let mut display = Self {
            id,
            bounds,
            backing: backing.max(1.0),
            work: PhysicalRect::default(),
        };
        display.work = display.physical_from_points(visible);
        display
    }

    pub fn physical_from_points(&self, rect: LogicalRect) -> PhysicalRect {
        PhysicalRect {
            x: (rect.x - self.bounds.x) * self.backing,
            y: (self.bounds.y + self.bounds.height - rect.y - rect.height) * self.backing,
            width: rect.width * self.backing,
            height: rect.height * self.backing,
        }
    }

    pub fn to_points(&self, rect: PhysicalRect) -> LogicalRect {
        LogicalRect {
            x: self.bounds.x + rect.x / self.backing,
            y: self.bounds.y + self.bounds.height - (rect.y + rect.height) / self.backing,
            width: rect.width / self.backing,
            height: rect.height / self.backing,
        }
    }

    pub fn clamp(&self, mut rect: PhysicalRect) -> PhysicalRect {
        let fit = (self.work.width / rect.width.max(1.0))
            .min(self.work.height / rect.height.max(1.0))
            .min(1.0);
        rect.width *= fit;
        rect.height *= fit;
        rect.x = rect.x.clamp(
            self.work.x,
            (self.work.x + self.work.width - rect.width).max(self.work.x),
        );
        rect.y = rect.y.clamp(
            self.work.y,
            (self.work.y + self.work.height - rect.height).max(self.work.y),
        );
        rect
    }

    pub fn restore(&self, saved: &WindowPosition, width: f64, height: f64) -> PhysicalRect {
        // Exactly matches the old sidecar: decode offsets using the saved
        // density, then re-encode at the selected display's current density.
        let old_backing = f64::from(saved.backing_scale).max(1.0);
        self.clamp(PhysicalRect {
            x: f64::from(saved.x) / old_backing * self.backing,
            y: f64::from(saved.y) / old_backing * self.backing,
            width,
            height,
        })
    }

    pub fn save(&self, rect: PhysicalRect) -> WindowPosition {
        WindowPosition {
            x: rect.x as f32,
            y: rect.y as f32,
            display_id: Some(self.id.clone()),
            backing_scale: self.backing as f32,
        }
    }

    pub fn default_rect(&self, width: f64, height: f64) -> PhysicalRect {
        self.clamp(PhysicalRect {
            x: self.work.x + self.work.width - width - 24.0 * self.backing,
            y: self.work.y + self.work.height - height - 24.0 * self.backing,
            width,
            height,
        })
    }
}

pub fn resize_anchored(rect: PhysicalRect, width: f64, height: f64) -> PhysicalRect {
    PhysicalRect {
        x: rect.x + (rect.width - width) / 2.0,
        y: rect.y + rect.height - height,
        width,
        height,
    }
}

pub fn legacy_points(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    backing: f64,
    global_top: f64,
) -> LogicalRect {
    LogicalRect {
        x: x / backing,
        y: global_top - y / backing - height / backing,
        width: width / backing,
        height: height / backing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn display() -> Display {
        Display::new(
            "42".into(),
            LogicalRect {
                x: -1280.0,
                y: -120.0,
                width: 1280.0,
                height: 800.0,
            },
            LogicalRect {
                x: -1280.0,
                y: -80.0,
                width: 1280.0,
                height: 736.0,
            },
            2.0,
        )
    }
    #[test]
    fn retina_and_negative_origin_roundtrip_is_physical() {
        let display = display();
        let rect = PhysicalRect {
            x: 200.0,
            y: 300.0,
            width: 192.0,
            height: 208.0,
        };
        assert_eq!(display.physical_from_points(display.to_points(rect)), rect);
        assert_eq!(display.to_points(rect).width, 96.0);
        assert_eq!(display.work.y, 48.0);
    }
    #[test]
    fn saved_density_and_missing_display_fallback_keep_legacy_offsets() {
        let display = display();
        let saved = WindowPosition {
            x: 100.0,
            y: 200.0,
            backing_scale: 1.0,
            display_id: Some("unavailable".into()),
        };
        let rect = display.restore(&saved, 192.0, 208.0);
        assert_eq!((rect.x, rect.y), (200.0, 400.0));
        let next = display.save(rect);
        assert_eq!(display.restore(&next, 192.0, 208.0), rect);
    }
    #[test]
    fn clamp_respects_menu_dock_and_oversized_pet_aspect() {
        let display = display();
        let rect = display.clamp(PhysicalRect {
            x: -200.0,
            y: 9000.0,
            width: 192.0,
            height: 208.0,
        });
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y + rect.height, display.work.y + display.work.height);
        let huge = display.clamp(PhysicalRect {
            width: 9000.0,
            height: 18000.0,
            ..rect
        });
        assert_eq!(huge.width / huge.height, 0.5);
        assert!(huge.height <= display.work.height);
    }
    #[test]
    fn resize_keeps_bottom_centre_and_legacy_top_left_conversion() {
        let rect = PhysicalRect {
            x: 100.0,
            y: 100.0,
            width: 192.0,
            height: 208.0,
        };
        let larger = resize_anchored(rect, 384.0, 416.0);
        assert_eq!(larger.x + larger.width / 2.0, rect.x + rect.width / 2.0);
        assert_eq!(larger.y + larger.height, rect.y + rect.height);
        let legacy = legacy_points(400.0, 600.0, 192.0, 208.0, 2.0, 1000.0);
        assert_eq!((legacy.x, legacy.y), (200.0, 596.0));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Bottom,
    Left,
    Right,
}

pub fn bubble_rect(display: &Display, pet: PhysicalRect, width: f64, height: f64) -> PhysicalRect {
    let gap = 10.0 * display.backing;
    let above = pet.y - height - gap;
    let y = if above >= display.work.y + 8.0 * display.backing {
        above
    } else {
        pet.y + pet.height + gap
    };
    display.clamp(PhysicalRect {
        x: pet.x + (pet.width - width) / 2.0,
        y,
        width,
        height,
    })
}

pub fn panel_rect(
    display: &Display,
    pet: PhysicalRect,
    width: f64,
    height: f64,
    held: Option<Side>,
) -> (Side, PhysicalRect) {
    let gap = 12.0 * display.backing;
    let bottom = pet.y + pet.height + gap;
    let below = display.work.y + display.work.height - bottom;
    let side = if below >= height && !matches!(held, Some(Side::Left | Side::Right)) {
        Side::Bottom
    } else if let Some(side @ (Side::Left | Side::Right)) = held {
        side
    } else if pet.x - display.work.x >= display.work.x + display.work.width - pet.x - pet.width {
        Side::Left
    } else {
        Side::Right
    };
    let (x, y) = match side {
        Side::Bottom => (pet.x + (pet.width - width) / 2.0, bottom),
        Side::Left => (
            pet.x - width - gap,
            pet.y + pet.height - height + 6.0 * display.backing,
        ),
        Side::Right => (
            pet.x + pet.width + gap,
            pet.y + pet.height - height + 6.0 * display.backing,
        ),
    };
    (
        side,
        display.clamp(PhysicalRect {
            x,
            y,
            width,
            height,
        }),
    )
}

#[cfg(test)]
mod panel_tests {
    use super::*;
    #[test]
    fn bubble_flips_and_composer_locks_side_at_the_dock() {
        let display = Display::new(
            "1".into(),
            LogicalRect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            LogicalRect {
                x: 0.0,
                y: 40.0,
                width: 800.0,
                height: 536.0,
            },
            2.0,
        );
        let high = PhysicalRect {
            x: 600.0,
            y: 48.0,
            width: 192.0,
            height: 208.0,
        };
        assert!(bubble_rect(&display, high, 400.0, 120.0).y > high.y);
        let low = PhysicalRect {
            x: 1200.0,
            y: display.work.y + display.work.height - 208.0,
            ..high
        };
        let (side, panel) = panel_rect(&display, low, 592.0, 88.0, None);
        assert_eq!(side, Side::Left);
        assert!(panel.x + panel.width <= low.x);
        assert_eq!(panel_rect(&display, high, 592.0, 88.0, Some(side)).0, side);
    }
}
