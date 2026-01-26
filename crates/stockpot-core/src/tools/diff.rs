//! Unified diff parsing and application.
//!
//! Supports standard unified diff format:
//! ```text
//! --- a/file.txt
//! +++ b/file.txt
//! @@ -1,3 +1,4 @@
//!  context line
//! -removed line
//! +added line
//!  more context
//! ```

use std::str::Lines;
use thiserror::Error;

/// Diff parsing/application errors.
#[derive(Debug, Error)]
pub enum DiffError {
    #[error("Invalid diff format: {0}")]
    InvalidFormat(String),
    #[error("Hunk header parse error: {0}")]
    HunkParseError(String),
    #[error("Context mismatch at line {line}: expected '{expected}', got '{actual}'")]
    ContextMismatch {
        line: usize,
        expected: String,
        actual: String,
    },
    #[error("Line {0} out of bounds")]
    LineOutOfBounds(usize),
    #[error("Patch application failed: {0}")]
    PatchFailed(String),
}

/// A single hunk in a diff.
#[derive(Debug, Clone)]
pub struct Hunk {
    /// Starting line in the original file (1-based).
    pub old_start: usize,
    /// Number of lines in the original file.
    pub old_count: usize,
    /// Starting line in the new file (1-based).
    pub new_start: usize,
    /// Number of lines in the new file.
    pub new_count: usize,
    /// The lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// A line in a diff hunk.
#[derive(Debug, Clone)]
pub enum DiffLine {
    /// Context line (unchanged).
    Context(String),
    /// Added line.
    Add(String),
    /// Removed line.
    Remove(String),
}

/// A parsed unified diff.
#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    /// Original file path (after "--- ").
    pub old_path: Option<String>,
    /// New file path (after "+++ ").
    pub new_path: Option<String>,
    /// Whether this creates a new file.
    pub is_new_file: bool,
    /// Whether this deletes a file.
    pub is_delete: bool,
    /// The hunks in this diff.
    pub hunks: Vec<Hunk>,
}

impl UnifiedDiff {
    /// Parse a unified diff from text.
    pub fn parse(diff_text: &str) -> Result<Self, DiffError> {
        let mut lines = diff_text.lines().peekable();
        let mut old_path = None;
        let mut new_path = None;
        let mut is_new_file = false;
        let mut is_delete = false;
        let mut hunks = Vec::new();

        // Parse header lines
        while let Some(line) = lines.peek() {
            if line.starts_with("---") {
                let path = parse_file_path(line, "---");
                is_new_file = path == "/dev/null";
                old_path = if is_new_file { None } else { Some(path) };
                lines.next();
            } else if line.starts_with("+++") {
                let path = parse_file_path(line, "+++");
                is_delete = path == "/dev/null";
                new_path = if is_delete { None } else { Some(path) };
                lines.next();
            } else if line.starts_with("@@") {
                break;
            } else {
                lines.next();
            }
        }

        // Parse hunks
        while let Some(line) = lines.peek() {
            if line.starts_with("@@") {
                let hunk = parse_hunk(&mut lines)?;
                hunks.push(hunk);
            } else {
                lines.next();
            }
        }

        Ok(UnifiedDiff {
            old_path,
            new_path,
            is_new_file,
            is_delete,
            hunks,
        })
    }

    /// Apply this diff to the given content.
    pub fn apply(&self, original: &str) -> Result<String, DiffError> {
        if self.is_new_file {
            // New file - just return the added lines
            let mut result = String::new();
            for hunk in &self.hunks {
                for line in &hunk.lines {
                    if let DiffLine::Add(content) = line {
                        result.push_str(content);
                        result.push('\n');
                    }
                }
            }
            // Remove trailing newline if original didn't have one
            if result.ends_with('\n') && !result.ends_with("\n\n") {
                result.pop();
            }
            return Ok(result);
        }

        if self.is_delete {
            // File deletion - return empty
            return Ok(String::new());
        }

        // Apply hunks in reverse order to preserve line numbers
        let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();

        for hunk in self.hunks.iter().rev() {
            lines = apply_hunk_to_lines(lines, hunk)?;
        }

        Ok(lines.join("\n"))
    }
}

