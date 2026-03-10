//! Rolling hash duplicate detection algorithm.
//!
//! Replaces patience diff + union-find with a rolling XOR hash approach.
//! Computes rolling hash fingerprints over windows of `min_match` consecutive
//! lines and uses a hash table for O(1) duplicate block lookup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Maps file paths to compact sequential numbers and back.
///
/// File numbers are 0-based and assigned in registration order.
#[derive(Debug)]
pub struct FileRegistry {
    name_to_num: HashMap<PathBuf, usize>,
    num_to_name: Vec<PathBuf>,
}

impl FileRegistry {
    /// Creates an empty file registry.
    pub fn new() -> Self {
        Self {
            name_to_num: HashMap::new(),
            num_to_name: Vec::new(),
        }
    }

    /// Registers a file path and returns its assigned number.
    /// If the path is already registered, returns the existing number.
    pub fn register(&mut self, path: PathBuf) -> usize {
        if let Some(&num) = self.name_to_num.get(&path) {
            return num;
        }
        let num = self.num_to_name.len();
        self.num_to_name.push(path.clone());
        self.name_to_num.insert(path, num);
        num
    }

    /// Returns the path for a given file number.
    pub fn get_path(&self, file_num: usize) -> &Path {
        &self.num_to_name[file_num]
    }

    /// Returns the number of registered files.
    pub fn len(&self) -> usize {
        self.num_to_name.len()
    }
}

/// A rolling hash block descriptor identifying a window of consecutive lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDescriptor {
    /// XOR of per-line hashes for this window.
    pub hash: u64,
    /// File number from the FileRegistry.
    pub file_num: usize,
    /// Starting line index (0-based, inclusive).
    pub start: usize,
    /// Ending line index (0-based, exclusive).
    pub end: usize,
}

/// Computes rolling XOR hash blocks over a sliding window of `min_match` lines.
///
/// Returns one `BlockDescriptor` per window position. If the file has fewer
/// than `min_match` lines, returns an empty Vec.
pub fn compute_rolling_hashes(
    hashes: &[u64],
    file_num: usize,
    min_match: usize,
) -> Vec<BlockDescriptor> {
    let n: usize = hashes.len();
    if n < min_match || min_match == 0 {
        return Vec::new();
    }

    let num_blocks: usize = n - min_match + 1;
    let mut blocks: Vec<BlockDescriptor> = Vec::with_capacity(num_blocks);

    // Compute the first window's hash
    let mut current_hash: u64 = 0;
    for hash in hashes.iter().take(min_match) {
        current_hash ^= hash;
    }

    blocks.push(BlockDescriptor {
        hash: current_hash,
        file_num,
        start: 0,
        end: min_match,
    });

    // Slide the window: remove outgoing line, add incoming line
    for i in 1..num_blocks {
        current_hash ^= hashes[i - 1]; // remove outgoing
        current_hash ^= hashes[i + min_match - 1]; // add incoming
        blocks.push(BlockDescriptor {
            hash: current_hash,
            file_num,
            start: i,
            end: i + min_match,
        });
    }

    blocks
}

/// A group of file locations that all contain the same duplicated code.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    /// Number of duplicated lines in this region.
    pub line_count: usize,
    /// All file locations containing this duplicate, sorted for deterministic output.
    /// Each entry is (filename, start, end) using 0-based half-open ranges.
    pub locations: Vec<(PathBuf, usize, usize)>,
    /// Pre-populated line content for verbose output (None if not verbose).
    pub content: Option<Vec<String>>,
}

/// Groups block descriptors by hash, keeping only groups with 2+ entries.
///
/// Returns a map from block hash to the list of block descriptors sharing
/// that hash. Single-entry groups (no duplicates) are filtered out.
pub fn group_blocks(blocks: Vec<BlockDescriptor>) -> HashMap<u64, Vec<BlockDescriptor>> {
    let mut groups: HashMap<u64, Vec<BlockDescriptor>> = HashMap::new();
    for block in blocks {
        groups.entry(block.hash).or_default().push(block);
    }
    // Keep only groups with 2+ entries (actual duplicates)
    groups.retain(|_, v| v.len() >= 2);
    groups
}

