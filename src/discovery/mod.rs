use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
mod modules;
mod paths;
use crate::config::Config;
use modules::Walker;
pub use modules::is_generated;

#[derive(Debug, Default)]
pub struct Options {
    pub manifest_path: Option<PathBuf>,
    pub workspace: bool,
    pub packages: Vec<String>,
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub source: String,
    pub edition: String,
}

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
    pub notices: Vec<String>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_default_members: Vec<String>,
    workspace_root: PathBuf,
    target_directory: PathBuf,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    src_path: PathBuf,
    edition: String,
}

/// Cargo-selected targets, separated from source scanning so exclusions can
/// be loaded before reading or parsing excluded files.
pub struct Targets {
    pub root: PathBuf,
    targets: Vec<Target>,
    target_directory: PathBuf,
}

/// Discover source targets without compiling, resolving dependencies, or
/// allowing Cargo to write lockfiles.
pub fn discover(options: &Options) -> Result<Project> {
    scan(load_targets(options)?, &Config::default())
}

pub fn load_targets(options: &Options) -> Result<Targets> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.args(["metadata", "--frozen", "--no-deps", "--format-version", "1"]);

    if let Some(path) = &options.manifest_path {
        command.arg("--manifest-path").arg(path);
    }

    let output = command.output().context("could not run Cargo metadata")?;

    if !output.status.success() {
        bail!(
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let metadata: Metadata =
        serde_json::from_slice(&output.stdout).context("Cargo returned invalid metadata")?;
    let manifest = options
        .manifest_path
        .as_deref()
        .map(Path::canonicalize)
        .transpose()?;
    let cwd = std::env::current_dir()?;
    let current = metadata
        .packages
        .iter()
        .filter(|p| {
            if let Some(path) = &manifest {
                p.manifest_path == *path
            } else {
                cwd.starts_with(p.manifest_path.parent().unwrap())
            }
        })
        .max_by_key(|p| p.manifest_path.components().count());
    let ids: BTreeSet<_> = if options.workspace {
        metadata.workspace_members.iter().cloned().collect()
    } else if !options.packages.is_empty() {
        let mut ids = BTreeSet::new();

        for name in &options.packages {
            let package = metadata
                .packages
                .iter()
                .find(|p| p.name == *name)
                .with_context(|| format!("unknown workspace package `{name}`"))?;
            ids.insert(package.id.clone());
        }

        ids
    } else if let Some(package) = current {
        BTreeSet::from([package.id.clone()])
    } else {
        metadata.workspace_default_members.iter().cloned().collect()
    };
    let targets = metadata
        .packages
        .into_iter()
        .filter(|p| ids.contains(&p.id))
        .flat_map(|p| p.targets)
        .collect();

    Ok(Targets {
        root: metadata.workspace_root.canonicalize()?,
        targets,
        target_directory: metadata.target_directory,
    })
}

pub fn scan(targets: Targets, config: &Config) -> Result<Project> {
    let mut walker = Walker {
        files: BTreeMap::new(),
        visited: BTreeSet::new(),
        active: BTreeSet::new(),
        notices: Vec::new(),
        excluded_root: targets.target_directory,
        root: targets.root.clone(),
        exclusions: config.exclusions()?,
    };

    for target in &targets.targets {
        walker.file(
            &target.src_path,
            target.src_path.parent().unwrap(),
            &target.edition,
        )?;
    }

    Ok(Project {
        root: targets.root,
        files: walker.files.into_values().collect(),
        notices: walker.notices,
    })
}
