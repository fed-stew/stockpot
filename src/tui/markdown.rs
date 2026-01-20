//! Simple markdown parser for TUI rendering
//!
//! Converts markdown text to styled spans for display in the activity feed.
//! Handles: bold, italic, inline code, code blocks with syntax highlighting,
//! headers, tables, blockquotes, links, and strikethrough.

use ratatui::style::{Modifier, Style};

use super::theme::Theme;

/// A styled text span from markdown parsing
#[derive(Debug, Clone, PartialEq)]
pub struct MdSpan {
    pub text: String,
    pub style: Style,
}

impl MdSpan {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::TEXT))
    }

    pub fn bold(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )
    }

    pub fn italic(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::TEXT)
                .add_modifier(Modifier::ITALIC),
        )
    }

    pub fn strikethrough(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::MUTED)
                .add_modifier(Modifier::CROSSED_OUT),
        )
    }

    pub fn code(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::COMMAND))
    }

    pub fn header(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::HEADER)
                .add_modifier(Modifier::BOLD),
        )
    }

    pub fn link_text(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::UNDERLINED),
        )
    }

    pub fn link_url(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::MUTED))
    }

    pub fn blockquote_bar(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::ACCENT))
    }

    pub fn blockquote_text(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Style::default()
                .fg(Theme::MUTED)
                .add_modifier(Modifier::ITALIC),
        )
    }

    // Syntax highlighting styles
    pub fn keyword(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::ACCENT))
    }

    pub fn string_lit(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::GREEN))
    }

    pub fn comment(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::MUTED))
    }

    pub fn number(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::YELLOW))
    }

    pub fn table_border(text: impl Into<String>) -> Self {
        Self::new(text, Style::default().fg(Theme::MUTED))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Parser state for inline formatting
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    Bold,
    Italic,
    BoldItalic,
    InlineCode,
    Strikethrough,
}

/// Parse markdown text into styled spans
pub fn parse_markdown(text: &str) -> Vec<MdSpan> {
    let lines: Vec<&str> = text.lines().collect();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Check for code block start
        if line.trim_start().starts_with("```") {
            let lang = line.trim_start().strip_prefix("```").unwrap_or("");
            let mut code_lines = Vec::new();
            i += 1;

            // Collect code block content
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }

            // Skip closing ```
            if i < lines.len() {
                i += 1;
            }

            // Highlight and add code block
            spans.extend(highlight_code_block(&code_lines, lang.trim()));
            if i < lines.len() {
                spans.push(MdSpan::plain("\n"));
            }
            continue;
        }

        // Check for table (line starts and ends with |)
        if is_table_line(line) {
            let table_start = i;
            let mut table_lines = vec![line];
            i += 1;

            // Collect consecutive table rows
            while i < lines.len() && is_table_line(lines[i]) {
                table_lines.push(lines[i]);
                i += 1;
            }

            // Get the line following the table (if any)
            let following_line = lines.get(i).copied();

            // Check if table is complete
            if is_table_complete(&table_lines, following_line) {
                // Render as pretty table with box-drawing
                if let Some(table_spans) = parse_table(&table_lines) {
                    spans.extend(table_spans);
                    if i < lines.len() {
                        spans.push(MdSpan::plain("\n"));
                    }
                    continue;
                }
            }

            // Incomplete or invalid table - show as raw text
            for (idx, tl) in table_lines.iter().enumerate() {
                spans.push(MdSpan::plain(*tl));
                if table_start + idx < lines.len() - 1 {
                    spans.push(MdSpan::plain("\n"));
                }
            }
            continue;
        }

        // Check for blockquote
        if line.trim_start().starts_with("> ") {
            let quote_content = line.trim_start().strip_prefix("> ").unwrap_or("");
            spans.push(MdSpan::blockquote_bar("│ "));
            // Parse inline markdown within the blockquote
            let inner_spans = parse_inline_markdown(quote_content);
            for inner in inner_spans {
                // Apply blockquote styling (italic + muted) to the content
                spans.push(MdSpan::new(
                    inner.text,
                    inner.style.fg(Theme::MUTED).add_modifier(Modifier::ITALIC),
                ));
            }
            if i < lines.len() - 1 {
                spans.push(MdSpan::plain("\n"));
            }
            i += 1;
            continue;
        }

        // Parse regular line with inline markdown
        spans.extend(parse_inline_markdown(line));
        if i < lines.len() - 1 {
            spans.push(MdSpan::plain("\n"));
        }
        i += 1;
    }

    spans
}

