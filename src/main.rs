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
    let cli = Cli::parse_from(args);
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("uncuddle: {error:#}");
            std::process::exit(2);
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    let project = uncuddle::discovery::discover(&uncuddle::discovery::Options {
        manifest_path: cli.manifest_path,
        workspace: cli.workspace,
        packages: cli.package,
    })?;
    for notice in project.notices {
        eprintln!("uncuddle: {notice}");
    }
    eprintln!(
        "uncuddle: discovered {} source files; spacing checks are not implemented yet",
        project.files.len()
    );
    Ok(2)
}
