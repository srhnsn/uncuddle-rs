use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

pub const DEFAULT_RULES: &[&str] = &[
    "function-spacing",
    "before-control-flow",
    "after-control-flow",
    "statement-groups",
    "before-exit",
    "before-tail-expression",
    "match-arm-spacing",
    "after-block-value",
    "assignment-kinds",
    "local-item-spacing",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// Retained for existing configs; every known rule is already enabled.
    pub enable: Vec<String>,
    pub disable: Vec<String>,
    pub short_block_max_statements: usize,
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable: Vec::new(),
            disable: Vec::new(),
            short_block_max_statements: 2,
            exclude: Vec::new(),
        }
    }
}

impl Config {
    /// Load one explicitly selected config or the workspace-root config.
    /// A missing implicit config uses defaults; a missing explicit one is an error.
    pub fn load(root: &Path, explicit: Option<&Path>) -> Result<Self> {
        let default = root.join("uncuddle.toml");
        let path = explicit.unwrap_or(&default);
        let config = match std::fs::read_to_string(path) {
            Ok(source) => toml::from_str(&source)
                .with_context(|| format!("invalid configuration {}", path.display()))?,
            Err(error) if explicit.is_none() && error.kind() == std::io::ErrorKind::NotFound => {
                Self::default()
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot read configuration {}", path.display()));
            }
        };
        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        let mut names = BTreeSet::new();

        for rule in self.enable.iter().chain(&self.disable) {
            if !DEFAULT_RULES.contains(&rule.as_str()) {
                bail!("unknown rule `{rule}`");
            }

            if !names.insert(rule) {
                bail!("rule `{rule}` is repeated or both enabled and disabled");
            }
        }

        self.exclusions()?;

        Ok(())
    }

    pub fn exclusions(&self) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();

        for pattern in &self.exclude {
            builder.add(
                Glob::new(pattern)
                    .with_context(|| format!("invalid exclusion pattern `{pattern}`"))?,
            );
        }

        Ok(builder.build()?)
    }

    pub fn enabled(&self, rule: &str) -> bool {
        DEFAULT_RULES.contains(&rule) && !self.disable.iter().any(|r| r == rule)
    }
}
