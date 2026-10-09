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
    /// Use an explicit configuration file instead of workspace-root uncuddle.toml.
    #[arg(long)]
    config: Option<PathBuf>,
    /// List rule identifiers without inspecting a Cargo project.
    #[arg(long)]
    list_rules: bool,
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
    if cli.list_rules {
        for rule in uncuddle::config::DEFAULT_RULES {
            println!("{rule} (default)");
        }

        return Ok(0);
    }

    let report = uncuddle::runner::run(
        &uncuddle::discovery::Options {
            manifest_path: cli.manifest_path,
            workspace: cli.workspace,
            packages: cli.package,
        },
        cli.fix,
        cli.config.as_deref(),
    )?;

    for notice in &report.notices {
        eprintln!("uncuddle: {notice}");
    }

    for diagnostic in &report.diagnostics {
        eprintln!("{diagnostic}");
    }

    eprintln!(
        "uncuddle: checked {} files; {} files fixed; {} violations",
        report.files_checked,
        report.files_fixed,
        report.diagnostics.len()
    );

    Ok(report.exit_code())
}
