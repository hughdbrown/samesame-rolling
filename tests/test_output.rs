//! Tests for output formatting.

use regex::Regex;
use samesame::grouping::group_duplicates;
use samesame::output::{format_json, format_text};
use samesame::types::{ComparisonResult, FileDescription, LineRange, Range};
use std::path::PathBuf;

fn make_test_files() -> (FileDescription, FileDescription) {
    let f1 = FileDescription {
        filename: PathBuf::from("src/file1.rs"),
        hashes: vec![1, 2, 3, 4, 5],
        lines: vec![
            "fn main() {".into(),
            "    println!(\"Hello\");".into(),
            "}".into(),
            "".into(),
            "fn other() {}".into(),
        ],
    };
    let f2 = FileDescription {
        filename: PathBuf::from("src/file2.rs"),
        hashes: vec![1, 2, 3, 6, 7],
        lines: vec![
            "fn main() {".into(),
            "    println!(\"Hello\");".into(),
            "}".into(),
            "".into(),
            "fn different() {}".into(),
        ],
    };
    (f1, f2)
}

#[test]
fn test_format_text_no_duplicates() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Diff {
            r1: Range::new(0, 5),
            r2: Range::new(0, 5),
        }],
    }];

    let groups = group_duplicates(&results, 5);
    let output = format_text(&groups, &results, false, 2, 1);
    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_text_with_duplicates() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, false, 2, 1);
    assert!(output.contains("Duplicate Code Found"));
    assert!(output.contains("file1.rs"));
    assert!(output.contains("file2.rs"));
    assert!(output.contains("3 lines"));
    assert!(output.contains("Summary:"));
}

#[test]
fn test_format_text_verbose() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, true, 2, 1);
    assert!(output.contains("fn main()"));
    assert!(output.contains("println!"));
}

#[test]
fn test_format_text_below_threshold() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    // Threshold is 5, but match is only 3 lines
    let groups = group_duplicates(&results, 5);
    let output = format_text(&groups, &results, false, 2, 1);
    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_json_no_duplicates() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Diff {
            r1: Range::new(0, 5),
            r2: Range::new(0, 5),
        }],
    }];

    let groups = group_duplicates(&results, 5);
    let output = format_json(&groups, &results, false, 2, 1);
    assert!(output.contains("\"duplicate_groups\": 0"));
    assert!(output.contains("\"duplicates\": []"));
}

#[test]
fn test_format_json_with_duplicates() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, false, 2, 1);
    assert!(output.contains("\"duplicate_groups\": 1"));
    assert!(output.contains("\"lines\": 3"));
    // Grouped format uses locations array
    assert!(output.contains("\"file\": \"src/file1.rs\""));
    assert!(output.contains("\"file\": \"src/file2.rs\""));
}

#[test]
fn test_format_json_verbose_includes_content() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, true, 2, 1);
    assert!(output.contains("\"content\":"));
    assert!(output.contains("fn main()"));
}

#[test]
fn test_format_json_not_verbose_no_content() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, false, 2, 1);
    assert!(!output.contains("\"content\":"));
}

#[test]
fn test_format_json_structure() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    }];

    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, false, 2, 1);

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["version"].is_string());
    assert!(parsed["summary"]["files_analyzed"].is_number());
    assert!(parsed["summary"]["pairs_compared"].is_number());
    assert!(parsed["summary"]["duplicate_groups"].is_number());
    assert!(parsed["duplicates"].is_array());
    // Each duplicate should have locations array
    let first = &parsed["duplicates"][0];
    assert!(first["locations"].is_array());
    assert!(first["lines"].is_number());
}

#[test]
fn test_format_text_multiple_results_grouped() {
    let f1 = FileDescription {
        filename: PathBuf::from("a.rs"),
        hashes: vec![1, 2, 3],
        lines: vec!["a".into(), "b".into(), "c".into()],
    };
    let f2 = FileDescription {
        filename: PathBuf::from("b.rs"),
        hashes: vec![1, 2, 3],
        lines: vec!["a".into(), "b".into(), "c".into()],
    };
    let f3 = FileDescription {
        filename: PathBuf::from("c.rs"),
        hashes: vec![1, 2, 3],
        lines: vec!["a".into(), "b".into(), "c".into()],
    };

    let results = vec![
        ComparisonResult {
            f1: &f1,
            f2: &f2,
            runs: vec![LineRange::Same {
                r1: Range::new(0, 3),
                r2: Range::new(0, 3),
            }],
        },
        ComparisonResult {
            f1: &f1,
            f2: &f3,
            runs: vec![LineRange::Same {
                r1: Range::new(0, 3),
                r2: Range::new(0, 3),
            }],
        },
    ];

    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, false, 3, 3);
    assert!(output.contains("a.rs"));
    assert!(output.contains("b.rs"));
    assert!(output.contains("c.rs"));
    // Should be 1 group (not 2 pairwise results), with 3 files
    assert!(output.contains("1 duplicate groups"));
    assert!(output.contains("3 lines duplicated across 3 files"));
}

