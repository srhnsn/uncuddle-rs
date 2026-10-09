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
    let mut analyses = Vec::new();
    for file in &project.files {
        let diagnostics = uncuddle::analysis::analyze(
            &file.path,
            &file.source,
            &uncuddle::config::Config::default(),
        )?;
        let fixed = if cli.fix {
            uncuddle::fix::apply(&file.source, &diagnostics)?
        } else {
            file.source.clone()
        };
        analyses.push((file, diagnostics, fixed));
    }
    let mut violations = 0;
    let mut fixed_files = 0;
    if cli.fix {
        // Preflight every changed source before modifying any file.
        for (file, _, fixed) in &analyses {
            if file.source != *fixed {
                uncuddle::fix::verify_writable(&file.path)?;
                uncuddle::fix::verify_unchanged(&file.path, &file.source)?;
            }
        }
    }
    for (file, diagnostics, fixed) in analyses {
        let remaining = if cli.fix {
            let remaining = uncuddle::analysis::analyze(
                &file.path,
                &fixed,
                &uncuddle::config::Config::default(),
            )?;
            uncuddle::fix::write_if_unchanged(&file.path, &file.source, &fixed)?;
            fixed_files += usize::from(file.source != fixed);
            remaining
        } else {
            diagnostics
        };
        for diagnostic in remaining {
            eprintln!("{diagnostic}");
            violations += 1;
        }
    }
    eprintln!(
        "uncuddle: checked {} files; {fixed_files} files fixed; {violations} violations",
        project.files.len()
    );
    Ok(i32::from(violations > 0))
}
