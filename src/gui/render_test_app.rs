//! Render performance test harness with optional markdown rendering.

use gpui::{
    div, list, point, prelude::*, px, rgb, Context, ListAlignment, ListState, Pixels, SharedString,
    Styled, WeakEntity, Window,
};
use gpui_component::text::TextView;
use std::collections::VecDeque;
use std::time::Instant;

use super::render_test::{FrameStats, TestCase};

/// The render test application
pub struct RenderTestApp {
    messages: Vec<String>,
    list_state: ListState,
    frame_stats: FrameStats,
    last_frame: Instant,
    scroll_position: Pixels,
    scroll_direction: f32,
    test_name: String,
    frames_remaining: usize,
    test_complete: bool,
    remaining_cases: VecDeque<TestCase>,
    all_results: Vec<(String, FrameStats)>,
    use_markdown: bool,
}

impl RenderTestApp {
    pub fn new(test_case: &TestCase, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_options(test_case, true, _window, cx)
    }

    pub fn new_with_options(
        test_case: &TestCase,
        use_markdown: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let messages = test_case.generate_content();
        let count = messages.len();
        let test_name = test_case.name();
        let mode = if use_markdown { "markdown" } else { "plain" };

        println!(
            "🧪 Starting test: {} ({} messages, {} chars) [{}]",
            test_name,
            count,
            messages.iter().map(|s| s.len()).sum::<usize>(),
            mode
        );

        let app = Self {
            messages,
            list_state: ListState::new(count, ListAlignment::Top, px(200.0)),
            frame_stats: FrameStats::new(500),
            last_frame: Instant::now(),
            scroll_position: px(0.0),
            scroll_direction: 1.0,
            test_name,
            frames_remaining: 300,
            test_complete: false,
            remaining_cases: VecDeque::new(),
            all_results: Vec::new(),
            use_markdown,
        };

        Self::start_animation_loop(cx);
        app
    }

    pub fn set_remaining_cases(&mut self, cases: VecDeque<TestCase>) {
        self.remaining_cases = cases;
    }

