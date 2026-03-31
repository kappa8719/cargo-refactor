use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_dir() -> TempDir {
    tempfile::tempdir().expect("create temp dir")
}

fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

fn read_file(dir: &TempDir, name: &str) -> String {
    fs::read_to_string(dir.path().join(name)).unwrap()
}

use cargo_refactor_lib::{process_file, Rename, RustPath};

// Helper to parse paths and build a rename
fn rename(from: &str, to: &str) -> Rename {
    Rename::new_rename(RustPath::parse(from).unwrap(), RustPath::parse(to).unwrap())
}

fn mv(from: &str, to: &str) -> Rename {
    Rename::new_move(RustPath::parse(from).unwrap(), RustPath::parse(to).unwrap())
}

// --------------------------------------------------------------------------
// use declarations
// --------------------------------------------------------------------------

#[test]
fn rename_simple_use() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::OldName;\nfn foo() {}",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("use a::b::c::NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "got: {out}");
}

#[test]
fn rename_grouped_use() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::{OldName, Other};\nfn foo() {}",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "got: {out}");
    assert!(out.contains("Other"), "got: {out}");
}

#[test]
fn rename_alias_use() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::OldName as Alias;\nfn foo() {}",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("NewName as Alias") || out.contains("NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "got: {out}");
    assert!(out.contains("Alias"), "got: {out}");
}

#[test]
fn rename_nested_grouped_use() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::{c::OldName, d::Other};\nfn foo() {}",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "got: {out}");
    assert!(out.contains("Other"), "got: {out}");
}

// --------------------------------------------------------------------------
// absolute path references
// --------------------------------------------------------------------------

#[test]
fn rename_absolute_path_in_code() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "fn foo(x: a::b::c::OldName) -> a::b::c::OldName { a::b::c::OldName::new() }",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(!out.contains("OldName"), "OldName still present: {out}");
    assert!(out.contains("NewName"), "NewName not found: {out}");
}

// --------------------------------------------------------------------------
// short name usage (rename only)
// --------------------------------------------------------------------------

#[test]
fn rename_short_name_after_use() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::OldName;\nfn foo() { let _x = OldName::new(); }",
    );
    process_file(&dir.path().join("a.rs"), &rename("a::b::c::OldName", "a::b::c::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(!out.contains("OldName"), "OldName still present: {out}");
    assert!(out.contains("NewName::new"), "NewName::new not found: {out}");
}

#[test]
fn move_does_not_rename_short_name() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::Foo;\nfn foo() { let _x = Foo::new(); }",
    );
    // move same name to different module
    process_file(&dir.path().join("a.rs"), &mv("a::b::c::Foo", "x::y::Foo"), false);
    let out = read_file(&dir, "a.rs");
    // use should be updated
    assert!(out.contains("x::y::Foo"), "expected x::y::Foo: {out}");
    // Short name Foo::new() should still be Foo::new() (name unchanged in move-only)
    assert!(out.contains("Foo::new"), "Foo::new should be unchanged: {out}");
}

#[test]
fn move_and_rename_short_name() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::OldName;\nfn foo() { let _x = OldName::new(); }",
    );
    // move with rename
    process_file(&dir.path().join("a.rs"), &mv("a::b::c::OldName", "x::y::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("x::y::NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "OldName still present: {out}");
    assert!(out.contains("NewName::new"), "NewName::new not found: {out}");
}

// --------------------------------------------------------------------------
// dry run
// --------------------------------------------------------------------------

#[test]
fn dry_run_does_not_modify_file() {
    let dir = setup_dir();
    let original = "use a::b::c::OldName;\nfn foo() {}";
    write_file(&dir, "a.rs", original);
    process_file(
        &dir.path().join("a.rs"),
        &rename("a::b::c::OldName", "a::b::c::NewName"),
        true,
    );
    let out = read_file(&dir, "a.rs");
    assert_eq!(out, original, "dry-run should not modify file");
}

// --------------------------------------------------------------------------
// path length change (from has different length than to)
// --------------------------------------------------------------------------

#[test]
fn rename_different_module_depth() {
    let dir = setup_dir();
    write_file(
        &dir,
        "a.rs",
        "use a::b::c::OldName;\nfn foo(x: a::b::c::OldName) {}",
    );
    process_file(&dir.path().join("a.rs"), &mv("a::b::c::OldName", "x::NewName"), false);
    let out = read_file(&dir, "a.rs");
    assert!(out.contains("x::NewName"), "got: {out}");
    assert!(!out.contains("OldName"), "got: {out}");
    assert!(!out.contains("a::b::c"), "got: {out}");
}
