use std::ops::Range;
use std::sync::OnceLock;

use gpui::{
    font, hsla, FontWeight, Hsla, SharedString, StrikethroughStyle, TextRun, TextStyle,
    UnderlineStyle,
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::gui::theme::Theme;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// Represents a clickable link region in the rendered markdown
#[derive(Debug, Clone)]
pub struct LinkRegion {
    /// Byte range in the rendered text where this link appears
    pub range: Range<usize>,
    /// The URL this link points to
    pub url: String,
}

#[derive(Debug, Clone, Default)]
pub struct RenderedMarkdown {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    /// Clickable link regions with their URLs
    pub links: Vec<LinkRegion>,
}

pub fn render_markdown(source: &str, text_style: &TextStyle, theme: &Theme) -> RenderedMarkdown {
    let mut text = String::new();
    let mut runs = Vec::new();

    let base_font = text_style.font();
    let base_color: Hsla = theme.text.into();
    let accent_color: Hsla = theme.accent.into();
    let _border_color: Hsla = theme.border.into();
    let panel_bg: Hsla = theme.panel_background.into();

    let code_font = font("monospace");
    // Subtle background for code
    let code_bg = hsla(0.0, 0.0, 0.25, 0.35);

    // Table colors
    let _table_header_bg = panel_bg; // Distinct header background
    let _table_row_even_bg = panel_bg; // Subtle zebra stripe (using panel bg)

    // Initialize Syntect
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let highlighter_theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.get("base16-mocha.dark"))
        .or_else(|| theme_set.themes.get("base16-eighties.dark"))
        .unwrap_or_else(|| theme_set.themes.values().next().unwrap());

    // State tracking
    let mut bold = false;
    let mut italic = false;
    let mut strikethrough = false;
    let mut header_level: Option<HeadingLevel> = None;
    let mut link_dest: Option<String> = None;
    let mut link_start: Option<usize> = None;
    let mut links: Vec<LinkRegion> = Vec::new();

    let mut in_code_block = false;
    let mut code_block_lang: Option<String> = None;
    let mut code_buffer = String::new();

    let mut in_block_quote = false;
    let mut in_list_item = false;
    let mut list_depth: usize = 0;
    let mut ordered_list_counters: Vec<usize> = Vec::new();

    // Enable GFM features
    let mut options = Options::empty();

    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(source, options);

    for event in parser {
        // Handle code block buffering content
        if in_code_block {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;

                    let syntax = code_block_lang
                        .as_ref()
                        .and_then(|lang| syntax_set.find_syntax_by_token(lang))
                        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

                    let mut highlighter = HighlightLines::new(syntax, highlighter_theme);

                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                        runs.push(TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: base_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }

                    for line in code_buffer.lines() {
                        let ranges: Vec<(syntect::highlighting::Style, &str)> = highlighter
                            .highlight_line(line, syntax_set)
                            .unwrap_or_default();

                        for (style, range_text) in ranges {
                            let range_color = syntect_color_to_hsla(style.foreground);
                            let len = range_text.len();
                            text.push_str(range_text);

                            let mut run_font = code_font.clone();
                            if style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::BOLD)
                            {
                                run_font = run_font.bold();
                            }
                            if style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::ITALIC)
                            {
                                run_font = run_font.italic();
                            }

                            runs.push(TextRun {
                                len,
                                font: run_font,
                                color: range_color,
                                background_color: Some(code_bg),
                                underline: None,
                                strikethrough: None,
                            });
                        }

                        text.push('\n');
                        runs.push(TextRun {
                            len: 1,
                            font: code_font.clone(),
                            color: base_color,
                            background_color: Some(code_bg),
                            underline: None,
                            strikethrough: None,
                        });
                    }

                    code_buffer.clear();
                    code_block_lang = None;

                    if !text.ends_with('\n') {
                        text.push('\n');
                        runs.push(TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: base_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }
                }
                Event::Text(t) => {
                    code_buffer.push_str(&t);
                }
                Event::SoftBreak | Event::HardBreak => {
                    code_buffer.push('\n');
                }
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Paragraph => {
                        // Don't add newline if we just added a list bullet
                        if in_list_item {
                            // Skip - bullet already positioned
                        } else if !text.is_empty()
                            && !text.ends_with("\n\n")
                            && !text.ends_with('\n')
                        {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                    }
                    Tag::Heading { level, .. } => {
                        if !text.is_empty() && !text.ends_with('\n') {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                        if !text.is_empty() && !text.ends_with("\n\n") {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                        header_level = Some(level);
                    }
                    Tag::BlockQuote => in_block_quote = true,
                    Tag::CodeBlock(kind) => {
                        in_code_block = true;
                        code_block_lang = match kind {
                            CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                            CodeBlockKind::Indented => None,
                        };
                    }
                    Tag::List(start) => {
                        if !text.is_empty() && !text.ends_with('\n') {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                        list_depth += 1;
                        if let Some(start_num) = start {
                            ordered_list_counters.push(start_num as usize);
                        } else {
                            ordered_list_counters.push(0); // 0 means unordered
                        }
                    }
                    Tag::Item => {
                        if !text.is_empty() && !text.ends_with('\n') {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                        // Add indentation for nested lists
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        if !indent.is_empty() {
                            text.push_str(&indent);
                            runs.push(TextRun {
                                len: indent.len(),
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                        // Add bullet or number
                        let marker = if let Some(counter) = ordered_list_counters.last_mut() {
                            if *counter > 0 {
                                let num = *counter;
                                *counter += 1;
                                format!("{}. ", num)
                            } else {
                                "• ".to_string()
                            }
                        } else {
                            "• ".to_string()
                        };
                        text.push_str(&marker);
                        runs.push(TextRun {
                            len: marker.len(),
                            font: base_font.clone(),
                            color: base_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                        in_list_item = true;
                    }
                    Tag::Emphasis => italic = true,
                    Tag::Strong => bold = true,
                    Tag::Strikethrough => strikethrough = true,

                    Tag::Link { dest_url, .. } => {
                        link_dest = Some(dest_url.to_string());
                        link_start = Some(text.len()); // Track where link text starts
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    TagEnd::Heading(_) => {
                        header_level = None;
                        text.push('\n');
                        runs.push(TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: base_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }
                    TagEnd::BlockQuote => in_block_quote = false,
                    TagEnd::CodeBlock => {}
                    TagEnd::Emphasis => italic = false,
                    TagEnd::Strong => bold = false,
                    TagEnd::Strikethrough => strikethrough = false,
                    TagEnd::Paragraph => {
                        text.push('\n');
                        runs.push(TextRun {
                            len: 1,
                            font: base_font.clone(),
                            color: base_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }
                    TagEnd::List(_) => {
                        list_depth = list_depth.saturating_sub(1);
                        ordered_list_counters.pop();
                        if !text.ends_with('\n') {
                            text.push('\n');
                            runs.push(TextRun {
                                len: 1,
                                font: base_font.clone(),
                                color: base_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                        }
                    }
                    TagEnd::Item => {
                        in_list_item = false;
                    }
                    TagEnd::Link => {
                        // Save the link region with its URL
                        if let (Some(start), Some(url)) = (link_start.take(), link_dest.take()) {
                            links.push(LinkRegion {
                                range: start..text.len(),
                                url,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                let mut font = base_font.clone();

                if bold {
                    font = font.bold();
                }
                if italic {
                    font = font.italic();
                }

                if let Some(level) = header_level {
                    font.weight = match level {
                        HeadingLevel::H1 => FontWeight::EXTRA_BOLD,
                        HeadingLevel::H2 => FontWeight::BOLD,
                        HeadingLevel::H3 => FontWeight::SEMIBOLD,
                        HeadingLevel::H4 => FontWeight::MEDIUM,
                        HeadingLevel::H5 => FontWeight::MEDIUM,
                        HeadingLevel::H6 => FontWeight::NORMAL,
                    };
                }

                if in_block_quote {
                    font = font.italic();
                }

                let color = if let Some(level) = header_level {
                    match level {
                        HeadingLevel::H1 | HeadingLevel::H2 => accent_color,
                        _ => base_color,
                    }
                } else if link_dest.is_some() {
                    accent_color
                } else {
                    base_color
                };

                let underline = if strikethrough {
                    Some(StrikethroughStyle {
                        color: Some(color),
                        thickness: 1.0.into(),
                    })
                } else {
                    None
                };

                let text_underline = if link_dest.is_some() {
                    Some(UnderlineStyle {
                        color: Some(color),
                        thickness: 1.0.into(),
                        wavy: false,
                    })
                } else {
                    None
                };

                let len = t.len();
                text.push_str(&t);
                runs.push(TextRun {
                    len,
                    font,
                    color,
                    background_color: None,
                    underline: text_underline,
                    strikethrough: underline,
                });
            }
            Event::Code(t) => {
                let len = t.len();
                text.push_str(&t);
                runs.push(TextRun {
                    len,
                    font: code_font.clone(),
                    color: base_color,
                    background_color: Some(code_bg),
                    underline: None,
                    strikethrough: None,
                });
            }
            Event::SoftBreak => {
                text.push('\n');
                runs.push(TextRun {
                    len: 1,
                    font: base_font.clone(),
                    color: base_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                });
            }
            Event::HardBreak => {
                text.push('\n');
                runs.push(TextRun {
                    len: 1,
                    font: base_font.clone(),
                    color: base_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                });
            }

            _ => {}
        }
    }

    let runs = merge_adjacent_runs(runs);

    RenderedMarkdown {
        text: SharedString::from(text),
        runs,
        links,
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

fn syntect_color_to_hsla(color: Color) -> Hsla {
    gpui::rgba(
        ((color.r as u32) << 24)
            | ((color.g as u32) << 16)
            | ((color.b as u32) << 8)
            | (color.a as u32),
    )
    .into()
}
