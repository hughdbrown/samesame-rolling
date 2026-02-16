//! Integration tests for the CLI binary.

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("samesame"))
}

fn create_duplicate_files(dir: &TempDir) {
    let content = "fn hello() {\n    println!(\"Hello\");\n    let x = 1;\n    let y = 2;\n    let z = x + y;\n}\n";
    fs::write(dir.path().join("a.rs"), content).unwrap();
    fs::write(dir.path().join("b.rs"), content).unwrap();
}

fn create_unique_files(dir: &TempDir) {
    fs::write(dir.path().join("a.rs"), "fn foo() {}\n").unwrap();
    fs::write(dir.path().join("b.rs"), "fn bar() {}\n").unwrap();
}

#[test]
fn test_exit_code_0_no_duplicates() {
    let dir = TempDir::new().unwrap();
    create_unique_files(&dir);

    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "1", "-q"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No duplicate code found"));
}

#[test]
fn test_exit_code_1_duplicates_found() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "3", "-q"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Duplicate Code Found"));
}

#[test]
fn test_exit_code_2_error() {
    let dir = TempDir::new().unwrap();
    // Empty directory with no matching files should error

    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-q"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn test_json_output_format() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "3", "-f", "json", "-q"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"duplicate_groups\""));
}

#[test]
fn test_verbose_output() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "3", "-v", "-q"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("fn hello()"));
}

#[test]
fn test_regex_filter() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    // Regex that matches - should find duplicates
    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "3", "-r", "^fn ", "-q"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Duplicate Code Found"));

    // Regex that doesn't match - should find no duplicates
    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "3", "-r", "^class ", "-q"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No duplicate code found"));
}

#[test]
fn test_min_match_threshold() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    // High threshold - no matches
    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "100", "-q"])
        .assert()
        .code(0);

    // Low threshold - matches found
    cmd()
        .args(["-d", dir.path().to_str().unwrap(), "-m", "2", "-q"])
        .assert()
        .code(1);
}

#[test]
fn test_explicit_file_args() {
    let dir = TempDir::new().unwrap();
    create_duplicate_files(&dir);

    let a = dir.path().join("a.rs");
    let b = dir.path().join("b.rs");

    cmd()
        .args([a.to_str().unwrap(), b.to_str().unwrap(), "-m", "3", "-q"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Duplicate Code Found"));
}

#[test]
fn test_invalid_regex() {
    cmd()
        .args(["-r", "[invalid", "dummy.rs"])
        .assert()
        .code(2);
}

#[test]
fn test_min_match_zero_rejected() {
    cmd()
        .args(["-m", "0", "dummy.rs"])
        .assert()
        .code(2);
}
