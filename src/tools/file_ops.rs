//! File operation tools.

use super::common::{is_text_file, should_ignore};
use grep_regex::RegexMatcher;
use grep_searcher::{Searcher, Sink, SinkMatch};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path not found: {0}")]
    NotFound(String),
    #[error("File too large: {0} bytes (max: {1})")]
    TooLarge(u64, u64),
    #[error("Grep error: {0}")]
    GrepError(String),
    #[error("File too large: ~{estimated_tokens} tokens ({total_lines} lines). Read in chunks using start_line and num_lines parameters. Suggested: start_line=1, num_lines={suggested_chunk_size}")]
    TokenLimitExceeded {
        estimated_tokens: usize,
        total_lines: usize,
        suggested_chunk_size: usize,
    },
}

/// File entry for directory listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub depth: usize,
}

/// Result of listing files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesResult {
    pub entries: Vec<FileEntry>,
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_size: u64,
    pub truncated: bool,
    pub max_entries: usize,
}

const LIST_FILES_DEFAULT_MAX_ENTRIES: usize = 2_000;
const LIST_FILES_HARD_MAX_ENTRIES: usize = 10_000;
const LIST_FILES_DEFAULT_MAX_DEPTH: usize = 10;
const LIST_FILES_HARD_MAX_DEPTH: usize = 50;

/// Context for recursive file listing.
struct ListFilesContext<'a> {
    base: &'a Path,
    entries: &'a mut Vec<FileEntry>,
    recursive: bool,
    max_depth: usize,
    max_entries: usize,
    truncated: &'a mut bool,
}

/// Maximum tokens allowed in a single file read to protect context window
const READ_FILE_MAX_TOKENS: usize = 10_000;
/// Approximate characters per token (conservative estimate)
const CHARS_PER_TOKEN: usize = 4;

/// List files in a directory.
pub fn list_files(
    directory: &str,
    recursive: bool,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
) -> Result<ListFilesResult, FileError> {
    let path = Path::new(directory);
    if !path.exists() {
        return Err(FileError::NotFound(directory.to_string()));
    }

    let max_entries = max_entries
        .unwrap_or(LIST_FILES_DEFAULT_MAX_ENTRIES)
        .clamp(1, LIST_FILES_HARD_MAX_ENTRIES);

    let mut entries = Vec::new();
    let mut total_files = 0;
    let mut total_dirs = 0;
    let mut total_size = 0u64;
    let mut truncated = false;

    let max_depth = max_depth
        .unwrap_or(LIST_FILES_DEFAULT_MAX_DEPTH)
        .min(LIST_FILES_HARD_MAX_DEPTH);

    let mut ctx = ListFilesContext {
        base: path,
        entries: &mut entries,
        recursive,
        max_depth,
        max_entries,
        truncated: &mut truncated,
    };
    list_files_recursive(&mut ctx, path, 0)?;

    for entry in &entries {
        if entry.is_dir {
            total_dirs += 1;
        } else {
            total_files += 1;
            total_size += entry.size;
        }
    }

    Ok(ListFilesResult {
        entries,
        total_files,
        total_dirs,
        total_size,
        truncated,
        max_entries,
    })
}

fn list_files_recursive(
    ctx: &mut ListFilesContext,
    dir: &Path,
    depth: usize,
) -> Result<(), FileError> {
    if depth > ctx.max_depth {
        return Ok(());
    }

    if ctx.entries.len() >= ctx.max_entries {
        *ctx.truncated = true;
        return Ok(());
    }

    let read_dir = fs::read_dir(dir);
    let mut dir_entries: Vec<_> = match read_dir {
        Ok(read_dir) => read_dir.filter_map(|e| e.ok()).collect(),
        Err(e) if depth == 0 => return Err(e.into()),
        Err(_) => return Ok(()),
    };

    dir_entries.sort_by_key(|a| a.file_name());

    for entry in dir_entries {
        if ctx.entries.len() >= ctx.max_entries {
            *ctx.truncated = true;
            break;
        }

        let path = entry.path();
        let relative = path.strip_prefix(ctx.base).unwrap_or(&path);
        let relative_str = relative.to_string_lossy().to_string();

        if should_ignore(&relative_str) {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let is_dir = file_type.is_dir();
        let name = entry.file_name().to_string_lossy().to_string();

        ctx.entries.push(FileEntry {
            path: relative_str.clone(),
            name,
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            depth,
        });

        if is_dir && ctx.recursive {
            list_files_recursive(ctx, &path, depth + 1)?;
        }
    }

    Ok(())
}

/// Read file contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub content: String,
    pub path: String,
    pub size: u64,
    pub lines: usize,
    pub estimated_tokens: usize,
}

