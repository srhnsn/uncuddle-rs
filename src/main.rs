use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "cargo uncuddle",
    version,
    about = "Check Rust statement spacing"
)]
struct Cli {
    /// Apply safe blank-line fixes instead of checking only.
    #[arg(long)]
    fix: bool,
    /// Check all workspace packages.
    #[arg(long, conflicts_with = "package")]
    workspace: bool,
    /// Check a named package (repeatable).
    #[arg(short = 'p', long)]
    package: Vec<String>,
    /// Path to the Cargo manifest.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

fn main() {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "uncuddle") {
        args.remove(1);
    }
    let _cli = Cli::parse_from(args);
    eprintln!("uncuddle: source analysis is not implemented yet");
    std::process::exit(2);
}
