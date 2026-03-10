# PRD: Replace Patience Diff with Rolling Hash

## Overview

Replace the patience diff algorithm and union-find grouping with a rolling XOR hash approach for finding duplicate code blocks across files. Instead of O(n^2) pairwise file comparison followed by union-find consolidation, compute rolling hash fingerprints over windows of `min_match` consecutive lines and use a hash table for O(1) duplicate lookup. This eliminates ~600 lines of complex diff/LCS/Hirschberg/union-find code and replaces it with a simpler, more direct algorithm.

## Goals

- Replace O(n^2) pairwise diff with O(n) per-file rolling hash + O(1) hash table grouping
- Eliminate `diff.rs` (patience diff, LCS, Hirschberg, LIS, merge_runs)
- Eliminate `union_find.rs` (union-find data structure)
- Eliminate `LineRange` and `ComparisonResult` types (pairwise diff artifacts)
- Maintain identical CLI interface and output format (text and JSON)
- Maintain identical exit codes (0/1/2)
- Support all existing features: regex filtering, verbose output, min_match threshold
- Detect matches longer than `min_match` by merging overlapping blocks
- Comprehensive test coverage for the new algorithm

## Non-Goals

- Changing the CLI interface or adding new flags
- Adding new output formats
- Handling blank line omission (deferred per user specification)
- Verifying hash collisions against actual content (existing design trade-off)
- Order-dependent hashing (XOR is commutative; accepted trade-off)
- Splitting into multiple crates (separate effort)

## Acceptance Criteria (as test descriptions)

1. `test_rolling_hash_basic` — A file with 10 lines and min_match=5 produces 6 rolling hash blocks (positions 0-4, 1-5, ..., 5-9).
2. `test_rolling_hash_short_file` — A file with fewer than min_match lines produces zero blocks.
3. `test_rolling_hash_exact_length` — A file with exactly min_match lines produces exactly 1 block.
4. `test_rolling_hash_xor_correctness` — The rolling hash at position i equals XOR of line hashes[i..i+min_match].
5. `test_rolling_hash_sliding` — Each successive hash correctly removes the outgoing line and adds the incoming line.
6. `test_file_registry_assigns_numbers` — Files are assigned sequential 0-based numbers. Looking up by number returns the correct path.
7. `test_file_registry_dedup` — The same filename inserted twice gets the same number.
8. `test_grouping_two_files_same_block` — Two files sharing an identical min_match-length block appear in the same hash group.
9. `test_grouping_no_match` — Two files with no shared blocks produce zero groups.
10. `test_grouping_three_files` — Three files sharing the same block produce one group with three locations.
11. `test_grouping_below_threshold` — Shared blocks shorter than min_match are not reported.
12. `test_merge_consecutive_blocks` — Two overlapping blocks (offset by 1) between the same files merge into a single region of min_match+1 lines.
13. `test_merge_long_run` — A run of L consecutive matching blocks merges into a single region of min_match+L-1 lines.
14. `test_merge_gap_no_merge` — Non-consecutive matching blocks remain separate groups.
15. `test_merge_three_files` — Overlapping blocks across 3 files merge correctly into one extended group.
16. `test_merge_partial_overlap` — When file C drops off mid-run, {A,B} extends further while {A,B,C} stops at the shorter match.
17. `test_regex_filter` — Only groups whose first line matches the regex are included in output.
18. `test_verbose_output` — Verbose mode shows line content from the first location.
19. `test_json_output_structure` — JSON output has the same structure as the current format.
20. `test_end_to_end_no_duplicates` — Files with no shared code produce exit code 0.
21. `test_end_to_end_with_duplicates` — Files with shared code produce exit code 1 and formatted output.

## Technical Decisions

### Rolling Hash: XOR of per-line BLAKE3 hashes

Each file already has per-line BLAKE3 hashes (truncated to u64). The rolling hash for a window of `min_match` lines is the XOR of those u64 values.

**Rolling step:** `new_hash = old_hash ^ outgoing_line_hash ^ incoming_line_hash`

**Trade-off:** XOR is commutative (order-independent), so lines `[A,B,C]` and `[C,B,A]` hash the same. Accepted: reordered duplicates are rare in practice, and the tool already accepts hash collisions.

