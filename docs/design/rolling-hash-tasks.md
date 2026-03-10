# Task List: Replace Patience Diff with Rolling Hash

## Stage 1: FileRegistry + Rolling Hash Computation

### Task 1.1: FileRegistry data structure

- **Tests** in `src/rolling_hash.rs` (inline `#[cfg(test)]`):
  - `test_file_registry_assigns_sequential_numbers` — Register 3 files, verify they get numbers 0, 1, 2.
  - `test_file_registry_get_path` — Register files, look up by number, verify correct paths returned.
  - `test_file_registry_dedup` — Register same path twice, verify same number returned and count stays 1.
  - `test_file_registry_len` — Verify `len()` returns the number of unique registered files.
- **Code** in `src/rolling_hash.rs`:
  - `FileRegistry` struct with `name_to_num: HashMap<PathBuf, usize>` and `num_to_name: Vec<PathBuf>`
  - `FileRegistry::new() -> Self`
  - `FileRegistry::register(&mut self, path: PathBuf) -> usize`
  - `FileRegistry::get_path(&self, file_num: usize) -> &Path`
  - `FileRegistry::len(&self) -> usize`
- **Config**: Add `pub mod rolling_hash;` to `src/lib.rs`
- **Verify**: `cargo test test_file_registry`

### Task 1.2: BlockDescriptor type and rolling hash computation

- **Tests** in `src/rolling_hash.rs`:
  - `test_rolling_hash_single_block` — File with exactly `min_match` lines produces 1 block with correct XOR hash.
  - `test_rolling_hash_multiple_blocks` — File with `min_match + 3` lines produces 4 blocks with correct hashes.
  - `test_rolling_hash_short_file` — File with fewer than `min_match` lines produces 0 blocks.
  - `test_rolling_hash_sliding_correctness` — Verify each block's hash equals manual XOR of the corresponding line hashes.
  - `test_rolling_hash_empty_file` — Empty file produces 0 blocks.
- **Code** in `src/rolling_hash.rs`:
  - `BlockDescriptor` struct: `{ hash: u64, file_num: usize, start: usize, end: usize }`
  - `compute_rolling_hashes(hashes: &[u64], file_num: usize, min_match: usize) -> Vec<BlockDescriptor>`
- **Verify**: `cargo test test_rolling_hash`

## Stage 2: Block Grouping

### Task 2.1: Hash table grouping of blocks

- **Tests** in `src/rolling_hash.rs`:
  - `test_group_blocks_two_files_match` — Two files sharing a block produce one group with 2 entries.
  - `test_group_blocks_no_match` — Two files with no shared blocks produce zero groups.
  - `test_group_blocks_three_files` — Three files sharing a block produce one group with 3 entries.
  - `test_group_blocks_multiple_groups` — Two distinct shared blocks produce two groups.
  - `test_group_blocks_self_duplication` — A block appearing twice in the same file produces a group.
- **Code** in `src/rolling_hash.rs`:
  - `group_blocks(blocks: Vec<BlockDescriptor>) -> HashMap<u64, Vec<BlockDescriptor>>`
  - Filters to entries with 2+ descriptors
- **Verify**: `cargo test test_group_blocks`

### Task 2.2: Convert groups to basic DuplicateGroup (no merging)

- **Tests** in `src/rolling_hash.rs`:
  - `test_basic_duplicate_groups` — Grouped blocks convert to DuplicateGroup with correct line_count and locations.
  - `test_basic_duplicate_groups_sorted` — Groups are sorted by line_count descending, then by first location.
  - `test_basic_duplicate_groups_empty` — No groups produces empty Vec.
- **Code** in `src/rolling_hash.rs`:
  - `blocks_to_duplicate_groups(groups: HashMap<u64, Vec<BlockDescriptor>>, registry: &FileRegistry) -> Vec<DuplicateGroup>`
  - Converts each hash group to a DuplicateGroup with locations resolved to PathBuf via registry
  - Uses a new DuplicateGroup variant without source_result_index/source_range_index
- **Verify**: `cargo test test_basic_duplicate`

## Stage 3: Block Merging for Extended Matches

### Task 3.1: Pair extraction and offset grouping

- **Tests** in `src/rolling_hash.rs`:
  - `test_extract_match_pairs` — A hash group with entries from files A and B produces pair (A, B, offset) with correct start positions.
  - `test_extract_match_pairs_three_files` — A hash group with 3 files produces 3 pairs: (A,B), (A,C), (B,C).
  - `test_extract_match_pairs_same_file` — Entries from the same file at different positions produce a self-pair.
- **Code** in `src/rolling_hash.rs`:
  - `MatchPairKey { file_a: usize, file_b: usize, offset: isize }` (where file_a < file_b, or file_a == file_b for self-duplication)
  - `extract_match_pairs(groups: &HashMap<u64, Vec<BlockDescriptor>>) -> HashMap<MatchPairKey, Vec<usize>>`
  - For each hash group with 2+ entries, for each pair of entries, record (file_a, file_b, offset) → start_a
- **Verify**: `cargo test test_extract_match_pairs`

### Task 3.2: Consecutive run detection and region merging

- **Tests** in `src/rolling_hash.rs`:
  - `test_merge_consecutive_two_blocks` — Start positions [0, 1] with min_match=5 merge into region of 6 lines.
  - `test_merge_consecutive_long_run` — Start positions [0, 1, 2, 3, 4] with min_match=5 merge into region of 9 lines.
  - `test_merge_gap_produces_two_regions` — Start positions [0, 1, 5, 6] produce two separate regions.
  - `test_merge_single_block` — A single start position produces one region of min_match lines.
  - `test_merge_duplicates_in_starts` — Duplicate start positions are deduplicated before merging.