pub fn read_file(
    path: &str,
    start_line: Option<usize>,
    num_lines: Option<usize>,
    max_size: Option<u64>,
) -> Result<ReadFileResult, FileError> {
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(FileError::NotFound(path.to_string()));
    }

    let metadata = fs::metadata(file_path)?;
    let max = max_size.unwrap_or(10 * 1024 * 1024); // 10MB default

    if metadata.len() > max {
        return Err(FileError::TooLarge(metadata.len(), max));
    }

    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Token-based protection (only for full file reads)
    if start_line.is_none() {
        let estimated_tokens = content.len() / CHARS_PER_TOKEN;
        if estimated_tokens > READ_FILE_MAX_TOKENS {
            let suggested_chunk = total_lines.div_ceil(4).min(500);
            return Err(FileError::TokenLimitExceeded {
                estimated_tokens,
                total_lines,
                suggested_chunk_size: suggested_chunk,
            });
        }
    }

    let content = if let Some(start) = start_line {
        let start_idx = start.saturating_sub(1); // 1-based to 0-based
        let end_idx = num_lines
            .map(|n| (start_idx + n).min(total_lines))
            .unwrap_or(total_lines);

        lines[start_idx..end_idx].join("\n")
    } else {
        content
    };

    let estimated_tokens = content.len() / CHARS_PER_TOKEN;

    Ok(ReadFileResult {
        content,
        path: path.to_string(),
        size: metadata.len(),
        lines: total_lines,
        estimated_tokens,
    })
}

/// Write content to a file.
pub fn write_file(path: &str, content: &str, create_dirs: bool) -> Result<(), FileError> {
    let file_path = Path::new(path);

    if create_dirs {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::write(file_path, content)?;
    Ok(())
}

/// Grep match result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatch {
    pub path: String,
    pub line_number: usize,
    pub content: String,
}

/// Grep results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepResult {
    pub matches: Vec<GrepMatch>,
    pub total_matches: usize,
}

/// Safety caps to prevent huge context blowups.
const GREP_HARD_MAX_MATCHES: usize = 200;
const GREP_DEFAULT_MAX_MATCHES: usize = 100;
const GREP_MAX_MATCHES_PER_FILE: usize = 10;
const GREP_MAX_LINE_LENGTH: usize = 512;
const GREP_MAX_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;
const GREP_MAX_DEPTH: usize = 10;

fn truncate_line(s: &str, max_chars: usize) -> (String, usize) {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        (s.to_string(), 0)
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        (truncated, char_count - max_chars)
    }
}

struct MatchCollector {
    matches: Vec<GrepMatch>,
    file_path: String,
    max_matches: usize,
    max_per_file: usize,
    file_match_count: usize,
}

impl Sink for MatchCollector {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.matches.len() >= self.max_matches {
            return Ok(false);
        }

        if self.file_match_count >= self.max_per_file {
            return Ok(false);
        }

        let raw = String::from_utf8_lossy(mat.bytes());
        let raw = raw.trim_end_matches(&['\r', '\n'][..]);
        let (mut line_content, truncated_chars) = truncate_line(raw, GREP_MAX_LINE_LENGTH);

        if truncated_chars > 0 {
            line_content.push_str(&format!(" [...{} more chars]", truncated_chars));
        }

        let line_number = mat.line_number().unwrap_or(0);
        let line_number = usize::try_from(line_number).unwrap_or(0);

        self.matches.push(GrepMatch {
            path: self.file_path.clone(),
            line_number,
            content: line_content,
        });

        self.file_match_count += 1;
        Ok(true)
    }
}

