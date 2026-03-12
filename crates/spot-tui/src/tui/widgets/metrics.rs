//! Metrics display widget (tokens, throughput)

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

pub struct MetricsWidget {
    pub model_name: String,
    pub throughput: Option<f64>,       // chars/sec
    pub context_usage: Option<String>, // "Used / Total"
}

impl MetricsWidget {
    pub fn new(model_name: String, throughput: Option<f64>, context_usage: Option<String>) -> Self {
        Self {
            model_name,
            throughput,
            context_usage,
        }
    }
}

impl Widget for MetricsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans = vec![
            Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.model_name, Style::default().fg(Color::Green)),
        ];

        if let Some(usage) = self.context_usage {
            spans.push(Span::raw(" │ "));
            spans.push(Span::styled("Ctx: ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(usage, Style::default().fg(Color::Cyan)));
        }

        if let Some(t) = self.throughput {
            spans.push(Span::raw(" │ "));
            spans.push(Span::styled(
                format!("{:.0} chars/s", t),
                Style::default().fg(Color::Yellow),
            ));
        }

        // Right-align
        let line = Line::from(spans);
        let width = line.width() as u16;
        if width <= area.width {
            buf.set_line(area.x + area.width - width, area.y, &line, width);
        }
    }
}
