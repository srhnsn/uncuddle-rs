use std::process::Command;

#[test]
fn cargo_and_direct_help_work() {
    for args in [vec!["--help"], vec!["uncuddle", "--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"))
            .args(args)
            .output()
            .unwrap();

        assert!(output.status.success());

        let stdout = String::from_utf8(output.stdout).unwrap();

        assert!(stdout.contains("--fix"));
        assert!(stdout.contains("--workspace"));
    }
}

fn fixture(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname='consumer'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("src/main.rs"), source).unwrap();

    dir
}

#[test]
fn check_fix_and_check_again_work_without_rustfmt_on_path() {
    let source =
        "fn main() {\n    let a = 1;\n    if a > 0 {\n        println!(\"positive\");\n    }\n}\n";
    let dir = fixture(source);
    let run = |fix: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"));
        command
            .arg("uncuddle")
            .arg("--manifest-path")
            .arg(dir.path().join("Cargo.toml"));
        // Metadata needs Cargo only; the runtime cannot find rustfmt here.
        command.env("CARGO", env!("CARGO")).env("PATH", dir.path());

        if fix {
            command.arg("--fix");
        }

        command.output().unwrap()
    };
    let check = run(false);

    assert_eq!(
        check.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/main.rs")).unwrap(),
        source
    );

    let fixed = run(true);

    assert!(
        fixed.status.success(),
        "{}",
        String::from_utf8_lossy(&fixed.stderr)
    );
    assert!(run(false).status.success());
    assert!(!dir.path().join("Cargo.lock").exists());

    let fmt = Command::new(env!("CARGO"))
        .args(["fmt", "--check", "--manifest-path"])
        .arg(dir.path().join("Cargo.toml"))
        .output()
        .unwrap();

    assert!(
        fmt.status.success(),
        "{}",
        String::from_utf8_lossy(&fmt.stdout)
    );
}

#[test]
fn unresolved_fixes_and_operational_errors_have_different_exit_codes() {
    let dir = fixture("fn main() { let a = 1; if a > 0 { work(); } }");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"))
            .arg("--manifest-path")
            .arg(dir.path().join("Cargo.toml"))
            .arg("--fix")
            .output()
            .unwrap()
    };

    assert_eq!(run().status.code(), Some(1));
    std::fs::write(dir.path().join("src/main.rs"), "fn main(").unwrap();
    assert_eq!(run().status.code(), Some(2));
}

#[test]
fn loads_config_before_parsing_excluded_files_and_supports_skip_file() {
    let dir =
        fixture("mod generated; mod ignored;\nfn main() {\nlet a = 1;\nif a > 0 { work(); }\n}\n");
    std::fs::write(dir.path().join("src/generated.rs"), "invalid syntax").unwrap();
    std::fs::write(
        dir.path().join("src/ignored.rs"),
        "// uncuddle:skip-file\ninvalid syntax",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("uncuddle.toml"),
        "disable=['before-control-flow']\nexclude=['src/generated.rs']",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"))
        .arg("--manifest-path")
        .arg(dir.path().join("Cargo.toml"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("skipped excluded"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("skip-file"));
    std::fs::write(dir.path().join("uncuddle.toml"), "enable=['unknown']").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"))
        .arg("--manifest-path")
        .arg(dir.path().join("Cargo.toml"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown rule"));
}

#[test]
fn list_rules_does_not_require_a_cargo_project() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-uncuddle"))
        .current_dir(dir.path())
        .arg("--list-rules")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("before-control-flow (default)"));
}
