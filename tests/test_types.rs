//! Tests for core data types.

use samesame::types::FileDescription;
use std::path::PathBuf;

// FileDescription tests
#[test]
fn test_file_description_len() {
    let fd = FileDescription {
        filename: PathBuf::from("test.rs"),
        hashes: vec![1, 2, 3, 4, 5],
        lines: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
    };
    assert_eq!(fd.len(), 5);
}

#[test]
fn test_file_description_is_empty_false() {
    let fd = FileDescription {
        filename: PathBuf::from("test.rs"),
        hashes: vec![1],
        lines: vec!["a".into()],
    };
    assert!(!fd.is_empty());
}

#[test]
fn test_file_description_is_empty_true() {
    let fd = FileDescription {
        filename: PathBuf::from("test.rs"),
        hashes: vec![],
        lines: vec![],
    };
    assert!(fd.is_empty());
}