    fn start_animation_loop(cx: &mut Context<Self>) {
        use tokio::time::{interval, Duration};

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut ticker = interval(Duration::from_millis(8));

                loop {
                    ticker.tick().await;

                    let should_continue = this
                        .update(cx, |app, cx| app.tick_frame(cx))
                        .unwrap_or(false);

                    if !should_continue {
                        break;
                    }
                }
            },
        )
        .detach();
    }

    fn tick_frame(&mut self, cx: &mut Context<Self>) -> bool {
        if self.test_complete {
            return false;
        }

        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame);
        self.last_frame = now;
        self.frame_stats.record(frame_time);

        let max_offset = self.list_state.max_offset_for_scrollbar();
        if max_offset.height > px(0.0) {
            self.scroll_position += px(self.scroll_direction * 15.0);

            if self.scroll_position >= max_offset.height {
                self.scroll_direction = -1.0;
                self.scroll_position = max_offset.height;
            } else if self.scroll_position <= px(0.0) {
                self.scroll_direction = 1.0;
                self.scroll_position = px(0.0);
            }

            self.list_state
                .set_offset_from_scrollbar(point(px(0.0), -self.scroll_position));
        }

        self.frames_remaining = self.frames_remaining.saturating_sub(1);
        cx.notify();

        if self.frames_remaining == 0 {
            self.test_complete = true;
            self.frame_stats.report(&self.test_name);

            self.all_results.push((
                self.test_name.clone(),
                std::mem::take(&mut self.frame_stats),
            ));

            if let Some(next_case) = self.remaining_cases.pop_front() {
                self.start_next_test(&next_case, cx);
                return true;
            } else {
                self.print_summary();
                cx.quit();
                return false;
            }
        }

        true
    }

    fn start_next_test(&mut self, test_case: &TestCase, cx: &mut Context<Self>) {
        let messages = test_case.generate_content();
        let count = messages.len();
        let test_name = test_case.name();
        let mode = if self.use_markdown {
            "markdown"
        } else {
            "plain"
        };

        println!(
            "\n🧪 Starting test: {} ({} messages, {} chars) [{}]",
            test_name,
            count,
            messages.iter().map(|s| s.len()).sum::<usize>(),
            mode
        );

        self.messages = messages;
        self.list_state = ListState::new(count, ListAlignment::Top, px(200.0));
        self.frame_stats = FrameStats::new(500);
        self.last_frame = Instant::now();
        self.scroll_position = px(0.0);
        self.scroll_direction = 1.0;
        self.test_name = test_name;
        self.frames_remaining = 300;
        self.test_complete = false;
        cx.notify();
    }

    fn print_summary(&self) {
        println!("\n");
        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║                    PERFORMANCE SUMMARY                        ║");
        println!("╚═══════════════════════════════════════════════════════════════╝");
        println!();

        let mut results: Vec<_> = self
            .all_results
            .iter()
            .map(|(name, stats)| (name.clone(), stats.max_ms(), stats.avg_ms(), stats.p99_ms()))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        println!("Ranked by worst frame time (max):\n");
        println!(
            "{:<40} {:>10} {:>10} {:>10}",
            "Test", "Max(ms)", "Avg(ms)", "P99(ms)"
        );
        println!("{}", "-".repeat(75));

        for (name, max, avg, p99) in &results {
            let indicator = if *max > 100.0 {
                "🔴"
            } else if *max > 33.0 {
                "🟡"
            } else {
                "🟢"
            };
            println!(
                "{} {:<38} {:>10.2} {:>10.2} {:>10.2}",
                indicator, name, max, avg, p99
            );
        }

        println!("\nLegend: 🔴 >100ms (severe) | 🟡 >33ms (stutter) | 🟢 OK\n");

        if let Some((worst_test, worst_max, _, _)) = results.first() {
            if *worst_max > 33.0 {
                println!("🎯 LIKELY CULPRIT: {}", worst_test);
                println!("   Worst frame time: {:.1}ms", worst_max);

                if worst_test.contains("LongLine") {
                    println!("   → Issue: Text shaping on long unbroken lines");
                } else if worst_test.contains("CodeBlock") {
                    println!("   → Issue: Code block rendering/highlighting");
                } else if worst_test.contains("Blockquotes") {
                    println!("   → Issue: Nested layout structures");
                } else if worst_test.contains("Messages") {
                    println!("   → Issue: List virtualization overhead");
                }
            } else {
                println!("✅ All tests passed! No significant performance issues detected.");
            }
        }
        println!();
    }
}

impl Render for RenderTestApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.messages.clone();
        let progress = 300 - self.frames_remaining;
        let avg_ms = self.frame_stats.avg_ms();
        let max_ms = self.frame_stats.max_ms();
        let scroll_pos: f32 = self.scroll_position.into();
        let use_markdown = self.use_markdown;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .child(
                div()
                    .id("test-header")
                    .p(px(12.0))
                    .border_b_1()
                    .border_color(rgb(0x333333))
                    .bg(rgb(0x252525))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(format!(
                                "🧪 {} [{}]",
                                self.test_name,
                                if use_markdown { "MD" } else { "TXT" }
                            ))
                            .child(format!("Frame {}/300", progress)),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0x888888))
                            .child(format!(
                                "Avg: {:.1}ms | Max: {:.1}ms | Scroll: {:.0}px",
                                avg_ms, max_ms, scroll_pos
                            )),
                    ),
            )
            .child(
                div().flex_1().min_h_0().min_w_0().overflow_hidden().child(
                    list(self.list_state.clone(), move |idx, _window, _cx| {
                        let content = messages.get(idx).cloned().unwrap_or_default();
                        let element_id = SharedString::from(format!("item-{}", idx));

                        div()
                            .id(idx)
                            .p(px(12.0))
                            .w_full()
                            .min_w_0()
                            .max_w_full()
                            .overflow_x_hidden()
                            .border_b_1()
                            .border_color(rgb(0x333333))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .max_w_full()
                                    .overflow_hidden()
                                    .when(use_markdown, |d| {
                                        // Use TextView with markdown parsing
                                        d.child(TextView::markdown(
                                            element_id.clone(),
                                            content.clone(),
                                        ))
                                    })
                                    .when(!use_markdown, |d| {
                                        // Plain text
                                        d.child(content)
                                    }),
                            )
                            .into_any_element()
                    })
                    .size_full(),
                ),
            )
    }
}