### File Registry: compact file numbering

A `FileRegistry` maps `PathBuf` to `usize` and back. This replaces carrying `PathBuf` through the comparison pipeline. File numbers are assigned in the order files are loaded.

**Implementation:** `HashMap<PathBuf, usize>` + `Vec<PathBuf>`.

### Block Grouping: HashMap<u64, Vec<BlockDescriptor>>

All rolling hash blocks from all files are inserted into a single hash table keyed by block hash. Entries with 2+ descriptors are duplicates. This replaces both pairwise comparison AND union-find — the hash table IS the grouping mechanism.

### Block Merging: file-set + consecutive-start merging

To find matches longer than `min_match`:

1. From each hash group with 2+ entries, extract all pairs of matching locations.
2. Group pairs by `(file_a, file_b, offset)` where offset = `start_b - start_a` and `file_a < file_b`.
3. Within each group, sort start positions and find maximal runs of consecutive integers.
4. Each run of length L represents a duplicate of `min_match + L - 1` lines.
5. For multi-file grouping (3+ files sharing a region): group merged pairs by overlapping file locations. Two pairs share a group if they have a common `(file, start, end)` location.

### Output Adaptation

`DuplicateGroup` loses its `source_result_index` and `source_range_index` fields (no more `ComparisonResult`). Instead, verbose content is retrieved directly from `FileDescription` via the file registry.

The `format_text` and `format_json` functions are adapted to take `&[FileDescription]` instead of `&[ComparisonResult]`.

### What Gets Removed

| Module | Lines | Reason |
|--------|-------|--------|
| `diff.rs` | ~480 | Patience diff, LCS, Hirschberg, LIS, merge_runs — all replaced by rolling hash |
| `union_find.rs` | ~40 | Union-find — replaced by hash table grouping |
| `types.rs: LineRange` | ~40 | Pairwise diff artifact — no longer needed |
| `types.rs: ComparisonResult` | ~25 | Pairwise diff artifact — no longer needed |
| `grouping.rs` (partial) | ~100 | Union-find-based grouping logic — replaced |
| `tests/test_diff.rs` | ~600 | Tests for removed algorithm |

## Design and Operation

### User Perspective

No change. Same CLI, same flags, same output format, same exit codes. The user runs:

```bash
cargo run -- -d . -g "**/*.rs" -m 5
```

And gets the same kinds of results. Matches may differ slightly due to algorithm differences (rolling hash finds exact block matches; patience diff found aligned regions).

### System Perspective: New Pipeline

```
CLI args → discover_files() → paths
         → read_file_if_text() in parallel → Vec<FileDescription>
         → FileRegistry assigns file numbers
         → compute_rolling_hashes() per file → Vec<BlockDescriptor>
         → group into HashMap<u64, Vec<BlockDescriptor>>
         → filter: keep groups with 2+ entries
         → merge_overlapping_blocks() → Vec<DuplicateGroup>
         → apply regex filter
         → format_text() or format_json()
         → print, exit with code
```

### Data Flow Detail

1. **File reading** (unchanged): Each file → `FileDescription { filename, hashes: Vec<u64>, lines: Vec<String> }`

2. **File numbering**: `FileRegistry` assigns each file a `usize`. Files with fewer than `min_match` lines are excluded from block generation (but kept in the registry for numbering consistency).

3. **Rolling hash generation**: For file `f` with `N` lines and `min_match = M`:
   - If `N < M`: skip, no blocks
   - First block: `hash = hashes[0] ^ hashes[1] ^ ... ^ hashes[M-1]`
   - Block i (for i = 1..N-M): `hash = prev_hash ^ hashes[i-1] ^ hashes[i+M-1]`
   - Each block: `BlockDescriptor { hash: u64, file_num: usize, start: usize, end: usize }`
   - `end = start + M` (half-open, consistent with existing `Range`)

4. **Grouping**: Insert all blocks into `HashMap<u64, Vec<BlockDescriptor>>`. Filter to entries with `len() >= 2`.

5. **Merging**: For each group, generate match pairs. Group by `(file_a, file_b, offset)`. Find consecutive runs. Convert to `DuplicateGroup`.

6. **Multi-file grouping**: After pairwise merging, consolidate groups that share a common `(file, start, end)` location into a single group with all participating files.

