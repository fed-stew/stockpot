//! Smooth scroll animation for the messages list.
//!
//! This module provides smooth scroll interpolation with momentum-based velocity
//! to fix the "ratchety" stop-and-go feeling during streaming.
//!
//! The animation uses:
//! - Adaptive target velocity based on distance (faster when far, slower when close)
//! - Momentum smoothing (low-pass filter) to coast through gaps between network packets
//! - Frame-rate independent delta-time calculations

use gpui::{point, px, Pixels};

use super::ChatApp;

/// Minimum target scroll speed in pixels per second (when very close to target)
const MIN_SCROLL_SPEED_PX_PER_SEC: f32 = 100.0;

/// Maximum target scroll speed in pixels per second (when far from target)  
const MAX_SCROLL_SPEED_PX_PER_SEC: f32 = 1500.0;

/// Distance at which we reach max scroll speed (pixels)
const FULL_SPEED_DISTANCE: f32 = 800.0;

/// Time constant for velocity INCREASE (fast attack when falling behind)
const VELOCITY_ATTACK_TIME: f32 = 0.05; // 50ms to ramp up

/// Time constant for velocity DECREASE (slow decay to coast through gaps)
const VELOCITY_DECAY_TIME: f32 = 0.20; // 200ms to slow down

/// Minimum velocity to maintain while actively streaming (pixels per second)
/// This prevents the scroll from fully stopping during long code blocks/tables
/// where tokens may arrive 1-2 seconds apart
const STREAMING_MIN_VELOCITY: f32 = 50.0;

/// Minimum scroll distance to bother animating (in pixels)
const MIN_SCROLL_THRESHOLD: Pixels = px(2.0);

impl ChatApp {
    /// Start a smooth scroll animation to the bottom of the messages list.
    ///
    /// This marks that we want to animate to bottom. The actual scrolling
    /// is handled by tick_scroll_animation() which runs at 240fps.
    pub(super) fn start_smooth_scroll_to_bottom(&mut self) {
        // If already animating, just let it continue (target is recomputed each tick anyway)
        if self.scroll_animation_target.is_some() {
            return;
        }

        // Mark that we want to animate to bottom
        self.scroll_animation_target = Some(point(px(0.), px(0.)));
        self.last_animation_tick = std::time::Instant::now();
        // Don't reset velocity - let it coast from previous motion if any
    }

    /// Tick the scroll animation with momentum-based velocity smoothing.
    ///
    /// Uses a low-pass filter on velocity to "coast" through gaps between
    /// network packets, eliminating the stop-and-go ratchet effect.
    ///
    /// Returns `true` if animation is still in progress.
    pub(super) fn tick_scroll_animation(&mut self) -> bool {
        // ALWAYS update timing first, even if we early return
        // This prevents stale timestamps from accumulating during layout changes
        let now = std::time::Instant::now();
        let delta_secs = now.duration_since(self.last_animation_tick).as_secs_f32();
        self.last_animation_tick = now;

        // Clamp delta to prevent huge jumps after pauses
        let delta_secs = delta_secs.min(0.05);

        // Skip if no animation requested or user has scrolled away
        if self.scroll_animation_target.is_none() || self.user_scrolled_away {
            self.scroll_animation_target = None;
            // During streaming, preserve minimum velocity so we don't stutter when user_scrolled_away toggles
            // (can happen when code blocks change layout suddenly)
            if self.is_generating {
                self.current_scroll_velocity =
                    self.current_scroll_velocity.max(STREAMING_MIN_VELOCITY);
            } else {
                self.current_scroll_velocity = 0.0;
            }
            return false;
        }

        // Get current position and compute target (always the bottom)
        let current_offset = self.messages_list_state.scroll_px_offset_for_scrollbar();
        let max_offset = self.messages_list_state.max_offset_for_scrollbar();

        // Target is the bottom: y = -max_offset.height
        let target_y = -max_offset.height;
        let current_y = current_offset.y;
        let distance = (target_y - current_y).abs();

        // If we're close enough AND velocity is low, we're at the target
        if distance < MIN_SCROLL_THRESHOLD && self.current_scroll_velocity < 50.0 {
            // Only snap if not already exactly at target
            let already_at_target = (current_y - target_y).abs() < px(0.5);
            if !already_at_target {
                self.messages_list_state
                    .set_offset_from_scrollbar(point(current_offset.x, target_y));
            }

            // Only stop animation state when streaming is complete
            if !self.is_generating {
                self.scroll_animation_target = None;
                self.current_scroll_velocity = 0.0;
            }
            // Return whether we actually moved (triggers render only if needed)
            return !already_at_target;
        }

        // 1. Calculate TARGET velocity based on distance (same adaptive logic)
        let full_speed_dist = px(FULL_SPEED_DISTANCE);
        let speed_factor = if distance >= full_speed_dist {
            1.0
        } else {
            (distance / full_speed_dist).min(1.0)
        };
        let target_speed = MIN_SCROLL_SPEED_PX_PER_SEC
            + (MAX_SCROLL_SPEED_PX_PER_SEC - MIN_SCROLL_SPEED_PX_PER_SEC) * speed_factor;

        // 2. Apply ASYMMETRIC momentum smoothing (frame-rate independent)
        // - Fast attack: quickly ramp up when target_speed > current (falling behind)
        // - Slow decay: gradually slow down when target_speed < current (coasting)
        let smoothing_time = if target_speed > self.current_scroll_velocity {
            VELOCITY_ATTACK_TIME // Ramp up fast
        } else {
            VELOCITY_DECAY_TIME // Coast down slowly
        };
        let blend = 1.0 - (-delta_secs / smoothing_time).exp();
        self.current_scroll_velocity =
            self.current_scroll_velocity + (target_speed - self.current_scroll_velocity) * blend;

        // 2b. Maintain minimum cruising velocity while streaming
        // This prevents full stops during long code blocks where tokens arrive slowly
        if self.is_generating && self.current_scroll_velocity < STREAMING_MIN_VELOCITY {
            self.current_scroll_velocity = STREAMING_MIN_VELOCITY;
        }

        // 3. Calculate movement using SMOOTHED velocity
        let max_delta = px(self.current_scroll_velocity * delta_secs);

        // Move toward target, but don't overshoot
        let delta = if distance < max_delta {
            target_y - current_y
        } else if target_y > current_y {
            max_delta
        } else {
            -max_delta
        };

        let new_y = current_y + delta;

        self.messages_list_state
            .set_offset_from_scrollbar(point(current_offset.x, new_y));

        true
    }
}
