//! rustfmt is a TEST dependency only. This suite intentionally fails if the
//! component is missing: silently skipping would hide compatibility regressions.
use std::path::Path;
use std::process::Command;
use uncuddle::{analysis::analyze_with_edition, config::Config, fix};

fn format(source: &str, edition: &str, config: &Path) -> String {
    let executable = std::env::var_os("UNCUDDLE_TEST_RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    // Exercise file formatting, as cargo fmt does. Stdin/stdout formatting has
    // different Auto newline behavior on Windows and is not our runtime path.
    // Close the temporary file handle before rustfmt opens it on Windows.
    let path = tempfile::Builder::new()
        .prefix("fixture-")
        .suffix(".rs")
        .tempfile_in(config.parent().unwrap())
        .unwrap()
        .into_temp_path();
    std::fs::write(&path, source).unwrap();
    let output = Command::new(executable)
        .args([
            "--edition",
            edition,
            "--style-edition",
            edition,
            "--config-path",
        ])
        .arg(config)
        .arg(&path)
        .output()
        .expect("install the rustfmt component before running compatibility tests");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read_to_string(path).unwrap()
}

#[test]
fn auto_newlines_preserve_lf_and_crlf_files_through_fixing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rustfmt.toml");
    std::fs::write(&path, "newline_style = 'Auto'\n").unwrap();

    let config = Config::default();

    for newline in ["\n", "\r\n"] {
        let source =
            "fn f() {\n    let a = 1;\n    if a > 0 {\n        work();\n    }\n    finish();\n}\n"
                .replace('\n', newline);
        let baseline = format(&source, "2024", &path);

        assert_eq!(
            baseline, source,
            "Auto must retain the input file's line endings"
        );

        let diagnostics =
            analyze_with_edition(Path::new("fixture.rs"), &baseline, "2024", &config).unwrap();

        assert!(!diagnostics.is_empty());

        let fixed = fix::apply(&baseline, &diagnostics).unwrap();

        assert_eq!(format(&fixed, "2024", &path), fixed);
        assert!(fixed.contains(&format!("let a = 1;{newline}{newline}    if")));
        assert!(
            analyze_with_edition(Path::new("fixture.rs"), &fixed, "2024", &config)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn rustfmt_never_reverts_uncuddle_fixes() {
    let directory = tempfile::tempdir().unwrap();
    let cases = [
        "fn f() { let a = 1; let b = 2; if a < b { work(); } finish(); }",
        "fn f() { let items = items(); for x in items { consume(x); } finish(); }",
        "fn f() { let mut x = 0; while x < 1 { x += 1; } finish(); }",
        "fn f() { let x = 1; loop { let a = 1; let b = 2; break a; } finish(); }",
        "fn f() { loop { let a = 1; let b = 2; continue; } }",
        "fn f() -> i32 { let a = 1; let b = 2; return a; }",
        "fn f() -> i32 { let a = 1; let b = 2; a }",
        "fn f() { let x = 1; log_unrelated(); let y = 2; }",
        "fn f() { let x = || { let a = 1; if a > 0 { work(); } finish(); }; }",
        "fn f() { let x = 1;\n// SAFETY: access is valid.\nunsafe { access(); } }",
        "fn f() { let x = 1; /* trailing\n\ncomment */\n// condition\nif x > 0 { work(); } }",
        "fn f() { let x = 1;\n/* leading\n\ncomment */\nif x > 0 { work(); } }",
        "fn f() { let café = 1; if café > 0 { work(); } }",
        "fn f() { let x = r#\"first\n\nlast\"#; if x.is_empty() { work(); } }",
        "fn f() { let x = 1; println!(\"{x}\"); custom! { let x = 1; if x > 0 { work(); } } }",
        "fn f() { let a = compute(); match a { 1 => { let x = 1; let y = 2; consume(x, y); } _ => other(), } finish(); }",
        "fn f() { let a = { work(); other() }; finish(); }",
        "fn f() { let x = 1; x = 2; consume(x); }",
        "fn f() { const X: i32 = 1; consume(X); }",
        "fn f() { let a = 1;\n#[allow(unused_variables)]\nlet b = { let x = 1; let y = 2; x }; }",
        "fn first() {}\n// Next function.\n/// Documentation.\n#[allow(unused)]\nfn second() {}",
        "fn first() {} /* trailing\n\ncomment */\n/* next function\n\ncomment */\nfn second() {}",
        "mod nested { fn first() {} fn second() {} }",
        "impl Example { fn first() {}\n// Next method.\n/// Documentation.\n#[allow(unused)]\nfn second() {} }",
        "impl Trait for Example { fn first() {} fn second() {} }",
        "trait Example { fn first(); fn second() {} fn third(); fn fourth() {} }",
        "fn outer() { fn first() {} fn second() {} }",
        "const FIRST: u8 = 1;\nfn middle() {}\nconst LAST: u8 = 2;",
    ];
    let configs = [
        "",
        "max_width = 60\nuse_small_heuristics = 'Off'\n",
        "max_width = 140\nuse_small_heuristics = 'Max'\n",
        "hard_tabs = true\n",
        "newline_style = 'Windows'\n",
    ];
    let rules = [
        Config::default(),
        Config {
            enable: uncuddle::config::OPTIONAL_RULES
                .iter()
                .map(|s| (*s).into())
                .collect(),
            ..Config::default()
        },
    ];

    for (config_index, options) in configs.iter().enumerate() {
        let path = directory.path().join(format!("{config_index}.toml"));
        std::fs::write(&path, options).unwrap();

        for edition in ["2015", "2018", "2021", "2024"] {
            let mut sources = cases.to_vec();

            if edition == "2015" {
                sources.push("fn async() { let dyn = 1; if dyn > 0 { work(); } finish(); } fn f(x: &dyn Trait) { let await = x; consume(await); }");
            }

            sources.push("fn f() { let Some(x) = value() else { return; }; if x > 0 { work(); } }");

            if edition != "2015" {
                sources.push("impl Example { async fn first() {} async fn second() {} }");
                sources.push("async fn f() { let x = read().await?; if x > 0 { work().await; } finish().await?; }");
                sources.push("fn f() { let x = async { let a = 1; let b = 2; a }; finish(); }");
            }

            for (case_index, source) in sources.into_iter().enumerate() {
                let baseline = format(source, edition, &path);

                for config in &rules {
                    let diagnostics =
                        analyze_with_edition(Path::new("fixture.rs"), &baseline, edition, config)
                            .unwrap();
                    let fixed = fix::apply(&baseline, &diagnostics).unwrap();
                    let reformatted = format(&fixed, edition, &path);

                    assert_eq!(
                        reformatted, fixed,
                        "case {case_index}, edition {edition}, options {options:?}"
                    );

                    let remaining =
                        analyze_with_edition(Path::new("fixture.rs"), &fixed, edition, config)
                            .unwrap();

                    assert!(remaining.is_empty(), "{remaining:?}\n{fixed}");
                    assert_eq!(fix::apply(&fixed, &remaining).unwrap(), fixed);
                }
            }
        }
    }
}
