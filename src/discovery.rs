use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use syn::visit::Visit;

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

/// Discover source targets without compiling, resolving dependencies, or
/// allowing Cargo to write lockfiles.
pub fn discover(options: &Options) -> Result<Project> {
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
    let mut walker = Walker {
        files: BTreeMap::new(),
        visited: BTreeSet::new(),
        active: BTreeSet::new(),
        notices: Vec::new(),
        excluded_root: metadata.target_directory,
    };
    for package in &metadata.packages {
        if ids.contains(&package.id) {
            for target in &package.targets {
                walker.file(
                    &target.src_path,
                    target.src_path.parent().unwrap(),
                    &target.edition,
                )?;
            }
        }
    }
    Ok(Project {
        root: metadata.workspace_root,
        files: walker.files.into_values().collect(),
        notices: walker.notices,
    })
}

pub fn is_generated(source: &str) -> bool {
    source.lines().take(10).any(|line| {
        let line = line.trim_start();
        (line.starts_with("//") || line.starts_with("/*") || line.starts_with('*'))
            && line.contains("@generated")
    })
}

struct Walker {
    files: BTreeMap<PathBuf, SourceFile>,
    visited: BTreeSet<(PathBuf, PathBuf)>,
    active: BTreeSet<PathBuf>,
    notices: Vec<String>,
    excluded_root: PathBuf,
}

impl Walker {
    fn file(&mut self, path: &Path, module_dir: &Path, edition: &str) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot read {}", path.display()))?;
        if path.starts_with(&self.excluded_root) {
            self.notices.push(format!(
                "skipped generated target output {}",
                path.display()
            ));
            return Ok(());
        }
        if self.active.contains(&path) {
            bail!("cyclic module path at {}", path.display());
        }
        if !self.visited.insert((path.clone(), module_dir.to_owned())) {
            return Ok(());
        }
        if let Some(old) = self.files.get(&path) {
            if old.edition != edition {
                bail!(
                    "{} is shared by targets with different editions",
                    path.display()
                );
            }
        }
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read UTF-8 source {}", path.display()))?;
        if is_generated(&source) {
            self.notices
                .push(format!("skipped @generated file {}", path.display()));
            return Ok(());
        }
        self.active.insert(path.clone());
        let ast = crate::parse(&source).map_err(|error| {
            let pos = error.span().start();
            anyhow::anyhow!(
                "{}:{}:{}: parse error: {error}",
                path.display(),
                pos.line,
                pos.column + 1
            )
        })?;
        let mut modules = Modules {
            walker: self,
            file: &path,
            dir: module_dir.to_owned(),
            edition,
            error: None,
        };
        modules.visit_file(&ast);
        if let Some(error) = modules.error {
            return Err(error);
        }
        self.active.remove(&path);
        self.files.entry(path.clone()).or_insert(SourceFile {
            path,
            source,
            edition: edition.to_owned(),
        });
        Ok(())
    }
}

struct Modules<'a> {
    walker: &'a mut Walker,
    file: &'a Path,
    dir: PathBuf,
    edition: &'a str,
    error: Option<anyhow::Error>,
}

impl Modules<'_> {
    fn module(&mut self, module: &syn::ItemMod) -> Result<()> {
        let mut paths = Vec::new();
        let mut conditional_path = false;
        for attr in &module.attrs {
            if attr.path().is_ident("path") {
                if let syn::Meta::NameValue(value) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(path),
                        ..
                    }) = &value.value
                    {
                        paths.push(self.dir.join(path.value()));
                    }
                }
            } else if attr.path().is_ident("cfg_attr") {
                let metas = attr.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )?;
                for meta in metas.iter().skip(1) {
                    if let syn::Meta::NameValue(value) = meta {
                        if value.path.is_ident("path") {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(path),
                                ..
                            }) = &value.value
                            {
                                conditional_path = true;
                                paths.push(self.dir.join(path.value()));
                            }
                        }
                    }
                }
            }
        }
        if let Some((_, items)) = &module.content {
            let old = self.dir.clone();
            self.dir = paths
                .first()
                .cloned()
                .unwrap_or_else(|| old.join(module.ident.to_string()));
            for item in items {
                self.visit_item(item);
            }
            self.dir = old;
            return Ok(());
        }
        if paths.is_empty() || conditional_path {
            let name = module.ident.to_string();
            let flat = self.dir.join(format!("{name}.rs"));
            let nested = self.dir.join(&name).join("mod.rs");
            if flat.is_file() && nested.is_file() {
                bail!(
                    "ambiguous module `{name}` in {}: both {} and {} exist",
                    self.file.display(),
                    flat.display(),
                    nested.display()
                );
            }
            if flat.is_file() {
                paths.push(flat);
            }
            if nested.is_file() {
                paths.push(nested);
            }
        }
        if paths.is_empty() {
            bail!(
                "unresolved module `{}` in {}",
                module.ident,
                self.file.display()
            );
        }
        for path in paths {
            let child_dir = if path.file_name().is_some_and(|n| n == "mod.rs") {
                path.parent().unwrap().to_owned()
            } else {
                path.with_extension("")
            };
            self.walker.file(&path, &child_dir, self.edition)?;
        }
        Ok(())
    }
}

impl<'ast> Visit<'ast> for Modules<'_> {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if self.error.is_none() {
            if let Err(error) = self.module(module) {
                self.error = Some(error);
            }
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("include") {
            self.walker.notices.push(format!(
                "{}:{}: skipped include! contents (macros are not expanded)",
                self.file.display(),
                mac.bang_token.span.start().line,
            ));
        }
    }
}