- **Code** in `src/rolling_hash.rs`:
  - `MergedRegion { file_a: usize, start_a: usize, file_b: usize, start_b: usize, line_count: usize }`
  - `merge_consecutive_runs(pairs: &HashMap<MatchPairKey, Vec<usize>>, min_match: usize) -> Vec<MergedRegion>`
  - Sorts starts, deduplicates, finds maximal consecutive runs, computes region length
- **Verify**: `cargo test test_merge_consecutive`

### Task 3.3: Multi-file group consolidation

- **Tests** in `src/rolling_hash.rs`:
  - `test_consolidate_two_files` — A single merged region between A and B produces one DuplicateGroup with 2 locations.
  - `test_consolidate_three_files` — Merged regions (A,B) and (A,C) sharing A's range produce one DuplicateGroup with 3 locations.
  - `test_consolidate_independent_groups` — Two merged regions with no shared locations produce two DuplicateGroups.
  - `test_consolidate_partial_overlap` — When A:0-8 matches B:10-18 (8 lines) and A:0-6 matches C:20-26 (6 lines), produces two groups: one for the 6-line match {A,B,C} and one for the remaining 2 lines {A,B} only. (Or simpler: keeps them as separate groups.)
- **Code** in `src/rolling_hash.rs`:
  - `consolidate_regions(regions: Vec<MergedRegion>, registry: &FileRegistry) -> Vec<DuplicateGroup>`
  - Groups MergedRegions that share a common (file, start, end) location
  - Converts file numbers to paths via registry
  - Sorts groups by line_count descending, then by first location
- **Verify**: `cargo test test_consolidate`

### Task 3.4: Top-level find_duplicates function

- **Tests** in `src/rolling_hash.rs`:
  - `test_find_duplicates_basic` — Two files with 5 identical lines produces one group.
  - `test_find_duplicates_extended` — Two files sharing 8 identical lines produces one group of 8 lines (not 4 groups of 5).
  - `test_find_duplicates_no_match` — Files with no shared content produce empty result.
  - `test_find_duplicates_below_threshold` — Shared content shorter than min_match produces empty result.
  - `test_find_duplicates_three_files` — Three files sharing content produce one group with 3 locations.
  - `test_find_duplicates_self_duplication` — A file with repeated blocks internally produces a group.
- **Code** in `src/rolling_hash.rs`:
  - `find_duplicates(files: &[FileDescription], min_match: usize) -> (FileRegistry, Vec<DuplicateGroup>)`
  - Orchestrates: register files → compute rolling hashes → group blocks → extract pairs → merge runs → consolidate → return
- **Verify**: `cargo test test_find_duplicates`

## Stage 4: Pipeline Integration

### Task 4.1: Adapt DuplicateGroup and output functions

- **Tests**: Update `tests/test_output.rs` to work with new function signatures (no ComparisonResult).
- **Code**:
  - Update `DuplicateGroup` in `grouping.rs`: remove `source_result_index` and `source_range_index`, add `content: Option<Vec<String>>` for verbose mode
  - Update `format_text()` signature: replace `results: &[ComparisonResult]` with direct group data
  - Update `format_json()` signature: same change
  - Both functions read content from `DuplicateGroup.content` instead of indexing into ComparisonResult
- **Verify**: `cargo test --test test_output`

### Task 4.2: Rewrite main.rs pipeline

- **Code** in `main.rs`:
  - Replace pairwise comparison pipeline with: `find_duplicates(&files, args.min_match)`
  - Populate `DuplicateGroup.content` when verbose mode is on (using file registry + FileDescription)
  - Apply regex filtering on DuplicateGroup (check first line of first location)
  - Update `filter_runs_by_regex` → `filter_groups_by_regex`
  - Remove imports of `compare_files`, `generate_pairs`, `ComparisonResult`, `LineRange`
- **Tests**: Update `tests/test_cli.rs` integration tests if needed.
- **Verify**: `cargo test` (all tests pass)

### Task 4.3: Verify end-to-end behavior

- **Tests**: Run existing CLI integration tests to verify identical behavior:
  - `cargo test --test test_cli`
  - Manual test: `cargo run -- -d src -g "**/*.rs" -m 5` produces sensible output
- **Verify**: All tests green, output matches expected format

## Stage 5: Remove Old Code + Cleanup

### Task 5.1: Delete obsolete modules and types

- **Code**:
  - Delete `src/diff.rs`
  - Delete `src/union_find.rs`
  - Remove `LineRange` and `ComparisonResult` from `src/types.rs`
  - Remove `generate_pairs` from `src/discovery.rs`
  - Remove `pub mod diff;` and `pub mod union_find;` from `src/lib.rs`
  - Remove old grouping logic from `src/grouping.rs` (keep DuplicateGroup, GroupInfo, LocationInfo types)
- **Verify**: `cargo check`

### Task 5.2: Delete and update test files

- **Code**:
  - Delete `tests/test_diff.rs`
  - Update `tests/test_types.rs`: remove tests for LineRange and ComparisonResult
  - Update `tests/test_discovery.rs`: remove tests for generate_pairs
- **Verify**: `cargo test`

### Task 5.3: Final cleanup and audit

- Run `cargo clippy` and fix any warnings
- Run `cargo fmt`
- Verify all tests pass: `cargo test`
- Verify release build: `cargo build --release`
- Review for any dead code, unused imports, or stale comments
- **Verify**: `cargo clippy -- -D warnings && cargo test && cargo build --release`
