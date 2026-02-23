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
            self.last_tick = Instant::now(); // Keep resetting last_tick while paused
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

impl SequenceTimer {
    pub fn new(fps: f32) -> Self {
        Self {
            fps: fps.max(1.0),
            paused: false,
            last_tick: Instant::now(),
            accumulator: 0.0,
        }
    }

    pub fn update(&mut self) -> usize {
        if self.paused {
            self.last_tick = Instant::now();
            return 0;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        self.accumulator += dt;
        let frame_duration = 1.0 / self.fps;

        let mut frames = 0;
        while self.accumulator >= frame_duration {
            frames += 1;
            self.accumulator -= frame_duration;
        }

        frames
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
        self.fps = fps.max(1.0);
    }
}
