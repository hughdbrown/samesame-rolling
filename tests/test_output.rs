//! Tests for output formatting.

use samesame::output::{format_json, format_text};
use samesame::rolling_hash::DuplicateGroup;
use std::path::PathBuf;

fn make_group(
    line_count: usize,
    locations: Vec<(&str, usize, usize)>,
    content: Option<Vec<&str>>,
) -> DuplicateGroup {
    DuplicateGroup {
        line_count,
        locations: locations
            .into_iter()
            .map(|(f, s, e)| (PathBuf::from(f), s, e))
            .collect(),
        content: content.map(|c| c.into_iter().map(String::from).collect()),
    }
}

#[test]
fn test_format_text_no_duplicates() {
    let groups: Vec<DuplicateGroup> = vec![];
    let output = format_text(&groups, false, 2);
    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_text_with_duplicates() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        None,
    )];
    let output = format_text(&groups, false, 2);
    assert!(output.contains("Duplicate Code Found"));
    assert!(output.contains("file1.rs"));
    assert!(output.contains("file2.rs"));
    assert!(output.contains("3 lines"));
    assert!(output.contains("Summary:"));
}

#[test]
fn test_format_text_verbose() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        Some(vec!["fn main() {", "    println!(\"Hello\");", "}"]),
    )];
    let output = format_text(&groups, true, 2);
    assert!(output.contains("fn main()"));
    assert!(output.contains("println!"));
}

#[test]
fn test_format_text_below_threshold() {
    // No groups because threshold filtered them out
    let groups: Vec<DuplicateGroup> = vec![];
    let output = format_text(&groups, false, 2);
    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_json_no_duplicates() {
    let groups: Vec<DuplicateGroup> = vec![];
    let output = format_json(&groups, false, 2);
    assert!(output.contains("\"duplicate_groups\": 0"));
    assert!(output.contains("\"duplicates\": []"));
}

#[test]
fn test_format_json_with_duplicates() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        None,
    )];
    let output = format_json(&groups, false, 2);
    assert!(output.contains("\"duplicate_groups\": 1"));
    assert!(output.contains("\"lines\": 3"));
    assert!(output.contains("\"file\": \"src/file1.rs\""));
    assert!(output.contains("\"file\": \"src/file2.rs\""));
}

#[test]
fn test_format_json_verbose_includes_content() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        Some(vec!["fn main() {", "    println!(\"Hello\");", "}"]),
    )];
    let output = format_json(&groups, true, 2);
    assert!(output.contains("\"content\":"));
    assert!(output.contains("fn main()"));
}

#[test]
fn test_format_json_not_verbose_no_content() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        Some(vec!["fn main() {", "    println!(\"Hello\");", "}"]),
    )];
    // Even though content is in the group, verbose=false should suppress it
    let output = format_json(&groups, false, 2);
    assert!(!output.contains("\"content\":"));
}

#[test]
fn test_format_json_structure() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.rs", 0, 3), ("src/file2.rs", 0, 3)],
        None,
    )];
    let output = format_json(&groups, false, 2);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["version"].is_string());
    assert!(parsed["summary"]["files_analyzed"].is_number());
    assert!(parsed["summary"]["duplicate_groups"].is_number());
    assert!(parsed["duplicates"].is_array());
    let first = &parsed["duplicates"][0];
    assert!(first["locations"].is_array());
    assert!(first["lines"].is_number());
}

#[test]
fn test_format_text_multiple_results_grouped() {
    let groups = vec![make_group(
        3,
        vec![("a.rs", 0, 3), ("b.rs", 0, 3), ("c.rs", 0, 3)],
        None,
    )];
    let output = format_text(&groups, false, 3);
    assert!(output.contains("a.rs"));
    assert!(output.contains("b.rs"));
    assert!(output.contains("c.rs"));
    assert!(output.contains("1 duplicate groups"));
    assert!(output.contains("3 lines duplicated across 3 files"));
}

#[test]
fn test_format_json_multiple_matches_same_pair() {
    let groups = vec![
        make_group(
            2,
            vec![("src/file1.rs", 0, 2), ("src/file2.rs", 0, 2)],
            None,
        ),
        make_group(
            2,
            vec![("src/file1.rs", 3, 5), ("src/file2.rs", 3, 5)],
            None,
        ),
    ];
    let output = format_json(&groups, false, 2);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["summary"]["duplicate_groups"], 2);
    assert_eq!(parsed["duplicates"].as_array().unwrap().len(), 2);
}

// ==================== Regex filtering tests ====================
// Regex filtering now happens in main.rs before output formatting.
// These tests verify that filtered groups produce correct output.

#[test]
fn test_format_text_regex_filtered_result() {
    // After regex filtering, only the matching group remains
    let groups = vec![make_group(
        3,
        vec![("src/file1.py", 0, 3), ("src/file2.py", 0, 3)],
        None,
    )];
    let output = format_text(&groups, false, 2);
    assert!(output.contains("Duplicate Code Found"));
    assert!(output.contains("3 lines"));
    assert!(output.contains("1 duplicate groups"));
}

#[test]
fn test_format_text_regex_all_filtered() {
    // After regex filtering, no groups remain
    let groups: Vec<DuplicateGroup> = vec![];
    let output = format_text(&groups, false, 2);
    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_json_regex_filtered_result() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.py", 4, 7), ("src/file2.py", 4, 7)],
        None,
    )];
    let output = format_json(&groups, false, 2);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["summary"]["duplicate_groups"], 1);
    let locations = &parsed["duplicates"][0]["locations"];
    assert_eq!(locations.as_array().unwrap().len(), 2);
    assert_eq!(locations[0]["start"], 5); // 1-based
}

#[test]
fn test_format_json_regex_all_filtered() {
    let groups: Vec<DuplicateGroup> = vec![];
    let output = format_json(&groups, false, 2);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["summary"]["duplicate_groups"], 0);
    assert!(parsed["duplicates"].as_array().unwrap().is_empty());
}

#[test]
fn test_format_text_regex_verbose_shows_content() {
    let groups = vec![make_group(
        3,
        vec![("src/file1.py", 0, 3), ("src/file2.py", 0, 3)],
        Some(vec![
            "def hello():",
            "    print(\"Hello\")",
            "    return None",
        ]),
    )];
    let output = format_text(&groups, true, 2);
    assert!(output.contains("def hello():"));
    assert!(output.contains("print(\"Hello\")"));
}
