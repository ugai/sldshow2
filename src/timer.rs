//! Interval-based timer for slideshow auto-advancement.

use std::ops::{Deref, DerefMut};
use std::time::{Duration, Instant};

pub use crate::config::TIMER_MIN;

/// Sanitize a raw timer value to a legal interval.
///
/// - Non-finite or `<= 0.0` → `0.0` (paused sentinel)
/// - `(0.0, TIMER_MIN)` → clamped up to `TIMER_MIN`
/// - `>= TIMER_MIN` → returned unchanged
pub fn sanitize_timer(value: f32) -> f32 {
    if !value.is_finite() || value <= 0.0 {
        0.0
    } else {
        value.max(TIMER_MIN)
    }
}

/// Shared pause/timing state embedded in both timer types.
///
/// Provides the common `toggle_pause` and `reset` operations so the logic
/// lives in exactly one place.
pub struct TimerBase {
    pub paused: bool,
    last_tick: Instant,
}

impl TimerBase {
    fn new(paused: bool) -> Self {
        Self {
            paused,
            last_tick: Instant::now(),
        }
    }

    /// Toggles the paused state, resets the tick clock on resume, and returns
    /// the new paused state.
    pub fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        if !self.paused {
            self.last_tick = Instant::now();
        }
        self.paused
    }

    /// Resets the tick clock to now (does not affect pause state).
    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
    }
}

pub struct SlideshowTimer {
    base: TimerBase,
    pub interval: Duration,
    paused_by_user: bool,
}

impl Deref for SlideshowTimer {
    type Target = TimerBase;
    fn deref(&self) -> &TimerBase {
        &self.base
    }
}

impl DerefMut for SlideshowTimer {
    fn deref_mut(&mut self) -> &mut TimerBase {
        &mut self.base
    }
}

impl SlideshowTimer {
    pub fn new(interval_secs: f32) -> Self {
        let sanitized = sanitize_timer(interval_secs);
        Self {
            base: TimerBase::new(sanitized <= 0.0),
            interval: Duration::from_secs_f32(if sanitized > 0.0 {
                sanitized
            } else {
                TIMER_MIN
            }),
            paused_by_user: false,
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
        let new_paused = self.base.toggle_pause();
        self.paused_by_user = new_paused;
        new_paused
    }

    pub fn set_duration(&mut self, duration_secs: f32) {
        let sanitized = sanitize_timer(duration_secs);
        if sanitized <= 0.0 {
            self.base.paused = true;
            self.paused_by_user = false;
        } else {
            self.interval = Duration::from_secs_f32(sanitized);
            self.base.reset();
            if !self.paused_by_user {
                self.base.paused = false;
            }
        }
    }

    /// Returns the configured interval in seconds regardless of pause state.
    pub fn interval_secs(&self) -> f32 {
        self.interval.as_secs_f32()
    }
}

pub struct SequenceTimer {
    base: TimerBase,
    pub fps: f32,
    accumulator: f32,
}

impl Deref for SequenceTimer {
    type Target = TimerBase;
    fn deref(&self) -> &TimerBase {
        &self.base
    }
}

impl DerefMut for SequenceTimer {
    fn deref_mut(&mut self) -> &mut TimerBase {
        &mut self.base
    }
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
            base: TimerBase::new(false),
            fps: sanitize_fps(fps),
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

    /// Delegates to `TimerBase::toggle_pause`.
    pub fn toggle_pause(&mut self) -> bool {
        self.base.toggle_pause()
    }

