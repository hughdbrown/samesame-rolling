//! Grouping of duplicate code regions using union-find.
//!
//! When N files share the same duplicated code region, pairwise comparison
//! produces N*(N-1)/2 results. This module consolidates those pairwise matches
//! into groups where each unique duplicate region appears once with all
//! participating file locations listed together.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::types::{ComparisonResult, LineRange};
use crate::union_find::UnionFind;

/// Uniquely identifies a code region: (filename, start_line, end_line).
/// Uses 0-based half-open ranges matching the internal `Range` type.
type MatchKey = (PathBuf, usize, usize);

/// A group of file locations that all contain the same duplicated code.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    /// Number of duplicated lines in this region.
    pub line_count: usize,
    /// All file locations containing this duplicate, sorted for deterministic output.
    /// Each entry is (filename, start, end) using 0-based half-open ranges.
    pub locations: Vec<(PathBuf, usize, usize)>,
    /// Index of the first ComparisonResult that contributed to this group,
    /// used to retrieve line content for verbose output.
    pub source_result_index: usize,
    /// Index of the first LineRange within that result, for content retrieval.
    pub source_range_index: usize,
}

/// Location within a duplicate group, for JSON output.
#[derive(Serialize)]
pub struct LocationInfo {
    pub file: String,
    pub start: usize,
    pub end: usize,
}

/// A duplicate group for JSON output.
#[derive(Serialize)]
pub struct GroupInfo {
    pub lines: usize,
    pub locations: Vec<LocationInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<String>>,
}

/// Assigns an index to a MatchKey, inserting it if new.
fn get_or_insert_key(
    key: MatchKey,
    keys: &mut Vec<MatchKey>,
    key_index: &mut HashMap<MatchKey, usize>,
    source_info: &mut Vec<(usize, usize)>,
    result_idx: usize,
    range_idx: usize,
) -> usize {
    *key_index.entry(key.clone()).or_insert_with(|| {
        let idx = keys.len();
        keys.push(key);
        source_info.push((result_idx, range_idx));
        idx
    })
}

