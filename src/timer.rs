//! Interval-based timer for slideshow auto-advancement.

use std::time::{Duration, Instant};

pub struct SlideshowTimer {
    pub interval: Duration,
    pub paused: bool,
    last_tick: Instant,
}

impl SlideshowTimer {
    pub fn new(interval_secs: f32) -> Self {
        Self {
            interval: Duration::from_secs_f32(interval_secs.max(0.1)),
            paused: interval_secs <= 0.0,
            last_tick: Instant::now(),
        }
    }

    pub fn update(&mut self) -> bool {
        if self.paused {
            return false;
        }

        let now = Instant::now();
        if now.duration_since(self.last_tick) >= self.interval {
            self.last_tick = now;
            true
        } else {
            false
        }
    }

    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        if !self.paused {
            self.last_tick = Instant::now();
        }
        self.paused
    }

    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
    }

    pub fn set_duration(&mut self, duration_secs: f32) {
        if duration_secs <= 0.0 {
            self.paused = true;
        } else {
            self.interval = Duration::from_secs_f32(duration_secs);
            self.paused = false;
            self.last_tick = Instant::now();
        }
    }

    /// Returns the configured interval in seconds regardless of pause state.
    pub fn interval_secs(&self) -> f32 {
        self.interval.as_secs_f32()
    }
}

pub struct SequenceTimer {
    pub fps: f32,
    pub paused: bool,
    last_tick: Instant,
    accumulator: f32,
}

/// Maximum number of sequence frames to advance in a single update tick.
///
/// If a long stall produces more due frames than this cap, the timer advances
/// at most this many frames and drops the remaining whole-frame backlog while
/// preserving the fractional remainder. This keeps playback responsive and
/// prevents a burst of `next_image()` calls in one frame.
const MAX_FRAME_ADVANCE_PER_TICK: usize = 8;

impl SequenceTimer {
    pub fn new(fps: f32) -> Self {
        Self {
            fps: sanitize_fps(fps),
            paused: false,
            last_tick: Instant::now(),
            accumulator: 0.0,
        }
    }

    pub fn update(&mut self) -> usize {
        if self.paused {
            return 0;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        self.advance_by(dt)
    }

    fn advance_by(&mut self, dt: f32) -> usize {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }

        if !self.accumulator.is_finite() {
            self.accumulator = 0.0;
        }

        self.accumulator += dt;
        let frame_duration = 1.0 / self.fps;
        if !frame_duration.is_finite() || frame_duration <= 0.0 {
            self.accumulator = 0.0;
            return 0;
        }
        let frames_due = (self.accumulator / frame_duration) as usize;
        if frames_due == 0 {
            return 0;
        }

        let frames_to_advance = frames_due.min(MAX_FRAME_ADVANCE_PER_TICK);

        if frames_due > MAX_FRAME_ADVANCE_PER_TICK {
            // Stall recovery policy: drop excess whole-frame backlog.
            self.accumulator %= frame_duration;
        } else {
            self.accumulator -= frame_duration * frames_to_advance as f32;
        }
        self.accumulator = self.accumulator.clamp(0.0, frame_duration);
        if self.accumulator >= frame_duration {
            self.accumulator = 0.0;
        }

        frames_to_advance
    }

    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        if !self.paused {
            self.last_tick = Instant::now();
        }
        self.paused
    }

    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
        self.accumulator = 0.0;
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.fps = sanitize_fps(fps);
    }
}

fn sanitize_fps(fps: f32) -> f32 {
    if !fps.is_finite() || fps <= 0.0 {
        1.0
    } else {
        fps.max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_by_caps_catch_up_and_drops_excess_backlog() {
        let mut timer = SequenceTimer::new(10.0);

        let frames = timer.advance_by(2.35);

        assert_eq!(frames, MAX_FRAME_ADVANCE_PER_TICK);
        assert!(timer.accumulator < 0.1);
        assert_eq!(timer.advance_by(0.0), 0);
    }

    #[test]
    fn advance_by_keeps_expected_remainder_under_cap() {
        let mut timer = SequenceTimer::new(10.0);

        let frames = timer.advance_by(0.35);

        assert_eq!(frames, 3);
        assert!((timer.accumulator - 0.05).abs() < 0.001);
    }
}