    /// Resets the tick clock and accumulator.
    pub fn reset(&mut self) {
        self.base.reset();
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
        fps.clamp(1.0, 240.0)
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

    // --- SlideshowTimer ---

    #[test]
    fn slideshow_timer_zero_interval_starts_paused() {
        let timer = SlideshowTimer::new(0.0);
        assert!(timer.paused);
    }

    #[test]
    fn slideshow_timer_negative_interval_starts_paused() {
        let timer = SlideshowTimer::new(-1.0);
        assert!(timer.paused);
    }

    #[test]
    fn slideshow_timer_positive_interval_not_paused() {
        let timer = SlideshowTimer::new(5.0);
        assert!(!timer.paused);
        assert!((timer.interval_secs() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn slideshow_timer_toggle_pause_flips_state() {
        let mut timer = SlideshowTimer::new(5.0);
        let now_paused = timer.toggle_pause();
        assert!(now_paused);
        assert!(timer.paused);
        let now_paused = timer.toggle_pause();
        assert!(!now_paused);
        assert!(!timer.paused);
    }

    #[test]
    fn slideshow_timer_set_duration_zero_pauses() {
        let mut timer = SlideshowTimer::new(5.0);
        timer.set_duration(0.0);
        assert!(timer.paused);
    }

    #[test]
    fn slideshow_timer_set_duration_positive_preserves_pause_state() {
        let mut timer = SlideshowTimer::new(5.0);
        timer.toggle_pause(); // manually pause
        assert!(timer.paused);
        timer.set_duration(3.0);
        assert!(
            timer.paused,
            "set_duration must not unpause a user-paused timer"
        );
        assert!((timer.interval_secs() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn slideshow_timer_set_duration_positive_unpauses_timer_paused_by_zero() {
        let mut timer = SlideshowTimer::new(5.0);
        timer.set_duration(0.0);
        assert!(timer.paused, "timer should be paused after set_duration(0)");
        timer.set_duration(3.0);
        assert!(
            !timer.paused,
            "set_duration with positive value must unpause a timer-paused timer"
        );
        assert!((timer.interval_secs() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn slideshow_timer_set_duration_positive_updates_interval_when_running() {
        let mut timer = SlideshowTimer::new(5.0);
        assert!(!timer.paused);
        timer.set_duration(3.0);
        assert!(!timer.paused);
        assert!((timer.interval_secs() - 3.0).abs() < 1e-5);
    }

    // --- SequenceTimer ---

    #[test]
    fn sequence_timer_invalid_fps_defaults_to_one() {
        assert_eq!(SequenceTimer::new(-5.0).fps, 1.0);
        assert_eq!(SequenceTimer::new(0.0).fps, 1.0);
        assert_eq!(SequenceTimer::new(f32::NAN).fps, 1.0);
    }

    #[test]
    fn advance_by_zero_dt_returns_no_frames() {
        let mut timer = SequenceTimer::new(10.0);
        assert_eq!(timer.advance_by(0.0), 0);
    }

    #[test]
    fn advance_by_negative_dt_returns_no_frames() {
        let mut timer = SequenceTimer::new(10.0);
        assert_eq!(timer.advance_by(-1.0), 0);
    }

    // --- sanitize_timer ---

    #[test]
    fn sanitize_timer_zero_returns_zero() {
        assert_eq!(sanitize_timer(0.0), 0.0);
    }

    #[test]
    fn sanitize_timer_negative_returns_zero() {
        assert_eq!(sanitize_timer(-1.0), 0.0);
        assert_eq!(sanitize_timer(-0.001), 0.0);
    }

    #[test]
    fn sanitize_timer_non_finite_returns_zero() {
        assert_eq!(sanitize_timer(f32::INFINITY), 0.0);
        assert_eq!(sanitize_timer(f32::NEG_INFINITY), 0.0);
        assert_eq!(sanitize_timer(f32::NAN), 0.0);
    }

    #[test]
    fn sanitize_timer_below_min_clamps_to_min() {
        assert_eq!(sanitize_timer(0.0001), TIMER_MIN);
        assert_eq!(sanitize_timer(0.05), TIMER_MIN);
        assert!((sanitize_timer(TIMER_MIN - f32::EPSILON) - TIMER_MIN).abs() < 1e-6);
    }

    #[test]
    fn sanitize_timer_at_min_preserved() {
        assert!((sanitize_timer(TIMER_MIN) - TIMER_MIN).abs() < 1e-6);
    }

    #[test]
    fn sanitize_timer_above_min_preserved() {
        assert!((sanitize_timer(5.0) - 5.0).abs() < 1e-6);
        assert!((sanitize_timer(3600.0) - 3600.0).abs() < 1e-6);
    }
}
