//! Common utilities for tools.

/// Directory patterns to ignore.
pub static IGNORE_PATTERNS: &[&str] = &[
    // Version control
    ".git",
    ".svn",
    ".hg",
    // Dependencies
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    // Build outputs
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    // IDE/Editor
    ".idea",
    ".vscode",
    // Cache
    ".cache",
    ".pytest_cache",
    ".mypy_cache",
    // Package managers
    ".npm",
    ".yarn",
    ".pnpm-store",
];

/// Check if a path should be ignored.
pub fn should_ignore(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    for pattern in IGNORE_PATTERNS {
        if path_lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Get file extension.
pub fn get_extension(path: &str) -> Option<&str> {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
}

/// Check if path is likely a text file.
pub fn is_text_file(path: &str) -> bool {
    let text_extensions = [
        "txt",
        "md",
        "rs",
        "py",
        "js",
        "ts",
        "tsx",
        "jsx",
        "json",
        "yaml",
        "yml",
        "toml",
        "ini",
        "cfg",
        "html",
        "css",
        "scss",
        "less",
        "sh",
        "bash",
        "zsh",
        "fish",
        "c",
        "h",
        "cpp",
        "hpp",
        "cc",
        "cxx",
        "go",
        "java",
        "kt",
        "swift",
        "rb",
        "php",
        "sql",
        "graphql",
        "proto",
        "xml",
        "svg",
        "dockerfile",
        "makefile",
    ];

    if let Some(ext) = get_extension(path) {
        text_extensions.contains(&ext.to_lowercase().as_str())
    } else {
        // No extension - check filename
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let text_files = [
            "Makefile",
            "Dockerfile",
            "Rakefile",
            "Gemfile",
            ".gitignore",
            ".env",
        ];
        text_files.contains(&name)
    }
}

// ============================================================================
// Token estimation utilities
// ============================================================================

/// Approximate characters per token (conservative estimate for code/text mix)
pub const CHARS_PER_TOKEN: usize = 4;

/// Default maximum tokens for tool output
pub const DEFAULT_MAX_OUTPUT_TOKENS: usize = 10_000;

/// Estimate tokens from text content.
/// Uses ~4 chars/token approximation which is conservative for most content.
#[inline]
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / CHARS_PER_TOKEN
}

/// Truncate text to fit within a token limit, with a message indicating truncation.
///
/// Returns the (possibly truncated) text and a boolean indicating if truncation occurred.
/// Attempts to truncate at a newline boundary for cleaner output.
pub fn truncate_to_token_limit(text: String, max_tokens: usize) -> (String, bool) {
    let estimated = estimate_tokens(&text);
    if estimated <= max_tokens {
        return (text, false);
    }

    let max_chars = max_tokens * CHARS_PER_TOKEN;
    let mut truncated: String = text.chars().take(max_chars).collect();

    // Try to find a clean break point
    if let Some(last_newline) = truncated.rfind('\n') {
        truncated.truncate(last_newline);
    }

    truncated.push_str(&format!(
        "\n\n[OUTPUT TRUNCATED: ~{} tokens exceeded {} token limit]",
        estimated, max_tokens
    ));

    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("12345678"), 2);
        assert_eq!(estimate_tokens(&"x".repeat(40_000)), 10_000);
    }

    #[test]
    fn test_truncate_to_token_limit_no_truncation() {
        let small = "hello world".to_string();
        let (result, truncated) = truncate_to_token_limit(small.clone(), 1000);
        assert!(!truncated);
        assert_eq!(result, small);
    }

    #[test]
    fn test_truncate_to_token_limit_with_truncation() {
        let large = "x".repeat(50_000); // ~12,500 tokens
        let (result, truncated) = truncate_to_token_limit(large, 1000);
        assert!(truncated);
        assert!(result.len() < 50_000);
        assert!(result.contains("OUTPUT TRUNCATED"));
        assert!(result.contains("token limit"));
    }

    #[test]
    fn test_truncate_at_newline_boundary() {
        let content = "line1\nline2\nline3\nline4".to_string();
        // Set limit that would cut in middle of "line3"
        let (result, truncated) = truncate_to_token_limit(content, 4); // 16 chars max
        assert!(truncated);
        // Should cut at newline before "line3" or "line4"
        assert!(result.contains("line1"));
        assert!(!result.contains("line4") || result.contains("TRUNCATED"));
    }
}
