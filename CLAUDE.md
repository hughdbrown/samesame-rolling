# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**samesame** is a Rust CLI tool that identifies repeated fragments of code across multiple files using a rolling XOR hash algorithm. It's designed to support refactoring by finding duplicate code regions.

## Build and Test Commands

```bash
# Build
cargo build
cargo build --release

# Run tests
cargo test

# Run a single test
cargo test test_name

# Run tests in a specific module
cargo test --test test_output

# Clippy and formatting
cargo clippy
cargo fmt

# Run the tool
cargo run -- [OPTIONS] [FILES]
cargo run -- -d . -g "**/*.rs" -m 5
```

## Architecture

The application follows a pipeline architecture:

1. **CLI Parsing** (`cli.rs`) → Parse arguments with clap derive macros
2. **File Discovery** (`discovery.rs`) → Find files via explicit paths or glob patterns
3. **File Processing** (`file.rs`) → Read files, normalize lines, compute BLAKE3 hashes
4. **Duplicate Detection** (`rolling_hash.rs`) → Rolling XOR hash to find duplicate regions
5. **Output Formatting** (`output.rs`) → Render results as text or JSON

### Key Data Flow

```
Files → FileDescription (path + line hashes + line content)
     → find_duplicates() computes rolling XOR hashes over sliding windows
     → Groups blocks by hash (O(1) lookup), extracts match pairs
     → Merges consecutive runs into extended regions
     → Consolidates pairwise regions into multi-file DuplicateGroups
     → format_text/format_json renders output
```

### Core Types

- `FileDescription`: Holds file path, per-line BLAKE3 hashes, and original line content
- `FileRegistry`: Maps file paths to compact sequential numbers and back
- `BlockDescriptor`: A rolling hash window (hash, file_num, start, end)
- `DuplicateGroup`: A group of file locations sharing duplicated code

### Rolling Hash Algorithm (`rolling_hash.rs`)

1. Compute rolling XOR hash over windows of `min_match` consecutive line hashes
2. Group all blocks by hash — entries with 2+ blocks are potential duplicates
3. Extract pairwise matches grouped by (file_a, file_b, offset)
4. Merge consecutive match positions into extended regions
5. Consolidate pairwise regions into multi-file groups using union-find

XOR is order-independent, so permuted lines can produce false positives,
but never false negatives. In practice this is rare for real code.

### Parallelism

Uses `rayon` for parallel file reading/hashing. The rolling hash pipeline runs single-threaded. No async runtime.

## CLI Options

- `-m, --match <N>`: Minimum matching lines to report (default: 5)
- `-d, --dir <PATH>`: Directory to scan
- `-g, --glob <PATTERN>`: Glob pattern (default: `**/*.rs`)
- `-f, --format <text|json>`: Output format
- `-v, --verbose`: Show matching line content
- `-q, --quiet`: Suppress warnings
- `-r, --regex <PATTERN>`: Only show matches where first line matches regex

## Exit Codes

- 0: No duplicates found
- 1: Duplicates found
- 2: Error occurred

## Design Decisions

- Binary files are skipped (detected via null bytes in first 8KB)
- Symlinks are skipped to avoid infinite loops
- Empty files are skipped
- Lines are trimmed before hashing but original indentation is preserved in output
- Non-UTF-8 bytes use lossy conversion
- Hash-only comparison (no content verification for hash collisions)