/// Converts hash groups into basic `DuplicateGroup` values (no merging).
///
/// Each hash group with 2+ entries becomes one `DuplicateGroup` with
/// `min_match` lines. Groups are sorted by line count descending, then
/// by first location for deterministic output.
pub fn blocks_to_duplicate_groups(
    groups: &HashMap<u64, Vec<BlockDescriptor>>,
    registry: &FileRegistry,
    min_match: usize,
) -> Vec<DuplicateGroup> {
    let mut result: Vec<DuplicateGroup> = groups
        .values()
        .map(|blocks| {
            let mut locations: Vec<(PathBuf, usize, usize)> = blocks
                .iter()
                .map(|b| (registry.get_path(b.file_num).to_path_buf(), b.start, b.end))
                .collect();
            locations.sort();

            DuplicateGroup {
                line_count: min_match,
                locations,
                content: None,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.line_count
            .cmp(&a.line_count)
            .then_with(|| a.locations.cmp(&b.locations))
    });

    result
}

/// Key identifying a pair of files at a particular alignment offset.
/// `file_a < file_b` for cross-file pairs, `file_a == file_b` for self-duplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MatchPairKey {
    file_a: usize,
    file_b: usize,
    offset: isize,
}

/// Extracts match pairs from hash groups, grouped by (file_a, file_b, offset).
///
/// For each hash group with 2+ entries, generates all pairs of matching
/// locations. Each pair is keyed by the two file numbers and their relative
/// offset (start_b - start_a). The value is a list of start positions in file_a.
fn extract_match_pairs(
    groups: &HashMap<u64, Vec<BlockDescriptor>>,
) -> HashMap<MatchPairKey, Vec<usize>> {
    let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();

    for blocks in groups.values() {
        for i in 0..blocks.len() {
            for j in (i + 1)..blocks.len() {
                let (a, b) = if blocks[i].file_num <= blocks[j].file_num {
                    (&blocks[i], &blocks[j])
                } else {
                    (&blocks[j], &blocks[i])
                };

                let key = MatchPairKey {
                    file_a: a.file_num,
                    file_b: b.file_num,
                    offset: b.start as isize - a.start as isize,
                };

                pairs.entry(key).or_default().push(a.start);
            }
        }
    }

    pairs
}

/// A merged region representing a duplicate span between two files.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MergedRegion {
    file_a: usize,
    start_a: usize,
    file_b: usize,
    start_b: usize,
    line_count: usize,
}

