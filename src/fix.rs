use crate::diagnostic::Diagnostic;
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

/// Apply deduplicated blank-line insertions in memory. Unfixable diagnostics
/// remain for the caller to report; this does not change any other whitespace.
pub fn apply(source: &str, diagnostics: &[Diagnostic]) -> Result<String> {
    let positions: BTreeSet<_> = diagnostics.iter().filter_map(|d| d.insert_at).collect();
    let mut output = source.to_owned();
    for position in positions.into_iter().rev() {
        if position == 0
            || position >= source.len()
            || !source.is_char_boundary(position)
            || source.as_bytes()[position - 1] != b'\n'
        {
            bail!("invalid blank-line edit at byte {position}");
        }
        let newline = if source[..position].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        output.insert_str(position, newline);
    }
    Ok(output)
}

/// Replace one regular file atomically, preserving permissions and detecting
/// changes since analysis. There is no multi-file transaction or lock.
pub fn write_if_unchanged(path: &Path, original: &str, replacement: &str) -> Result<()> {
    if original == replacement {
        return Ok(());
    }
    let metadata = verify_writable(path)?;
    let mut temporary = tempfile::Builder::new().prefix(".uncuddle-").tempfile_in(
        path.parent()
            .context("source file has no parent directory")?,
    )?;
    temporary.write_all(replacement.as_bytes())?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    temporary.as_file().sync_all()?;
    verify_unchanged(path, original)?;
    temporary
        .persist(path)
        .with_context(|| format!("cannot atomically replace {}", path.display()))?;
    Ok(())
}

pub fn verify_unchanged(path: &Path, original: &str) -> Result<()> {
    if std::fs::read(path).with_context(|| format!("cannot re-read {}", path.display()))?
        != original.as_bytes()
    {
        bail!(
            "{} changed since analysis; refusing to overwrite it",
            path.display()
        );
    }
    Ok(())
}

pub fn verify_writable(path: &Path) -> Result<std::fs::Metadata> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("cannot inspect {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.permissions().readonly() {
        bail!(
            "refusing to replace non-regular or read-only file {}",
            path.display()
        );
    }
    Ok(metadata)
}