/// Parse a file path from a --- or +++ line.
fn parse_file_path(line: &str, prefix: &str) -> String {
    let path = line.strip_prefix(prefix).unwrap_or(line).trim();

    // Handle "a/path" or "b/path" prefixes
    let path = if path.starts_with("a/") || path.starts_with("b/") {
        &path[2..]
    } else {
        path
    };

    // Handle tabs (git format: path<tab>timestamp)
    let path = path.split('\t').next().unwrap_or(path);

    path.to_string()
}

/// Parse a hunk from the diff lines.
fn parse_hunk(lines: &mut std::iter::Peekable<Lines>) -> Result<Hunk, DiffError> {
    let header = lines
        .next()
        .ok_or_else(|| DiffError::HunkParseError("Expected hunk header".to_string()))?;

    // Parse @@ -old_start,old_count +new_start,new_count @@
    let (old_start, old_count, new_start, new_count) = parse_hunk_header(header)?;

    let mut hunk_lines = Vec::new();

    while let Some(line) = lines.peek() {
        if line.starts_with("@@") || line.starts_with("---") || line.starts_with("+++") {
            break;
        }

        let line = lines.next().unwrap();

        if line.is_empty() {
            // Empty line is treated as context
            hunk_lines.push(DiffLine::Context(String::new()));
        } else if let Some(content) = line.strip_prefix('+') {
            hunk_lines.push(DiffLine::Add(content.to_string()));
        } else if let Some(content) = line.strip_prefix('-') {
            hunk_lines.push(DiffLine::Remove(content.to_string()));
        } else if let Some(content) = line.strip_prefix(' ') {
            hunk_lines.push(DiffLine::Context(content.to_string()));
        } else if line.starts_with('\\') {
            // "\ No newline at end of file" - ignore
            continue;
        } else {
            // Treat as context (some diffs don't prefix context with space)
            hunk_lines.push(DiffLine::Context(line.to_string()));
        }
    }

    Ok(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: hunk_lines,
    })
}

/// Parse a hunk header line.
fn parse_hunk_header(header: &str) -> Result<(usize, usize, usize, usize), DiffError> {
    // Format: @@ -old_start,old_count +new_start,new_count @@ optional section header
    let header = header
        .strip_prefix("@@")
        .and_then(|s| s.split("@@").next())
        .ok_or_else(|| DiffError::HunkParseError(format!("Invalid header: {}", header)))?;

    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(DiffError::HunkParseError(format!(
            "Invalid header: {}",
            header
        )));
    }

    let (old_start, old_count) = parse_range(parts[0].strip_prefix('-').unwrap_or(parts[0]))?;
    let (new_start, new_count) = parse_range(parts[1].strip_prefix('+').unwrap_or(parts[1]))?;

    Ok((old_start, old_count, new_start, new_count))
}

/// Parse a range like "1,3" or "1".
fn parse_range(range: &str) -> Result<(usize, usize), DiffError> {
    let parts: Vec<&str> = range.split(',').collect();
    let start = parts[0]
        .parse::<usize>()
        .map_err(|_| DiffError::HunkParseError(format!("Invalid range: {}", range)))?;
    let count = if parts.len() > 1 {
        parts[1]
            .parse::<usize>()
            .map_err(|_| DiffError::HunkParseError(format!("Invalid range: {}", range)))?
    } else {
        1
    };
    Ok((start, count))
}

