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
        self.interval = Duration::from_secs_f32(duration_secs.max(0.1));
    }

    pub fn duration(&self) -> f32 {
        self.interval.as_secs_f32()
    }
}
