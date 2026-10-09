use uncuddle::config::Config;

#[test]
fn missing_implicit_config_uses_defaults_but_explicit_config_is_required() {
    let dir = tempfile::tempdir().unwrap();

    assert!(
        Config::load(dir.path(), None)
            .unwrap()
            .enabled("before-control-flow")
    );
    assert!(Config::load(dir.path(), Some(&dir.path().join("missing.toml"))).is_err());
}

#[test]
fn rejects_unknown_keys_rules_conflicts_and_bad_globs() {
    let dir = tempfile::tempdir().unwrap();

    for text in [
        "typo = true",
        "enable = ['typo']",
        "enable = ['before-exit']\ndisable = ['before-exit']",
        "exclude = ['[']",
        "short-block-max-statements = -1",
    ] {
        std::fs::write(dir.path().join("uncuddle.toml"), text).unwrap();
        assert!(Config::load(dir.path(), None).is_err(), "{text}");
    }
}

#[test]
fn enables_optional_rules_disables_defaults_and_normalizes_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("uncuddle.toml"), "enable=['assignment-kinds']\ndisable=['statement-groups']\nshort-block-max-statements=4\nexclude=['src/generated/**']\n").unwrap();
    let config = Config::load(dir.path(), None).unwrap();

    assert!(config.enabled("assignment-kinds"));
    assert!(!config.enabled("statement-groups"));
    assert_eq!(config.short_block_max_statements, 4);
    assert!(
        config
            .exclusions()
            .unwrap()
            .is_match("src/generated/file.rs")
    );
}
