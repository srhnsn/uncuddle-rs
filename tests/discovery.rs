use std::fs;
use std::path::{Path, PathBuf};
use uncuddle::discovery::{Options, discover};

fn put(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn names(root: &Path, files: &[uncuddle::discovery::SourceFile]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|f| f.path.strip_prefix(root).unwrap().to_owned())
        .collect()
}

#[test]
fn follows_targets_inline_modules_paths_and_inactive_cfg() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    put(
        &root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nmissing_uncuddle_dependency='999'\n",
    );
    put(
        &root,
        "src/lib.rs",
        "mod child; mod inner { mod leaf; } #[path=\"other.rs\"] mod custom; #[cfg(windows)] mod windows;",
    );
    put(&root, "src/child.rs", "mod deep;");
    put(&root, "src/child/deep.rs", "");
    put(&root, "src/inner/leaf.rs", "");
    put(&root, "src/other.rs", "");
    put(&root, "src/windows.rs", "");
    put(&root, "build.rs", "fn main() {}");
    put(&root, "tests/integration.rs", "");
    put(&root, "examples/example.rs", "fn main() {}");
    let project = discover(&Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    })
    .unwrap();
    let got = names(&root, &project.files);

    for path in [
        "src/lib.rs",
        "src/child.rs",
        "src/child/deep.rs",
        "src/inner/leaf.rs",
        "src/other.rs",
        "src/windows.rs",
        "build.rs",
        "tests/integration.rs",
        "examples/example.rs",
    ] {
        assert!(
            got.contains(&PathBuf::from(path)),
            "missing {path}: {got:?}"
        );
    }

    assert!(!root.join("Cargo.lock").exists());
}

#[test]
fn selects_workspace_defaults_and_explicit_packages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    put(
        &root,
        "Cargo.toml",
        "[workspace]\nmembers=['a','b']\ndefault-members=['a']\nresolver='3'\n",
    );

    for name in ["a", "b"] {
        put(
            &root,
            &format!("{name}/Cargo.toml"),
            &format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n"),
        );
        put(&root, &format!("{name}/src/lib.rs"), "");
    }

    let mut options = Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    };

    assert_eq!(discover(&options).unwrap().files.len(), 1);

    options.workspace = true;

    assert_eq!(discover(&options).unwrap().files.len(), 2);

    options.workspace = false;
    options.packages = vec!["b".into()];

    assert!(
        discover(&options).unwrap().files[0]
            .path
            .ends_with("b/src/lib.rs")
    );

    options.packages = vec!["unknown".into()];

    assert!(
        discover(&options)
            .unwrap_err()
            .to_string()
            .contains("unknown workspace package")
    );
}

#[test]
fn reports_missing_modules_and_parse_errors_and_skips_generated() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\n",
    );
    let options = Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    };

    put(root, "src/lib.rs", "mod missing;");
    assert!(
        discover(&options)
            .unwrap_err()
            .to_string()
            .contains("unresolved module")
    );
    put(root, "src/lib.rs", "fn broken(");
    assert!(
        discover(&options)
            .unwrap_err()
            .to_string()
            .contains("parse error")
    );
    put(root, "src/lib.rs", "// @generated\ninvalid syntax");

    let project = discover(&options).unwrap();

    assert!(project.files.is_empty());
    assert!(project.notices[0].contains("@generated"));
}

#[test]
fn path_attributes_in_non_root_files_are_relative_to_the_source_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    put(
        &root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\n",
    );
    put(&root, "src/lib.rs", "mod child;");
    put(
        &root,
        "src/child.rs",
        "#[path=\"sibling.rs\"] mod sibling; mod inline { #[path=\"nested.rs\"] mod nested; }",
    );
    put(&root, "src/sibling.rs", "");
    put(&root, "src/child/inline/nested.rs", "");
    let project = discover(&Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    })
    .unwrap();

    assert_eq!(project.files.len(), 4);
}

#[test]
fn reports_cycles_and_include_contents_without_expanding_macros() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\n",
    );
    let options = Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    };

    put(root, "src/lib.rs", "#[path=\"lib.rs\"] mod cycle;");
    assert!(
        discover(&options)
            .unwrap_err()
            .to_string()
            .contains("cyclic module")
    );
    put(
        root,
        "src/lib.rs",
        "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));",
    );

    let project = discover(&options).unwrap();

    assert_eq!(project.files.len(), 1);
    assert!(project.notices[0].contains("include!"));
}

#[test]
fn scans_all_static_cfg_attr_path_alternatives_and_deduplicates_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\n",
    );
    put(
        root,
        "src/lib.rs",
        "#[cfg_attr(windows, path=\"windows.rs\")] mod platform; #[path=\"windows.rs\"] mod alias;",
    );
    put(root, "src/platform.rs", "");
    put(root, "src/windows.rs", "");
    let project = discover(&Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    })
    .unwrap();

    assert_eq!(project.files.len(), 3);
}

#[test]
fn follows_nested_cfg_attr_and_inline_path_alternatives() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\n",
    );
    put(
        root,
        "src/lib.rs",
        "#[cfg_attr(windows, cfg_attr(feature=\"special\", path=\"alternate\"))] mod inline { mod child; }",
    );
    put(root, "src/inline/child.rs", "");
    put(root, "src/alternate/child.rs", "");
    let project = discover(&Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    })
    .unwrap();

    assert_eq!(project.files.len(), 3);
}

#[test]
fn raw_and_2015_keyword_module_names_use_plain_filenames() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "Cargo.toml",
        "[package]\nname='probe'\nversion='0.1.0'\nedition='2015'\n",
    );
    put(
        root,
        "src/lib.rs",
        "mod async; mod r#type; mod r#match { mod child; }",
    );
    put(root, "src/async.rs", "");
    put(root, "src/type.rs", "");
    put(root, "src/match/child.rs", "");
    let project = discover(&Options {
        manifest_path: Some(root.join("Cargo.toml")),
        ..Options::default()
    })
    .unwrap();

    assert_eq!(project.files.len(), 4);
}