/// Parse inline markdown (bold, italic, code, headers, links, strikethrough)
fn parse_inline_markdown(text: &str) -> Vec<MdSpan> {
    // First check for header at line start
    if text.trim_start().starts_with('#') {
        let trimmed = text.trim_start();
        let mut level = 0;
        for c in trimmed.chars() {
            if c == '#' {
                level += 1;
            } else {
                break;
            }
        }
        let header_text = trimmed[level..].trim_start();
        return vec![MdSpan::header(header_text)];
    }

    // Pre-process: extract links and replace with placeholders
    let (processed_text, links) = extract_links(text);

    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut state = State::Normal;
    let mut chars = processed_text.chars().peekable();
    let mut link_idx = 0;

    while let Some(ch) = chars.next() {
        // Check for link placeholder
        if ch == '\x00' {
            // Flush current text
            if !current_text.is_empty() {
                spans.push(make_span(&current_text, state));
                current_text.clear();
            }
            // Insert link spans
            if link_idx < links.len() {
                let (link_text, link_url) = &links[link_idx];
                spans.push(MdSpan::link_text(link_text));
                spans.push(MdSpan::plain(" "));
                spans.push(MdSpan::link_url(format!("({})", link_url)));
                link_idx += 1;
            }
            continue;
        }

        // Handle inline code (highest priority)
        if ch == '`' {
            if !current_text.is_empty() {
                spans.push(make_span(&current_text, state));
                current_text.clear();
            }

            if state == State::InlineCode {
                state = State::Normal;
            } else {
                state = State::InlineCode;
            }
            continue;
        }

        if state == State::InlineCode {
            current_text.push(ch);
            continue;
        }

        // Handle strikethrough (~~)
        if ch == '~' && chars.peek() == Some(&'~') {
            chars.next(); // consume second ~
            if !current_text.is_empty() {
                spans.push(make_span(&current_text, state));
                current_text.clear();
            }
            state = match state {
                State::Strikethrough => State::Normal,
                _ => State::Strikethrough,
            };
            continue;
        }

        // Handle bold (***) or bold+italic
        if ch == '*' && chars.peek() == Some(&'*') {
            chars.next();

            if chars.peek() == Some(&'*') {
                chars.next();
                if !current_text.is_empty() {
                    spans.push(make_span(&current_text, state));
                    current_text.clear();
                }
                state = match state {
                    State::BoldItalic => State::Normal,
                    _ => State::BoldItalic,
                };
                continue;
            }

            if !current_text.is_empty() {
                spans.push(make_span(&current_text, state));
                current_text.clear();
            }
            state = match state {
                State::Bold => State::Normal,
                State::BoldItalic => State::Italic,
                State::Italic => State::BoldItalic,
                _ => State::Bold,
            };
            continue;
        }

        // Handle italic (*)
        if ch == '*' {
            let is_closing = matches!(state, State::Italic | State::BoldItalic);
            let next_is_space = chars.peek() == Some(&' ');

            if is_closing || !next_is_space {
                if !current_text.is_empty() {
                    spans.push(make_span(&current_text, state));
                    current_text.clear();
                }
                state = match state {
                    State::Italic => State::Normal,
                    State::BoldItalic => State::Bold,
                    State::Bold => State::BoldItalic,
                    _ => State::Italic,
                };
                continue;
            }
        }

        // Handle underscore bold/italic
        if ch == '_' {
            if chars.peek() == Some(&'_') {
                chars.next();
                if !current_text.is_empty() {
                    spans.push(make_span(&current_text, state));
                    current_text.clear();
                }
                state = match state {
                    State::Bold => State::Normal,
                    _ => State::Bold,
                };
                continue;
            } else {
                // Single _ - check context
                let prev_is_space = current_text.is_empty() || current_text.ends_with(' ');
                let next_is_space = chars.peek() == Some(&' ') || chars.peek().is_none();
                
                // Only treat as italic if not surrounded by spaces (word boundary)
                if !prev_is_space && !next_is_space {
                    current_text.push(ch);
                    continue;
                }
                
                if !current_text.is_empty() {
                    spans.push(make_span(&current_text, state));
                    current_text.clear();
                }
                state = match state {
                    State::Italic => State::Normal,
                    _ => State::Italic,
                };
                continue;
            }
        }

        current_text.push(ch);
    }

    if !current_text.is_empty() {
        spans.push(make_span(&current_text, state));
    }

    spans
}

