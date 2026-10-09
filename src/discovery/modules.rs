use super::SourceFile;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::ext::IdentExt;
use syn::visit::Visit;

pub fn is_generated(source: &str) -> bool {
    crate::source::skip_reason(source) == Some("@generated")
}

pub(super) struct Walker {
    pub(super) files: BTreeMap<PathBuf, SourceFile>,
    pub(super) visited: BTreeSet<(PathBuf, PathBuf)>,
    pub(super) active: BTreeSet<PathBuf>,
    pub(super) notices: Vec<String>,
    pub(super) excluded_root: PathBuf,
    pub(super) root: PathBuf,
    pub(super) exclusions: globset::GlobSet,
}

impl Walker {
    pub(super) fn file(&mut self, path: &Path, module_dir: &Path, edition: &str) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot read {}", path.display()))?;
        let relative = path
            .strip_prefix(&self.root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if self.exclusions.is_match(&relative) {
            self.notices
                .push(format!("skipped excluded file {}", path.display()));
            return Ok(());
        }

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

        if let Some(reason) = crate::source::skip_reason(&source) {
            self.notices
                .push(format!("skipped {reason} file {}", path.display()));
            return Ok(());
        }

        self.active.insert(path.clone());

        let ast = crate::parse_with_edition(&source, edition).map_err(|error| {
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
            path_base: path.parent().unwrap().to_owned(),
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
    path_base: PathBuf,
    edition: &'a str,
    error: Option<anyhow::Error>,
}

impl Modules<'_> {
    fn module(&mut self, module: &syn::ItemMod) -> Result<()> {
        let variants = super::paths::resolve(&module.attrs, &self.path_base)?;
        let mut paths = variants.explicit;

        if let Some((_, items)) = &module.content {
            let old = self.dir.clone();
            let old_base = self.path_base.clone();

            if variants.use_default {
                paths.push(old.join(module.ident.unraw().to_string()));
            }

            for dir in paths {
                self.dir = dir;
                self.path_base = self.dir.clone();

                for item in items {
                    self.visit_item(item);
                }
            }

            self.dir = old;
            self.path_base = old_base;

            return Ok(());
        }

        if variants.use_default {
            let name = module.ident.unraw().to_string();
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
