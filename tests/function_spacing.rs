use std::path::Path;
use uncuddle::{analysis::analyze, config::Config, fix};

fn check(source: &str, config: &Config) -> Vec<uncuddle::diagnostic::Diagnostic> {
    analyze(Path::new("functions.rs"), source, config).unwrap()
}

#[test]
fn separates_short_functions_in_every_supported_scope() {
    for source in [
        "fn a() {}\nfn b() {}\n",
        "mod nested {\n    fn a() {}\n    fn b() {}\n}\n",
        "impl Example {\n    fn a() {}\n    fn b() {}\n}\n",
        "impl Trait for Example {\n    fn a() {}\n    fn b() {}\n}\n",
        "trait Example {\n    fn a() {}\n    fn b() {}\n}\n",
        "fn outer() {\n    fn a() {}\n    fn b() {}\n}\n",
        "impl Example {\n    async fn a() {}\n    async fn b() {}\n}\n",
    ] {
        let config = Config {
            short_block_max_statements: usize::MAX,
            ..Config::default()
        };
        let diagnostics = check(source, &config);

        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].rule, "function-spacing");

        let fixed = fix::apply(source, &diagnostics).unwrap();

        assert!(fixed.contains("{}\n\n"));
        assert!(check(&fixed, &config).is_empty());
        assert_eq!(fix::apply(&fixed, &check(&fixed, &config)).unwrap(), fixed);
    }
}

#[test]
fn separates_function_bodies_from_other_items_but_preserves_declaration_groups() {
    for source in [
        "const A: u8 = 1;\nfn a() {}\nconst B: u8 = 2;\n",
        "impl Example {\n    const A: u8 = 1;\n    fn a() {}\n    const B: u8 = 2;\n}\n",
        "trait Example {\n    fn a();\n    fn b() {}\n    fn c();\n}\n",
        "fn outer() {\n    const A: u8 = 1;\n    fn a() {}\n    const B: u8 = 2;\n}\n",
    ] {
        let diagnostics = check(source, &Config::default());

        assert_eq!(diagnostics.len(), 2, "{source}: {diagnostics:?}");
        assert!(diagnostics.iter().all(|d| d.rule == "function-spacing"));
    }

    for source in [
        "trait Example {\n    fn a();\n    fn b();\n}\n",
        "unsafe extern \"C\" {\n    fn a();\n    fn b();\n}\n",
        "const A: u8 = 1;\nconst B: u8 = 2;\n",
        "fn outer() {\n    fn a() {}\n\n    work();\n}\n",
    ] {
        assert!(check(source, &Config::default()).is_empty(), "{source}");
    }
}

#[test]
fn keeps_method_comments_documentation_and_attributes_attached_with_both_newline_styles() {
    let source = "impl Example {\n    fn a() {} // previous\n    // next method\n    /// Documentation.\n    #[allow(unused)]\n    fn b() {}\n}\n";
    let expected = source.replace("// previous\n", "// previous\n\n");

    for newline in ["\n", "\r\n"] {
        let source = source.replace('\n', newline);
        let diagnostics = check(&source, &Config::default());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            fix::apply(&source, &diagnostics).unwrap(),
            expected.replace('\n', newline)
        );
    }

    let source = "fn a() {} /* previous\n\ncomment */\n/* next\n\ncomment */\nfn b() {}\n";
    let diagnostics = check(source, &Config::default());

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        fix::apply(source, &diagnostics).unwrap(),
        source.replace("comment */\n/* next", "comment */\n\n/* next")
    );
}

#[test]
fn preserves_existing_gaps_and_can_be_disabled() {
    let separated = "impl Example {\n    fn a() {}\n\n    /// Documentation.\n    fn b() {}\n}\n";

    assert!(check(separated, &Config::default()).is_empty());

    let cuddled = separated.replace("{}\n\n", "{}\n");
    let config = Config {
        disable: vec!["function-spacing".into()],
        ..Config::default()
    };

    config.validate().unwrap();
    assert!(check(&cuddled, &config).is_empty());
}

#[test]
fn reports_same_line_functions_without_changing_tokens() {
    let source = "impl Example { fn a() {} fn b() {} }";
    let diagnostics = check(source, &Config::default());

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "function-spacing");
    assert_eq!(diagnostics[0].insert_at, None);
    assert_eq!(fix::apply(source, &diagnostics).unwrap(), source);
}

#[test]
fn respects_skipped_containers_and_opaque_macro_bodies() {
    for source in [
        "#[rustfmt::skip]\nimpl Example { fn a() {} fn b() {} }",
        "#[rustfmt::skip]\nmod nested { fn a() {} fn b() {} }",
        "#[rustfmt::skip]\ntrait Example { fn a() {} fn b() {} }",
        "#[rustfmt::skip]\nfn outer() { fn a() {} fn b() {} }",
        "custom! { fn a() {} fn b() {} }",
    ] {
        assert!(check(source, &Config::default()).is_empty(), "{source}");
    }
}