/// Extract links from text and replace with placeholders
fn extract_links(text: &str) -> (String, Vec<(String, String)>) {
    let mut result = String::new();
    let mut links = Vec::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            // Try to parse link: [text](url)
            let mut link_text = String::new();
            let mut found_close_bracket = false;

            while let Some(&c) = chars.peek() {
                if c == ']' {
                    chars.next();
                    found_close_bracket = true;
                    break;
                }
                link_text.push(chars.next().unwrap());
            }

            if found_close_bracket && chars.peek() == Some(&'(') {
                chars.next(); // consume (
                let mut url = String::new();
                let mut found_close_paren = false;

                while let Some(&c) = chars.peek() {
                    if c == ')' {
                        chars.next();
                        found_close_paren = true;
                        break;
                    }
                    url.push(chars.next().unwrap());
                }

                if found_close_paren && !link_text.is_empty() {
                    // Valid link - add placeholder
                    result.push('\x00');
                    links.push((link_text, url));
                    continue;
                } else {
                    // Not a valid link, output as-is
                    result.push('[');
                    result.push_str(&link_text);
                    result.push(']');
                    result.push('(');
                    result.push_str(&url);
                }
            } else {
                // Not a link
                result.push('[');
                result.push_str(&link_text);
                if found_close_bracket {
                    result.push(']');
                }
            }
        } else {
            result.push(ch);
        }
    }

    (result, links)
}

fn make_span(text: &str, state: State) -> MdSpan {
    match state {
        State::Normal => MdSpan::plain(text),
        State::Bold => MdSpan::bold(text),
        State::Italic => MdSpan::italic(text),
        State::BoldItalic => MdSpan::new(
            text,
            Style::default()
                .fg(Theme::TEXT)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
        ),
        State::InlineCode => MdSpan::code(text),
        State::Strikethrough => MdSpan::strikethrough(text),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Code Block Syntax Highlighting
// ─────────────────────────────────────────────────────────────────────────────

/// Keywords for common languages
const KEYWORDS: &[&str] = &[
    // Rust
    "fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while",
    "loop", "return", "break", "continue", "struct", "enum", "impl", "trait",
    "pub", "use", "mod", "crate", "self", "Self", "super", "where", "async",
    "await", "move", "ref", "type", "dyn", "as", "in", "unsafe",
    // Python
    "def", "class", "import", "from", "try", "except", "finally",
    "with", "lambda", "yield", "global", "nonlocal", "assert", "pass",
    "raise", "True", "False", "None", "and", "or", "not", "is", "elif",
    // JavaScript/TypeScript
    "function", "var", "class", "extends", "new",
    "this", "typeof", "instanceof", "delete", "void",
    "export", "default",
    "true", "false", "null", "undefined",
    // Go
    "func", "package", "interface",
    "map", "chan", "go", "defer", "select", "case", "fallthrough",
    "range", "nil",
];

/// Highlight a code block with simple syntax highlighting
fn highlight_code_block(lines: &[&str], _language: &str) -> Vec<MdSpan> {
    let mut spans = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        spans.extend(highlight_code_line(line));
        if i < lines.len() - 1 {
            spans.push(MdSpan::plain("\n"));
        }
    }

    spans
}

/// Highlight a single line of code
fn highlight_code_line(line: &str) -> Vec<MdSpan> {
    let mut spans = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        // Comments (// or #)
        if ch == '/' && chars.peek() == Some(&'/') {
            if !current.is_empty() {
                spans.extend(tokenize_code(&current));
                current.clear();
            }
            let mut comment = String::from("//");
            chars.next(); // consume second /
            for c in chars.by_ref() {
                comment.push(c);
            }
            spans.push(MdSpan::comment(comment));
            continue;
        }

        // Python/shell comment
        if ch == '#' {
            if !current.is_empty() {
                spans.extend(tokenize_code(&current));
                current.clear();
            }
            let mut comment = String::from("#");
            for c in chars.by_ref() {
                comment.push(c);
            }
            spans.push(MdSpan::comment(comment));
            continue;
        }

        // Strings
        if ch == '"' || ch == '\'' {
            if !current.is_empty() {
                spans.extend(tokenize_code(&current));
                current.clear();
            }
            let quote = ch;
            let mut string = String::from(ch);
            let mut escaped = false;
            while let Some(c) = chars.next() {
                string.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    break;
                }
            }
            spans.push(MdSpan::string_lit(string));
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        spans.extend(tokenize_code(&current));
    }

    // If line is empty, return a single empty span
    if spans.is_empty() {
        spans.push(MdSpan::plain(""));
    }

    spans
}