/// Search for a pattern in files.
pub fn grep(
    pattern: &str,
    directory: &str,
    max_results: Option<usize>,
) -> Result<GrepResult, FileError> {
    let requested = max_results.unwrap_or(GREP_DEFAULT_MAX_MATCHES);
    let max_matches = requested.min(GREP_HARD_MAX_MATCHES);

    if pattern.is_empty() {
        return Err(FileError::GrepError(
            "pattern must not be empty".to_string(),
        ));
    }

    let path = PathBuf::from(directory);
    let abs_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(&path)
    };

    if !abs_path.exists() {
        return Err(FileError::NotFound(directory.to_string()));
    }

    if !abs_path.is_dir() {
        return Err(FileError::GrepError(format!(
            "Not a directory: {}",
            directory
        )));
    }

    // Support a tiny subset of common ripgrep-ish flags embedded in the pattern.
    let (pattern, case_insensitive) = if let Some(rest) = pattern.strip_prefix("--ignore-case ") {
        (rest, true)
    } else if let Some(rest) = pattern.strip_prefix("-i ") {
        (rest, true)
    } else {
        (pattern, false)
    };

    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(FileError::GrepError(
            "pattern must not be empty".to_string(),
        ));
    }

    let regex_pattern = if case_insensitive {
        format!("(?i){}", pattern)
    } else {
        pattern.to_string()
    };

    // Try regex, then fall back to literal (same behavior as ticca-desktop).
    let matcher = RegexMatcher::new_line_matcher(&regex_pattern)
        .or_else(|_| {
            let escaped = regex::escape(pattern);
            let escaped_pattern = if case_insensitive {
                format!("(?i){}", escaped)
            } else {
                escaped
            };
            RegexMatcher::new_line_matcher(&escaped_pattern)
        })
        .map_err(|e| FileError::GrepError(format!("Invalid search pattern: {}", e)))?;

    let walker = WalkBuilder::new(&abs_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .max_depth(Some(GREP_MAX_DEPTH))
        .max_filesize(Some(GREP_MAX_FILE_SIZE_BYTES))
        .filter_entry(|e| !should_ignore(&e.path().to_string_lossy()))
        .build();

    let mut searcher = Searcher::new();
    let mut matches: Vec<GrepMatch> = Vec::new();

    for entry in walker.flatten() {
        if matches.len() >= max_matches {
            break;
        }

        let entry_path = entry.path();

        let is_file = entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }

        let entry_path_str = entry_path.to_string_lossy().to_string();
        if should_ignore(&entry_path_str) || !is_text_file(&entry_path_str) {
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(&abs_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry_path_str.clone());

        let mut collector = MatchCollector {
            matches: Vec::new(),
            file_path: relative_path,
            max_matches: max_matches - matches.len(),
            max_per_file: GREP_MAX_MATCHES_PER_FILE,
            file_match_count: 0,
        };

        if searcher
            .search_path(&matcher, entry_path, &mut collector)
            .is_ok()
        {
            matches.extend(collector.matches);
        }
    }

    Ok(GrepResult {
        total_matches: matches.len(),
        matches,
    })
}

