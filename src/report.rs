use crate::file::{FileOutcome, FileResult};

pub struct Summary {
    pub modified: usize,
    pub warnings: usize,
    pub skipped: usize,
}

pub fn print_results(results: &[FileResult], dry_run: bool) -> Summary {
    let mut modified = 0usize;
    let mut warning_count = 0usize;
    let mut skipped = 0usize;

    for result in results {
        match &result.outcome {
            FileOutcome::Modified { changes } => {
                modified += 1;
                println!("[MODIFIED]  {}", result.path.display());
                for change in changes {
                    println!("{change}");
                }
            }
            FileOutcome::WouldModify { changes } => {
                modified += 1;
                println!("[WOULD MODIFY]  {}", result.path.display());
                for change in changes {
                    println!("{change}");
                }
            }
            FileOutcome::Unchanged => {}
            FileOutcome::Skipped { reason } => {
                skipped += 1;
                println!("[SKIPPED]  {} — {reason}", result.path.display());
            }
        }

        for warn in &result.warnings {
            warning_count += 1;
            println!("[WARNING]  {} — {}", result.path.display(), warn.message);
        }
    }

    println!();
    if dry_run {
        println!(
            "총 {}개 파일이 수정될 예정, {}개 경고, {}개 스킵",
            modified, warning_count, skipped
        );
    } else {
        println!(
            "총 {}개 파일 수정, {}개 경고, {}개 스킵",
            modified, warning_count, skipped
        );
    }

    Summary {
        modified,
        warnings: warning_count,
        skipped,
    }
}