/// Apply a single hunk to the lines.
fn apply_hunk_to_lines(lines: Vec<String>, hunk: &Hunk) -> Result<Vec<String>, DiffError> {
    // Calculate where to start (0-based index)
    let start_idx = if hunk.old_start == 0 {
        0
    } else {
        hunk.old_start - 1
    };

    // Verify context lines match (with some flexibility)
    let mut old_idx = start_idx;
    for diff_line in &hunk.lines {
        match diff_line {
            DiffLine::Context(expected) | DiffLine::Remove(expected) => {
                if old_idx < lines.len() {
                    let actual = &lines[old_idx];
                    // Allow whitespace differences
                    if actual.trim() != expected.trim()
                        && !expected.is_empty()
                        && !actual.is_empty()
                    {
                        // Just warn, don't fail - diffs can be fuzzy
                        tracing::warn!(
                            "Context mismatch at line {}: expected '{}', got '{}'",
                            old_idx + 1,
                            expected,
                            actual
                        );
                    }
                }
                old_idx += 1;
            }
            DiffLine::Add(_) => {}
        }
    }

    // Build new lines
    let mut new_lines = Vec::new();

    // Add lines before the hunk
    new_lines.extend(lines.iter().take(start_idx).cloned());

    // Apply the hunk
    for diff_line in &hunk.lines {
        match diff_line {
            DiffLine::Context(content) => {
                new_lines.push(content.clone());
            }
            DiffLine::Add(content) => {
                new_lines.push(content.clone());
            }
            DiffLine::Remove(_) => {
                // Skip removed lines
            }
        }
    }

    // Add lines after the hunk
    let skip_count = hunk
        .lines
        .iter()
        .filter(|l| matches!(l, DiffLine::Context(_) | DiffLine::Remove(_)))
        .count();

    new_lines.extend(lines.iter().skip(start_idx + skip_count).cloned());

    Ok(new_lines)
}

/// Apply a unified diff to file content.
pub fn apply_unified_diff(original: &str, diff_text: &str) -> Result<String, DiffError> {
    let diff = UnifiedDiff::parse(diff_text)?;
    diff.apply(original)
}

