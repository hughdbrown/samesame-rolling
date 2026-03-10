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
}
