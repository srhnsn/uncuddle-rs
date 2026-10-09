use std::path::PathBuf;

/// A single source boundary violation. Several rules at the same boundary
/// produce one diagnostic and, when safe, one edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub message: String,
    pub insert_at: Option<usize>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: {}{}",
            self.path.display(),
            self.line,
            self.column,
            self.rule,
            self.message,
            if self.insert_at.is_none() {
                " (requires manual separation)"
            } else {
                ""
            }
        )
    }
}