/// Apply a unified diff to a file.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grep_finds_matches_and_line_numbers() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("a.txt");
        fs::write(&file_path, "foo\nbar\nfoo\n").expect("write failed");

        let result = grep("foo", dir.path().to_str().unwrap(), None).expect("grep failed");
        assert_eq!(result.total_matches, 2);

        assert!(result.matches[0].path.ends_with("a.txt"));
        assert_eq!(result.matches[0].line_number, 1);
        assert_eq!(result.matches[0].content, "foo");

        assert!(result.matches[1].path.ends_with("a.txt"));
        assert_eq!(result.matches[1].line_number, 3);
        assert_eq!(result.matches[1].content, "foo");
    }

    #[test]
    fn grep_respects_max_results() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("a.txt");
        fs::write(&file_path, "foo\nfoo\nfoo\n").expect("write failed");

        let result = grep("foo", dir.path().to_str().unwrap(), Some(1)).expect("grep failed");
        assert_eq!(result.total_matches, 1);
    }

    #[test]
    fn grep_ignores_common_artifacts() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let good = dir.path().join("a.txt");
        fs::write(&good, "foo\n").expect("write failed");

        let ignored_dir = dir.path().join("target");
        fs::create_dir_all(&ignored_dir).expect("mkdir failed");
        fs::write(ignored_dir.join("b.txt"), "foo\n").expect("write failed");

        let result = grep("foo", dir.path().to_str().unwrap(), None).expect("grep failed");
        assert_eq!(result.total_matches, 1);
        assert!(result.matches[0].path.ends_with("a.txt"));
    }

    #[test]
    fn grep_falls_back_to_literal_on_invalid_regex() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("a.txt");
        fs::write(&file_path, "(paren)\n").expect("write failed");

        let result = grep("(", dir.path().to_str().unwrap(), None).expect("grep failed");
        assert_eq!(result.total_matches, 1);
        assert!(result.matches[0].content.contains("(paren)"));
    }

    #[test]
    fn list_files_respects_max_entries() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        fs::write(dir.path().join("a.txt"), "a").expect("write failed");
        fs::write(dir.path().join("b.txt"), "b").expect("write failed");

        let result = list_files(dir.path().to_str().unwrap(), false, Some(1), Some(1))
            .expect("list_files failed");

        assert_eq!(result.entries.len(), 1);
        assert!(result.truncated);
        assert_eq!(result.max_entries, 1);
    }

    #[test]
    fn read_file_rejects_large_token_count() {
        // Create a temp file with content exceeding token limit
        // 10,000 tokens * 4 chars = 40,000 chars minimum to trigger
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("large.txt");

        // Create content that will exceed 10k tokens (~50k chars)
        let large_content = "x".repeat(50_000);
        fs::write(&file_path, &large_content).expect("write failed");

        let result = read_file(file_path.to_str().unwrap(), None, None, None);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, FileError::TokenLimitExceeded { .. }));

        // Verify the error message includes helpful guidance
        let err_msg = err.to_string();
        assert!(err_msg.contains("tokens"));
        assert!(err_msg.contains("start_line"));
        assert!(err_msg.contains("num_lines"));
    }

    #[test]
    fn read_file_allows_chunked_reads_of_large_files() {
        // Same large file, but reading with start_line should work
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("large.txt");

        // Create a file with 1000 lines
        let lines: Vec<String> = (1..=1000)
            .map(|i| format!("Line {} content here", i))
            .collect();
        let content = lines.join("\n");
        fs::write(&file_path, &content).expect("write failed");

        // Reading first 100 lines should work even though full file might be large
        let result = read_file(file_path.to_str().unwrap(), Some(1), Some(100), None);

        assert!(result.is_ok());
        let read_result = result.unwrap();
        assert!(read_result.content.contains("Line 1"));
        assert!(read_result.content.contains("Line 100"));
        assert!(!read_result.content.contains("Line 101"));
    }

    #[test]
    fn read_file_small_file_includes_token_estimate() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let file_path = dir.path().join("small.txt");

        // 100 chars = ~25 tokens
        let content = "a".repeat(100);
        fs::write(&file_path, &content).expect("write failed");

        let result = read_file(file_path.to_str().unwrap(), None, None, None);

        assert!(result.is_ok());
        let read_result = result.unwrap();
        assert_eq!(read_result.estimated_tokens, 25); // 100 / 4
    }

    // =========================================================================
    // Additional list_files Edge Cases (from PR)
    // =========================================================================

    #[test]
    fn test_list_files_tracks_depth_correctly() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        fs::create_dir(dir.path().join("sub")).expect("mkdir failed");
        fs::write(dir.path().join("root.txt"), "root").expect("write failed");
        fs::write(dir.path().join("sub").join("child.txt"), "child").expect("write failed");

        let result = list_files(dir.path().to_str().unwrap(), true, None, None).unwrap();

        let root_entry = result
            .entries
            .iter()
            .find(|e| e.name == "root.txt")
            .unwrap();
        let child_entry = result
            .entries
            .iter()
            .find(|e| e.name == "child.txt")
            .unwrap();

        assert_eq!(root_entry.depth, 0);
        assert_eq!(child_entry.depth, 1);
    }
}
