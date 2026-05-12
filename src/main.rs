//! samesame - A tool to identify repeated fragments of code across multiple files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rayon::prelude::*;

use samesame::cli::{Args, OutputFormat};
use samesame::discovery::discover_files;
use samesame::error::SameError;
use samesame::file::{read_file_if_text, read_lines};
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

/// Fetch a file's lines through `cache`, reading from disk on first request.
/// Returns an empty Vec if the file cannot be read — the caller decides what
/// that means (regex: no match, verbose: no content).
fn lines_for<'a>(cache: &'a mut HashMap<PathBuf, Vec<String>>, path: &Path) -> &'a [String] {
    cache
        .entry(path.to_path_buf())
        .or_insert_with(|| read_lines(path).unwrap_or_default())
}

/// Filter duplicate groups by regex, keeping only groups whose first line
/// at the first location matches the pattern. Re-reads matched files lazily.
fn filter_groups_by_regex(groups: &mut Vec<DuplicateGroup>, regex: &regex::Regex) {
    let mut cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    groups.retain(|group| {
        if let Some((path, start, _end)) = group.locations.first() {
            let lines = lines_for(&mut cache, path);
            if *start < lines.len() {
                return regex.is_match(&lines[*start]);
            }
        }
        false
    });
}

/// Populate content for verbose output by re-reading matched files lazily.
fn populate_content(groups: &mut [DuplicateGroup]) {
    let mut cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for group in groups.iter_mut() {
        if let Some((path, start, end)) = group.locations.first() {
            let lines = lines_for(&mut cache, path);
            if *end <= lines.len() {
                group.content = Some(lines[*start..*end].to_vec());
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
    let (_registry, mut groups) = find_duplicates(&files, args.min_match);

    if !args.quiet {
        eprintln!("Found {} duplicate groups", groups.len());
    }

    // Apply regex filter
    if let Some(ref regex) = args.regex {
        filter_groups_by_regex(&mut groups, regex);
    }

    // Populate content for verbose output
    if args.verbose {
        populate_content(&mut groups);
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
