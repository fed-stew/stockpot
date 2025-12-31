//! File operation tools.

use super::common::should_ignore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("File too large: {0} bytes (max: {1})")]
    TooLarge(u64, u64),
    #[error("Binary file: {0}")]
    BinaryFile(String),
    #[error("Grep error: {0}")]
    GrepError(String),
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
}

/// List files in a directory.
pub fn list_files(
    directory: &str,
    recursive: bool,
    max_depth: Option<usize>,
) -> Result<ListFilesResult, FileError> {
    let path = Path::new(directory);
    if !path.exists() {
        return Err(FileError::NotFound(directory.to_string()));
    }

    let mut entries = Vec::new();
    let mut total_files = 0;
    let mut total_dirs = 0;
    let mut total_size = 0u64;

    list_files_recursive(path, path, &mut entries, recursive, max_depth.unwrap_or(10), 0)?;

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
    })
}

fn list_files_recursive(
    base: &Path,
    dir: &Path,
    entries: &mut Vec<FileEntry>,
    recursive: bool,
    max_depth: usize,
    depth: usize,
) -> Result<(), FileError> {
    if depth > max_depth {
        return Ok(());
    }

    let mut dir_entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    
    dir_entries.sort_by_key(|a| a.file_name());

    for entry in dir_entries {
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap_or(&path);
        let relative_str = relative.to_string_lossy().to_string();

        if should_ignore(&relative_str) {
            continue;
        }

        let metadata = entry.metadata()?;
        let is_dir = metadata.is_dir();
        let name = entry.file_name().to_string_lossy().to_string();

        entries.push(FileEntry {
            path: relative_str.clone(),
            name,
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            depth,
        });

        if is_dir && recursive {
            list_files_recursive(base, &path, entries, recursive, max_depth, depth + 1)?;
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

    let content = if let Some(start) = start_line {
        let start_idx = start.saturating_sub(1); // 1-based to 0-based
        let end_idx = num_lines
            .map(|n| (start_idx + n).min(total_lines))
            .unwrap_or(total_lines);
        
        lines[start_idx..end_idx].join("\n")
    } else {
        content
    };

    Ok(ReadFileResult {
        content,
        path: path.to_string(),
        size: metadata.len(),
        lines: total_lines,
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

/// Search for a pattern in files using ripgrep.
pub fn grep(
    pattern: &str,
    directory: &str,
    max_results: Option<usize>,
) -> Result<GrepResult, FileError> {
    let max = max_results.unwrap_or(100);
    
    // Try ripgrep first, fall back to grep
    let output = Command::new("rg")
        .args([
            "--line-number",
            "--no-heading",
            "--max-count", &max.to_string(),
            pattern,
            directory,
        ])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => {
            // Fall back to grep
            Command::new("grep")
                .args(["-rn", "--max-count", &max.to_string(), pattern, directory])
                .output()?
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();

    for line in stdout.lines() {
        // Format: path:line_number:content
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 3 {
            matches.push(GrepMatch {
                path: parts[0].to_string(),
                line_number: parts[1].parse().unwrap_or(0),
                content: parts[2].to_string(),
            });
        }
    }

    Ok(GrepResult {
        total_matches: matches.len(),
        matches,
    })
}

/// Apply a unified diff to a file.
/// 
/// Parses the unified diff format and applies it to the file:
/// ```text
/// --- a/file.txt
/// +++ b/file.txt
/// @@ -1,3 +1,4 @@
///  context line
/// -removed line
/// +added line
/// ```
pub fn apply_diff(path: &str, diff_text: &str) -> Result<(), FileError> {
    use super::diff::{UnifiedDiff, apply_unified_diff};
    
    // Parse the diff to check if it's a new file
    let parsed = UnifiedDiff::parse(diff_text)
        .map_err(|e| FileError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string()
        )))?;
    
    // Read original content (or empty for new files)
    let original = if parsed.is_new_file {
        String::new()
    } else if Path::new(path).exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    // Handle file deletion
    if parsed.is_delete {
        if Path::new(path).exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }

    // Apply the diff
    let patched = apply_unified_diff(&original, diff_text)
        .map_err(|e| FileError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string()
        )))?;
    
    // Write back
    write_file(path, &patched, true)?;
    
    Ok(())
}
