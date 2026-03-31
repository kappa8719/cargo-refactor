use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::rename::Rename;
use crate::rewriter::{Rewriter, Warning};

pub struct FileResult {
    pub path: PathBuf,
    pub outcome: FileOutcome,
    pub warnings: Vec<Warning>,
}

pub enum FileOutcome {
    Modified { changes: Vec<String> },
    Unchanged,
    Skipped { reason: String },
    WouldModify { changes: Vec<String> },
}

pub fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|s| s.to_str()) == Some("rs")
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn process_file(path: &Path, rename: &Rename, dry_run: bool) -> FileResult {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return FileResult {
                path: path.to_path_buf(),
                outcome: FileOutcome::Skipped {
                    reason: format!("read error: {e}"),
                },
                warnings: Vec::new(),
            };
        }
    };

    let mut ast = match syn::parse_file(&source) {
        Ok(ast) => ast,
        Err(e) => {
            return FileResult {
                path: path.to_path_buf(),
                outcome: FileOutcome::Skipped {
                    reason: format!("syn parse error: {e}"),
                },
                warnings: Vec::new(),
            };
        }
    };

    let mut rw = Rewriter::new(rename);
    rw.rewrite_file(&mut ast);

    let warnings = rw.warnings.clone();

    if !rw.changed {
        return FileResult {
            path: path.to_path_buf(),
            outcome: FileOutcome::Unchanged,
            warnings,
        };
    }

    let new_source = prettyplease::unparse(&ast);

    // Build a simple diff summary (line-level)
    let changes = summarize_changes(&source, &new_source);

    if dry_run {
        return FileResult {
            path: path.to_path_buf(),
            outcome: FileOutcome::WouldModify { changes },
            warnings,
        };
    }

    if let Err(e) = std::fs::write(path, &new_source) {
        return FileResult {
            path: path.to_path_buf(),
            outcome: FileOutcome::Skipped {
                reason: format!("write error: {e}"),
            },
            warnings,
        };
    }

    FileResult {
        path: path.to_path_buf(),
        outcome: FileOutcome::Modified { changes },
        warnings,
    }
}

fn summarize_changes(before: &str, after: &str) -> Vec<String> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let mut changes = Vec::new();

    let common_prefix = before_lines
        .iter()
        .zip(after_lines.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let common_suffix = before_lines[common_prefix..]
        .iter()
        .rev()
        .zip(after_lines[common_prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let before_changed = &before_lines[common_prefix..before_lines.len() - common_suffix];
    let after_changed = &after_lines[common_prefix..after_lines.len() - common_suffix];

    let mut i = 0;
    let mut j = 0;
    while i < before_changed.len() || j < after_changed.len() {
        let b = before_changed.get(i).map(|s| s.trim());
        let a = after_changed.get(j).map(|s| s.trim());
        match (b, a) {
            (Some(before_line), Some(after_line)) => {
                if before_line != after_line {
                    changes.push(format!("  {before_line:<50} → {after_line}"));
                }
                i += 1;
                j += 1;
            }
            (Some(before_line), None) => {
                changes.push(format!("  - {before_line}"));
                i += 1;
            }
            (None, Some(after_line)) => {
                changes.push(format!("  + {after_line}"));
                j += 1;
            }
            (None, None) => break,
        }
    }

    if changes.is_empty() {
        changes.push("  (formatting changes only)".to_string());
    }

    changes
}
