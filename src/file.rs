//! File reading, normalization, and hashing.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::{Result, SameError};
use crate::types::FileDescription;

/// Size of buffer for binary detection (8KB).
const BINARY_CHECK_SIZE: usize = 8192;

/// Check if a file appears to be binary by looking for null bytes.
pub fn is_binary_file(path: &Path) -> Result<bool> {
    let file = File::open(path).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; BINARY_CHECK_SIZE];

    let bytes_read = reader.read(&mut buffer).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Check for null bytes in the read portion
    Ok(buffer[..bytes_read].contains(&0))
}

/// Hash a normalized line using xxh3 (non-cryptographic, very fast).
/// Returns a 64-bit hash.
pub fn hash_line(line: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(line.as_bytes())
}

/// Read a file and create a FileDescription.
///
/// Stores only per-line hashes (computed from trimmed lines).
/// Original line content is not retained — call `read_lines` to
/// re-fetch it on demand for verbose/regex output.
/// Non-UTF-8 bytes are handled with lossy conversion.
pub fn read_file(path: &Path) -> Result<FileDescription> {
    let content = std::fs::read(path).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    let text = String::from_utf8_lossy(&content);
    let hashes: Vec<u64> = text.lines().map(|line| hash_line(line.trim())).collect();

    Ok(FileDescription {
        filename: path.to_path_buf(),
        hashes,
    })
}

/// Read a file, skipping if it's binary or empty.
///
/// Returns None for binary files or empty files.
/// Reads the file only once, checking for binary content in-memory.
pub fn read_file_if_text(path: &Path) -> Result<Option<FileDescription>> {
    let content = std::fs::read(path).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Check for binary content (null bytes in first 8KB)
    let check_len = content.len().min(BINARY_CHECK_SIZE);
    if content[..check_len].contains(&0) {
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&content);
    let hashes: Vec<u64> = text.lines().map(|line| hash_line(line.trim())).collect();

    if hashes.is_empty() {
        return Ok(None);
    }

    Ok(Some(FileDescription {
        filename: path.to_path_buf(),
        hashes,
    }))
}

/// Re-read a file and return its lines with original indentation preserved.
///
/// Used at output time for `--verbose` and `--regex` paths, which only
/// need line content for the small subset of files that contain matches.
/// Non-UTF-8 bytes are handled with lossy conversion (same as read_file).
pub fn read_lines(path: &Path) -> Result<Vec<String>> {
    let content = std::fs::read(path).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    let text = String::from_utf8_lossy(&content);
    Ok(text.lines().map(|line| line.to_string()).collect())
}
