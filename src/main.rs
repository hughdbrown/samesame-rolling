//! samesame - A tool to identify repeated fragments of code across multiple files.

use std::process::ExitCode;

use rayon::prelude::*;

use regex::Regex;

use samesame::cli::{Args, OutputFormat};
use samesame::diff::compare_files;
use samesame::discovery::{discover_files, generate_pairs};
use samesame::error::SameError;
use samesame::file::read_file_if_text;
use samesame::grouping::group_duplicates;
use samesame::output::{format_json, format_text};
use samesame::types::{ComparisonResult, FileDescription, LineRange};

fn main() -> ExitCode {
    let args = Args::parse_args();

    match run(&args) {
        Ok(has_duplicates) => {
            if has_duplicates {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(2)
        }
    }
}

/// Remove `Same` runs whose first line doesn't match the regex,
/// converting them to `Diff` runs so they are ignored by output formatting.
fn filter_runs_by_regex(result: &mut ComparisonResult<'_>, regex: &Regex) {
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

fn run(args: &Args) -> Result<bool, SameError> {
    // Discover files
    let paths = discover_files(
        &args.files,
        args.directory.as_deref(),
        &args.glob_pattern,
    )?;

    if !args.quiet {
        eprintln!("Found {} files to analyze", paths.len());
    }

    // Read files in parallel, filtering out binary and empty files
    let files: Vec<FileDescription> = paths
        .par_iter()
        .filter_map(|path| {
            match read_file_if_text(path) {
                Ok(Some(desc)) => Some(desc),
                Ok(None) => {
                    // Binary or empty file - skip silently
                    None
                }
                Err(e) => {
                    if !args.quiet {
                        eprintln!("Warning: {}", e);
                    }
                    None
                }
            }
        })
        .collect();

    if files.is_empty() {
        return Err(SameError::NoFilesFound);
    }

    if !args.quiet {
        eprintln!("Loaded {} text files", files.len());
    }

    // Generate pairs for comparison
    let pairs = generate_pairs(files.len());
    let pairs_count = pairs.len();

    if !args.quiet {
        eprintln!("Comparing {} file pairs", pairs_count);
    }

    // Compare pairs in parallel
    let mut results: Vec<ComparisonResult<'_>> = pairs
        .par_iter()
        .map(|&(i, j)| compare_files(&files[i], &files[j]))
        .collect();

    // Apply regex filter early to avoid processing filtered-out matches
    if let Some(ref regex) = args.regex {
        for result in &mut results {
            filter_runs_by_regex(result, regex);
        }
    }

    // Group pairwise results into deduplicated groups
    let groups = group_duplicates(&results, args.min_match);
    let has_duplicates = !groups.is_empty();

    // Format and print output
    let output = match args.format {
        OutputFormat::Text => format_text(
            &groups,
            &results,
            args.verbose,
            files.len(),
            pairs_count,
        ),
        OutputFormat::Json => format_json(
            &groups,
            &results,
            args.verbose,
            files.len(),
            pairs_count,
        ),
    };

    println!("{}", output);

    Ok(has_duplicates)
}
