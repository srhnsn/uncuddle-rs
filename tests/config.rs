use uncuddle::config::Config;

#[test]
fn missing_implicit_config_uses_defaults_but_explicit_config_is_required() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load(dir.path(), None).unwrap();

    for rule in uncuddle::config::DEFAULT_RULES {
        assert!(config.enabled(rule), "{rule} should be on without a config");
    }

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
fn accepts_existing_enable_settings_disables_rules_and_normalizes_exclusions() {
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

#[test]
fn empty_config_enables_every_rule_and_each_rule_can_be_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("uncuddle.toml");
    std::fs::write(&path, "").unwrap();

    let config = Config::load(dir.path(), None).unwrap();

    for rule in uncuddle::config::DEFAULT_RULES {
        assert!(
            config.enabled(rule),
            "{rule} should be on in an empty config"
        );
        std::fs::write(&path, format!("disable = ['{rule}']\n")).unwrap();

        let config = Config::load(dir.path(), None).unwrap();

        for candidate in uncuddle::config::DEFAULT_RULES {
            assert_eq!(config.enabled(candidate), candidate != rule);
        }
    }
}
