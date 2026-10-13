//! Pure Windows composer placement policy, tested without moving a pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Bottom,
    Left,
    Right,
}

// Rectangles use physical [left, top, right, bottom] coordinates.
pub fn choose_side(
    pet: [i32; 4],
    work: [i32; 4],
    height: i32,
    width: i32,
    current: Option<Side>,
) -> Side {
    let below = work[3] - pet[3] - 12 - 8;
    let right = work[2] - pet[2] - 12 - 8;
    let left = pet[0] - work[0] - 12 - 8;
    match current {
        Some(Side::Bottom) | None => {
            if below >= height {
                Side::Bottom
            } else if right >= left {
                Side::Right
            } else {
                Side::Left
            }
        }
        Some(side @ (Side::Left | Side::Right)) => {
            if below >= height + 16 {
                return Side::Bottom;
            }
            let (available, other) = if side == Side::Right {
                (right, left)
            } else {
                (left, right)
            };
            if available >= width || available >= other {
                side
            } else if side == Side::Right {
                Side::Left
            } else {
                Side::Right
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_open_composer_follows_a_drag_to_bottom_and_back_with_hysteresis() {
        let work = [0, 0, 1920, 1080];
        let mut side = None;
        for (bottom, expected) in [
            (980, Side::Bottom),
            (1016, Side::Bottom),
            (1020, Side::Right),
            (1016, Side::Right),
            (1000, Side::Bottom),
        ] {
            side = Some(choose_side(
                [500, bottom - 208, 788, bottom],
                work,
                44,
                296,
                side,
            ));
            assert_eq!(side, Some(expected));
        }
    }
    #[test]
    fn horizontal_drag_switches_only_when_current_side_no_longer_fits() {
        let work = [0, 0, 1920, 1080];
        let mut side = Some(Side::Right);
        for (left, expected) in [
            (500, Side::Right),
            (1100, Side::Right),
            (1400, Side::Left),
            (1000, Side::Left),
            (0, Side::Right),
        ] {
            side = Some(choose_side(
                [left, 850, left + 288, 1060],
                work,
                44,
                296,
                side,
            ));
            assert_eq!(side, Some(expected));
        }
    }
    #[test]
    fn side_selection_is_invariant_under_negative_monitor_origin() {
        let work = [0, 0, 1920, 1080];
        let pet = [1400, 850, 1688, 1060];
        for current in [
            None,
            Some(Side::Bottom),
            Some(Side::Left),
            Some(Side::Right),
        ] {
            let shifted_pet = [pet[0] - 1920, pet[1] - 200, pet[2] - 1920, pet[3] - 200];
            assert_eq!(
                choose_side(pet, work, 44, 296, current),
                choose_side(shifted_pet, [-1920, -200, 0, 880], 44, 296, current)
            );
        }
    }
}
