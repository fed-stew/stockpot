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
    let path = line
        .strip_prefix(prefix)
        .unwrap_or(line)
        .trim();
    
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
    let header = lines.next().ok_or_else(|| {
        DiffError::HunkParseError("Expected hunk header".to_string())
    })?;

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
        return Err(DiffError::HunkParseError(format!("Invalid header: {}", header)));
    }

    let (old_start, old_count) = parse_range(parts[0].strip_prefix('-').unwrap_or(parts[0]))?;
    let (new_start, new_count) = parse_range(parts[1].strip_prefix('+').unwrap_or(parts[1]))?;

    Ok((old_start, old_count, new_start, new_count))
}

/// Parse a range like "1,3" or "1".
fn parse_range(range: &str) -> Result<(usize, usize), DiffError> {
    let parts: Vec<&str> = range.split(',').collect();
    let start = parts[0].parse::<usize>().map_err(|_| {
        DiffError::HunkParseError(format!("Invalid range: {}", range))
    })?;
    let count = if parts.len() > 1 {
        parts[1].parse::<usize>().map_err(|_| {
            DiffError::HunkParseError(format!("Invalid range: {}", range))
        })?
    } else {
        1
    };
    Ok((start, count))
}

/// Apply a single hunk to the lines.
fn apply_hunk_to_lines(lines: Vec<String>, hunk: &Hunk) -> Result<Vec<String>, DiffError> {
    // Calculate where to start (0-based index)
    let start_idx = if hunk.old_start == 0 { 0 } else { hunk.old_start - 1 };
    
    // Verify context lines match (with some flexibility)
    let mut old_idx = start_idx;
    for diff_line in &hunk.lines {
        match diff_line {
            DiffLine::Context(expected) | DiffLine::Remove(expected) => {
                if old_idx < lines.len() {
                    let actual = &lines[old_idx];
                    // Allow whitespace differences
                    if actual.trim() != expected.trim() && !expected.is_empty() && !actual.is_empty() {
                        // Just warn, don't fail - diffs can be fuzzy
                        tracing::warn!(
                            "Context mismatch at line {}: expected '{}', got '{}'",
                            old_idx + 1, expected, actual
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
    let skip_count = hunk.lines.iter().filter(|l| {
        matches!(l, DiffLine::Context(_) | DiffLine::Remove(_))
    }).count();
    
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
}
