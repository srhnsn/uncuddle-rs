use std::path::Path;
use uncuddle::analysis::analyze;
use uncuddle::config::Config;
use uncuddle::diagnostic::Diagnostic;

fn check(source: &str) -> Vec<Diagnostic> {
    analyze(Path::new("test.rs"), source, &Config::default()).unwrap()
}

#[test]
fn setup_is_separate_from_every_control_flow_kind() {
    for control in [
        "if a < b { work(); }",
        "match a { _ => work() }",
        "for x in a { work(); }",
        "while a < b { work(); }",
        "loop { break; }",
        "if let Some(x) = a { work(); }",
        "while let Some(x) = a { work(); }",
    ] {
        let source = format!(
            "fn f() {{\n    let a = compute();\n    let b = compute();\n    {control}\n}}\n"
        );
        let got = check(&source);

        assert_eq!(got.len(), 1, "{control}: {got:?}");
        assert_eq!(got[0].rule, "before-control-flow");
        assert_eq!(got[0].line, 4);
    }
}

#[test]
fn separates_after_blocks_without_splitting_else_chains() {
    let source = "fn f() {\n    if a { work(); } else if b { other(); } else { last(); }\n    finish();\n}\n";
    let got = check(source);

    assert_eq!(got.len(), 1);
    assert_eq!(got[0].rule, "after-control-flow");
    assert!(check("fn f() {\n    if a { work(); } else { other(); }\n}\n").is_empty());
}

#[test]
fn preserves_related_bindings_assignments_mutation_and_consumption() {
    for body in [
        "let a = 1;\nlet b = 2;",
        "let mut values = Vec::new();\nvalues.push(1);",
        "let (a, b) = pair();\nconsume(a, b);",
        "let a = 1;\na += 1;",
        "builder.configure();\nlet result = builder.finish();",
        "let x = compute();\nx",
    ] {
        assert!(
            check(&format!("fn f() {{\n{body}\n}}\n")).is_empty(),
            "{body}"
        );
    }

    let got = check("fn f() {\nlet value = compute();\nlog_unrelated();\n}\n");

    assert_eq!(got[0].rule, "statement-groups");
}

#[test]
fn separates_explicit_and_implicit_returns_in_larger_blocks() {
    for last in ["return a;", "a", "a.await?", "Ok(a)"] {
        let source =
            format!("async fn f() {{\nlet a = compute();\nlet b = compute();\n{last}\n}}\n");
        let got = check(&source);

        assert_eq!(got.len(), 1, "{last}: {got:?}");
        assert_eq!(
            got[0].rule,
            if last.starts_with("return") {
                "before-exit"
            } else {
                "before-tail-expression"
            }
        );
    }

    for body in [
        "let a = compute();\nreturn a;",
        "let a = compute();\na",
        "return 1;",
    ] {
        assert!(check(&format!("fn f() {{\n{body}\n}}\n")).is_empty());
    }
}

#[test]
fn handles_break_values_and_continue_with_statement_counts() {
    for last in ["break a;", "continue;"] {
        let source = format!("fn f() {{\nloop {{\nlet a = 1;\nlet b = 2;\n{last}\n}}\n}}\n");
        let got = check(&source);

        assert_eq!(got.len(), 1);
        assert_eq!(got[0].rule, "before-exit");
    }

    // Physical line count does not turn a two-statement block into a large one.
    assert!(check("fn f() {\nlet a = compute(\n1,\n2,\n3,\n);\nreturn a;\n}\n").is_empty());
}

#[test]
fn walks_closures_async_and_unsafe_but_not_macro_bodies() {
    for inner in ["||", "async", "unsafe"] {
        let source = format!(
            "fn f() {{\nlet closure = {inner} {{\nlet a = 1;\nif a > 0 {{ work(); }}\n}};\n}}\n"
        );

        assert_eq!(check(&source).len(), 1, "{inner}");
    }

    assert!(check("fn f() { custom! { let a = 1; if a > 0 { work(); } } }").is_empty());
    // Macro arguments are not interpreted as Rust identifiers (including strings).
    assert_eq!(
        check("fn f() {\nlet x = 1;\nprintln!(\"{x}\");\n}\n")[0].rule,
        "statement-groups"
    );
}

#[test]
fn does_not_split_let_else_or_expression_initializers() {
    for body in [
        "let Some(x) = value() else { return; };\nconsume(x);",
        "let a = 1;\nlet b = if a > 0 { 1 } else { 0 };",
        "let a = 1;\nlet b = match a { _ => 0 };",
        "let x = read().await?;\nconsume(x).await?;",
    ] {
        assert!(
            check(&format!("async fn f() {{\n{body}\n}}\n")).is_empty(),
            "{body}"
        );
    }
}

#[test]
fn preserves_existing_blank_lines_and_raw_literal_contents() {
    assert!(check("fn f() {\nlet a = 1;\n\nif a > 0 { work(); }\n\nfinish();\n}\n").is_empty());

    let source =
        "fn f() {\nlet a = r#\"first\n\nif something { not_code(); }\nlast\"#;\nconsume(a);\n}\n";

    assert!(check(source).is_empty());
}

