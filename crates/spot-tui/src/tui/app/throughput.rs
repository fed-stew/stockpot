//! Throughput tracking for streaming output.

use std::time::Instant;

use super::TuiApp;

impl TuiApp {
    pub fn update_throughput(&mut self, chars: usize) {
        let now = Instant::now();
        self.throughput_samples.push((chars, now));

        // Remove samples older than 2 seconds
        self.throughput_samples
            .retain(|(_, t)| now.duration_since(*t).as_secs_f64() < 2.0);

        self.tick_throughput();
    }

    pub fn tick_throughput(&mut self) {
        let now = Instant::now();
        // Recalculate based on current samples
        let total_chars: usize = self.throughput_samples.iter().map(|(c, _)| c).sum();

        if let Some((_, first_time)) = self.throughput_samples.first() {
            let duration = now.duration_since(*first_time).as_secs_f64();
            if duration > 0.1 {
                self.current_throughput_cps = total_chars as f64 / duration;
            }
        }
    }

    pub fn reset_throughput(&mut self) {
        self.throughput_samples.clear();
        self.current_throughput_cps = 0.0;
    }
}