/// Merges consecutive match pairs into extended regions.
///
/// For each (file_a, file_b, offset) group, sorts start positions,
/// deduplicates them, and finds maximal runs of consecutive integers.
/// Each run of length L represents a duplicate of `min_match + L - 1` lines.
fn merge_consecutive_runs(
    pairs: &HashMap<MatchPairKey, Vec<usize>>,
    min_match: usize,
) -> Vec<MergedRegion> {
    let mut regions: Vec<MergedRegion> = Vec::new();

    for (key, starts) in pairs {
        let mut sorted_starts: Vec<usize> = starts.clone();
        sorted_starts.sort_unstable();
        sorted_starts.dedup();

        if sorted_starts.is_empty() {
            continue;
        }

        // Find maximal runs of consecutive integers
        let mut run_start: usize = sorted_starts[0];
        let mut run_end: usize = sorted_starts[0];

        for &s in &sorted_starts[1..] {
            if s == run_end + 1 {
                run_end = s;
            } else {
                // Emit the current run
                let run_len: usize = run_end - run_start + 1;
                let line_count: usize = min_match + run_len - 1;
                regions.push(MergedRegion {
                    file_a: key.file_a,
                    start_a: run_start,
                    file_b: key.file_b,
                    start_b: (run_start as isize + key.offset) as usize,
                    line_count,
                });
                run_start = s;
                run_end = s;
            }
        }

        // Emit the final run
        let run_len: usize = run_end - run_start + 1;
        let line_count: usize = min_match + run_len - 1;
        regions.push(MergedRegion {
            file_a: key.file_a,
            start_a: run_start,
            file_b: key.file_b,
            start_b: (run_start as isize + key.offset) as usize,
            line_count,
        });
    }

    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_registry_assigns_sequential_numbers() {
        let mut reg = FileRegistry::new();
        assert_eq!(reg.register(PathBuf::from("a.rs")), 0);
        assert_eq!(reg.register(PathBuf::from("b.rs")), 1);
        assert_eq!(reg.register(PathBuf::from("c.rs")), 2);
    }

    #[test]
    fn test_file_registry_get_path() {
        let mut reg = FileRegistry::new();
        reg.register(PathBuf::from("src/main.rs"));
        reg.register(PathBuf::from("src/lib.rs"));
        assert_eq!(reg.get_path(0), Path::new("src/main.rs"));
        assert_eq!(reg.get_path(1), Path::new("src/lib.rs"));
    }

    #[test]
    fn test_file_registry_dedup() {
        let mut reg = FileRegistry::new();
        let first = reg.register(PathBuf::from("a.rs"));
        let second = reg.register(PathBuf::from("a.rs"));
        assert_eq!(first, second);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_file_registry_len() {
        let mut reg = FileRegistry::new();
        assert_eq!(reg.len(), 0);
        reg.register(PathBuf::from("a.rs"));
        assert_eq!(reg.len(), 1);
        reg.register(PathBuf::from("b.rs"));
        assert_eq!(reg.len(), 2);
        reg.register(PathBuf::from("a.rs")); // duplicate
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn test_rolling_hash_single_block() {
        let hashes: Vec<u64> = vec![1, 2, 3, 4, 5];
        let blocks = compute_rolling_hashes(&hashes, 0, 5);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].hash, 1 ^ 2 ^ 3 ^ 4 ^ 5);
        assert_eq!(blocks[0].file_num, 0);
        assert_eq!(blocks[0].start, 0);
        assert_eq!(blocks[0].end, 5);
    }

    #[test]
    fn test_rolling_hash_multiple_blocks() {
        let hashes: Vec<u64> = vec![10, 20, 30, 40, 50, 60, 70, 80];
        let blocks = compute_rolling_hashes(&hashes, 0, 5);
        assert_eq!(blocks.len(), 4); // 8 - 5 + 1 = 4
        assert_eq!(blocks[0].hash, 10 ^ 20 ^ 30 ^ 40 ^ 50);
        assert_eq!(blocks[1].hash, 20 ^ 30 ^ 40 ^ 50 ^ 60);
        assert_eq!(blocks[2].hash, 30 ^ 40 ^ 50 ^ 60 ^ 70);
        assert_eq!(blocks[3].hash, 40 ^ 50 ^ 60 ^ 70 ^ 80);
        assert_eq!(blocks[0].start, 0);
        assert_eq!(blocks[1].start, 1);
        assert_eq!(blocks[2].start, 2);
        assert_eq!(blocks[3].start, 3);
    }

    #[test]
    fn test_rolling_hash_short_file() {
        let hashes: Vec<u64> = vec![1, 2, 3];
        let blocks = compute_rolling_hashes(&hashes, 0, 5);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_rolling_hash_sliding_correctness() {
        let hashes: Vec<u64> = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11];
        let min_match: usize = 5;
        let blocks = compute_rolling_hashes(&hashes, 0, min_match);

        // Verify each block's hash equals manual XOR of the window
        for block in &blocks {
            let expected: u64 = hashes[block.start..block.end]
                .iter()
                .fold(0u64, |acc, &h| acc ^ h);
            assert_eq!(block.hash, expected, "Block at start={} has wrong hash", block.start);
        }
    }

    #[test]
    fn test_rolling_hash_empty_file() {
        let hashes: Vec<u64> = vec![];
        let blocks = compute_rolling_hashes(&hashes, 0, 5);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_group_blocks_two_files_match() {
        // Two files with identical 5-line content
        let shared: Vec<u64> = vec![10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 1, 5));

        let groups = group_blocks(all_blocks);
        assert_eq!(groups.len(), 1);
        let group = groups.values().next().unwrap();
        assert_eq!(group.len(), 2);
        assert_eq!(group[0].file_num, 0);
        assert_eq!(group[1].file_num, 1);
    }

    #[test]
    fn test_group_blocks_no_match() {
        let hashes_a: Vec<u64> = vec![1, 2, 3, 4, 5];
        let hashes_b: Vec<u64> = vec![6, 7, 8, 9, 10];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&hashes_a, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&hashes_b, 1, 5));

        let groups = group_blocks(all_blocks);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_blocks_three_files() {
        let shared: Vec<u64> = vec![10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 1, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 2, 5));

        let groups = group_blocks(all_blocks);
        assert_eq!(groups.len(), 1);
        let group = groups.values().next().unwrap();
        assert_eq!(group.len(), 3);
    }

    #[test]
    fn test_group_blocks_multiple_groups() {
        // Two distinct shared blocks
        let shared_a: Vec<u64> = vec![10, 20, 30, 40, 50];
        let shared_b: Vec<u64> = vec![60, 70, 80, 90, 100];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        // File 0 has block A, file 1 has block A, file 2 has block B, file 3 has block B
        all_blocks.extend(compute_rolling_hashes(&shared_a, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_a, 1, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_b, 2, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_b, 3, 5));

        let groups = group_blocks(all_blocks);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_basic_duplicate_groups() {
        let shared: Vec<u64> = vec![10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 1, 5));

        let mut reg = FileRegistry::new();
        reg.register(PathBuf::from("a.rs"));
        reg.register(PathBuf::from("b.rs"));

        let groups = group_blocks(all_blocks);
        let dup_groups = blocks_to_duplicate_groups(&groups, &reg, 5);
        assert_eq!(dup_groups.len(), 1);
        assert_eq!(dup_groups[0].line_count, 5);
        assert_eq!(dup_groups[0].locations.len(), 2);
        // Locations should be sorted: a.rs before b.rs
        assert_eq!(dup_groups[0].locations[0].0, PathBuf::from("a.rs"));
        assert_eq!(dup_groups[0].locations[1].0, PathBuf::from("b.rs"));
    }

    #[test]
    fn test_basic_duplicate_groups_sorted() {
        // Create two distinct shared blocks with different hash values
        let shared_a: Vec<u64> = vec![10, 20, 30, 40, 50];
        let shared_b: Vec<u64> = vec![60, 70, 80, 90, 100];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared_a, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_a, 1, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_b, 2, 5));
        all_blocks.extend(compute_rolling_hashes(&shared_b, 3, 5));

        let mut reg = FileRegistry::new();
        reg.register(PathBuf::from("a.rs"));
        reg.register(PathBuf::from("b.rs"));
        reg.register(PathBuf::from("c.rs"));
        reg.register(PathBuf::from("d.rs"));

        let groups = group_blocks(all_blocks);
        let dup_groups = blocks_to_duplicate_groups(&groups, &reg, 5);
        assert_eq!(dup_groups.len(), 2);
        // Both have same line_count=5, so sorted by first location
        assert!(dup_groups[0].locations[0] <= dup_groups[1].locations[0]);
    }

    #[test]
    fn test_basic_duplicate_groups_empty() {
        let groups: HashMap<u64, Vec<BlockDescriptor>> = HashMap::new();
        let reg = FileRegistry::new();
        let dup_groups = blocks_to_duplicate_groups(&groups, &reg, 5);
        assert!(dup_groups.is_empty());
    }

    #[test]
    fn test_merge_consecutive_two_blocks() {
        let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 10 };
        pairs.insert(key, vec![0, 1]);

        let regions = merge_consecutive_runs(&pairs, 5);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].line_count, 6); // 5 + 2 - 1
        assert_eq!(regions[0].start_a, 0);
        assert_eq!(regions[0].start_b, 10);
    }

    #[test]
    fn test_merge_consecutive_long_run() {
        let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 0 };
        pairs.insert(key, vec![0, 1, 2, 3, 4]);

        let regions = merge_consecutive_runs(&pairs, 5);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].line_count, 9); // 5 + 5 - 1
    }

    #[test]
    fn test_merge_gap_produces_two_regions() {
        let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 0 };
        pairs.insert(key, vec![0, 1, 5, 6]);

        let regions = merge_consecutive_runs(&pairs, 5);
        assert_eq!(regions.len(), 2);
        // First run: starts [0, 1] → 6 lines
        let r1 = regions.iter().find(|r| r.start_a == 0).unwrap();
        assert_eq!(r1.line_count, 6);
        // Second run: starts [5, 6] → 6 lines
        let r2 = regions.iter().find(|r| r.start_a == 5).unwrap();
        assert_eq!(r2.line_count, 6);
    }

    #[test]
    fn test_merge_single_block() {
        let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 5 };
        pairs.insert(key, vec![3]);

        let regions = merge_consecutive_runs(&pairs, 5);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].line_count, 5); // min_match
        assert_eq!(regions[0].start_a, 3);
        assert_eq!(regions[0].start_b, 8);
    }

    #[test]
    fn test_merge_duplicates_in_starts() {
        let mut pairs: HashMap<MatchPairKey, Vec<usize>> = HashMap::new();
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 0 };
        // Duplicate start positions (can happen from multiple hash groups)
        pairs.insert(key, vec![0, 0, 1, 1, 2]);

        let regions = merge_consecutive_runs(&pairs, 5);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].line_count, 7); // 5 + 3 - 1
    }

    #[test]
    fn test_extract_match_pairs_two_files() {
        let shared: Vec<u64> = vec![10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 1, 5));

        let groups = group_blocks(all_blocks);
        let pairs = extract_match_pairs(&groups);

        // One pair: (file 0, file 1, offset 0), with start position [0]
        assert_eq!(pairs.len(), 1);
        let key = MatchPairKey { file_a: 0, file_b: 1, offset: 0 };
        assert!(pairs.contains_key(&key));
        assert_eq!(pairs[&key], vec![0]);
    }

    #[test]
    fn test_extract_match_pairs_three_files() {
        let shared: Vec<u64> = vec![10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&shared, 0, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 1, 5));
        all_blocks.extend(compute_rolling_hashes(&shared, 2, 5));

        let groups = group_blocks(all_blocks);
        let pairs = extract_match_pairs(&groups);

        // Three pairs: (0,1), (0,2), (1,2)
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains_key(&MatchPairKey { file_a: 0, file_b: 1, offset: 0 }));
        assert!(pairs.contains_key(&MatchPairKey { file_a: 0, file_b: 2, offset: 0 }));
        assert!(pairs.contains_key(&MatchPairKey { file_a: 1, file_b: 2, offset: 0 }));
    }

    #[test]
    fn test_extract_match_pairs_same_file() {
        // Self-duplication: same block at positions 0 and 6 in the same file
        let hashes: Vec<u64> = vec![10, 20, 30, 40, 50, 99, 10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&hashes, 0, 5));

        let groups = group_blocks(all_blocks);
        let pairs = extract_match_pairs(&groups);

        // Should have a self-pair with file_a == file_b == 0
        let self_pair = pairs.keys().find(|k| k.file_a == 0 && k.file_b == 0);
        assert!(self_pair.is_some());
    }

    #[test]
    fn test_group_blocks_self_duplication() {
        // Same block appears at two positions in the same file
        // File content: [10, 20, 30, 40, 50, 99, 10, 20, 30, 40, 50]
        let hashes: Vec<u64> = vec![10, 20, 30, 40, 50, 99, 10, 20, 30, 40, 50];
        let mut all_blocks: Vec<BlockDescriptor> = Vec::new();
        all_blocks.extend(compute_rolling_hashes(&hashes, 0, 5));

        let groups = group_blocks(all_blocks);
        // The block XOR(10,20,30,40,50) appears at positions 0 and 6
        assert!(!groups.is_empty());
        let matching_group = groups.values().find(|g| {
            g.iter().any(|b| b.start == 0) && g.iter().any(|b| b.start == 6)
        });
        assert!(matching_group.is_some());
    }
}
