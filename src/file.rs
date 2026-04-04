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
/// Lines are stored with original indentation intact.
/// Hashes are computed from trimmed lines for comparison.
/// Non-UTF-8 bytes are handled with lossy conversion.
pub fn read_file(path: &Path) -> Result<FileDescription> {
    let content = std::fs::read(path).map_err(|e| SameError::FileRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Use lossy conversion for non-UTF-8 bytes
    let text = String::from_utf8_lossy(&content);

    let mut lines = Vec::new();
    let mut hashes = Vec::new();

    for line in text.lines() {
        // Store original line with indentation
        lines.push(line.to_string());
        // Hash the trimmed version for comparison
        hashes.push(hash_line(line.trim()));
    }

    Ok(FileDescription {
        filename: path.to_path_buf(),
        hashes,
        lines,
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

    // Use lossy conversion for non-UTF-8 bytes
    let text = String::from_utf8_lossy(&content);

    let mut lines = Vec::new();
    let mut hashes = Vec::new();

    for line in text.lines() {
        lines.push(line.to_string());
        hashes.push(hash_line(line.trim()));
    }

    // Skip empty files
    if lines.is_empty() {
        return Ok(None);
    }

    Ok(Some(FileDescription {
        filename: path.to_path_buf(),
        hashes,
        lines,
    }))
}
