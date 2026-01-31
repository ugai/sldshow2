//! Performance diagnostics for identifying bottlenecks

use bevy::prelude::*;
use std::time::Instant;

/// Performance diagnostics plugin
pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameTimings>()
            .init_resource::<TransitionMetrics>()
            .add_systems(Update, (
                track_frame_time,
                log_performance_metrics.after(track_frame_time),
            ));
    }
}

/// Frame timing data
#[derive(Resource)]
pub struct FrameTimings {
    pub last_frame_start: Instant,
    pub frame_times: Vec<f32>,
    pub max_samples: usize,
    pub last_log_time: f32,
}

impl Default for FrameTimings {
    fn default() -> Self {
        Self {
            last_frame_start: Instant::now(),
            frame_times: Vec::with_capacity(300), // 5 seconds at 60fps
            max_samples: 300,
            last_log_time: 0.0,
        }
    }
}

/// Transition-specific metrics
#[derive(Resource, Default)]
pub struct TransitionMetrics {
    pub transition_starts: usize,
    pub transition_completions: usize,
    pub transition_cancellations: usize,
    pub last_transition_duration: f32,
}

/// Track frame time every frame
fn track_frame_time(
    mut timings: ResMut<FrameTimings>,
) {
    let now = Instant::now();
    let frame_time = now.duration_since(timings.last_frame_start).as_secs_f32();
    timings.last_frame_start = now;

    // Store frame time (in milliseconds for easier reading)
    let frame_time_ms = frame_time * 1000.0;
    timings.frame_times.push(frame_time_ms);

    // Log extreme spikes immediately for debugging
    if frame_time_ms > 200.0 {
        error!("🔥 EXTREME FRAME SPIKE: {:.2}ms (should be ~16.67ms)", frame_time_ms);
    }

    // Keep only recent samples
    if timings.frame_times.len() > timings.max_samples {
        timings.frame_times.remove(0);
    }
}

/// Log performance metrics periodically
fn log_performance_metrics(
    mut timings: ResMut<FrameTimings>,
    metrics: Res<TransitionMetrics>,
    time: Res<Time>,
) {
    // Log every 5 seconds (with proper debouncing)
    let elapsed = time.elapsed_secs();
    if elapsed - timings.last_log_time >= 5.0 {
        timings.last_log_time = elapsed;

        if timings.frame_times.is_empty() {
            return;
        }

        let avg_frame_time: f32 = timings.frame_times.iter().sum::<f32>() / timings.frame_times.len() as f32;
        let max_frame_time = timings.frame_times.iter().copied().fold(0.0f32, f32::max);
        let min_frame_time = timings.frame_times.iter().copied().fold(1000.0f32, f32::min);

        // Count frames over 16.67ms (60fps threshold)
        let slow_frames = timings.frame_times.iter().filter(|&&t| t > 16.67).count();
        let slow_frame_pct = (slow_frames as f32 / timings.frame_times.len() as f32) * 100.0;

        info!("=== PERFORMANCE METRICS ===");
        info!("Frame Time: avg={:.2}ms, min={:.2}ms, max={:.2}ms",
              avg_frame_time, min_frame_time, max_frame_time);
        info!("FPS estimate: {:.1}", 1000.0 / avg_frame_time);
        info!("Slow frames (>16.67ms): {} ({:.1}%)", slow_frames, slow_frame_pct);

        // Warn about extreme spikes
        if max_frame_time > 100.0 {
            warn!("⚠️  EXTREME SPIKE DETECTED: {:.2}ms frame time!", max_frame_time);
            warn!("This is likely causing visible stuttering/hitching");
        }

        info!("Transitions: starts={}, completions={}, cancellations={}",
              metrics.transition_starts,
              metrics.transition_completions,
              metrics.transition_cancellations);

        // Warn about high cancellation rate
        if metrics.transition_starts > 0 {
            let cancel_rate = (metrics.transition_cancellations as f32 / metrics.transition_starts as f32) * 100.0;
            if cancel_rate > 30.0 {
                warn!("⚠️  High transition cancel rate: {:.1}%", cancel_rate);
            }
        }

        if metrics.last_transition_duration > 0.0 {
            info!("Last transition duration: {:.2}s", metrics.last_transition_duration);
        }
        info!("==========================");
    }
}

/// Track transition start (call from handle_transition_events)
pub fn track_transition_start(metrics: &mut TransitionMetrics) {
    metrics.transition_starts += 1;
}

/// Track transition completion (call from update_transitions)
pub fn track_transition_complete(metrics: &mut TransitionMetrics, duration: f32) {
    metrics.transition_completions += 1;
    metrics.last_transition_duration = duration;
}

/// Track transition cancellation (call from detect_image_change)
pub fn track_transition_cancel(metrics: &mut TransitionMetrics) {
    metrics.transition_cancellations += 1;
}