7. **Regex filtering**: For each `DuplicateGroup`, check if the first line at the first location matches the regex. If not, discard the group.

8. **Output**: Format and print using the same text/JSON structure as today.

### Error Handling

No new error modes. File read errors, no-files-found, and invalid-glob errors remain unchanged.

### Edge Cases

- **Empty files**: Skipped during file reading (unchanged).
- **Binary files**: Skipped during file reading (unchanged).
- **Files shorter than min_match**: No blocks generated; file cannot participate in any match.
- **Single file**: No duplicates possible (all blocks are from the same file, but self-matches within the same file at different positions ARE detected).
- **Identical files**: Every block matches; merging produces one group covering the entire file.
- **All lines identical**: XOR of identical lines may produce 0 (even count) or the line hash (odd count). Consecutive blocks with the same hash are grouped. This can produce false positives across files with different repeated-line content when min_match is even.
- **Self-duplication within a file**: A block appearing at two positions within the SAME file is a valid duplicate (copy-paste within a file). The algorithm handles this naturally since each block carries its file number and start position.

## Test Strategy

### Unit Tests (new module: `rolling_hash.rs`)
- Rolling hash computation: correct values, sliding window, edge cases
- FileRegistry: assignment, lookup, dedup
- Block grouping: 2 files, 3 files, no matches, threshold filtering
- Block merging: consecutive runs, gaps, multi-file, partial overlap

### Integration Tests (updated: `tests/test_cli.rs`, `tests/test_output.rs`)
- End-to-end with temp files: duplicates found, no duplicates, regex, verbose, JSON
- Exit codes preserved
- Output format unchanged

### Removed Tests
- `tests/test_diff.rs` — entire file (patience diff tests)
- `tests/test_types.rs` — tests for `LineRange` and `ComparisonResult` (removed types)
- `grouping.rs` inline tests — replaced by new rolling hash tests

## Rollback and Safety

This is developed on branch `refactor-rolling-hash`. The feature can be abandoned by switching back to `main`. No database, no configuration file changes, no external API changes. The CLI interface is unchanged, so downstream users (if any) are unaffected.

## Implementation Stages

### Stage 1: FileRegistry + Rolling Hash Computation
- New types: `FileRegistry`, `BlockDescriptor`
- Rolling XOR hash function
- Unit tests for both
- **Deliverable**: `cargo test` passes with new tests alongside existing tests
- ~2 files touched: new `rolling_hash.rs`, update `lib.rs`

### Stage 2: Block Grouping
- Hash table grouping: `HashMap<u64, Vec<BlockDescriptor>>`
- Filter to groups with 2+ entries
- Convert to basic `DuplicateGroup` (min_match-length blocks only, no merging)
- Unit tests
- **Deliverable**: `cargo test` passes
- ~1 file touched: `rolling_hash.rs`

### Stage 3: Block Merging for Extended Matches
- Pair extraction from hash groups
- Grouping by `(file_a, file_b, offset)`
- Consecutive run detection
- Multi-file group consolidation
- Unit tests for all merging scenarios
- **Deliverable**: `cargo test` passes
- ~1 file touched: `rolling_hash.rs`

### Stage 4: Pipeline Integration
- Rewrite `main.rs::run()` to use rolling hash pipeline
- Adapt `output.rs` to work without `ComparisonResult`
- Update `DuplicateGroup` to remove `source_result_index`/`source_range_index`
- Regex filtering on `DuplicateGroup`
- Update integration tests (`test_cli.rs`, `test_output.rs`)
- **Deliverable**: `cargo run -- -d . -g "**/*.rs" -m 5` produces correct output; all tests pass
- ~4 files touched: `main.rs`, `output.rs`, `grouping.rs`, `rolling_hash.rs`

### Stage 5: Remove Old Code + Cleanup
- Delete `diff.rs`, `union_find.rs`
- Remove `LineRange`, `ComparisonResult` from `types.rs`
- Remove `generate_pairs` from `discovery.rs`
- Delete `tests/test_diff.rs`
- Update `tests/test_types.rs` (remove tests for deleted types)
- Update `lib.rs` module declarations
- **Deliverable**: `cargo test` passes, `cargo clippy` clean
- ~6 files touched
