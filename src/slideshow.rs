use bevy::prelude::*;

/// Slideshow timer plugin
pub struct SlideshowPlugin;

impl Plugin for SlideshowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlideshowTimer>()
            .add_event::<SlideshowAdvanceEvent>()
            .add_systems(Update, update_slideshow_timer);
    }
}

/// Slideshow timer resource
#[derive(Resource)]
pub struct SlideshowTimer {
    /// Timer for auto-advancing slides
    pub timer: Timer,
    /// Whether the slideshow is paused
    pub paused: bool,
    /// Interval in seconds (0 = paused)
    pub interval: f32,
}

impl Default for SlideshowTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(10.0, TimerMode::Repeating),
            paused: false,
            interval: 10.0,
        }
    }
}

impl SlideshowTimer {
    /// Create a new timer with the given interval
    pub fn new(interval: f32) -> Self {
        let paused = interval <= 0.0;
        Self {
            timer: Timer::from_seconds(interval.max(0.1), TimerMode::Repeating),
            paused,
            interval,
        }
    }

    /// Set the interval
    #[allow(dead_code)]
    pub fn set_interval(&mut self, interval: f32) {
        self.interval = interval;
        self.paused = interval <= 0.0;

        if !self.paused {
            self.timer = Timer::from_seconds(interval, TimerMode::Repeating);
        }
    }

    /// Pause the slideshow
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume the slideshow
    pub fn resume(&mut self) {
        if self.interval > 0.0 {
            self.paused = false;
        }
    }

    /// Toggle pause state
    pub fn toggle_pause(&mut self) {
        if self.paused {
            self.resume();
        } else {
            self.pause();
        }
    }

    /// Reset the timer
    pub fn reset(&mut self) {
        self.timer.reset();
    }
}

/// Event emitted when slideshow should advance
#[derive(Event)]
pub struct SlideshowAdvanceEvent;

/// Update slideshow timer and emit advance events
fn update_slideshow_timer(
    mut timer: ResMut<SlideshowTimer>,
    time: Res<Time>,
    mut events: EventWriter<SlideshowAdvanceEvent>,
) {
    if timer.paused {
        return;
    }

    if timer.timer.tick(time.delta()).just_finished() {
        events.send(SlideshowAdvanceEvent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_creation() {
        let timer = SlideshowTimer::new(5.0);
        assert_eq!(timer.interval, 5.0);
        assert!(!timer.paused);

        let paused_timer = SlideshowTimer::new(0.0);
        assert!(paused_timer.paused);
    }

    #[test]
    fn test_pause_resume() {
        let mut timer = SlideshowTimer::new(5.0);

        timer.pause();
        assert!(timer.paused);

        timer.resume();
        assert!(!timer.paused);

        timer.toggle_pause();
        assert!(timer.paused);
    }
}
