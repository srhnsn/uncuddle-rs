# uncuddle-rs

A Rust statement-spacing linter inspired by [WSL](https://github.com/bombsimon/wsl).
It separates statement groups with blank lines and preserves short, related sequences.
Run it as a Cargo subcommand or use its analysis library.

## Install and use

Build requirements: Rust 1.85 or later and Cargo. From this checkout:

```sh
cargo install --path . --locked
cargo uncuddle                    # check, without changing files
cargo uncuddle --fix              # apply safe blank-line insertions
cargo uncuddle --workspace        # all workspace members
cargo uncuddle -p my-package      # select a package; repeat -p as needed
cargo uncuddle --manifest-path path/to/Cargo.toml
cargo uncuddle --list-rules
```

`cargo-uncuddle` also works directly, with the same flags. Checking is the default.
Exit codes: **0** clean or successfully fixed; **1** spacing violations remain;
**2** configuration, source parsing, discovery, or file-writing error.

```rust
let a = compute_a();
let b = compute_b();

if a < b {
    work();
}

finish();
```

Setup is separated from control flow even when the condition uses those variables.
Consecutive bindings stay together. Immediate related consumption or mutation can
stay adjacent: `let mut values = Vec::new();` followed by `values.push(x);`.
Related-use checks are conservative syntax checks, not ownership or type analysis.
Macro arguments are opaque, including implicit captures in formatting strings.

## Rules and configuration

| Rule | Default | Policy |
| --- | --- | --- |
| `function-spacing` | On | Separate function definitions and methods from sibling items, including short bodies. |
| `before-control-flow` | On | Separate standalone if/match/for/while/loop from preceding statements. |
| `after-control-flow` | On | Separate completed standalone control flow from subsequent statements. |
| `statement-groups` | On | Separate unrelated transitions between bindings/assignments and expressions. |
| `before-exit` | On | Separate return/break/continue in larger immediate blocks. |
| `before-tail-expression` | On | Apply the exit rule to implicit return expressions. |
| `assignment-kinds` | Off | Separate new bindings from mutation. |
| `local-item-spacing` | Off | Separate local items from executable statements. |
| `after-block-value` | Off | Separate multiline block-valued expressions from unrelated subsequent work. |
| `match-arm-spacing` | Off | Separate an arm with a larger block body from the next arm. |

The default short-block exemption is two immediate statements/expressions,
including the exit or tail. Physical line count does not affect it. Single-expression
bodies and `let x = compute(); x` stay compact. Overlapping rules produce one
boundary diagnostic and one insertion. Existing blank-line boundaries are retained.
Function spacing applies in files, inline modules, impls, traits with default method
bodies, and between local items. Consecutive trait signatures without bodies stay
compact. Comments, documentation, and attributes stay attached to their items.
Disable it with `disable = ["function-spacing"]` if you prefer grouped methods.

Create `uncuddle.toml` at the **workspace root**, or pass `--config path/to/file.toml`:

```toml
enable = ["match-arm-spacing"]
disable = ["statement-groups"]
short-block-max-statements = 2
exclude = ["src/generated/**", "tests/fixtures/**"]
```

Exclusions are glob patterns relative to the workspace root, using `/` on all
platforms. Excluded files are not parsed, and their module subtrees are not scanned.
Unknown keys/rules, duplicate rule entries, contradictory enable/disable entries,
and invalid globs are errors. A missing implicit config uses defaults; a missing
explicit `--config` file is an error.

To opt out a whole file, put `// uncuddle:skip-file` in its leading comments.
`@generated` in a leading comment within the first ten lines also opts out.
The tool respects `#[rustfmt::skip]` on supported items and block-bearing expressions.
Use file exclusions for other generated or deliberately unusual syntax.

## Safety and rustfmt compatibility

**Rustfmt is never invoked or required at runtime**, including during `--fix`.
Fixes only insert whole blank lines at unambiguous boundaries between source lines;
tokens, literal contents, comments, indentation, and statement order are preserved.
Leading standalone comments stay attached to the following statement, including
SAFETY comments. Inline trailing comments stay with the preceding statement.
CRLF/LF endings and file permissions are preserved.

If two statements occupy the same line, uncuddle reports an unfixed diagnostic.
Formatting the source first with `cargo fmt` can make that boundary fixable, but
formatted input is not a runtime prerequisite. The fixer plans and preflights all
files, checks for source changes, and replaces each changed file atomically.
Writes across multiple files are not a transaction; don't concurrently edit files
being fixed. Read-only/non-regular files are not replaced.

Automated tests run actual fixes through rustfmt on Rust 1.85 and current stable,
all four editions, and default/narrow/wide/tab/Windows-newline configurations.
They assert that rustfmt leaves uncuddle's fixed, already formatted input unchanged,
that fixes are idempotent, and that no violations remain afterward. Rustfmt is a
**development test dependency**: install its component before running tests.
Contradictory nightly settings such as `blank_lines_upper_bound = 0` are incompatible.

`let ... else`, `?`, `.await`, else-if chains, attributes, and doc comments are kept
intact. Normal Rust blocks inside closures, async code, and unsafe code are analyzed.
Macro bodies are neither expanded nor edited. No cleanup/lifetime transformations
or special rules for methods named `push`, `send`, `spawn`, or `drop` are performed.

## Source discovery and limitations

Cargo metadata selects the current package or virtual workspace's default members;
`--workspace` and `-p` override selection. Library, binary, test, example, benchmark,
and build-script targets are included. Discovery follows ordinary modules, inline
modules, literal `#[path]`, and static `cfg_attr` path alternatives, including
inactive cfg branches. Shared source files are deduplicated. Different editions
sharing the same file, missing/ambiguous/cyclic modules, and non-literal module
paths produce explicit errors.

Discovery uses `cargo metadata --frozen --no-deps`: it does not compile the consumer,
resolve/fetch application dependencies, change its lockfile, or execute build scripts.
Target output and generated files are skipped. Dependency source trees aren't
scanned unless they are selected workspace packages or explicitly referenced modules.
All `include!` contents are skipped with a notice; generated/dynamic modules and
macro-produced code need a separate expanded-code tool. Counts refer to files
actually inspected, and skips are printed explicitly.

## Development

```sh
rustup component add rustfmt clippy
cargo test --locked
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --bin cargo-uncuddle -- --workspace
```

The tests intentionally fail if rustfmt is unavailable. Set `UNCUDDLE_TEST_RUSTFMT`
to a specific rustfmt executable to test another toolchain. CI runs Rust 1.85 and
stable on Linux and Windows. See [the design](docs/design.md) for the WSL rule mapping
and [the architecture](docs/architecture.md) for extension points.
