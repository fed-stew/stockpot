//! Render performance test harness for diagnosing scroll stuttering.
//!
//! Run with: `cargo run -- --render-test`
//!
//! This generates synthetic stress test content and measures frame times
//! during programmatic scrolling to identify performance bottlenecks.

use std::collections::VecDeque;
use std::time::Duration;

/// Test case types for stress testing
#[derive(Debug, Clone)]
pub enum TestCase {
    /// Single line with N characters (tests text shaping)
    LongLine { chars: usize, label: String },
    /// Nested blockquotes to depth N (tests layout recursion)
    NestedBlockquotes { depth: usize },
    /// Code block with N lines (tests syntax highlighting)
    LargeCodeBlock { lines: usize, lang: String },
    /// Many short messages (tests list virtualization)
    ManyMessages { count: usize },
    /// Mixed content simulating real usage
    RealisticChat { messages: usize },
}

impl TestCase {
    /// Generate the markdown content for this test case
    pub fn generate_content(&self) -> Vec<String> {
        match self {
            TestCase::LongLine { chars, label } => {
                vec![format!(
                    "## Test: {} ({} chars)\n\n{}",
                    label,
                    chars,
                    "x".repeat(*chars)
                )]
            }
            TestCase::NestedBlockquotes { depth } => {
                let mut content = String::new();
                content.push_str(&format!(
                    "## Test: Nested Blockquotes (depth {})\n\n",
                    depth
                ));
                for i in 0..*depth {
                    content.push_str(&">".repeat(i + 1));
                    content.push_str(&format!(" Level {} content here\n", i + 1));
                }
                vec![content]
            }
            TestCase::LargeCodeBlock { lines, lang } => {
                let mut content = String::new();
                content.push_str(&format!("## Test: Code Block ({} lines)\n\n", lines));
                content.push_str(&format!("```{}\n", lang));
                for i in 0..*lines {
                    content.push_str(&format!(
                        "fn function_{}(x: i32) -> i32 {{ x * {} + {} }}\n",
                        i,
                        i,
                        i * 2
                    ));
                }
                content.push_str("```\n");
                vec![content]
            }
            TestCase::ManyMessages { count } => (0..*count)
                .map(|i| format!("Message {} with some content.", i))
                .collect(),
            TestCase::RealisticChat { messages } => {
                let mut result = Vec::new();
                for i in 0..*messages {
                    match i % 5 {
                        0 => {
                            result.push(format!("# Question {}\n\nHow do I implement feature X?", i))
                        }
                        1 => result.push(
                            "Here's how to implement it:\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\nThis works because...".to_string(),
                        ),
                        2 => result.push("Can you explain more?".to_string()),
                        3 => result.push(
                            "Sure! The key points are:\n\n1. First point\n2. Second point\n3. Third point\n\n> Important note: remember this!".to_string(),
                        ),
                        _ => result.push("Thanks!".to_string()),
                    }
                }
                result
            }
        }
    }

    pub fn name(&self) -> String {
        match self {
            TestCase::LongLine { chars, label } => format!("LongLine({}, {})", label, chars),
            TestCase::NestedBlockquotes { depth } => format!("NestedBlockquotes({})", depth),
            TestCase::LargeCodeBlock { lines, lang } => format!("CodeBlock({}, {})", lines, lang),
            TestCase::ManyMessages { count } => format!("ManyMessages({})", count),
            TestCase::RealisticChat { messages } => format!("RealisticChat({})", messages),
        }
    }
}

/// Frame timing statistics
#[derive(Debug)]
pub struct FrameStats {
    pub frame_times: VecDeque<Duration>,
    pub max_samples: usize,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self::new(500)
    }
}

impl FrameStats {
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn record(&mut self, duration: Duration) {
        if self.frame_times.len() >= self.max_samples {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(duration);
    }

    pub fn avg_ms(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let sum: Duration = self.frame_times.iter().sum();
        sum.as_secs_f64() * 1000.0 / self.frame_times.len() as f64
    }

    pub fn max_ms(&self) -> f64 {
        self.frame_times
            .iter()
            .max()
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    pub fn min_ms(&self) -> f64 {
        self.frame_times
            .iter()
            .min()
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    pub fn p99_ms(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<_> = self.frame_times.iter().collect();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted
            .get(idx.min(sorted.len() - 1))
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    pub fn jank_count(&self, threshold_ms: f64) -> usize {
        self.frame_times
            .iter()
            .filter(|d| d.as_secs_f64() * 1000.0 > threshold_ms)
            .count()
    }

    pub fn report(&self, test_name: &str) {
        let jank_16 = self.jank_count(16.67); // 60fps threshold
        let jank_33 = self.jank_count(33.33); // 30fps threshold
        let total = self.frame_times.len();

        println!("┌─────────────────────────────────────────────────────────────");
        println!("│ Test: {}", test_name);
        println!("├─────────────────────────────────────────────────────────────");
        println!("│ Frames sampled: {}", total);
        if self.avg_ms() > 0.0 {
            println!(
                "│ Avg frame time: {:.2}ms ({:.0} fps)",
                self.avg_ms(),
                1000.0 / self.avg_ms()
            );
        }
        println!("│ Min frame time: {:.2}ms", self.min_ms());
        println!("│ Max frame time: {:.2}ms", self.max_ms());
        println!("│ P99 frame time: {:.2}ms", self.p99_ms());
        if total > 0 {
            println!(
                "│ Jank frames (>16.67ms): {} ({:.1}%)",
                jank_16,
                jank_16 as f64 / total as f64 * 100.0
            );
            println!(
                "│ Severe jank (>33.33ms): {} ({:.1}%)",
                jank_33,
                jank_33 as f64 / total as f64 * 100.0
            );
        }
        println!("└─────────────────────────────────────────────────────────────");

        // Verdict
        if self.max_ms() > 100.0 {
            println!("⚠️  SEVERE STUTTER DETECTED (>100ms frame)");
        } else if jank_33 > 0 {
            println!("⚠️  STUTTER DETECTED (frames >33ms)");
        } else if total > 0 && jank_16 as f64 / total as f64 > 0.05 {
            println!("⚠️  MINOR JANK (>5% frames over 16ms)");
        } else {
            println!("✅ SMOOTH (no significant jank)");
        }
        println!();
    }
}

/// Generate all standard test cases
pub fn standard_test_cases() -> Vec<TestCase> {
    vec![
        // Long line tests - the suspected culprit
        TestCase::LongLine {
            chars: 100,
            label: "short".into(),
        },
        TestCase::LongLine {
            chars: 1000,
            label: "medium".into(),
        },
        TestCase::LongLine {
            chars: 5000,
            label: "long".into(),
        },
        TestCase::LongLine {
            chars: 10000,
            label: "very_long".into(),
        },
        TestCase::LongLine {
            chars: 50000,
            label: "extreme".into(),
        },
        // Nested blockquotes
        TestCase::NestedBlockquotes { depth: 3 },
        TestCase::NestedBlockquotes { depth: 7 },
        TestCase::NestedBlockquotes { depth: 15 },
        // Code blocks
        TestCase::LargeCodeBlock {
            lines: 50,
            lang: "rust".into(),
        },
        TestCase::LargeCodeBlock {
            lines: 200,
            lang: "rust".into(),
        },
        TestCase::LargeCodeBlock {
            lines: 500,
            lang: "rust".into(),
        },
        // Many messages (virtualization test)
        TestCase::ManyMessages { count: 100 },
        TestCase::ManyMessages { count: 500 },
        // Realistic
        TestCase::RealisticChat { messages: 50 },
    ]
}
