//! Tests for file reading, normalization, and hashing.

use samesame::error::{Result, SameError};
use samesame::file::{hash_line, is_binary_file, read_file, read_file_if_text, read_lines};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn test_hash_line_consistency() {
    let line = "fn main() {";
    let hash1 = hash_line(line);
    let hash2 = hash_line(line);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_hash_line_different() {
    let hash1 = hash_line("fn main() {");
    let hash2 = hash_line("fn main() {}");
    assert_ne!(hash1, hash2);
}

#[test]
fn test_hash_line_empty() {
    let hash = hash_line("");
    // Empty string should still produce a valid hash
    assert!(hash != 0 || hash == 0); // Just verify it doesn't panic
}

#[test]
fn test_hash_line_whitespace_matters() {
    let hash1 = hash_line("hello");
    let hash2 = hash_line(" hello");
    let hash3 = hash_line("hello ");
    // After normalization in read_file, these would be same,
    // but hash_line itself doesn't normalize
    assert_ne!(hash1, hash2);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_read_file() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "  line one  ").unwrap();
    writeln!(temp, "line two").unwrap();
    writeln!(temp, "   line three   ").unwrap();

    let desc = read_file(temp.path())?;

    assert_eq!(desc.hashes.len(), 3);

    // Hashes are computed from trimmed lines.
    let hash_trimmed = hash_line("line one");
    assert_eq!(desc.hashes[0], hash_trimmed);

    // Original line content is recoverable via read_lines (preserves indentation).
    let lines = read_lines(temp.path())?;
    assert_eq!(lines, vec!["  line one  ", "line two", "   line three   "]);

    Ok(())
}

#[test]
fn test_read_file_empty() -> Result<()> {
    let temp = NamedTempFile::new().unwrap();
    // Don't write anything

    let desc = read_file(temp.path())?;
    assert!(desc.hashes.is_empty());

    Ok(())
}

#[test]
fn test_read_file_single_line() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    write!(temp, "single line no newline").unwrap();

    let desc = read_file(temp.path())?;
    assert_eq!(desc.hashes.len(), 1);
    assert_eq!(read_lines(temp.path())?, vec!["single line no newline"]);

    Ok(())
}

#[test]
fn test_read_file_preserves_filename() -> Result<()> {
    let temp = NamedTempFile::new().unwrap();
    let desc = read_file(temp.path())?;
    assert_eq!(desc.filename, temp.path());

    Ok(())
}

#[test]
fn test_read_file_nonexistent() {
    let result = read_file(Path::new("/nonexistent/path/file.rs"));
    assert!(result.is_err());
    if let Err(SameError::FileRead { path, .. }) = result {
        assert_eq!(path, Path::new("/nonexistent/path/file.rs"));
    } else {
        panic!("Expected FileRead error");
    }
}

#[test]
fn test_read_file_hashes_match_for_same_content() -> Result<()> {
    let mut temp1 = NamedTempFile::new().unwrap();
    let mut temp2 = NamedTempFile::new().unwrap();

    writeln!(temp1, "same content").unwrap();
    writeln!(temp2, "same content").unwrap();

    let desc1 = read_file(temp1.path())?;
    let desc2 = read_file(temp2.path())?;

    assert_eq!(desc1.hashes, desc2.hashes);

    Ok(())
}

#[test]
fn test_is_binary_file() -> Result<()> {
    // Create a text file
    let mut text_file = NamedTempFile::new().unwrap();
    writeln!(text_file, "Hello, world!").unwrap();
    assert!(!is_binary_file(text_file.path())?);

    // Create a binary file
    let mut binary_file = NamedTempFile::new().unwrap();
    binary_file.write_all(&[0x00, 0x01, 0x02]).unwrap();
    assert!(is_binary_file(binary_file.path())?);

    Ok(())
}

#[test]
fn test_is_binary_file_empty() -> Result<()> {
    let temp = NamedTempFile::new().unwrap();
    // Empty file is not binary
    assert!(!is_binary_file(temp.path())?);

    Ok(())
}

#[test]
fn test_is_binary_file_null_in_middle() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(b"text\x00more text").unwrap();
    assert!(is_binary_file(temp.path())?);

    Ok(())
}

#[test]
fn test_is_binary_file_nonexistent() {
    let result = is_binary_file(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn test_read_file_if_text_normal() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "some text").unwrap();

    let result = read_file_if_text(temp.path())?;
    assert!(result.is_some());

    let desc = result.unwrap();
    assert_eq!(desc.hashes.len(), 1);

    Ok(())
}

#[test]
fn test_read_file_if_text_binary_returns_none() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(&[0x00, 0x01, 0x02]).unwrap();

    let result = read_file_if_text(temp.path())?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn test_read_file_if_text_empty_returns_none() -> Result<()> {
    let temp = NamedTempFile::new().unwrap();
    // Empty file

    let result = read_file_if_text(temp.path())?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn test_read_file_if_text_nonexistent() {
    let result = read_file_if_text(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn test_read_file_with_tabs() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "\t\tindented with tabs\t\t").unwrap();

    let desc = read_file(temp.path())?;
    // Hash is computed from the trimmed line.
    let expected_hash = hash_line("indented with tabs");
    assert_eq!(desc.hashes[0], expected_hash);
    // Lines fetched via read_lines preserve original tabs.
    assert_eq!(read_lines(temp.path())?[0], "\t\tindented with tabs\t\t");

    Ok(())
}

#[test]
fn test_read_file_blank_lines() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "line 1").unwrap();
    writeln!(temp, "").unwrap();
    writeln!(temp, "line 3").unwrap();

    let desc = read_file(temp.path())?;
    assert_eq!(desc.hashes.len(), 3);
    assert_eq!(read_lines(temp.path())?, vec!["line 1", "", "line 3"]);

    Ok(())
}

#[test]
fn test_read_file_unicode() -> Result<()> {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "Hello 世界 🌍").unwrap();

    let desc = read_file(temp.path())?;
    assert_eq!(desc.hashes.len(), 1);
    assert_eq!(read_lines(temp.path())?[0], "Hello 世界 🌍");

    Ok(())
}