/// Tokenize code and apply keyword/number highlighting
fn tokenize_code(text: &str) -> Vec<MdSpan> {
    let mut spans = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                spans.push(classify_token(&current));
                current.clear();
            }
            spans.push(MdSpan::plain(ch.to_string()));
        }
    }

    if !current.is_empty() {
        spans.push(classify_token(&current));
    }

    spans
}

/// Classify a token as keyword, number, or plain
fn classify_token(token: &str) -> MdSpan {
    if KEYWORDS.contains(&token) {
        MdSpan::keyword(token)
    } else if token.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '_')
        && token.chars().any(|c| c.is_ascii_digit())
    {
        MdSpan::number(token)
    } else {
        MdSpan::plain(token)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Table Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Column alignment for table cells
#[derive(Clone, Copy, Default, Debug, PartialEq)]
enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Check if a character is an emoji
/// Emojis render as 1 char wide in most terminals (especially macOS),
/// despite unicode-width reporting them as 2.
fn is_emoji(c: char) -> bool {
    matches!(
        c,
        '\u{1F300}'..='\u{1F9FF}'    // Misc Symbols, Emoticons, etc. (✅❌🔥 etc.)
        | '\u{2600}'..='\u{26FF}'    // Misc Symbols (⚠⚡☀ etc.)
        | '\u{2700}'..='\u{27BF}'    // Dingbats (✓✗✂ etc.)
        | '\u{FE00}'..='\u{FE0F}'    // Variation Selectors
        | '\u{1F000}'..='\u{1F02F}'  // Mahjong, Dominos
        | '\u{1F0A0}'..='\u{1F0FF}'  // Playing Cards
        | '\u{2300}'..='\u{23FF}'    // Misc Technical (⌘⌛ etc.)
        | '\u{2B50}'..='\u{2B55}'    // Stars, circles (⭐⭕ etc.)
        | '\u{25A0}'..='\u{25FF}'    // Geometric Shapes (●○■□ etc.)
        | '\u{2190}'..='\u{21FF}'    // Arrows (←→↑↓ etc.)
    )
}

/// Calculate display width of a string, treating emojis as 1 char wide
fn display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    s.chars()
        .map(|c| {
            if is_emoji(c) {
                1 // Emojis are 1 char wide in most terminals
            } else {
                UnicodeWidthChar::width(c).unwrap_or(1)
            }
        })
        .sum()
}

/// Parse alignment from a separator cell (e.g., ":---", ":---:", "---:")
fn parse_alignment(sep_cell: &str) -> Alignment {
    let trimmed = sep_cell.trim();
    let starts_colon = trimmed.starts_with(':');
    let ends_colon = trimmed.ends_with(':');

    match (starts_colon, ends_colon) {
        (true, true) => Alignment::Center,
        (false, true) => Alignment::Right,
        _ => Alignment::Left, // Default or :--- is left
    }
}

/// Pad a cell to the specified width with the given alignment
fn pad_cell(cell: &str, width: usize, align: Alignment) -> String {
    let cell_width = display_width(cell);
    let padding = width.saturating_sub(cell_width);

    match align {
        Alignment::Left => format!("{}{}", cell, " ".repeat(padding)),
        Alignment::Right => format!("{}{}", " ".repeat(padding), cell),
        Alignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            format!("{}{}{}", " ".repeat(left_pad), cell, " ".repeat(right_pad))
        }
    }
}

/// Check if a line looks like a table row
/// Must start with | and either end with | or have multiple | characters
fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return false;
    }
    // Either ends with | or has at least 2 | characters (multiple columns)
    trimmed.ends_with('|') || trimmed.matches('|').count() >= 2
}