#[test]
fn attaches_leading_comments_to_the_next_statement() {
    let source = "fn f() {\nlet a = 1; // previous\n// condition\nif a > 0 { work(); }\n}\n";
    let got = check(source);

    assert_eq!(got.len(), 1);
    assert_eq!(got[0].insert_at, Some(source.find("// condition").unwrap()));

    let source =
        "fn f() {\nlet a = 1; /* previous\n\ncomment */\n// condition\nif a > 0 { work(); }\n}\n";
    let got = check(source);

    assert_eq!(
        got.len(),
        1,
        "blank lines inside comments are not statement boundaries"
    );
    assert_eq!(got[0].insert_at, Some(source.find("// condition").unwrap()));
}

#[test]
fn comments_on_the_same_line_need_manual_separation() {
    let got = check("fn f() { let a = 1; if a > 0 { work(); } }");

    assert_eq!(got.len(), 1);
    assert_eq!(got[0].insert_at, None);
    assert!(got[0].to_string().contains("manual separation"));
}

#[test]
fn skips_rustfmt_skip_functions_and_tracks_unicode_byte_offsets() {
    assert!(check("#[rustfmt::skip]\nfn f() { let a = 1; if a > 0 { work(); } }").is_empty());

    let source = "fn f() {\nlet café = 1;\nif café > 0 { work(); }\n}\n";
    let got = check(source);

    assert_eq!(got[0].insert_at, Some(source.find("if café").unwrap()));
}

#[test]
fn closure_shadowing_is_not_evidence_of_consuming_the_previous_binding() {
    let got = check("fn f() {\nlet x = 1;\nconsume(|| { let x = 2; x });\n}\n");

    assert_eq!(got[0].rule, "statement-groups");
}

#[test]
fn disabled_rules_and_short_block_threshold_are_respected() {
    let config = Config {
        disable: vec!["before-control-flow".into()],
        ..Config::default()
    };

    assert!(
        analyze(
            Path::new("test.rs"),
            "fn f() {\nlet a = 1;\nif a > 0 { work(); }\n}",
            &config
        )
        .unwrap()
        .is_empty()
    );

    let config = Config {
        short_block_max_statements: 1,
        ..Config::default()
    };
    let got = analyze(Path::new("test.rs"), "fn f() {\nlet a = 1;\na\n}", &config).unwrap();

    assert_eq!(got[0].rule, "before-tail-expression");
}

#[test]
fn skip_file_directives_only_apply_in_leading_comments() {
    assert!(check("// uncuddle:skip-file\ninvalid syntax").is_empty());

    let source = "fn f() {\nlet x = r#\"first\n// uncuddle:skip-file\n// @generated\nlast\"#;\nif x.is_empty() { work(); }\n}";

    assert_eq!(check(source).len(), 1);
    assert!(!uncuddle::discovery::is_generated(source));
}

#[test]
fn optional_rules_have_explicit_boundaries_and_stay_off_by_default() {
    let cases = [
        ("assignment-kinds", "fn f() {\nlet x = 1;\nx = 2;\n}"),
        (
            "local-item-spacing",
            "fn f() {\nconst X: i32 = 1;\nconsume(X);\n}",
        ),
        (
            "after-block-value",
            "fn f() {\nlet x = {\ncompute()\n};\nlet y = 1;\n}",
        ),
        (
            "match-arm-spacing",
            "fn f() {\nmatch x {\n0 => {\nlet a = 1;\nlet b = 2;\nconsume(a,b);\n}\n_ => other(),\n}\n}",
        ),
    ];

    for (rule, source) in cases {
        assert!(check(source).is_empty(), "{rule}: {source}");

        let config = Config {
            enable: vec![rule.into()],
            ..Config::default()
        };
        let got = analyze(Path::new("test.rs"), source, &config).unwrap();

        assert_eq!(got.len(), 1, "{rule}: {got:?}");
        assert_eq!(got[0].rule, rule);
    }
}

#[test]
fn supports_2015_keyword_identifiers_without_rewriting_source() {
    let source = "fn async() {\nlet dyn = 1;\nif dyn > 0 { work(); }\n}\nfn f(x: &dyn Trait) { let await = x; consume(await); }";
    let config = Config::default();
    let got =
        uncuddle::analysis::analyze_with_edition(Path::new("legacy.rs"), source, "2015", &config)
            .unwrap();

    assert_eq!(got.len(), 1);
    assert_eq!(got[0].insert_at, Some(source.find("if dyn").unwrap()));

    let fixed = uncuddle::fix::apply(source, &got).unwrap();

    assert!(fixed.contains("fn async()"));
    assert!(fixed.contains("&dyn Trait"));
    assert!(!fixed.contains("r#dyn"));
}

#[test]
fn raw_and_plain_spellings_refer_to_the_same_binding() {
    assert!(check("fn f() {\nlet r#value = 1;\nconsume(value);\n}").is_empty());
    assert!(check("fn f() {\nlet value = 1;\nconsume(r#value);\n}").is_empty());
}
