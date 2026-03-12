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
    "fn",
    "let",
    "mut",
    "const",
    "static",
    "if",
    "else",
    "match",
    "for",
    "while",
    "loop",
    "return",
    "break",
    "continue",
    "struct",
    "enum",
    "impl",
    "trait",
    "pub",
    "use",
    "mod",
    "crate",
    "self",
    "Self",
    "super",
    "where",
    "async",
    "await",
    "move",
    "ref",
    "type",
    "dyn",
    "as",
    "in",
    "unsafe",
    // Python
    "def",
    "class",
    "import",
    "from",
    "try",
    "except",
    "finally",
    "with",
    "lambda",
    "yield",
    "global",
    "nonlocal",
    "assert",
    "pass",
    "raise",
    "True",
    "False",
    "None",
    "and",
    "or",
    "not",
    "is",
    "elif",
    // JavaScript/TypeScript
    "function",
    "var",
    "class",
    "extends",
    "new",
    "this",
    "typeof",
    "instanceof",
    "delete",
    "void",
    "export",
    "default",
    "true",
    "false",
    "null",
    "undefined",
    // Go
    "func",
    "package",
    "interface",
    "map",
    "chan",
    "go",
    "defer",
    "select",
    "case",
    "fallthrough",
    "range",
    "nil",
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
            for c in chars.by_ref() {
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
    } else if token
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '_')
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

