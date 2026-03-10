//! Tests for file discovery and filtering.

use samesame::discovery::{deduplicate_paths, discover_files, is_hidden, is_symlink, scan_glob};
use samesame::error::SameError;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn test_is_hidden() {
    assert!(is_hidden(Path::new(".hidden")));
    assert!(is_hidden(Path::new("/path/to/.hidden")));
    assert!(!is_hidden(Path::new("visible")));
    assert!(!is_hidden(Path::new("/path/to/visible")));
}

#[test]
fn test_is_hidden_dotfile() {
    assert!(is_hidden(Path::new(".gitignore")));
    assert!(is_hidden(Path::new("/home/user/.bashrc")));
}

#[test]
fn test_is_symlink_regular_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("regular.txt");
    File::create(&file_path).unwrap();

    assert!(!is_symlink(&file_path));
}

#[cfg(unix)]
#[test]
fn test_is_symlink_actual_symlink() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("target.txt");
    File::create(&file_path).unwrap();

    let link_path = temp.path().join("link.txt");
    symlink(&file_path, &link_path).unwrap();

    assert!(is_symlink(&link_path));
}

#[test]
fn test_is_symlink_nonexistent() {
    assert!(!is_symlink(Path::new("/nonexistent/path")));
}

#[test]
fn test_deduplicate_paths_no_duplicates() {
    let paths = vec![
        PathBuf::from("/a/b.rs"),
        PathBuf::from("/a/c.rs"),
        PathBuf::from("/a/d.rs"),
    ];
    let result = deduplicate_paths(paths.clone());
    assert_eq!(result.len(), 3);
}

#[test]
fn test_deduplicate_paths_with_duplicates() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("test.rs");
    File::create(&file_path).unwrap();

    let paths = vec![file_path.clone(), file_path.clone(), file_path.clone()];
    let result = deduplicate_paths(paths);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_deduplicate_paths_empty() {
    let paths: Vec<PathBuf> = vec![];
    let result = deduplicate_paths(paths);
    assert!(result.is_empty());
}

#[test]
fn test_discover_files_explicit_paths() {
    let temp = TempDir::new().unwrap();
    let file1 = temp.path().join("a.rs");
    let file2 = temp.path().join("b.rs");
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    let files = vec![
        file1.to_string_lossy().to_string(),
        file2.to_string_lossy().to_string(),
    ];

    let result = discover_files(&files, None, "*.rs").unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_discover_files_with_directory() {
    let temp = TempDir::new().unwrap();
    let file1 = temp.path().join("a.rs");
    let file2 = temp.path().join("b.rs");
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "*.rs").unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_discover_files_no_files_found() {
    let temp = TempDir::new().unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "*.nonexistent");
    assert!(matches!(result, Err(SameError::NoFilesFound)));
}

#[test]
fn test_discover_files_skips_directories() {
    let temp = TempDir::new().unwrap();
    let subdir = temp.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let file = temp.path().join("test.rs");
    File::create(&file).unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "*.rs").unwrap();

    // Should only find the file, not the directory
    assert_eq!(result.len(), 1);
}

#[test]
fn test_discover_files_skips_hidden() {
    let temp = TempDir::new().unwrap();
    let hidden = temp.path().join(".hidden.rs");
    let visible = temp.path().join("visible.rs");
    File::create(&hidden).unwrap();
    File::create(&visible).unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "*.rs").unwrap();

    // Should only find visible file
    assert_eq!(result.len(), 1);
    assert!(result[0].file_name().unwrap().to_str().unwrap() == "visible.rs");
}

#[test]
fn test_discover_files_nested_glob() {
    let temp = TempDir::new().unwrap();
    let subdir = temp.path().join("src");
    fs::create_dir(&subdir).unwrap();

    let file1 = temp.path().join("root.rs");
    let file2 = subdir.join("nested.rs");
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "**/*.rs").unwrap();

    assert_eq!(result.len(), 2);
}

#[test]
fn test_discover_files_explicit_nonexistent_skipped() {
    let files = vec!["/nonexistent/file.rs".to_string()];
    let result = discover_files(&files, None, "*.rs");

    // Should return error since no valid files were found
    assert!(matches!(result, Err(SameError::NoFilesFound)));
}

#[test]
fn test_discover_files_mixed_explicit_and_directory() {
    let temp = TempDir::new().unwrap();
    let explicit = temp.path().join("explicit.rs");
    let from_glob = temp.path().join("from_glob.rs");
    File::create(&explicit).unwrap();
    File::create(&from_glob).unwrap();

    let files = vec![explicit.to_string_lossy().to_string()];

    let result = discover_files(&files, Some(temp.path().to_str().unwrap()), "*.rs").unwrap();

    // Should deduplicate
    assert_eq!(result.len(), 2);
}

#[test]
fn test_scan_glob_valid_pattern() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("test.rs");
    File::create(&file).unwrap();

    let pattern = format!("{}/*.rs", temp.path().display());
    let result = scan_glob(&pattern).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_scan_glob_no_matches() {
    let temp = TempDir::new().unwrap();
    let pattern = format!("{}/*.nonexistent", temp.path().display());
    let result = scan_glob(&pattern).unwrap();
    assert!(result.is_empty());
}

#[cfg(unix)]
#[test]
fn test_discover_files_skips_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let real_file = temp.path().join("real.rs");
    let link_file = temp.path().join("link.rs");

    File::create(&real_file).unwrap();
    symlink(&real_file, &link_file).unwrap();

    let result = discover_files(&[], Some(temp.path().to_str().unwrap()), "*.rs").unwrap();

    // Should only find real file, not symlink
    assert_eq!(result.len(), 1);
    assert!(result[0].file_name().unwrap() == "real.rs");
}

#[test]
fn test_discover_files_directory_with_trailing_slash() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("test.rs");
    File::create(&file).unwrap();

    let dir_with_slash = format!("{}/", temp.path().display());
    let result = discover_files(&[], Some(&dir_with_slash), "*.rs").unwrap();
    assert_eq!(result.len(), 1);
}