#[test]
fn test_format_json_multiple_matches_same_pair() {
    let (f1, f2) = make_test_files();
    let results = vec![ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![
            LineRange::Same {
                r1: Range::new(0, 2),
                r2: Range::new(0, 2),
            },
            LineRange::Diff {
                r1: Range::new(2, 3),
                r2: Range::new(2, 3),
            },
            LineRange::Same {
                r1: Range::new(3, 5),
                r2: Range::new(3, 5),
            },
        ],
    }];

    let groups = group_duplicates(&results, 2);
    let output = format_json(&groups, &results, false, 2, 1);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Should have 2 independent groups (two different regions)
    assert_eq!(parsed["summary"]["duplicate_groups"], 2);
    assert_eq!(parsed["duplicates"].as_array().unwrap().len(), 2);
}

// ==================== Regex filtering tests ====================

fn make_mixed_files() -> (FileDescription, FileDescription) {
    let f1 = FileDescription {
        filename: PathBuf::from("src/file1.py"),
        hashes: vec![1, 2, 3, 4, 5, 6, 7, 8],
        lines: vec![
            "def hello():".into(),
            "    print(\"Hello\")".into(),
            "    return None".into(),
            "".into(),
            "class MyClass:".into(),
            "    def __init__(self):".into(),
            "        pass".into(),
            "".into(),
        ],
    };
    let f2 = FileDescription {
        filename: PathBuf::from("src/file2.py"),
        hashes: vec![1, 2, 3, 4, 5, 6, 7, 8],
        lines: vec![
            "def hello():".into(),
            "    print(\"Hello\")".into(),
            "    return None".into(),
            "".into(),
            "class MyClass:".into(),
            "    def __init__(self):".into(),
            "        pass".into(),
            "".into(),
        ],
    };
    (f1, f2)
}

/// Apply regex filtering to a ComparisonResult, retaining only Same runs
/// whose first line matches the regex (mirrors main.rs filter_runs_by_regex).
fn filter_by_regex(result: &mut ComparisonResult<'_>, regex: &Regex) {
    result.runs.retain(|run| match run {
        LineRange::Same { r1, .. } => {
            if r1.start < result.f1.lines.len() {
                regex.is_match(&result.f1.lines[r1.start])
            } else {
                false
            }
        }
        LineRange::Diff { .. } => true,
    });
}

#[test]
fn test_format_text_regex_filters_matches() {
    let (f1, f2) = make_mixed_files();
    let mut result = ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![
            LineRange::Same {
                r1: Range::new(0, 3),
                r2: Range::new(0, 3),
            },
            LineRange::Diff {
                r1: Range::new(3, 4),
                r2: Range::new(3, 4),
            },
            LineRange::Same {
                r1: Range::new(4, 7),
                r2: Range::new(4, 7),
            },
        ],
    };

    let regex = Regex::new(r"^def ").unwrap();
    filter_by_regex(&mut result, &regex);
    let results = vec![result];
    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, false, 2, 1);

    assert!(output.contains("Duplicate Code Found"));
    assert!(output.contains("3 lines"));
    assert!(output.contains("1 duplicate groups"));
}

#[test]
fn test_format_text_regex_filters_all() {
    let (f1, f2) = make_mixed_files();
    let mut result = ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(4, 7),
            r2: Range::new(4, 7),
        }],
    };

    let regex = Regex::new(r"^def ").unwrap();
    filter_by_regex(&mut result, &regex);
    let results = vec![result];
    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, false, 2, 1);

    assert!(output.contains("No duplicate code found"));
}

#[test]
fn test_format_json_regex_filters_matches() {
    let (f1, f2) = make_mixed_files();
    let mut result = ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![
            LineRange::Same {
                r1: Range::new(0, 3),
                r2: Range::new(0, 3),
            },
            LineRange::Diff {
                r1: Range::new(3, 4),
                r2: Range::new(3, 4),
            },
            LineRange::Same {
                r1: Range::new(4, 7),
                r2: Range::new(4, 7),
            },
        ],
    };

    let regex = Regex::new(r"^class ").unwrap();
    filter_by_regex(&mut result, &regex);
    let results = vec![result];
    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, false, 2, 1);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["summary"]["duplicate_groups"], 1);
    let locations = &parsed["duplicates"][0]["locations"];
    assert_eq!(locations.as_array().unwrap().len(), 2);
    // The match should start at line 5 (1-indexed)
    assert_eq!(locations[0]["start"], 5);
}

#[test]
fn test_format_json_regex_filters_all() {
    let (f1, f2) = make_mixed_files();
    let mut result = ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    };

    let regex = Regex::new(r"^struct ").unwrap();
    filter_by_regex(&mut result, &regex);
    let results = vec![result];
    let groups = group_duplicates(&results, 3);
    let output = format_json(&groups, &results, false, 2, 1);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["summary"]["duplicate_groups"], 0);
    assert!(parsed["duplicates"].as_array().unwrap().is_empty());
}

#[test]
fn test_format_text_regex_verbose_shows_content() {
    let (f1, f2) = make_mixed_files();
    let mut result = ComparisonResult {
        f1: &f1,
        f2: &f2,
        runs: vec![LineRange::Same {
            r1: Range::new(0, 3),
            r2: Range::new(0, 3),
        }],
    };

    let regex = Regex::new(r"^def ").unwrap();
    filter_by_regex(&mut result, &regex);
    let results = vec![result];
    let groups = group_duplicates(&results, 3);
    let output = format_text(&groups, &results, true, 2, 1);

    assert!(output.contains("def hello():"));
    assert!(output.contains("print(\"Hello\")"));
}