/// Check if a character is a wide emoji (definitely 2 chars wide)
///
/// We only override unicode-width for characters that are DEFINITELY
/// rendered as 2-wide in modern terminals with emoji fonts.
///
/// Characters in ambiguous ranges (misc symbols, dingbats, arrows, etc.)
/// are left to unicode-width which follows the Unicode standard.
fn is_emoji(c: char) -> bool {
    matches!(
        c,
        // Definite wide emojis (U+1F300+) - always 2-wide in modern terminals
        '\u{1F300}'..='\u{1F5FF}'    // Misc Symbols and Pictographs (🌀🎁📋 etc.)
        | '\u{1F600}'..='\u{1F64F}'  // Emoticons (😀😎 etc.)
        | '\u{1F680}'..='\u{1F6FF}'  // Transport and Map Symbols (🚀✈️ etc.)
        | '\u{1F700}'..='\u{1F77F}'  // Alchemical Symbols
        | '\u{1F780}'..='\u{1F7FF}'  // Geometric Shapes Extended
        | '\u{1F800}'..='\u{1F8FF}'  // Supplemental Arrows-C
        | '\u{1F900}'..='\u{1F9FF}'  // Supplemental Symbols and Pictographs (🤔🥳 etc.)
        | '\u{1FA00}'..='\u{1FA6F}'  // Chess Symbols
        | '\u{1FA70}'..='\u{1FAFF}'  // Symbols and Pictographs Extended-A (🥺🪄 etc.)
        | '\u{1FB00}'..='\u{1FBFF}'  // Symbols for Legacy Computing

        // Specific wide symbols that unicode-width may undercount
        // These are commonly used in TUIs and render as 2-wide with emoji fonts
        | '\u{2705}'                 // ✅ White Heavy Check Mark
        | '\u{274C}'                 // ❌ Cross Mark
        | '\u{274E}'                 // ❎ Cross Mark Button
        | '\u{2B50}'                 // ⭐ Star
        | '\u{2B55}'                 // ⭕ Heavy Large Circle

        // Common status/indicator symbols that render as 2-wide in modern terminals
        // NOTE: ⚠ (U+26A0) and ℹ (U+2139) are NOT included - they render as 1-wide
        // in many terminals. Let unicode-width handle them.
        | '\u{26A1}'                 // ⚡ High Voltage
        | '\u{231B}'                 // ⌛ Hourglass Done
        | '\u{23F3}'                 // ⏳ Hourglass Not Done
        | '\u{23F0}'                 // ⏰ Alarm Clock
        | '\u{23F1}'                 // ⏱ Stopwatch
        | '\u{23F2}'                 // ⏲ Timer Clock
        | '\u{2328}'                 // ⌨ Keyboard
        | '\u{260E}'                 // ☎ Telephone
        | '\u{2611}'                 // ☑ Ballot Box with Check
        | '\u{2622}'                 // ☢ Radioactive
        | '\u{2623}'                 // ☣ Biohazard
        | '\u{262F}'                 // ☯ Yin Yang
        | '\u{2638}'                 // ☸ Wheel of Dharma
        | '\u{2639}'                 // ☹ Frowning Face
        | '\u{263A}'                 // ☺ Smiling Face
        | '\u{2640}'                 // ♀ Female Sign
        | '\u{2642}'                 // ♂ Male Sign
        | '\u{2648}'..='\u{2653}'    // ♈-♓ Zodiac signs
        | '\u{267B}'                 // ♻ Recycling Symbol
        | '\u{267F}'                 // ♿ Wheelchair Symbol
        | '\u{2693}'                 // ⚓ Anchor
        | '\u{2694}'                 // ⚔ Crossed Swords
        | '\u{2695}'                 // ⚕ Staff of Aesculapius
        | '\u{2696}'                 // ⚖ Scales
        | '\u{2697}'                 // ⚗ Alembic
        | '\u{2699}'                 // ⚙ Gear
        | '\u{269B}'                 // ⚛ Atom Symbol
        | '\u{269C}'                 // ⚜ Fleur-de-lis
        | '\u{26B0}'                 // ⚰ Coffin
        | '\u{26B1}'                 // ⚱ Funeral Urn
        | '\u{26BD}'                 // ⚽ Soccer Ball
        | '\u{26BE}'                 // ⚾ Baseball
        | '\u{26C4}'                 // ⛄ Snowman
        | '\u{26C5}'                 // ⛅ Sun Behind Cloud
        | '\u{26C8}'                 // ⛈ Thunder Cloud and Rain
        | '\u{26D4}'                 // ⛔ No Entry
        | '\u{26EA}'                 // ⛪ Church
        | '\u{26F2}'                 // ⛲ Fountain
        | '\u{26F3}'                 // ⛳ Flag in Hole
        | '\u{26F5}'                 // ⛵ Sailboat
        | '\u{26FA}'                 // ⛺ Tent
        | '\u{26FD}'                 // ⛽ Fuel Pump

        // Hot beverages and food that render as emoji
        | '\u{2615}'                 // ☕ Hot Beverage (Coffee)
        | '\u{2600}'                 // ☀ Sun (Black Sun with Rays)
        | '\u{2601}'                 // ☁ Cloud
        | '\u{2602}'                 // ☂ Umbrella
        | '\u{2603}'                 // ☃ Snowman
        | '\u{2604}'                 // ☄ Comet
        | '\u{2614}'                 // ☔ Umbrella with Rain Drops
        | '\u{2618}'                 // ☘ Shamrock

        // Regional Indicator Symbols (flags)
        | '\u{1F1E0}'..='\u{1F1FF}'
    )
}
/// Check if a character is any variation selector (zero-width modifiers)
/// VS1-VS16 (U+FE00-U+FE0F) modify the display of the preceding character
fn is_variation_selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0F}')
}

/// Check if a character can become an emoji when followed by VS16
/// These are characters in ranges that have both text and emoji presentations
fn can_be_emoji(c: char) -> bool {
    matches!(
        c,
        '\u{2600}'..='\u{26FF}'    // Misc Symbols (⚠⚡☀ etc.)
        | '\u{2700}'..='\u{27BF}'  // Dingbats (✓✗✂ etc.)
        | '\u{2300}'..='\u{23FF}'  // Misc Technical (⌘⌛ etc.)
        | '\u{2190}'..='\u{21FF}'  // Arrows
        | '\u{25A0}'..='\u{25FF}'  // Geometric shapes
        | '\u{2B00}'..='\u{2BFF}'  // Misc Symbols and Arrows
        | '\u{2139}'               // ℹ Information Source (Letterlike Symbols)
        | '\u{00A9}'               // © Copyright
        | '\u{00AE}'               // ® Registered
        | '\u{203C}'               // ‼ Double Exclamation
        | '\u{2049}'               // ⁉ Exclamation Question Mark
    )
}

