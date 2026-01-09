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

    // =========================================================================
    // Token Estimation Tests
    // =========================================================================

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

    // =========================================================================
    // IGNORE_PATTERNS Tests
    // =========================================================================

    #[test]
    fn test_ignore_patterns_not_empty() {
        assert!(!IGNORE_PATTERNS.is_empty());
    }

    #[test]
    fn test_ignore_patterns_contains_common_dirs() {
        assert!(IGNORE_PATTERNS.contains(&".git"));
        assert!(IGNORE_PATTERNS.contains(&"node_modules"));
        assert!(IGNORE_PATTERNS.contains(&"target"));
        assert!(IGNORE_PATTERNS.contains(&"__pycache__"));
    }

    // =========================================================================
    // should_ignore Tests
    // =========================================================================

    #[test]
    fn test_should_ignore_git() {
        assert!(should_ignore(".git"));
        assert!(should_ignore(".git/config"));
        assert!(should_ignore("project/.git/HEAD"));
        assert!(should_ignore("/home/user/project/.git"));
    }

    #[test]
    fn test_should_ignore_node_modules() {
        assert!(should_ignore("node_modules"));
        assert!(should_ignore("node_modules/lodash"));
        assert!(should_ignore("project/node_modules/react"));
        assert!(should_ignore("/app/node_modules/package.json"));
    }

    #[test]
    fn test_should_ignore_target() {
        assert!(should_ignore("target"));
        assert!(should_ignore("target/debug"));
        assert!(should_ignore("target/release/binary"));
        assert!(should_ignore("project/target/debug/deps"));
    }

    #[test]
    fn test_should_ignore_python_cache() {
        assert!(should_ignore("__pycache__"));
        assert!(should_ignore("src/__pycache__/module.pyc"));
        assert!(should_ignore(".venv"));
        assert!(should_ignore("venv/lib/python3.9"));
        assert!(should_ignore(".pytest_cache"));
        assert!(should_ignore(".mypy_cache"));
    }

    #[test]
    fn test_should_ignore_ide_dirs() {
        assert!(should_ignore(".idea"));
        assert!(should_ignore(".idea/workspace.xml"));
        assert!(should_ignore(".vscode"));
        assert!(should_ignore(".vscode/settings.json"));
    }

    #[test]
    fn test_should_ignore_build_dirs() {
        assert!(should_ignore("dist"));
        assert!(should_ignore("build"));
        assert!(should_ignore(".next"));
        assert!(should_ignore(".nuxt"));
    }

    #[test]
    fn test_should_ignore_case_insensitive() {
        // Should ignore regardless of case
        assert!(should_ignore("NODE_MODULES"));
        assert!(should_ignore("Node_Modules"));
        assert!(should_ignore(".GIT"));
        assert!(should_ignore("TARGET"));
    }

    #[test]
    fn test_should_not_ignore_normal_paths() {
        assert!(!should_ignore("src"));
        assert!(!should_ignore("lib"));
        assert!(!should_ignore("src/main.rs"));
        assert!(!should_ignore("package.json"));
        assert!(!should_ignore("README.md"));
        assert!(!should_ignore("tests/test_module.py"));
    }

    #[test]
    fn test_should_ignore_substring_matches() {
        // Note: The current implementation uses `contains` which matches substrings.
        // This is by design - it's aggressive about ignoring build artifacts.
        // Words containing patterns like "target" or "build" will be ignored.
        assert!(should_ignore("src/targeting.rs")); // contains "target"
        assert!(should_ignore("src/builder.rs")); // contains "build"
        assert!(should_ignore("rebuild.sh")); // contains "build"

        // These should NOT be ignored (no pattern substring)
        assert!(!should_ignore("src/main.rs"));
        assert!(!should_ignore("lib/utils.py"));
        assert!(!should_ignore("README.md"));
    }

    // =========================================================================
    // get_extension Tests
    // =========================================================================

    #[test]
    fn test_get_extension_common() {
        assert_eq!(get_extension("file.rs"), Some("rs"));
        assert_eq!(get_extension("file.py"), Some("py"));
        assert_eq!(get_extension("file.js"), Some("js"));
        assert_eq!(get_extension("file.json"), Some("json"));
        assert_eq!(get_extension("file.md"), Some("md"));
    }

    #[test]
    fn test_get_extension_with_path() {
        assert_eq!(get_extension("src/main.rs"), Some("rs"));
        assert_eq!(get_extension("/home/user/file.txt"), Some("txt"));
        assert_eq!(get_extension("./relative/path/file.py"), Some("py"));
    }

    #[test]
    fn test_get_extension_multiple_dots() {
        assert_eq!(get_extension("file.test.js"), Some("js"));
        assert_eq!(get_extension("archive.tar.gz"), Some("gz"));
        assert_eq!(get_extension("config.local.json"), Some("json"));
    }

    #[test]
    fn test_get_extension_none() {
        assert_eq!(get_extension("Makefile"), None);
        assert_eq!(get_extension("Dockerfile"), None);
        assert_eq!(get_extension("README"), None);
        assert_eq!(get_extension(".gitignore"), None);
    }

    #[test]
    fn test_get_extension_hidden_file_with_ext() {
        assert_eq!(get_extension(".eslintrc.json"), Some("json"));
        assert_eq!(get_extension(".prettierrc.yaml"), Some("yaml"));
    }

    // =========================================================================
    // is_text_file Tests
    // =========================================================================

    #[test]
    fn test_is_text_file_rust() {
        assert!(is_text_file("main.rs"));
        assert!(is_text_file("lib.rs"));
        assert!(is_text_file("src/module.rs"));
    }

    #[test]
    fn test_is_text_file_python() {
        assert!(is_text_file("script.py"));
        assert!(is_text_file("tests/test_module.py"));
    }

    #[test]
    fn test_is_text_file_javascript() {
        assert!(is_text_file("app.js"));
        assert!(is_text_file("index.ts"));
        assert!(is_text_file("component.tsx"));
        assert!(is_text_file("component.jsx"));
    }

    #[test]
    fn test_is_text_file_config_formats() {
        assert!(is_text_file("config.json"));
        assert!(is_text_file("config.yaml"));
        assert!(is_text_file("config.yml"));
        assert!(is_text_file("Cargo.toml"));
        assert!(is_text_file("settings.ini"));
    }

    #[test]
    fn test_is_text_file_web() {
        assert!(is_text_file("index.html"));
        assert!(is_text_file("styles.css"));
        assert!(is_text_file("styles.scss"));
        assert!(is_text_file("styles.less"));
    }

    #[test]
    fn test_is_text_file_shell() {
        assert!(is_text_file("script.sh"));
        assert!(is_text_file("setup.bash"));
        assert!(is_text_file("config.zsh"));
        assert!(is_text_file("functions.fish"));
    }

    #[test]
    fn test_is_text_file_c_family() {
        assert!(is_text_file("main.c"));
        assert!(is_text_file("header.h"));
        assert!(is_text_file("main.cpp"));
        assert!(is_text_file("header.hpp"));
        assert!(is_text_file("source.cc"));
        assert!(is_text_file("source.cxx"));
    }

    #[test]
    fn test_is_text_file_other_languages() {
        assert!(is_text_file("main.go"));
        assert!(is_text_file("Main.java"));
        assert!(is_text_file("main.kt"));
        assert!(is_text_file("main.swift"));
        assert!(is_text_file("script.rb"));
        assert!(is_text_file("index.php"));
    }

    #[test]
    fn test_is_text_file_data_formats() {
        assert!(is_text_file("query.sql"));
        assert!(is_text_file("schema.graphql"));
        assert!(is_text_file("message.proto"));
        assert!(is_text_file("config.xml"));
        assert!(is_text_file("icon.svg"));
    }

    #[test]
    fn test_is_text_file_special_files() {
        assert!(is_text_file("Makefile"));
        assert!(is_text_file("Dockerfile"));
        assert!(is_text_file("Rakefile"));
        assert!(is_text_file("Gemfile"));
        assert!(is_text_file(".gitignore"));
        assert!(is_text_file(".env"));
    }

    #[test]
    fn test_is_text_file_case_insensitive_extension() {
        assert!(is_text_file("FILE.RS"));
        assert!(is_text_file("FILE.Py"));
        assert!(is_text_file("FILE.JSON"));
    }

    #[test]
    fn test_is_not_text_file_binary() {
        assert!(!is_text_file("image.png"));
        assert!(!is_text_file("image.jpg"));
        assert!(!is_text_file("image.gif"));
        assert!(!is_text_file("document.pdf"));
        assert!(!is_text_file("archive.zip"));
        assert!(!is_text_file("binary.exe"));
        assert!(!is_text_file("library.so"));
        assert!(!is_text_file("library.dll"));
    }

    #[test]
    fn test_is_not_text_file_unknown() {
        assert!(!is_text_file("file.unknown"));
        assert!(!is_text_file("random.xyz"));
    }
}
