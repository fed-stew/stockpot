//! Miniature bar chart for displaying LLM throughput (chars/sec)

use gpui::{div, prelude::*, px, Rgba, Styled};

/// Configuration for the throughput chart
pub struct ThroughputChartProps {
    /// Historical throughput samples (most recent last)
    pub samples: Vec<f64>,
    /// Current throughput value
    pub current_cps: f64,
    /// Whether streaming is active
    pub is_active: bool,
    /// Maximum expected CPS for scaling (e.g., 2000)
    pub max_cps: f64,
    /// Colors
    pub bar_color: Rgba,
    pub bar_color_fast: Rgba,
    pub background_color: Rgba,
}

impl Default for ThroughputChartProps {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            current_cps: 0.0,
            is_active: false,
            max_cps: 2000.0,
            bar_color: gpui::rgb(0x55aacc),        // Cyan-ish
            bar_color_fast: gpui::rgb(0x66cc66),   // Green
            background_color: gpui::rgb(0x333333), // Dark gray
        }
    }
}

/// Render a miniature bar chart showing throughput history
pub fn throughput_chart(props: ThroughputChartProps) -> impl IntoElement {
    let num_bars = 8;
    let bar_width = 5.0_f32;
    let bar_gap = 2.5_f32;
    let chart_height = 17.0_f32;

    // Pad samples to always have num_bars entries (efficient O(1) prepending)
    let samples_to_use: Vec<f64> = props.samples.iter().rev().take(num_bars).cloned().collect();
    let padding_needed = num_bars.saturating_sub(samples_to_use.len());
    let mut display_samples = vec![0.0; padding_needed];
    display_samples.extend(samples_to_use.into_iter().rev());

    let bars: Vec<_> = display_samples
        .into_iter()
        .map(|cps| {
            // Normalize to 0.0-1.0 range
            let normalized = (cps / props.max_cps).clamp(0.0, 1.0);
            let height = (normalized as f32 * chart_height).max(1.0); // Min 1px when > 0

            // Color based on speed (green = fast, cyan = normal)
            let color = if cps > props.max_cps * 0.7 {
                props.bar_color_fast
            } else {
                props.bar_color
            };

            div()
                .w(px(bar_width))
                .h(px(height))
                .bg(if cps > 0.0 {
                    color
                } else {
                    props.background_color
                })
                .rounded(px(1.))
        })
        .collect();

    // Container
    div()
        .flex()
        .items_end()
        .gap(px(bar_gap))
        .h(px(chart_height))
        .px(px(4.))
        .py(px(2.))
        .rounded(px(4.))
        .when(!props.is_active, |d| d.opacity(0.5))
        .children(bars)
}