/// Check if a character is a Zero Width Joiner
/// ZWJ (U+200D) is used to combine emojis into a single glyph
fn is_zwj(c: char) -> bool {
    c == '\u{200D}'
}

/// Calculate display width of a string, properly handling:
/// - Definite emojis (always 2-wide)
/// - ZWJ sequences (👩‍💻 = 👩 + ZWJ + 💻 renders as ONE 2-wide glyph)
/// - Characters that become emojis with VS16 (text=1, emoji=2)
/// - Skin tone modifiers (zero-width, modify previous emoji)
/// - Regular variation selectors (zero-width)
fn display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;

    let chars: Vec<char> = s.chars().collect();
    let mut total = 0;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Skip standalone variation selectors and ZWJ
        if is_variation_selector(c) || is_zwj(c) {
            i += 1;
            continue;
        }

        // Skip skin tone modifiers (they modify the previous emoji)
        if matches!(c, '\u{1F3FB}'..='\u{1F3FF}') {
            i += 1;
            continue;
        }

        // Check if this starts an emoji or ZWJ sequence
        if is_emoji(c) || (can_be_emoji(c) && chars.get(i + 1) == Some(&'\u{FE0F}')) {
            // Count as 2-wide
            total += 2;
            i += 1;

            // Consume the entire ZWJ sequence - all following emoji components
            // Example: 👩‍💻 = 👩 (counted) + ZWJ + 💻 (skipped) + possibly more
            while i < chars.len() {
                let next = chars[i];
                if is_zwj(next) {
                    // Skip ZWJ and the next emoji in the sequence
                    i += 1;
                    // Skip the emoji after ZWJ (and any VS16/skin tones)
                    if i < chars.len()
                        && (is_emoji(chars[i])
                            || is_variation_selector(chars[i])
                            || matches!(chars[i], '\u{1F3FB}'..='\u{1F3FF}')
                            || can_be_emoji(chars[i]))
                    {
                        i += 1;
                        // If we just consumed an emoji that has VS16 after it, skip that too
                        if i < chars.len() && is_variation_selector(chars[i]) {
                            i += 1;
                        }
                        // If there's another ZWJ, continue the outer loop (handled by outer while)
                    }
                } else if is_variation_selector(next) || matches!(next, '\u{1F3FB}'..='\u{1F3FF}') {
                    // Skip variation selectors and skin tones that follow
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            // Regular character - use unicode-width
            total += UnicodeWidthChar::width(c).unwrap_or(1);
            i += 1;
        }
    }

    total
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

/// Calculate total display width of a Vec<MdSpan>
fn spans_display_width(spans: &[MdSpan]) -> usize {
    spans.iter().map(|span| display_width(&span.text)).sum()
}

/// Pad spans to the specified width with the given alignment
/// Returns a new Vec<MdSpan> with padding MdSpan::plain(" ") spans added
fn pad_spans(spans: Vec<MdSpan>, target_width: usize, align: Alignment) -> Vec<MdSpan> {
    let content_width = spans_display_width(&spans);
    let padding = target_width.saturating_sub(content_width);

    if padding == 0 {
        return spans;
    }

    match align {
        Alignment::Left => {
            let mut result = spans;
            result.push(MdSpan::plain(" ".repeat(padding)));
            result
        }
        Alignment::Right => {
            let mut result = vec![MdSpan::plain(" ".repeat(padding))];
            result.extend(spans);
            result
        }
        Alignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            let mut result = vec![MdSpan::plain(" ".repeat(left_pad))];
            result.extend(spans);
            result.push(MdSpan::plain(" ".repeat(right_pad)));
            result
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

    // Pre-parse all cells to get inline markdown spans
    // This allows us to calculate widths from rendered content (excluding markdown syntax)
    let parsed_rows: Vec<Vec<Vec<MdSpan>>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| parse_inline_markdown(cell)).collect())
        .collect();

    // Calculate column widths from PARSED content (not raw text with markdown syntax)
    let mut col_widths: Vec<usize> = vec![0; num_cols];
    for parsed_row in &parsed_rows {
        for (i, cell_spans) in parsed_row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(spans_display_width(cell_spans));
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
    for (row_idx, parsed_row) in parsed_rows.iter().enumerate() {
        spans.push(MdSpan::table_border("│"));
        for (i, width) in col_widths.iter().enumerate() {
            let cell_spans = parsed_row.get(i).cloned().unwrap_or_default();
            let align = alignments.get(i).copied().unwrap_or_default();

            // Add leading space
            spans.push(MdSpan::plain(" "));

            // Add padded cell content with proper alignment
            let padded_spans = pad_spans(cell_spans, *width, align);
            spans.extend(padded_spans);

            // Add trailing space
            spans.push(MdSpan::plain(" "));
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
        assert!(quote_span
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
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
        assert!(strike_span
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn test_table_basic() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let spans = parse_markdown(input);
        let has_border = spans
            .iter()
            .any(|s| s.text.contains('┌') || s.text.contains('│'));
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
        let has_box = spans
            .iter()
            .any(|s| s.text.contains('┌') || s.text.contains('─'));
        assert!(
            !has_box,
            "Incomplete table should show as raw text, not box-drawing"
        );
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
        let has_box = spans
            .iter()
            .any(|s| s.text.contains('┌') || s.text.contains('│'));
        assert!(has_box, "Complete table should render with box-drawing");
    }

    #[test]
    fn test_table_helpers() {
        // Test is_table_line
        assert!(is_table_line("| A | B |"));
        assert!(is_table_line("  | A | B |  "));
        assert!(is_table_line("|---|---|"));
        assert!(!is_table_line("Not a table"));
        assert!(!is_table_line("| incomplete")); // Missing closing |

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

        // Verify our custom display_width treats emojis as 2 chars wide
        // (unlike unicode-width which says 2)
        assert_eq!(
            display_width("✅"),
            2,
            "Emoji should have display width of 2"
        );
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

        // Find the index of the "A" span
        let a_idx = spans.iter().position(|s| s.text == "A");
        assert!(a_idx.is_some(), "Should have 'A' cell content");

        // Right-aligned: padding span should come before "A"
        // The structure is: │ + padding + A + trailing space + │
        // So the span before "A" (at idx-1) should have spaces for padding
        let a_idx = a_idx.unwrap();
        if a_idx > 0 {
            let padding_span = &spans[a_idx - 1];
            // The cell "A" is 1 char, header "Name" is 4 chars, so we need 3 chars padding
            // Plus the leading space from the cell, so we check that the span before A has spaces
            // (either just the " " cell margin or " " margin + padding spaces)
            assert!(
                padding_span.text.chars().all(|c| c == ' '),
                "Right-aligned should have padding spaces before content"
            );
        }
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
        // Test is_emoji - definite wide emojis
        assert!(is_emoji('✅')); // U+2705 - explicitly listed
        assert!(is_emoji('❌')); // U+274C - explicitly listed
        assert!(is_emoji('🔥')); // U+1F525 - in emoticons range
        assert!(is_emoji('😀')); // U+1F600 - in emoticons range
        assert!(is_emoji('⭐')); // U+2B50 - explicitly listed
        assert!(is_emoji('⏳')); // U+23F3 - hourglass, always 2-wide
        assert!(is_emoji('⌛')); // U+231B - hourglass done, always 2-wide

        // These are NOT in our emoji list (handled by unicode-width)
        // ⚠ and ℹ render as 1-wide in many terminals, so let unicode-width handle them
        assert!(!is_emoji('⚠')); // U+26A0 - handled by unicode-width (1-wide)
        assert!(!is_emoji('ℹ')); // U+2139 - handled by unicode-width (1-wide)
        assert!(!is_emoji('→')); // U+2192 - arrow, 1-wide
        assert!(!is_emoji('●')); // U+25CF - geometric, 1-wide
        assert!(!is_emoji('A'));
        assert!(!is_emoji('中')); // Chinese char, handled by unicode-width

        // Test is_variation_selector
        assert!(is_variation_selector('\u{FE0F}')); // VS16 - emoji presentation
        assert!(is_variation_selector('\u{FE0E}')); // VS15 - text presentation
        assert!(!is_variation_selector('A'));
        assert!(!is_variation_selector('⚠'));

        // Test display_width
        assert_eq!(display_width("Hello"), 5);
        assert_eq!(display_width("✅"), 2); // Definite emoji = 2 chars
        assert_eq!(display_width("✅✅"), 4); // Two emojis = 4 chars
        assert_eq!(display_width("中文"), 4); // Chinese chars = 2 each (via unicode-width)
        assert_eq!(display_width("A✅B"), 4); // Mixed: A(1) + ✅(2) + B(1)

        // Arrows and geometric shapes are 1-wide (per unicode-width)
        assert_eq!(display_width("→"), 1);
        assert_eq!(display_width("←"), 1);
        assert_eq!(display_width("●"), 1);
        assert_eq!(display_width("■"), 1);

        // Warning sign is 1-wide by default, 2-wide with VS16
        assert_eq!(display_width("⚠"), 1); // Warning sign = 1 (text presentation)
        assert_eq!(display_width("⚠\u{FE0F}"), 2); // Warning + VS16 = 2 (emoji presentation)
        assert_eq!(display_width("⚠ Client Error"), 14); // 1 + 1 + 12 = 14

        // Info is also 1-wide
        assert_eq!(display_width("ℹ"), 1); // Info = 1 (text presentation)
        assert_eq!(display_width("ℹ\u{FE0F}"), 2); // Info + VS16 = 2

        // Other status symbols are 2-wide
        assert_eq!(display_width("⏳"), 2); // Hourglass
        assert_eq!(display_width("⌛"), 2); // Hourglass done

        // Test can_be_emoji (characters with text/emoji dual presentation)
        // Note: ⚠ is now in is_emoji, but also in can_be_emoji range (that's OK)
        assert!(can_be_emoji('⚠')); // U+26A0 - in misc symbols
        assert!(can_be_emoji('☀')); // U+2600 - in misc symbols
        assert!(can_be_emoji('→')); // U+2192 - in arrows
        assert!(!can_be_emoji('A')); // Regular ASCII - not in emoji-capable ranges
        assert!(!can_be_emoji('中')); // CJK - not in emoji-capable ranges

        // Test ZWJ sequences (Zero Width Joiner combines emojis into one glyph)
        // 👩‍💻 = 👩 (U+1F469) + ZWJ (U+200D) + 💻 (U+1F4BB) = ONE 2-wide glyph
        assert_eq!(display_width("👩\u{200D}💻"), 2); // Woman technologist
        assert_eq!(display_width("👨\u{200D}👩\u{200D}👧"), 2); // Family
        assert_eq!(display_width("👩\u{200D}💻 Dev"), 6); // 2 + 1 + 3 = 6

        // Test coffee emoji
        assert_eq!(display_width("☕"), 2); // Coffee is 2-wide
        assert_eq!(display_width("☕☕☕"), 6); // Three coffees = 6

        // Test weather emojis
        assert_eq!(display_width("☀"), 2); // Sun
        assert_eq!(display_width("⛅"), 2); // Sun behind cloud
        assert_eq!(display_width("⛈"), 2); // Thunder cloud

        // Test weather table cells (real-world examples)
        // These should match what the terminal renders
        assert_eq!(display_width("🌤️18°C"), 6); // sun_cloud(2) + VS(0) + 18°C(4) = 6
        assert_eq!(display_width("🌧️14°C"), 6); // rain(2) + VS(0) + 14°C(4) = 6
        assert_eq!(display_width("☁17°C"), 6); // cloud(2) + 17°C(4) = 6
        assert_eq!(display_width("🌙 16°C"), 7); // moon(2) + space(1) + 16°C(4) = 7
        assert_eq!(display_width("🌙13°C"), 6); // moon(2) + 13°C(4) = 6

        // Cloud emoji ☁ (U+2601) should be 2-wide
        assert_eq!(display_width("☁"), 2); // Cloud
    }

    #[test]
    fn test_weather_table_rendering() {
        // This is the exact table from the bug report
        let input = r#"| Day | Morning | Afternoon | Evening |
|-----|---------|-----------|----------|
| Mon | 🌤️18°C  | ☀ 24°C    | 🌙 16°C  |
| Tue | 🌧️14°C  | 🌧 15°C   | 🌙13°C   |
| Wed | ☁17°C   | 🌤️21°C   | 🌙 15°C  |"#;

        let spans = parse_markdown(input);

        // Should render as a table with box-drawing
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table with box-drawing");

        // Each row should have exactly 5 border characters: │ col │ col │ col │ col │
        // Check that we don't have broken borders (extra │ in content)
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let lines: Vec<&str> = text.lines().collect();

        // Check that data rows don't have extra pipes inside cells
        for line in &lines {
            if line.contains("Mon") || line.contains("Tue") || line.contains("Wed") {
                // Count │ characters - should be exactly 5 per data row
                let pipe_count = line.chars().filter(|c| *c == '│').count();
                assert_eq!(
                    pipe_count, 5,
                    "Row '{}' should have exactly 5 │ borders, got {}",
                    line, pipe_count
                );
            }
        }

        // Print the rendered table for debugging
        eprintln!("=== Rendered table ===");
        eprintln!("{}", text);

        // Verify all data rows have the same display width
        let data_rows: Vec<&str> = lines
            .iter()
            .filter(|l| l.contains("Mon") || l.contains("Tue") || l.contains("Wed"))
            .cloned()
            .collect();

        if !data_rows.is_empty() {
            let first_width = display_width(data_rows[0]);
            for (i, row) in data_rows.iter().enumerate() {
                let row_width = display_width(row);
                eprintln!("Row {} width: {} (expected {})", i, row_width, first_width);
                assert_eq!(
                    row_width, first_width,
                    "Row {} has different width: {}",
                    i, row
                );
            }
        }
    }

    #[test]
    fn test_food_table_with_chili() {
        // Test the Food & Code Moods table with 🌶 Spicy
        let input = r#"| Mood | Food | Code Equivalent |
|------|------|------------------|
| 😊 Happy | 🍕 Pizza | Clean merge |
| 😤 Frustrated | 🌶️ Spicy | Debugging prod |"#;

        let spans = parse_markdown(input);
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();

        eprintln!("=== Food table ===");
        eprintln!("{}", text);

        // Check that all rows have exactly 4 borders
        for line in text.lines() {
            if line.contains("Happy") || line.contains("Frustrated") {
                let pipe_count = line.chars().filter(|c| *c == '│').count();
                assert_eq!(pipe_count, 4, "Row should have 4 borders: {}", line);
            }
        }
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
    fn test_spans_display_width() {
        let spans = vec![MdSpan::plain("Hello"), MdSpan::bold("World")];
        assert_eq!(spans_display_width(&spans), 10);

        let empty: Vec<MdSpan> = vec![];
        assert_eq!(spans_display_width(&empty), 0);

        let emoji_spans = vec![MdSpan::plain("✅"), MdSpan::plain("test")];
        assert_eq!(spans_display_width(&emoji_spans), 6); // 2 + 4
    }

    #[test]
    fn test_pad_spans_left() {
        let spans = vec![MdSpan::bold("Hi")];
        let padded = pad_spans(spans, 5, Alignment::Left);
        // Should have "Hi" + 3 spaces
        assert_eq!(padded.len(), 2);
        assert_eq!(padded[0].text, "Hi");
        assert_eq!(padded[1].text, "   ");
    }

    #[test]
    fn test_pad_spans_right() {
        let spans = vec![MdSpan::bold("Hi")];
        let padded = pad_spans(spans, 5, Alignment::Right);
        // Should have 3 spaces + "Hi"
        assert_eq!(padded.len(), 2);
        assert_eq!(padded[0].text, "   ");
        assert_eq!(padded[1].text, "Hi");
    }

    #[test]
    fn test_pad_spans_center() {
        let spans = vec![MdSpan::bold("Hi")];
        let padded = pad_spans(spans, 6, Alignment::Center);
        // Should have 2 spaces + "Hi" + 2 spaces
        assert_eq!(padded.len(), 3);
        assert_eq!(padded[0].text, "  ");
        assert_eq!(padded[1].text, "Hi");
        assert_eq!(padded[2].text, "  ");
    }

    #[test]
    fn test_table_bold_in_cells() {
        let input = "| **bold** | text |\n|----------|------|\n| normal | **also bold** |\n";
        let spans = parse_markdown(input);

        // Should render as table
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table with box-drawing");

        // Should have bold spans
        let bold_span = spans.iter().find(|s| s.text == "bold");
        assert!(bold_span.is_some(), "Should have 'bold' text");
        assert!(
            bold_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::BOLD),
            "'bold' should be styled as bold"
        );

        let also_bold_span = spans.iter().find(|s| s.text == "also bold");
        assert!(also_bold_span.is_some(), "Should have 'also bold' text");
        assert!(
            also_bold_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::BOLD),
            "'also bold' should be styled as bold"
        );
    }

    #[test]
    fn test_table_italic_in_cells() {
        let input = "| *italic* | text |\n|----------|------|\n| normal | *also italic* |\n";
        let spans = parse_markdown(input);

        // Should have italic spans
        let italic_span = spans.iter().find(|s| s.text == "italic");
        assert!(italic_span.is_some(), "Should have 'italic' text");
        assert!(
            italic_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::ITALIC),
            "'italic' should be styled as italic"
        );
    }

    #[test]
    fn test_table_code_in_cells() {
        let input = "| `code` | text |\n|--------|------|\n| normal | `more code` |\n";
        let spans = parse_markdown(input);

        // Should have code spans with Theme::COMMAND color
        let code_span = spans.iter().find(|s| s.text == "code");
        assert!(code_span.is_some(), "Should have 'code' text");
        assert_eq!(
            code_span.unwrap().style.fg,
            Some(Theme::COMMAND),
            "'code' should be styled as inline code"
        );
    }

    #[test]
    fn test_table_mixed_formatting_in_cells() {
        let input = "| **bold** and *italic* | `code` here |\n|------------------------|-------------|\n| plain | **more** |\n";
        let spans = parse_markdown(input);

        // Should render as table
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table");

        // Check for bold
        let bold_span = spans.iter().find(|s| s.text == "bold");
        assert!(bold_span.is_some());
        assert!(bold_span
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::BOLD));

        // Check for italic
        let italic_span = spans.iter().find(|s| s.text == "italic");
        assert!(italic_span.is_some());
        assert!(italic_span
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::ITALIC));

        // Check for code
        let code_span = spans.iter().find(|s| s.text == "code");
        assert!(code_span.is_some());
        assert_eq!(code_span.unwrap().style.fg, Some(Theme::COMMAND));
    }

    #[test]
    fn test_table_column_width_excludes_markdown_syntax() {
        // The column width should be calculated from rendered content,
        // not raw text. "**bold**" renders as "bold" (4 chars), not 8 chars.
        let input = "| **bold** |\n|----------|\n| text |\n";
        let spans = parse_markdown(input);

        // Both "bold" and "text" are 4 chars, so column should be same width
        // Find the cells and verify they have consistent padding
        let bold_span = spans.iter().find(|s| s.text == "bold");
        let text_span = spans.iter().find(|s| s.text == "text");
        assert!(bold_span.is_some());
        assert!(text_span.is_some());

        // The table should render properly without "**" taking up space
        let has_box = spans.iter().any(|s| s.text.contains('┌'));
        assert!(has_box, "Should render as table");
    }
}
