use std::ops::Range;

use gpui::{font, hsla, Hsla, SharedString, TextRun, TextStyle};

use crate::gui::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownSegment {
    Plain(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Header(u8, String),
}

#[derive(Debug, Clone, Default)]
pub struct RenderedMarkdown {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
}

pub fn render_markdown(source: &str, text_style: &TextStyle, theme: &Theme) -> RenderedMarkdown {
    eprintln!(
        "[markdown] render_markdown called, source len={}",
        source.len()
    );
    let segments = parse_markdown(source);
    eprintln!("[markdown] parsed {} segments:", segments.len());
    for (i, seg) in segments.iter().take(10).enumerate() {
        eprintln!("[markdown]   segment {}: {:?}", i, seg);
    }

    let mut text = String::new();
    let mut runs = Vec::new();

    let base_font = text_style.font();
    let base_color: Hsla = theme.text.into();
    let header_color: Hsla = theme.text.into();

    let code_font = font("monospace");
    let code_bg = hsla(0.0, 0.0, 0.25, 0.35);

    for segment in segments {
        let (segment_text, font, color, background_color) = match segment {
            MarkdownSegment::Plain(s) => (s, base_font.clone(), base_color, None),
            MarkdownSegment::Bold(s) => (s, base_font.clone().bold(), base_color, None),
            MarkdownSegment::Italic(s) => (s, base_font.clone().italic(), base_color, None),
            MarkdownSegment::BoldItalic(s) => {
                (s, base_font.clone().bold().italic(), base_color, None)
            }
            MarkdownSegment::Code(s) => (s, code_font.clone(), base_color, Some(code_bg)),
            MarkdownSegment::Header(_level, s) => (s, base_font.clone().bold(), header_color, None),
        };

        if segment_text.is_empty() {
            continue;
        }

        let len = segment_text.len();
        text.push_str(&segment_text);
        runs.push(TextRun {
            len,
            font,
            color,
            background_color,
            underline: None,
            strikethrough: None,
        });
    }

    let runs = merge_adjacent_runs(runs);
    let preview: String = text.chars().take(100).collect();
    eprintln!("[markdown] final text (first 100): '{}'", preview);
    eprintln!(
        "[markdown] render_markdown complete, text len={}, runs={}",
        text.len(),
        runs.len()
    );

    RenderedMarkdown {
        text: SharedString::from(text),
        runs,
    }
}

pub fn apply_selection_background(
    runs: &[TextRun],
    mut selection: Range<usize>,
    selection_bg: Hsla,
) -> Vec<TextRun> {
    let total_len: usize = runs.iter().map(|r| r.len).sum();
    selection.start = selection.start.min(total_len);
    selection.end = selection.end.min(total_len);
    if selection.is_empty() {
        return runs.to_vec();
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;

    for run in runs {
        let run_start = cursor;
        let run_end = cursor + run.len;
        cursor = run_end;

        if selection.end <= run_start || selection.start >= run_end {
            out.push(run.clone());
            continue;
        }

        if selection.start > run_start {
            let mut prefix = run.clone();
            prefix.len = selection.start - run_start;
            out.push(prefix);
        }

        let sel_start = selection.start.max(run_start);
        let sel_end = selection.end.min(run_end);
        if sel_end > sel_start {
            let mut selected = run.clone();
            selected.len = sel_end - sel_start;
            selected.background_color = Some(selection_bg);
            out.push(selected);
        }

        if selection.end < run_end {
            let mut suffix = run.clone();
            suffix.len = run_end - selection.end;
            out.push(suffix);
        }
    }

    merge_adjacent_runs(out)
}

fn merge_adjacent_runs(runs: Vec<TextRun>) -> Vec<TextRun> {
    let mut merged: Vec<TextRun> = Vec::new();

    for run in runs {
        if run.len == 0 {
            continue;
        }

        if let Some(last) = merged.last_mut() {
            if last.font == run.font
                && last.color == run.color
                && last.background_color == run.background_color
                && last.underline == run.underline
                && last.strikethrough == run.strikethrough
            {
                last.len += run.len;
                continue;
            }
        }

        merged.push(run);
    }

    merged
}

fn parse_markdown(source: &str) -> Vec<MarkdownSegment> {
    let mut segments = Vec::new();

    for part in source.split_inclusive('\n') {
        let (line, has_newline) = match part.strip_suffix('\n') {
            Some(line) => (line, true),
            None => (part, false),
        };

        if let Some((level, header_text)) = parse_header_line(line) {
            segments.push(MarkdownSegment::Header(level, header_text.to_string()));
        } else {
            segments.extend(parse_inline(line));
        }

        if has_newline {
            segments.push(MarkdownSegment::Plain("\n".to_string()));
        }
    }

    // Handle empty input or a trailing empty segment when `source` doesn't end with '\n'.
    if source.is_empty() {
        segments.clear();
    }

    segments
}

fn parse_header_line(line: &str) -> Option<(u8, &str)> {
    let mut level = 0u8;
    for ch in line.chars() {
        if ch == '#' {
            level += 1;
        } else {
            break;
        }
    }

    if (1..=6).contains(&level) {
        let rest = &line[level as usize..];
        if let Some(rest) = rest.strip_prefix(' ') {
            return Some((level, rest));
        }
    }

    None
}

fn parse_inline(input: &str) -> Vec<MarkdownSegment> {
    let mut segments = Vec::new();
    let mut i = 0usize;

    while i < input.len() {
        let remaining = &input[i..];

        // Try to match code: `code`
        if let Some(stripped) = remaining.strip_prefix('`') {
            if let Some(end) = stripped.find('`') {
                let code = &stripped[..end];
                segments.push(MarkdownSegment::Code(code.to_string()));
                i += 1 + end + 1;
                continue;
            }

            segments.push(MarkdownSegment::Plain("`".to_string()));
            i += 1;
            continue;
        }

        // Try to match bold+italic: ***text***
        if let Some(stripped) = remaining.strip_prefix("***") {
            if let Some(end) = stripped.find("***") {
                let inner = &stripped[..end];
                segments.push(MarkdownSegment::BoldItalic(inner.to_string()));
                i += 3 + end + 3;
                continue;
            }

            segments.push(MarkdownSegment::Plain("***".to_string()));
            i += 3;
            continue;
        }

        // Try to match bold: **text**
        if let Some(stripped) = remaining.strip_prefix("**") {
            if let Some(end) = stripped.find("**") {
                let inner = &stripped[..end];
                eprintln!("[markdown] found BOLD: '{}'", inner);
                segments.push(MarkdownSegment::Bold(inner.to_string()));
                i += 2 + end + 2;
                continue;
            }

            segments.push(MarkdownSegment::Plain("**".to_string()));
            i += 2;
            continue;
        }

        // Try to match italic: *text*
        if let Some(stripped) = remaining.strip_prefix('*') {
            if let Some(end) = stripped.find('*') {
                let inner = &stripped[..end];
                if !inner.is_empty() {
                    segments.push(MarkdownSegment::Italic(inner.to_string()));
                    i += 1 + end + 1;
                    continue;
                }
            }

            segments.push(MarkdownSegment::Plain("*".to_string()));
            i += 1;
            continue;
        }

        // Try to match italic: _text_
        if let Some(stripped) = remaining.strip_prefix('_') {
            if let Some(end) = stripped.find('_') {
                let inner = &stripped[..end];
                if !inner.is_empty() {
                    segments.push(MarkdownSegment::Italic(inner.to_string()));
                    i += 1 + end + 1;
                    continue;
                }
            }

            segments.push(MarkdownSegment::Plain("_".to_string()));
            i += 1;
            continue;
        }

        // No markdown matched - consume plain text until next potential delimiter.
        // IMPORTANT: Always advance at least 1 character to prevent infinite loops.
        let first_len = remaining.chars().next().unwrap().len_utf8();
        let next_special = remaining[first_len..]
            .find(|c| ['`', '*', '_'].contains(&c))
            .map(|pos| first_len + pos)
            .unwrap_or(remaining.len());

        let advance = next_special.max(first_len);
        let plain = &remaining[..advance];
        segments.push(MarkdownSegment::Plain(plain.to_string()));
        i += advance;
    }

    segments
}