/// Check if a table is complete and ready for pretty-printing
/// A table is complete when:
/// 1. It has at least 2 lines (header + separator or header + data)
/// 2. It has a separator row (contains ---)
/// 3. It's followed by a blank line, non-table line, or end of content
fn is_table_complete(table_lines: &[&str], following_line: Option<&str>) -> bool {
    // Must have at least header and separator
    if table_lines.len() < 2 {
        return false;
    }

    // Must have a separator row (line containing ---)
    let has_separator = table_lines.iter().any(|l| {
        let trimmed = l.trim().trim_matches('|');
        trimmed.split('|').any(|cell| {
            let cell = cell.trim();
            !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':' || c == ' ')
        })
    });

    if !has_separator {
        return false;
    }

    // Check what follows the table
    match following_line {
        None => true, // End of content
        Some(line) => {
            let trimmed = line.trim();
            // Complete if followed by blank line or non-table content
            trimmed.is_empty() || !is_table_line(line)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Table Parsing
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a markdown table into styled spans (only call for complete tables)
fn parse_table(lines: &[&str]) -> Option<Vec<MdSpan>> {
    if lines.len() < 2 {
        return None;
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut separator_cells: Vec<String> = Vec::new();
    let mut separator_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim().to_string())
            .collect();

        // Check if this is a separator row
        if cells
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
        {
            separator_idx = Some(i);
            separator_cells = cells; // Save separator cells for alignment parsing
        } else {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return None;
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return None;
    }

    // Parse alignments from separator row
    let alignments: Vec<Alignment> = (0..num_cols)
        .map(|i| {
            separator_cells
                .get(i)
                .map(|s| parse_alignment(s))
                .unwrap_or_default()
        })
        .collect();

    // Calculate column widths using our custom display_width (handles emojis correctly)
    let mut col_widths: Vec<usize> = vec![0; num_cols];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(display_width(cell));
            }
        }
    }

    // Ensure minimum width of 3
    for w in &mut col_widths {
        *w = (*w).max(3);
    }

    let mut spans = Vec::new();

    // Top border
    spans.extend(render_table_border(&col_widths, '┌', '┬', '┐'));
    spans.push(MdSpan::plain("\n"));

    // Render rows
    for (row_idx, row) in rows.iter().enumerate() {
        spans.push(MdSpan::table_border("│"));
        for (i, width) in col_widths.iter().enumerate() {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let align = alignments.get(i).copied().unwrap_or_default();
            let padded = pad_cell(cell, *width, align);
            spans.push(MdSpan::plain(format!(" {} ", padded)));
            spans.push(MdSpan::table_border("│"));
        }
        spans.push(MdSpan::plain("\n"));

        // Add separator after header row
        if row_idx == 0 && separator_idx.is_some() {
            spans.extend(render_table_border(&col_widths, '├', '┼', '┤'));
            spans.push(MdSpan::plain("\n"));
        }
    }

    // Bottom border
    spans.extend(render_table_border(&col_widths, '└', '┴', '┘'));

    Some(spans)
}

