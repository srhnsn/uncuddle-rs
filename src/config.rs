use serde::Deserialize;

pub const DEFAULT_RULES: &[&str] = &[
    "before-control-flow",
    "after-control-flow",
    "statement-groups",
    "before-exit",
    "before-tail-expression",
];

pub const OPTIONAL_RULES: &[&str] = &[
    "match-arm-spacing",
    "after-block-value",
    "assignment-kinds",
    "local-item-spacing",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
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
    pub fn enabled(&self, rule: &str) -> bool {
        !self.disable.iter().any(|r| r == rule)
            && (DEFAULT_RULES.contains(&rule) || self.enable.iter().any(|r| r == rule))
    }
}
