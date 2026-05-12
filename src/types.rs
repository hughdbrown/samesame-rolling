//! Core data structures for the samesame application.

use std::path::PathBuf;

/// Represents a file's content as a sequence of line hashes.
///
/// Lines themselves are not stored — only their hashes. Original line
/// content is re-read lazily at output time for `--verbose` / `--regex`.
#[derive(Debug)]
pub struct FileDescription {
    /// Path to the source file.
    pub filename: PathBuf,
    /// Hash of each line (after normalization).
    pub hashes: Vec<u64>,
}

impl FileDescription {
    /// Returns the number of lines in the file.
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// Returns true if the file has no lines.
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}
