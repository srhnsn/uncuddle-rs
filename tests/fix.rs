use std::path::Path;
use uncuddle::{analysis::analyze, config::Config, fix};

#[test]
fn fixes_are_idempotent_and_preserve_crlf_comments_and_literals() {
    let source = "fn f() {\r\n    let text = r#\"a\r\n\r\nb\"#; // trailing\r\n    // condition\r\n    if text.is_empty() { work(); }\r\n    finish();\r\n}\r\n";
    let config = Config::default();
    let diagnostics = analyze(Path::new("test.rs"), source, &config).unwrap();
    let fixed = fix::apply(source, &diagnostics).unwrap();
    assert!(fixed.contains("; // trailing\r\n\r\n    // condition\r\n"));
    assert!(fixed.contains("r#\"a\r\n\r\nb\"#"));
    assert!(!fixed.replace("\r\n", "").contains('\n'));
    let remaining = analyze(Path::new("test.rs"), &fixed, &config).unwrap();
    assert!(remaining.is_empty());
    assert_eq!(fix::apply(&fixed, &remaining).unwrap(), fixed);
}

#[test]
fn preserves_safety_comments_and_does_not_edit_macro_bodies() {
    let source = "fn f() {\n    let x = 1;\n    // SAFETY: invariant is satisfied.\n    unsafe { access(); }\n    custom! { let x = 1; if x > 0 { work(); } }\n}\n";
    let config = Config::default();
    let fixed = fix::apply(
        source,
        &analyze(Path::new("test.rs"), source, &config).unwrap(),
    )
    .unwrap();
    assert!(fixed.contains("\n\n    // SAFETY: invariant is satisfied.\n    unsafe"));
    assert!(fixed.contains("custom! { let x = 1; if x > 0 { work(); } }"));
}

#[test]
fn same_line_diagnostics_remain_unfixed() {
    let source = "fn f() { let a = 1; if a > 0 { work(); } }";
    let diagnostics = analyze(Path::new("test.rs"), source, &Config::default()).unwrap();
    assert_eq!(fix::apply(source, &diagnostics).unwrap(), source);
    assert!(!diagnostics.is_empty());
}

#[test]
fn refuses_concurrent_changes_and_leaves_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.rs");
    std::fs::write(&path, "original").unwrap();
    assert!(
        fix::write_if_unchanged(&path, "stale", "replacement")
            .unwrap_err()
            .to_string()
            .contains("changed since analysis")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    fix::write_if_unchanged(&path, "original", "replacement").unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "replacement");
}

#[cfg(unix)]
#[test]
fn preserves_permissions_and_rejects_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.rs");
    std::fs::write(&path, "original").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    fix::write_if_unchanged(&path, "original", "replacement").unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let link = dir.path().join("link.rs");
    symlink(&path, &link).unwrap();
    assert!(fix::write_if_unchanged(&link, "replacement", "changed").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "replacement");
}