/// Groups pairwise comparison results into deduplicated groups using union-find.
///
/// Each `Same` run in a `ComparisonResult` connects two (filename, start, end) regions.
/// By unioning all such connections, we discover which regions across all files
/// represent the same duplicated code.
pub fn group_duplicates(
    results: &[ComparisonResult<'_>],
    min_match: usize,
) -> Vec<DuplicateGroup> {
    let mut keys: Vec<MatchKey> = Vec::new();
    let mut key_index: HashMap<MatchKey, usize> = HashMap::new();
    let mut source_info: Vec<(usize, usize)> = Vec::new();

    // First pass: collect all keys and build union-find connections
    let mut unions: Vec<(usize, usize)> = Vec::new();

    for (result_idx, result) in results.iter().enumerate() {
        for (range_idx, run) in result.runs.iter().enumerate() {
            if let LineRange::Same { r1, r2 } = run {
                if r1.len() < min_match {
                    continue;
                }

                let key1: MatchKey = (result.f1.filename.clone(), r1.start, r1.end);
                let key2: MatchKey = (result.f2.filename.clone(), r2.start, r2.end);

                let idx1 = get_or_insert_key(
                    key1,
                    &mut keys,
                    &mut key_index,
                    &mut source_info,
                    result_idx,
                    range_idx,
                );
                let idx2 = get_or_insert_key(
                    key2,
                    &mut keys,
                    &mut key_index,
                    &mut source_info,
                    result_idx,
                    range_idx,
                );

                unions.push((idx1, idx2));
            }
        }
    }

    if keys.is_empty() {
        return Vec::new();
    }

    // Build union-find and apply all unions
    let mut uf = UnionFind::new(keys.len());
    for (a, b) in unions {
        uf.union(a, b);
    }

    // Collect groups by root representative
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..keys.len() {
        let root = uf.find(i);
        groups.entry(root).or_default().push(i);
    }

    // Build DuplicateGroup from each group
    let mut result_groups: Vec<DuplicateGroup> = groups
        .into_values()
        .map(|member_indices| {
            let first_idx = member_indices[0];
            let line_count = keys[first_idx].2 - keys[first_idx].1;

            let mut locations: Vec<(PathBuf, usize, usize)> = member_indices
                .iter()
                .map(|&i| keys[i].clone())
                .collect();
            locations.sort();

            let (src_result, src_range) = source_info[first_idx];

            DuplicateGroup {
                line_count,
                locations,
                source_result_index: src_result,
                source_range_index: src_range,
            }
        })
        .collect();

    // Sort: largest line count first, then by first location for determinism
    result_groups.sort_by(|a, b| {
        b.line_count
            .cmp(&a.line_count)
            .then_with(|| a.locations.cmp(&b.locations))
    });

    result_groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileDescription, Range};

    fn make_file(name: &str, n_lines: usize) -> FileDescription {
        FileDescription {
            filename: PathBuf::from(name),
            hashes: vec![0; n_lines],
            lines: (0..n_lines).map(|i| format!("line {i}")).collect(),
        }
    }

    #[test]
    fn test_two_files_one_match() {
        let f1 = make_file("a.rs", 10);
        let f2 = make_file("b.rs", 10);
        let results = vec![ComparisonResult {
            f1: &f1,
            f2: &f2,
            runs: vec![LineRange::Same {
                r1: Range::new(0, 5),
                r2: Range::new(0, 5),
            }],
        }];

        let groups = group_duplicates(&results, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].line_count, 5);
        assert_eq!(groups[0].locations.len(), 2);
    }

    #[test]
    fn test_three_files_same_region_one_group() {
        let f1 = make_file("a.rs", 10);
        let f2 = make_file("b.rs", 10);
        let f3 = make_file("c.rs", 10);

        let results = vec![
            ComparisonResult {
                f1: &f1,
                f2: &f2,
                runs: vec![LineRange::Same {
                    r1: Range::new(0, 5),
                    r2: Range::new(0, 5),
                }],
            },
            ComparisonResult {
                f1: &f1,
                f2: &f3,
                runs: vec![LineRange::Same {
                    r1: Range::new(0, 5),
                    r2: Range::new(0, 5),
                }],
            },
            ComparisonResult {
                f1: &f2,
                f2: &f3,
                runs: vec![LineRange::Same {
                    r1: Range::new(0, 5),
                    r2: Range::new(0, 5),
                }],
            },
        ];

        let groups = group_duplicates(&results, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].locations.len(), 3);
        assert_eq!(groups[0].line_count, 5);
    }

    #[test]
    fn test_two_independent_groups() {
        let f1 = make_file("a.rs", 20);
        let f2 = make_file("b.rs", 20);

        let results = vec![ComparisonResult {
            f1: &f1,
            f2: &f2,
            runs: vec![
                LineRange::Same {
                    r1: Range::new(0, 5),
                    r2: Range::new(0, 5),
                },
                LineRange::Diff {
                    r1: Range::new(5, 10),
                    r2: Range::new(5, 10),
                },
                LineRange::Same {
                    r1: Range::new(10, 15),
                    r2: Range::new(10, 15),
                },
            ],
        }];

        let groups = group_duplicates(&results, 3);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_no_matches() {
        let f1 = make_file("a.rs", 10);
        let f2 = make_file("b.rs", 10);
        let results = vec![ComparisonResult {
            f1: &f1,
            f2: &f2,
            runs: vec![LineRange::Diff {
                r1: Range::new(0, 10),
                r2: Range::new(0, 10),
            }],
        }];

        let groups = group_duplicates(&results, 3);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_below_threshold() {
        let f1 = make_file("a.rs", 10);
        let f2 = make_file("b.rs", 10);
        let results = vec![ComparisonResult {
            f1: &f1,
            f2: &f2,
            runs: vec![LineRange::Same {
                r1: Range::new(0, 3),
                r2: Range::new(0, 3),
            }],
        }];

        let groups = group_duplicates(&results, 5);
        assert!(groups.is_empty());
    }
}