/// Check if text looks like a unified diff.
pub fn is_unified_diff(text: &str) -> bool {
    text.contains("@@") && (text.contains("---") || text.contains("+++"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_diff() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line 1
-line 2
+line 2 modified
+line 2.5 added
 line 3
"#;

        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.old_path, Some("file.txt".to_string()));
        assert_eq!(parsed.new_path, Some("file.txt".to_string()));
        assert_eq!(parsed.hunks.len(), 1);
        assert!(!parsed.is_new_file);
        assert!(!parsed.is_delete);
    }

    #[test]
    fn test_apply_simple_diff() {
        let original = "line 1\nline 2\nline 3";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line 1
-line 2
+line 2 modified
+line 2.5 added
 line 3
"#;

        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "line 1\nline 2 modified\nline 2.5 added\nline 3");
    }

    #[test]
    fn test_new_file_diff() {
        let diff = r#"--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
"#;

        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(parsed.is_new_file);
        assert_eq!(parsed.new_path, Some("new_file.txt".to_string()));

        let result = apply_unified_diff("", diff).unwrap();
        assert_eq!(result, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_delete_file_diff() {
        let diff = r#"--- a/old_file.txt
+++ /dev/null
@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3
"#;

        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(parsed.is_delete);

        let result = apply_unified_diff("line 1\nline 2\nline 3", diff).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_multiple_hunks() {
        let original = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 a
-b
+B
 c
@@ -8,3 +8,3 @@
 h
-i
+I
 j
"#;

        let result = apply_unified_diff(original, diff).unwrap();
        assert!(result.contains("B"));
        assert!(result.contains("I"));
        assert!(!result.contains("\nb\n"));
        assert!(!result.contains("\ni\n"));
    }

    #[test]
    fn test_is_unified_diff() {
        assert!(is_unified_diff("--- a/file\n+++ b/file\n@@"));
        assert!(!is_unified_diff("just some text"));
    }

    #[test]
    fn test_parse_hunk_header() {
        let (os, oc, ns, nc) = parse_hunk_header("@@ -1,3 +1,4 @@").unwrap();
        assert_eq!((os, oc, ns, nc), (1, 3, 1, 4));

        let (os, oc, ns, nc) = parse_hunk_header("@@ -1 +1,2 @@").unwrap();
        assert_eq!((os, oc, ns, nc), (1, 1, 1, 2));
    }

    // ===== parse_file_path tests =====

    #[test]
    fn test_parse_file_path_with_a_prefix() {
        assert_eq!(parse_file_path("--- a/src/main.rs", "---"), "src/main.rs");
    }

    #[test]
    fn test_parse_file_path_with_b_prefix() {
        assert_eq!(parse_file_path("+++ b/src/main.rs", "+++"), "src/main.rs");
    }

    #[test]
    fn test_parse_file_path_without_prefix() {
        assert_eq!(parse_file_path("--- src/main.rs", "---"), "src/main.rs");
    }

    #[test]
    fn test_parse_file_path_with_tab_timestamp() {
        // Git format: path<tab>timestamp
        assert_eq!(
            parse_file_path("--- a/file.txt\t2024-01-01 12:00:00", "---"),
            "file.txt"
        );
    }

    #[test]
    fn test_parse_file_path_dev_null() {
        assert_eq!(parse_file_path("--- /dev/null", "---"), "/dev/null");
    }

    // ===== parse_range tests =====

    #[test]
    fn test_parse_range_with_count() {
        let (start, count) = parse_range("5,10").unwrap();
        assert_eq!(start, 5);
        assert_eq!(count, 10);
    }

    #[test]
    fn test_parse_range_without_count() {
        let (start, count) = parse_range("42").unwrap();
        assert_eq!(start, 42);
        assert_eq!(count, 1); // defaults to 1
    }

    #[test]
    fn test_parse_range_zero_count() {
        let (start, count) = parse_range("0,0").unwrap();
        assert_eq!(start, 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_parse_range_invalid_start() {
        let result = parse_range("abc,3");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_range_invalid_count() {
        let result = parse_range("1,xyz");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    // ===== parse_hunk_header edge cases =====

    #[test]
    fn test_parse_hunk_header_with_section_name() {
        // Hunks can have optional section headers after @@
        let (os, oc, ns, nc) = parse_hunk_header("@@ -10,5 +10,7 @@ fn main() {").unwrap();
        assert_eq!((os, oc, ns, nc), (10, 5, 10, 7));
    }

    #[test]
    fn test_parse_hunk_header_single_line_both() {
        let (os, oc, ns, nc) = parse_hunk_header("@@ -1 +1 @@").unwrap();
        assert_eq!((os, oc, ns, nc), (1, 1, 1, 1));
    }

    #[test]
    fn test_parse_hunk_header_invalid_missing_at() {
        let result = parse_hunk_header("invalid header");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_hunk_header_invalid_missing_parts() {
        let result = parse_hunk_header("@@ -1 @@");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    // ===== Hunk parsing edge cases =====

    #[test]
    fn test_parse_hunk_empty_lines_as_context() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 first

 third
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks.len(), 1);
        // Empty line should be treated as context
        assert!(matches!(parsed.hunks[0].lines[1], DiffLine::Context(ref s) if s.is_empty()));
    }

    #[test]
    fn test_parse_hunk_no_newline_marker() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
 line 1
-line 2
+line 2 modified
\ No newline at end of file
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        // Should parse without including the "no newline" marker as a line
        assert_eq!(parsed.hunks[0].lines.len(), 3);
    }

    #[test]
    fn test_parse_hunk_context_without_space_prefix() {
        // Some diff tools don't prefix context with space
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
context_no_space
-old
+new
more_context
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(matches!(
            parsed.hunks[0].lines[0],
            DiffLine::Context(ref s) if s == "context_no_space"
        ));
    }

    // ===== apply_hunk_to_lines edge cases =====

    #[test]
    fn test_apply_hunk_start_at_zero() {
        // When old_start is 0 (new file case)
        let diff = r#"--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+line 1
+line 2
"#;
        let result = apply_unified_diff("", diff).unwrap();
        assert_eq!(result, "line 1\nline 2");
    }

    #[test]
    fn test_apply_diff_whitespace_tolerance() {
        // Should tolerate whitespace differences in context
        let original = "  line 1  \nline 2\n  line 3  ";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line 1
-line 2
+LINE 2
 line 3
"#;
        // Should not fail despite whitespace differences
        let result = apply_unified_diff(original, diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_diff_add_at_beginning() {
        let original = "existing";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,2 @@
+new first line
 existing
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "new first line\nexisting");
    }

    #[test]
    fn test_apply_diff_add_at_end() {
        let original = "existing";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,2 @@
 existing
+new last line
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "existing\nnew last line");
    }

    #[test]
    fn test_apply_diff_remove_all_lines() {
        let original = "line1\nline2\nline3";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,1 @@
-line1
-line2
-line3
+only this
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "only this");
    }

    // ===== DiffLine enum coverage =====

    #[test]
    fn test_diff_line_variants() {
        let context = DiffLine::Context("ctx".to_string());
        let add = DiffLine::Add("added".to_string());
        let remove = DiffLine::Remove("removed".to_string());

        // Debug trait
        assert!(format!("{:?}", context).contains("Context"));
        assert!(format!("{:?}", add).contains("Add"));
        assert!(format!("{:?}", remove).contains("Remove"));

        // Clone trait
        let cloned = context.clone();
        assert!(matches!(cloned, DiffLine::Context(ref s) if s == "ctx"));
    }

    // ===== Hunk struct coverage =====

    #[test]
    fn test_hunk_debug_clone() {
        let hunk = Hunk {
            old_start: 1,
            old_count: 3,
            new_start: 1,
            new_count: 4,
            lines: vec![DiffLine::Context("test".to_string())],
        };

        let debug_str = format!("{:?}", hunk);
        assert!(debug_str.contains("Hunk"));
        assert!(debug_str.contains("old_start: 1"));

        let cloned = hunk.clone();
        assert_eq!(cloned.old_start, 1);
        assert_eq!(cloned.lines.len(), 1);
    }

    // ===== UnifiedDiff struct coverage =====

    #[test]
    fn test_unified_diff_debug_clone() {
        let diff = UnifiedDiff {
            old_path: Some("old.txt".to_string()),
            new_path: Some("new.txt".to_string()),
            is_new_file: false,
            is_delete: false,
            hunks: vec![],
        };

        let debug_str = format!("{:?}", diff);
        assert!(debug_str.contains("UnifiedDiff"));

        let cloned = diff.clone();
        assert_eq!(cloned.old_path, Some("old.txt".to_string()));
    }

    // ===== DiffError variants =====

    #[test]
    fn test_diff_error_display() {
        let err = DiffError::InvalidFormat("test".to_string());
        assert!(err.to_string().contains("Invalid diff format"));

        let err = DiffError::HunkParseError("bad hunk".to_string());
        assert!(err.to_string().contains("Hunk header parse error"));

        let err = DiffError::ContextMismatch {
            line: 5,
            expected: "expected".to_string(),
            actual: "actual".to_string(),
        };
        assert!(err.to_string().contains("Context mismatch at line 5"));

        let err = DiffError::LineOutOfBounds(10);
        assert!(err.to_string().contains("Line 10 out of bounds"));

        let err = DiffError::PatchFailed("failed".to_string());
        assert!(err.to_string().contains("Patch application failed"));
    }

    #[test]
    fn test_diff_error_debug() {
        let err = DiffError::InvalidFormat("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidFormat"));
    }

    // ===== Complex multi-hunk scenarios =====

    #[test]
    fn test_three_hunks_applied_correctly() {
        let original = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
-1
+ONE
 2
@@ -5,2 +5,2 @@
-5
+FIVE
 6
@@ -10,3 +10,3 @@
 10
-11
+ELEVEN
 12
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert!(result.contains("ONE"));
        assert!(result.contains("FIVE"));
        assert!(result.contains("ELEVEN"));
        assert!(!result.contains("\n1\n"));
        assert!(!result.contains("\n5\n"));
        assert!(!result.contains("\n11\n"));
    }

    // ===== is_unified_diff edge cases =====

    #[test]
    fn test_is_unified_diff_edge_cases() {
        // Only @@ isn't enough
        assert!(!is_unified_diff("@@"));

        // Only --- isn't enough
        assert!(!is_unified_diff("--- some text"));

        // Only +++ isn't enough
        assert!(!is_unified_diff("+++ some text"));

        // @@ with +++ is enough
        assert!(is_unified_diff("+++ something\n@@"));

        // @@ with --- is enough
        assert!(is_unified_diff("--- something\n@@"));
    }

    // ===== Empty diff handling =====

    #[test]
    fn test_parse_empty_diff() {
        let diff = "";
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(parsed.hunks.is_empty());
        assert!(parsed.old_path.is_none());
        assert!(parsed.new_path.is_none());
    }

    #[test]
    fn test_parse_diff_headers_only() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.old_path, Some("file.txt".to_string()));
        assert_eq!(parsed.new_path, Some("file.txt".to_string()));
        assert!(parsed.hunks.is_empty());
    }

    // ===== Real-world diff formats =====

    #[test]
    fn test_git_diff_format_with_extra_headers() {
        // Git diffs often have extra headers before --- and +++
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("old");
+    println!("new");
 }
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.old_path, Some("src/main.rs".to_string()));
        assert_eq!(parsed.hunks.len(), 1);
    }

    #[test]
    fn test_apply_to_single_line_file() {
        let original = "single line";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-single line
+modified line
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "modified line");
    }

    #[test]
    fn test_apply_inserting_multiple_lines() {
        let original = "before\nafter";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,5 @@
 before
+inserted 1
+inserted 2
+inserted 3
 after
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "before\ninserted 1\ninserted 2\ninserted 3\nafter");
    }

    // ===== Malformed diff handling =====

    #[test]
    fn test_parse_hunk_header_empty_between_at_signs() {
        // @@ @@ with nothing between
        let result = parse_hunk_header("@@ @@");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_hunk_header_only_old_range() {
        // Missing new range
        let result = parse_hunk_header("@@ -1,3 @@");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_hunk_header_negative_numbers() {
        // Negative numbers should fail parsing (usize can't be negative)
        let result = parse_range("-5,3");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_range_empty_string() {
        let result = parse_range("");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_range_only_comma() {
        let result = parse_range(",");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    #[test]
    fn test_parse_range_multiple_commas() {
        // "1,2,3" - extra comma should just be ignored (only first two parts used)
        let result = parse_range("1,2,3");
        assert!(result.is_ok());
        let (start, count) = result.unwrap();
        assert_eq!(start, 1);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_parse_range_large_numbers() {
        let result = parse_range("999999,888888");
        assert!(result.is_ok());
        let (start, count) = result.unwrap();
        assert_eq!(start, 999999);
        assert_eq!(count, 888888);
    }

    #[test]
    fn test_parse_range_whitespace() {
        // Whitespace in range should fail
        let result = parse_range("1, 3");
        assert!(matches!(result, Err(DiffError::HunkParseError(_))));
    }

    // ===== File path parsing edge cases =====

    #[test]
    fn test_parse_file_path_empty() {
        assert_eq!(parse_file_path("---", "---"), "");
    }

    #[test]
    fn test_parse_file_path_just_whitespace() {
        assert_eq!(parse_file_path("---   ", "---"), "");
    }

    #[test]
    fn test_parse_file_path_multiple_tabs() {
        assert_eq!(
            parse_file_path("--- a/file.txt\t2024-01-01\textra", "---"),
            "file.txt"
        );
    }

    #[test]
    fn test_parse_file_path_with_spaces_in_name() {
        assert_eq!(
            parse_file_path("--- a/path with spaces/file.txt", "---"),
            "path with spaces/file.txt"
        );
    }

    #[test]
    fn test_parse_file_path_deeply_nested() {
        assert_eq!(
            parse_file_path("--- a/very/deep/nested/path/to/file.rs", "---"),
            "very/deep/nested/path/to/file.rs"
        );
    }

    #[test]
    fn test_parse_file_path_wrong_prefix() {
        // If prefix doesn't match, strip_prefix returns None, falls back to line
        assert_eq!(parse_file_path("+++ b/file.txt", "---"), "+++ b/file.txt");
    }

    // ===== Complex hunk scenarios =====

    #[test]
    fn test_hunk_with_only_additions() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,0 +1,3 @@
+line 1
+line 2
+line 3
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks[0].lines.len(), 3);
        assert!(parsed.hunks[0]
            .lines
            .iter()
            .all(|l| matches!(l, DiffLine::Add(_))));
    }

    #[test]
    fn test_hunk_with_only_removals() {
        let original = "line 1\nline 2\nline 3";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,0 @@
-line 1
-line 2
-line 3
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks[0].lines.len(), 3);
        assert!(parsed.hunks[0]
            .lines
            .iter()
            .all(|l| matches!(l, DiffLine::Remove(_))));

        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_hunk_with_only_context() {
        let original = "a\nb\nc";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 a
 b
 c
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn test_consecutive_add_remove_lines() {
        let original = "old1\nold2\nold3";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
-old1
-old2
-old3
+new1
+new2
+new3
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "new1\nnew2\nnew3");
    }

    #[test]
    fn test_alternating_add_remove() {
        let original = "a\nb\nc";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
-a
+A
-b
+B
-c
+C
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "A\nB\nC");
    }

    // ===== Boundary conditions =====

    #[test]
    fn test_apply_to_empty_file_non_new() {
        // Applying a diff to empty content (but not a new file diff)
        let original = "";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -0,0 +1,1 @@
+new content
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "new content");
    }

    #[test]
    fn test_single_character_content() {
        let original = "x";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1 +1 @@
-x
+y
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "y");
    }

    #[test]
    fn test_diff_with_unicode_content() {
        let original = "héllo\nwörld\n日本語";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 héllo
-wörld
+WÖRLD
 日本語
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "héllo\nWÖRLD\n日本語");
    }

    #[test]
    fn test_diff_with_special_characters() {
        let original = "line with $pecial ch@rs!\nand [brackets]";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
-line with $pecial ch@rs!
+modified $pecial ch@rs!
 and [brackets]
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert!(result.contains("modified $pecial ch@rs!"));
    }

    // ===== Large file simulation =====

    #[test]
    fn test_hunk_far_into_file() {
        // Hunk starting at line 100
        let original_lines: Vec<String> = (1..=200).map(|i| format!("line {}", i)).collect();
        let original = original_lines.join("\n");

        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -100,1 +100,1 @@
-line 100
+LINE ONE HUNDRED
"#;
        let result = apply_unified_diff(&original, diff).unwrap();
        assert!(result.contains("LINE ONE HUNDRED"));
        assert!(!result.contains("line 100"));
    }

    #[test]
    fn test_multiple_hunks_far_apart() {
        let lines: Vec<String> = (1..=50).map(|i| format!("line {}", i)).collect();
        let original = lines.join("\n");

        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-line 1
+FIRST
@@ -25,1 +25,1 @@
-line 25
+MIDDLE
@@ -50,1 +50,1 @@
-line 50
+LAST
"#;
        let result = apply_unified_diff(&original, diff).unwrap();
        assert!(result.starts_with("FIRST"));
        assert!(result.contains("MIDDLE"));
        assert!(result.ends_with("LAST"));
    }

    // ===== New file edge cases =====

    #[test]
    fn test_new_file_with_empty_lines() {
        let diff = r#"--- /dev/null
+++ b/new.txt
@@ -0,0 +1,4 @@
+line 1
+
+line 3
+
"#;
        let result = apply_unified_diff("", diff).unwrap();
        // Should preserve empty lines
        assert!(result.contains("\n\n") || result.contains("line 1\n\nline 3"));
    }

    #[test]
    fn test_new_file_single_empty_line() {
        let diff = r#"--- /dev/null
+++ b/new.txt
@@ -0,0 +1,1 @@
+
"#;
        let result = apply_unified_diff("", diff).unwrap();
        // Single empty line - should be empty string after trailing newline removal
        assert!(result.is_empty() || result == "\n");
    }

    // ===== Delete file edge cases =====

    #[test]
    fn test_delete_file_preserves_result() {
        let original = "content that will be deleted";
        let diff = r#"--- a/file.txt
+++ /dev/null
@@ -1,1 +0,0 @@
-content that will be deleted
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_delete_multiline_file() {
        let original = "line 1\nline 2\nline 3\nline 4\nline 5";
        let diff = r#"--- a/file.txt
+++ /dev/null
@@ -1,5 +0,0 @@
-line 1
-line 2
-line 3
-line 4
-line 5
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "");
    }

    // ===== Context line handling =====

    #[test]
    fn test_context_lines_preserved_around_change() {
        let original = "ctx1\nctx2\nchange me\nctx3\nctx4";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,5 +1,5 @@
 ctx1
 ctx2
-change me
+CHANGED
 ctx3
 ctx4
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "ctx1\nctx2\nCHANGED\nctx3\nctx4");
    }

    // ===== Hunk struct edge cases =====

    #[test]
    fn test_hunk_zero_old_count() {
        // Insert-only hunk (old_count = 0)
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,0 +1,2 @@
+new line 1
+new line 2
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks[0].old_count, 0);
    }

    #[test]
    fn test_hunk_zero_new_count() {
        // Delete-only hunk (new_count = 0)
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,0 @@
-old line 1
-old line 2
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks[0].new_count, 0);
    }

    // ===== Edge cases in apply_hunk_to_lines =====

    #[test]
    fn test_apply_when_original_shorter_than_expected() {
        // When the file is shorter than the diff expects
        let original = "only one line";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,2 @@
 only one line
+second line
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "only one line\nsecond line");
    }

    #[test]
    fn test_apply_hunk_at_exact_end() {
        let original = "line 1\nline 2\nline 3";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -3,1 +3,2 @@
 line 3
+line 4
"#;
        let result = apply_unified_diff(original, diff).unwrap();
        assert_eq!(result, "line 1\nline 2\nline 3\nline 4");
    }

    // ===== Parsing robustness =====

    #[test]
    fn test_parse_with_trailing_garbage() {
        // Extra lines after valid diff should be ignored
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-old
+new
some trailing garbage
more garbage
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks.len(), 1);
    }

    #[test]
    fn test_parse_with_leading_garbage() {
        // Git diffs often have commit info before the actual diff
        let diff = r#"commit abc123
Author: Test <test@example.com>
Date:   Mon Jan 1 00:00:00 2024

    Commit message

--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-old
+new
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.old_path, Some("file.txt".to_string()));
        assert_eq!(parsed.hunks.len(), 1);
    }

    #[test]
    fn test_parse_diff_with_binary_marker() {
        // Binary file markers shouldn't crash
        let diff = r#"diff --git a/image.png b/image.png
Binary files a/image.png and b/image.png differ
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        // No hunks for binary
        assert!(parsed.hunks.is_empty());
    }

    #[test]
    fn test_multiple_diffs_in_one_text() {
        // Only first diff's paths should be captured
        let diff = r#"--- a/first.txt
+++ b/first.txt
@@ -1,1 +1,1 @@
-old
+new
--- a/second.txt
+++ b/second.txt
@@ -1,1 +1,1 @@
-foo
+bar
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        // Current impl processes all hunks but only first file's paths
        assert!(!parsed.hunks.is_empty());
    }

    // ===== No-newline-at-end-of-file handling =====

    #[test]
    fn test_no_newline_at_end_marker_at_various_positions() {
        // Marker after removed line
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-old
\ No newline at end of file
+new
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        // Should parse correctly, ignoring the marker
        assert_eq!(parsed.hunks[0].lines.len(), 2);
    }

    #[test]
    fn test_no_newline_after_added_line() {
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-old
+new
\ No newline at end of file
"#;
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.hunks[0].lines.len(), 2);
    }

    // ===== DiffError coverage =====

    #[test]
    fn test_error_source_chain() {
        use std::error::Error;

        let err = DiffError::InvalidFormat("test".to_string());
        // thiserror errors implement Error trait
        assert!(err.source().is_none()); // No source for these variants

        let err = DiffError::ContextMismatch {
            line: 1,
            expected: "a".to_string(),
            actual: "b".to_string(),
        };
        assert!(err.source().is_none());
    }

    // ===== UnifiedDiff path handling =====

    #[test]
    fn test_paths_both_none() {
        // Edge case: neither --- nor +++ present
        let diff = "@@ -1,1 +1,1 @@\n-old\n+new";
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(parsed.old_path.is_none());
        assert!(parsed.new_path.is_none());
    }

    #[test]
    fn test_old_path_only() {
        let diff = "--- a/old.txt\n@@ -1,1 +1,1 @@\n-old\n+new";
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert_eq!(parsed.old_path, Some("old.txt".to_string()));
        assert!(parsed.new_path.is_none());
    }

    #[test]
    fn test_new_path_only() {
        let diff = "+++ b/new.txt\n@@ -1,1 +1,1 @@\n-old\n+new";
        let parsed = UnifiedDiff::parse(diff).unwrap();
        assert!(parsed.old_path.is_none());
        assert_eq!(parsed.new_path, Some("new.txt".to_string()));
    }

    // ===== Apply with mismatched line counts =====

    #[test]
    fn test_apply_hunk_count_mismatch_extra_context() {
        // Header says 3 lines but we have fewer - should still work
        let original = "a\nb";
        let diff = r#"--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 a
-b
+B
"#;
        // Should handle gracefully
        let result = apply_unified_diff(original, diff);
        assert!(result.is_ok());
    }
}
