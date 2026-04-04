//! File discovery and filtering.

use std::path::{Path, PathBuf};

use glob::glob;

use crate::error::{Result, SameError};

/// Discover files from explicit paths and/or directory scanning.
pub fn discover_files(
    explicit_files: &[String],
    directory: Option<&str>,
    glob_pattern: &str,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // Add explicit file paths
    for path in explicit_files {
        let path = PathBuf::from(path);
        if path.is_file() && !is_symlink(&path) {
            files.push(path);
        }
    }

    // Scan directory with glob pattern
    if let Some(dir) = directory {
        let pattern = format!("{}/{}", dir.trim_end_matches('/'), glob_pattern);
        let discovered = scan_glob(&pattern)?;
        files.extend(discovered);
    }

    // If no directory specified but no explicit files either, scan current directory
    if directory.is_none() && explicit_files.is_empty() {
        let discovered = scan_glob(glob_pattern)?;
        files.extend(discovered);
    }

    // Remove duplicates while preserving order
    files = deduplicate_paths(files);

    if files.is_empty() {
        return Err(SameError::NoFilesFound);
    }

    Ok(files)
}

/// Scan using a glob pattern.
pub fn scan_glob(pattern: &str) -> Result<Vec<PathBuf>> {
    let entries = glob(pattern).map_err(|e| SameError::InvalidGlob {
        pattern: pattern.to_string(),
        message: e.msg.to_string(),
    })?;

    let mut files = Vec::new();

    for entry in entries {
        match entry {
            Ok(path) => {
                // Skip symlinks, directories, and hidden files
                if path.is_file() && !is_symlink(&path) && !is_hidden(&path) {
                    files.push(path);
                }
            }
            Err(_) => {
                // Skip entries we can't read
                continue;
            }
        }
    }

    Ok(files)
}

/// Check if a path is a symlink.
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Check if a path is hidden (starts with .).
pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

/// Remove duplicate paths, keeping first occurrence.
pub fn deduplicate_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = rustc_hash::FxHashSet::default();
    let mut result = Vec::with_capacity(paths.len());

    for path in paths {
        // Canonicalize to handle relative vs absolute paths
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if seen.insert(canonical) {
            result.push(path);
        }
    }

    result
}
