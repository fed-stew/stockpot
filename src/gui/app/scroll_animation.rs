//! Smooth scroll animation for the messages list.
//!
//! This module provides smooth scroll interpolation with momentum-based velocity
//! to fix the "ratchety" stop-and-go feeling during streaming.

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
const STREAMING_MIN_VELOCITY: f32 = 50.0;

/// Minimum scroll distance to bother animating (in pixels)
const MIN_SCROLL_THRESHOLD: Pixels = px(2.0);

/// Threshold for re-enabling autoscroll when user scrolls back to bottom (0.90 = 90%)
const AUTOSCROLL_RE_ENABLE_THRESHOLD: f32 = 0.90;

impl ChatApp {
    /// Start a smooth scroll animation to the bottom of the messages list.
    pub(super) fn start_smooth_scroll_to_bottom(&mut self) {
        // If user has scrolled away, don't force scroll - respect their position
        if self.user_scrolled_away {
            return;
        }

        // If already animating, just let it continue
        if self.scroll_animation_target.is_some() {
            return;
        }

        self.scroll_animation_target = Some(point(px(0.), px(0.)));
        self.last_animation_tick = std::time::Instant::now();
    }

    /// Calculate the current scroll ratio (0.0 = top, 1.0 = bottom)
    fn calculate_scroll_ratio(&self) -> f32 {
        let max_offset = self.messages_list_state.max_offset_for_scrollbar();
        let current_offset = self.messages_list_state.scroll_px_offset_for_scrollbar();

        if max_offset.height > px(0.0) {
            (-current_offset.y / max_offset.height).clamp(0.0, 1.0)
        } else {
            1.0 // No scrollable content = at bottom
        }
    }

    /// Check if user has scrolled back near the bottom and should re-enable autoscroll.
    /// ONLY checks during active streaming - once streaming stops, scroll state is preserved.
    pub(super) fn check_autoscroll_re_enable(&mut self) {
        // Only re-enable autoscroll during active streaming!
        // Once streaming stops, respect user's scroll position until next generation starts.
        if !self.is_generating {
            return;
        }

        if self.user_scrolled_away {
            let scroll_ratio = self.calculate_scroll_ratio();
            if scroll_ratio >= AUTOSCROLL_RE_ENABLE_THRESHOLD {
                tracing::debug!(
                    "Re-enabling autoscroll: scroll_ratio={:.1}% (threshold={}%)",
                    scroll_ratio * 100.0,
                    AUTOSCROLL_RE_ENABLE_THRESHOLD * 100.0
                );
                self.user_scrolled_away = false;
                self.scroll_animation_target = Some(point(px(0.), px(0.)));
            }
        }
    }

    /// Mark that user has scrolled away from the bottom (disables autoscroll during streaming)
    pub(super) fn mark_user_scrolled_away(&mut self) {
        if !self.user_scrolled_away && self.is_generating {
            tracing::debug!("User scrolled away - disabling autoscroll");
            self.user_scrolled_away = true;
            self.scroll_animation_target = None;
        }
    }

    /// Tick the scroll animation with momentum-based velocity smoothing.
    /// Returns `true` if animation is still in progress.
    pub(super) fn tick_scroll_animation(&mut self) -> bool {
        let now = std::time::Instant::now();
        let delta_secs = now.duration_since(self.last_animation_tick).as_secs_f32();
        self.last_animation_tick = now;
        let delta_secs = delta_secs.min(0.05);

        // Check if user scrolled back to bottom (only during streaming)
        self.check_autoscroll_re_enable();

        // Skip if no animation or user scrolled away
        if self.scroll_animation_target.is_none() || self.user_scrolled_away {
            self.scroll_animation_target = None;
            if self.is_generating {
                self.current_scroll_velocity =
                    self.current_scroll_velocity.max(STREAMING_MIN_VELOCITY);
            } else {
                self.current_scroll_velocity = 0.0;
            }
            return false;
        }

        let current_offset = self.messages_list_state.scroll_px_offset_for_scrollbar();
        let max_offset = self.messages_list_state.max_offset_for_scrollbar();

        // Detect user scroll UP (only during streaming)
        if self.is_generating {
            let expected_offset = self.last_scroll_offset_y;
            let actual_offset = current_offset.y;
            let scroll_delta = actual_offset - expected_offset;

            if scroll_delta > px(2.0) {
                self.mark_user_scrolled_away();
                return false;
            }
        }

        let target_y = -max_offset.height;
        let current_y = current_offset.y;
        let distance = (target_y - current_y).abs();

        // Close enough to target
        if distance < MIN_SCROLL_THRESHOLD && self.current_scroll_velocity < 50.0 {
            let already_at_target = (current_y - target_y).abs() < px(0.5);
            if !already_at_target {
                self.messages_list_state
                    .set_offset_from_scrollbar(point(current_offset.x, target_y));
                self.last_scroll_offset_y = target_y;
            }

            if !self.is_generating {
                self.scroll_animation_target = None;
                self.current_scroll_velocity = 0.0;
            }
            return !already_at_target;
        }

        // Adaptive velocity based on distance
        let full_speed_dist = px(FULL_SPEED_DISTANCE);
        let speed_factor = if distance >= full_speed_dist {
            1.0
        } else {
            (distance / full_speed_dist).min(1.0)
        };
        let target_speed = MIN_SCROLL_SPEED_PX_PER_SEC
            + (MAX_SCROLL_SPEED_PX_PER_SEC - MIN_SCROLL_SPEED_PX_PER_SEC) * speed_factor;

        // Asymmetric momentum smoothing
        let smoothing_time = if target_speed > self.current_scroll_velocity {
            VELOCITY_ATTACK_TIME
        } else {
            VELOCITY_DECAY_TIME
        };
        let blend = 1.0 - (-delta_secs / smoothing_time).exp();
        self.current_scroll_velocity =
            self.current_scroll_velocity + (target_speed - self.current_scroll_velocity) * blend;

        if self.is_generating && self.current_scroll_velocity < STREAMING_MIN_VELOCITY {
            self.current_scroll_velocity = STREAMING_MIN_VELOCITY;
        }

        let max_delta = px(self.current_scroll_velocity * delta_secs);
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
        self.last_scroll_offset_y = new_y;

        true
    }
}
