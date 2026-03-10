//! samesame - A tool to identify repeated fragments of code across multiple files.

use std::process::ExitCode;

use rayon::prelude::*;

use samesame::cli::{Args, OutputFormat};
use samesame::discovery::discover_files;
use samesame::error::SameError;
use samesame::file::read_file_if_text;
use samesame::output::{format_json, format_text};
use samesame::rolling_hash::{DuplicateGroup, find_duplicates};
use samesame::types::FileDescription;

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

/// Filter duplicate groups by regex, keeping only groups whose first line
/// at the first location matches the pattern.
fn filter_groups_by_regex(
    groups: &mut Vec<DuplicateGroup>,
    regex: &regex::Regex,
    files: &[FileDescription],
    registry: &samesame::rolling_hash::FileRegistry,
) {
    groups.retain(|group| {
        if let Some((path, start, _end)) = group.locations.first() {
            // Find the FileDescription for this path
            for file in files {
                if &file.filename == path && start < &file.lines.len() {
                    return regex.is_match(&file.lines[*start]);
                }
            }
            // If file not found (shouldn't happen), keep the group
            let _ = (registry, path); // suppress unused warning
            true
        } else {
            false
        }
    });
}

/// Populate content for verbose output by looking up lines from FileDescriptions.
fn populate_content(groups: &mut [DuplicateGroup], files: &[FileDescription]) {
    for group in groups.iter_mut() {
        if let Some((path, start, end)) = group.locations.first() {
            for file in files {
                if &file.filename == path {
                    if *end <= file.lines.len() {
                        group.content = Some(file.lines[*start..*end].to_vec());
                    }
                    break;
                }
            }
        }
    }
}

fn run(args: &Args) -> Result<bool, SameError> {
    // Discover files
    let paths = discover_files(&args.files, args.directory.as_deref(), &args.glob_pattern)?;

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

    // Find duplicates using rolling hash
    let (registry, mut groups) = find_duplicates(&files, args.min_match);

    if !args.quiet {
        eprintln!("Found {} duplicate groups", groups.len());
    }

    // Apply regex filter
    if let Some(ref regex) = args.regex {
        filter_groups_by_regex(&mut groups, regex, &files, &registry);
    }

    // Populate content for verbose output
    if args.verbose {
        populate_content(&mut groups, &files);
    }

    let has_duplicates = !groups.is_empty();

    // Format and print output
    let output = match args.format {
        OutputFormat::Text => format_text(&groups, args.verbose, files.len()),
        OutputFormat::Json => format_json(&groups, args.verbose, files.len()),
    };

    println!("{}", output);

    Ok(has_duplicates)
}
