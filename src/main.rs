use std::path::PathBuf;

use clap::{Parser, Subcommand};

use cargo_refactor_lib::{
    file::{collect_rs_files, process_file},
    path::RustPath,
    rename::Rename,
    report::print_results,
};

#[derive(Parser)]
#[command(name = "cargo-refactor", about = "Refactor Rust fully-qualified paths")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Rename the last segment of a path (name change)
    Rename {
        /// Fully qualified path to rename from (e.g. a::b::c::OldName)
        from: String,
        /// Fully qualified path to rename to (e.g. a::b::c::NewName)
        to: String,
        /// Target directory (default: current directory)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Print changes without modifying files
        #[arg(long)]
        dry_run: bool,
    },
    /// Move a path (module change), optionally renaming
    Move {
        /// Fully qualified path to move from (e.g. a::b::c::OldName)
        from: String,
        /// Fully qualified path to move to (e.g. x::y::OldName)
        to: String,
        /// Target directory (default: current directory)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Print changes without modifying files
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Rename {
            from,
            to,
            dir,
            dry_run,
        } => {
            let from_path = parse_path_or_exit(&from);
            let to_path = parse_path_or_exit(&to);
            let rename = Rename::new_rename(from_path, to_path);
            run(rename, dir, dry_run);
        }
        Commands::Move {
            from,
            to,
            dir,
            dry_run,
        } => {
            let from_path = parse_path_or_exit(&from);
            let to_path = parse_path_or_exit(&to);
            let rename = Rename::new_move(from_path, to_path);
            run(rename, dir, dry_run);
        }
    }
}

fn parse_path_or_exit(s: &str) -> RustPath {
    RustPath::parse(s).unwrap_or_else(|e| {
        eprintln!("error: invalid path '{s}': {e}");
        std::process::exit(1);
    })
}

fn run(rename: Rename, dir: Option<PathBuf>, dry_run: bool) {
    let base_dir = dir.unwrap_or_else(|| PathBuf::from("."));

    if !base_dir.is_dir() {
        eprintln!("error: '{}' is not a directory", base_dir.display());
        std::process::exit(1);
    }

    let files = collect_rs_files(&base_dir);
    if files.is_empty() {
        eprintln!("No .rs files found in '{}'", base_dir.display());
        return;
    }

    let results: Vec<_> = files
        .iter()
        .map(|f| process_file(f, &rename, dry_run))
        .collect();

    print_results(&results, dry_run);
}