fn render_table_border(widths: &[usize], left: char, mid: char, right: char) -> Vec<MdSpan> {
    let mut spans = Vec::new();
    spans.push(MdSpan::table_border(left.to_string()));

    for (i, width) in widths.iter().enumerate() {
        spans.push(MdSpan::table_border("─".repeat(width + 2)));
        if i < widths.len() - 1 {
            spans.push(MdSpan::table_border(mid.to_string()));
        }
    }

    spans.push(MdSpan::table_border(right.to_string()));
    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let spans = parse_markdown("Hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Hello world");
    }

    #[test]
    fn test_bold() {
        let spans = parse_markdown("Hello **bold** world");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "Hello ");
        assert_eq!(spans[1].text, "bold");
        assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[2].text, " world");
    }

    #[test]
    fn test_italic() {
        let spans = parse_markdown("Hello *italic* world");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "italic");
        assert!(spans[1].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_inline_code() {
        let spans = parse_markdown("Use `code` here");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "code");
    }

    #[test]
    fn test_header() {
        let spans = parse_markdown("# Header");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Header");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_code_block_keywords() {
        let spans = parse_markdown("```rust\nfn main() {\n}\n```");
        let fn_span = spans.iter().find(|s| s.text == "fn");
        assert!(fn_span.is_some(), "Should have 'fn' keyword");
        // Check it's highlighted as keyword (ACCENT color)
        let fn_span = fn_span.unwrap();
        assert_eq!(fn_span.style.fg, Some(Theme::ACCENT));
    }

    #[test]
    fn test_code_block_strings() {
        let spans = parse_markdown("```\nlet x = \"hello\"\n```");
        let string_span = spans.iter().find(|s| s.text.contains("hello"));
        assert!(string_span.is_some());
        // Check it's highlighted as string (GREEN color)
        let string_span = string_span.unwrap();
        assert_eq!(string_span.style.fg, Some(Theme::GREEN));
    }

    #[test]
    fn test_code_block_comments() {
        let spans = parse_markdown("```\n// comment here\n```");
        let comment_span = spans.iter().find(|s| s.text.contains("comment"));
        assert!(comment_span.is_some());
        // Check it's highlighted as comment (MUTED color)
        let comment_span = comment_span.unwrap();
        assert_eq!(comment_span.style.fg, Some(Theme::MUTED));
    }

    #[test]
    fn test_code_block_numbers() {
        let spans = parse_markdown("```\nlet x = 42\n```");
        let num_span = spans.iter().find(|s| s.text == "42");
        assert!(num_span.is_some());
        // Check it's highlighted as number (YELLOW color)
        let num_span = num_span.unwrap();
        assert_eq!(num_span.style.fg, Some(Theme::YELLOW));
    }

    #[test]
    fn test_blockquote() {
        let spans = parse_markdown("> This is quoted");
        // Should have the bar prefix
        let bar_span = spans.iter().find(|s| s.text == "│ ");
        assert!(bar_span.is_some(), "Should have blockquote bar");
        // Should have the quoted text with italic
        let quote_span = spans.iter().find(|s| s.text.contains("quoted"));
        assert!(quote_span.is_some());
        assert!(quote_span.unwrap().style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_link() {
        let spans = parse_markdown("[Click here](https://example.com)");
        // Should have link text
        let text_span = spans.iter().find(|s| s.text == "Click here");
        assert!(text_span.is_some(), "Should have link text");
        // Should have URL
        let url_span = spans.iter().find(|s| s.text.contains("example.com"));
        assert!(url_span.is_some(), "Should have URL");
    }

    #[test]
    fn test_strikethrough() {
        let spans = parse_markdown("This is ~~deleted~~ text");
        let strike_span = spans.iter().find(|s| s.text == "deleted");
        assert!(strike_span.is_some(), "Should have strikethrough text");
        assert!(
            strike_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
    }

    #[test]
    fn test_table_basic() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let spans = parse_markdown(input);
        let has_border = spans.iter().any(|s| s.text.contains('┌') || s.text.contains('│'));
        assert!(has_border);
    }

    #[test]
    fn test_table_content() {
        let input = "| Name | Age |\n|------|-----|\n| John | 30 |";
        let spans = parse_markdown(input);
        let has_name = spans.iter().any(|s| s.text.contains("Name"));
        let has_john = spans.iter().any(|s| s.text.contains("John"));
        assert!(has_name);
        assert!(has_john);
    }

    #[test]
    fn test_mixed() {
        let spans = parse_markdown("Normal **bold** and *italic* and `code`");
        assert!(spans.len() >= 5);
    }

    #[test]
    fn test_link_in_text() {
        let spans = parse_markdown("Check out [this link](https://rust-lang.org) for more");
        let has_text = spans.iter().any(|s| s.text == "this link");
        let has_url = spans.iter().any(|s| s.text.contains("rust-lang.org"));
        assert!(has_text, "Should extract link text");
        assert!(has_url, "Should extract URL");
    }

    #[test]
    fn test_incomplete_table_no_separator() {
        // Table without separator row should show as raw text
        let input = "| A | B |\n| 1 | 2 |";
        let spans = parse_markdown(input);
        // Should NOT have box-drawing characters
        let has_box = spans.iter().any(|s| s.text.contains('┌') || s.text.contains('─'));
        assert!(!has_box, "Incomplete table should show as raw text, not box-drawing");
        // Should have the raw pipe characters
        let has_raw = spans.iter().any(|s| s.text.contains("| A |"));
        assert!(has_raw, "Should show raw table text");
    }

    #[test]
    fn test_incomplete_table_single_line() {
        // Single line should show as raw text
        let input = "| Header |";
        let spans = parse_markdown(input);
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(!has_box, "Single line should not render as table");
    }

    #[test]
    fn test_complete_table_with_separator() {
        // Complete table with separator should render with box-drawing
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let spans = parse_markdown(input);
        let has_box = spans.iter().any(|s| s.text.contains('┌') || s.text.contains('│'));
        assert!(has_box, "Complete table should render with box-drawing");
    }

    #[test]
    fn test_table_helpers() {
        // Test is_table_line
        assert!(is_table_line("| A | B |"));
        assert!(is_table_line("  | A | B |  "));
        assert!(is_table_line("|---|---|"));
        assert!(!is_table_line("Not a table"));
        assert!(!is_table_line("| incomplete"));  // Missing closing |

        // Test is_table_complete
        let complete = vec!["| A | B |", "|---|---|", "| 1 | 2 |"];
        assert!(is_table_complete(&complete, None));
        assert!(is_table_complete(&complete, Some("")));
        assert!(is_table_complete(&complete, Some("Some text")));

        let incomplete_no_sep = vec!["| A | B |", "| 1 | 2 |"];
        assert!(!is_table_complete(&incomplete_no_sep, None));

        let single_line = vec!["| A | B |"];
        assert!(!is_table_complete(&single_line, None));
    }

    #[test]
    fn test_table_emoji_alignment() {
        // Table with emojis - should use our custom display width for alignment
        let input = "| Status | Name |\n|--------|------|\n| ✅ | Pass |\n| ❌ | Fail |\n";
        let spans = parse_markdown(input);

        // Should render as table
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Emoji table should render with box-drawing");

        // Check that cells are padded correctly
        let status_cells: Vec<&MdSpan> = spans
            .iter()
            .filter(|s| s.text.contains('✅') || s.text.contains('❌'))
            .collect();

        assert!(!status_cells.is_empty(), "Should have emoji cells");

        // Verify our custom display_width treats emojis as 1 char wide
        // (unlike unicode-width which says 2)
        assert_eq!(display_width("✅"), 1, "Emoji should have display width of 1");
        assert_eq!("✅".len(), 3, "Emoji has byte length of 3");
    }

    #[test]
    fn test_table_alignment_left() {
        let input = "| Name |\n|:-----|\n| A |\n";
        let spans = parse_markdown(input);
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table");
        // Left alignment means content is at the start with trailing spaces
    }

    #[test]
    fn test_table_alignment_right() {
        let input = "| Name |\n|-----:|\n| A |\n";
        let spans = parse_markdown(input);
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table");
        // Find the cell with "A" - should have leading spaces
        let a_cell = spans.iter().find(|s| s.text.contains('A'));
        assert!(a_cell.is_some());
        // Right-aligned: spaces come before the content
        let text = &a_cell.unwrap().text;
        assert!(text.trim_start() != text.trim(), "Right-aligned should have leading spaces");
    }

    #[test]
    fn test_table_alignment_center() {
        let input = "| Name |\n|:----:|\n| A |\n";
        let spans = parse_markdown(input);
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table");
    }

    #[test]
    fn test_display_width_helpers() {
        // Test is_emoji
        assert!(is_emoji('✅'));
        assert!(is_emoji('❌'));
        assert!(is_emoji('⚠'));
        assert!(!is_emoji('A'));
        assert!(!is_emoji('中')); // Chinese char, not emoji

        // Test display_width
        assert_eq!(display_width("Hello"), 5);
        assert_eq!(display_width("✅"), 1); // Emoji = 1 char
        assert_eq!(display_width("✅✅"), 2); // Two emojis = 2 chars
        assert_eq!(display_width("中文"), 4); // Chinese chars = 2 each
        assert_eq!(display_width("A✅B"), 3); // Mixed: A(1) + ✅(1) + B(1)
    }

    #[test]
    fn test_parse_alignment() {
        assert_eq!(parse_alignment("---"), Alignment::Left);
        assert_eq!(parse_alignment(":---"), Alignment::Left);
        assert_eq!(parse_alignment("---:"), Alignment::Right);
        assert_eq!(parse_alignment(":---:"), Alignment::Center);
        assert_eq!(parse_alignment("  :---:  "), Alignment::Center);
    }

    #[test]
    fn test_pad_cell() {
        assert_eq!(pad_cell("A", 5, Alignment::Left), "A    ");
        assert_eq!(pad_cell("A", 5, Alignment::Right), "    A");
        assert_eq!(pad_cell("A", 5, Alignment::Center), "  A  ");
        // Emoji padding
        assert_eq!(pad_cell("✅", 3, Alignment::Left), "✅  "); // 1 char + 2 spaces
    }
}
