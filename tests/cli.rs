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
