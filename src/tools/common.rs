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
