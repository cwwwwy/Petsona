use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use petsona_core::pet::state::{PetEngine, PetState};
use petsona_core::pet::{PetAtlas, PetEntry};

/// Fallback delay when no animation is available.
pub const IDLE_REPAINT: Duration = Duration::from_secs(1);

/// Platform-neutral state for one active pet.
///
/// This owns the loaded atlas, animation engine and animation clock. UI
/// shells own textures and render the sprite returned by this session.
pub struct PetSession {
    pub entry: PetEntry,
    pub atlas: PetAtlas,
    pub engine: PetEngine,
    pub cell_width: f32,
    pub cell_height: f32,
    pub anim_started: Instant,
    pub last_state: PetState,
    pub last_sprite: u32,
}

impl PetSession {
    pub fn load(entry: PetEntry) -> Result<Self> {
        let (atlas, warnings) = PetAtlas::open(&entry.dir, &entry.manifest)
            .with_context(|| format!("cannot open pet '{}'", entry.id))?;
        for warning in warnings {
            tracing::warn!(pet = %entry.id, %warning, "pet atlas warning");
        }
        let frame = atlas.frame;
        let engine = PetEngine::from_atlas(&atlas, &entry.manifest);
        Ok(Self {
            entry,
            atlas,
            engine,
            cell_width: frame.width as f32,
            cell_height: frame.height as f32,
            anim_started: Instant::now(),
            last_state: PetState::Idle,
            last_sprite: 0,
        })
    }

    /// Sprite that was drawn last, used for the window / tray icon.
    pub fn current_sprite_index(&self) -> u32 {
        self.last_sprite
    }

    /// Time until the current animation can display a different frame.
    pub fn next_frame_after(&self) -> Duration {
        let elapsed_ms = self.anim_started.elapsed().as_secs_f32() * 1000.0;
        if let Some(after) = self.engine.gaze_next_frame_after(elapsed_ms) {
            return after;
        }
        let Some(animation) = self.engine.current_animation() else {
            return IDLE_REPAINT;
        };
        if animation.total_ms <= 0.0 || animation.durations_ms.is_empty() {
            return IDLE_REPAINT;
        }

        let time_ms = if animation.loop_anim {
            elapsed_ms % animation.total_ms
        } else {
            elapsed_ms
        };
        if !animation.loop_anim && time_ms >= animation.total_ms {
            return Duration::from_millis(1);
        }

        let mut frame_end = 0.0;
        for duration in &animation.durations_ms {
            frame_end += duration.max(1.0);
            if time_ms < frame_end {
                let remaining_ms = (frame_end - time_ms).clamp(1.0, 60_000.0);
                return Duration::from_millis(remaining_ms.ceil() as u64);
            }
        }
        Duration::from_millis(1)
    }

    /// Resolve and advance the current sprite for the shell renderer.
    pub fn current_sprite(&mut self, elapsed_ms: f32) -> u32 {
        let state = self.engine.current();
        if state != self.last_state {
            self.last_state = state;
            self.anim_started = Instant::now();
        }

        if self.engine.gaze_visible() {
            if let Some(sprite) = self.engine.gaze_sprite_at(elapsed_ms) {
                self.last_sprite = sprite;
                return sprite;
            }
            // The return segment completed. Start the base/event animation
            // from its first frame instead of reusing the gaze elapsed time.
            self.anim_started = Instant::now();
            self.last_state = self.engine.current();
            let sprite = self
                .engine
                .current_animation()
                .and_then(|animation| animation.sprite_at(0.0))
                .unwrap_or(0);
            self.last_sprite = sprite;
            return sprite;
        }

        let (sprite, finished) = {
            let Some(animation) = self.engine.current_animation() else {
                return 0;
            };
            (
                animation.sprite_at(elapsed_ms).unwrap_or(0),
                elapsed_ms >= animation.total_ms && animation.total_ms > 0.0,
            )
        };
        if finished && self.engine.current().is_one_shot() {
            self.engine.on_one_shot_finished();
            self.anim_started = Instant::now();
            self.last_state = self.engine.current();
        }
        self.last_sprite = sprite;
        sprite
    }
}
