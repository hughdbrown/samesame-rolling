//! Output formatting for duplicate detection results.

use serde::Serialize;

use crate::grouping::{DuplicateGroup, GroupInfo, LocationInfo};
use crate::types::ComparisonResult;

/// Summary statistics for JSON output.
#[derive(Serialize)]
pub struct Summary {
    pub files_analyzed: usize,
    pub pairs_compared: usize,
    pub duplicate_groups: usize,
    pub total_duplicate_lines: usize,
}

/// Complete JSON output structure.
#[derive(Serialize)]
pub struct JsonOutput {
    pub version: String,
    pub summary: Summary,
    pub duplicates: Vec<GroupInfo>,
}

/// Format results as human-readable text.
pub fn format_text(
    groups: &[DuplicateGroup],
    results: &[ComparisonResult<'_>],
    verbose: bool,
    files_count: usize,
    pairs_count: usize,
) -> String {
    let mut output = String::new();

    if groups.is_empty() {
        output.push_str("No duplicate code found.\n");
        return output;
    }

    output.push_str("=== Duplicate Code Found ===\n\n");

    let mut total_duplicate_lines = 0usize;

    for group in groups {
        total_duplicate_lines += group.line_count;

        output.push_str(&format!(
            "{} lines duplicated across {} files:\n",
            group.line_count,
            group.locations.len(),
        ));

        for (path, start, end) in &group.locations {
            output.push_str(&format!(
                "  {}  lines {}-{}\n",
                path.display(),
                start + 1,
                end,
            ));
        }

        if verbose {
            // Show the duplicated content once, from the first source
            let result = &results[group.source_result_index];
            let (file, r1_start) = {
                // Use f1's lines from the source result
                (&result.f1.lines, group.locations[0].1)
            };
            let r1_end = group.locations[0].2;

            output.push('\n');
            for i in r1_start..r1_end {
                if i < file.len() {
                    output.push_str(&format!("  {:>4} | {}\n", i + 1, file[i]));
                }
            }
        }

        output.push_str("\n---\n\n");
    }

    output.push_str(&format!(
        "Summary: {} files analyzed, {} pairs compared, {} duplicate groups ({} lines)\n",
        files_count,
        pairs_count,
        groups.len(),
        total_duplicate_lines,
    ));

    output
}

/// Format results as JSON.
pub fn format_json(
    groups: &[DuplicateGroup],
    results: &[ComparisonResult<'_>],
    verbose: bool,
    files_count: usize,
    pairs_count: usize,
) -> String {
    let mut total_duplicate_lines = 0usize;

    let duplicates: Vec<GroupInfo> = groups
        .iter()
        .map(|group| {
            total_duplicate_lines += group.line_count;

            let locations: Vec<LocationInfo> = group
                .locations
                .iter()
                .map(|(path, start, end)| LocationInfo {
                    file: path.display().to_string(),
                    start: start + 1, // 1-based for display
                    end: *end,
                })
                .collect();

            let content = if verbose {
                let result = &results[group.source_result_index];
                let (start, end) = (group.locations[0].1, group.locations[0].2);
                Some(result.f1.lines[start..end].to_vec())
            } else {
                None
            };

            GroupInfo {
                lines: group.line_count,
                locations,
                content,
            }
        })
        .collect();

    let output = JsonOutput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        summary: Summary {
            files_analyzed: files_count,
            pairs_compared: pairs_count,
            duplicate_groups: duplicates.len(),
            total_duplicate_lines,
        },
        duplicates,
    };

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}
