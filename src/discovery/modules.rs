use super::SourceFile;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::visit::Visit;

pub fn is_generated(source: &str) -> bool {
    source.lines().take(10).any(|line| {
        let line = line.trim_start();
        (line.starts_with("//") || line.starts_with("/*") || line.starts_with('*'))
            && line.contains("@generated")
    })
}

pub(super) struct Walker {
    pub(super) files: BTreeMap<PathBuf, SourceFile>,
    pub(super) visited: BTreeSet<(PathBuf, PathBuf)>,
    pub(super) active: BTreeSet<PathBuf>,
    pub(super) notices: Vec<String>,
    pub(super) excluded_root: PathBuf,
}

impl Walker {
    pub(super) fn file(&mut self, path: &Path, module_dir: &Path, edition: &str) -> Result<()> {
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
