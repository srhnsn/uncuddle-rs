# uncuddle-rs: agreed design

Approved implementation design. Optional extensions are individually opt-in.

## Decisions from the discussion

- Provide a Cargo subcommand: `cargo uncuddle`.
- Use Rust-adapted WSL defaults rather than translating every Go rule literally.
- Default to checking; apply edits only with `--fix`.
- Separate setup statements from a following `if`, even when the condition uses their variables. Keep the setup statements together. Use the same default for `match`, `for`, `while`, and `loop`.
- Treat implicit return (tail) expressions like explicit returns, with short bodies exempt.
- Use rustfmt only in development and automated tests. The installed tool must not invoke or require rustfmt, including in fix mode.
- Make focused commits after implementation milestones. Do not push or publish without a request.

## Evidence

Reviewed WSL v5 README, all CHECKS.md rules/configuration examples, and its test fixture structure at commit `dd6a1bdce2d597b3686757c4816fd0c3af686025`.

Executed 384 formatting experiments: 24 source scenarios, four configurations (defaults, narrow width with small heuristics off, wide width with maximal small heuristics, tabs), two editions (2021 and 2024), and two toolchains (Rust 1.85.0/rustfmt 1.8.0 and Rust 1.99.0/rustfmt 1.10.0). Every result was stable on a second formatting pass. These are formatter probes, not tests of an uncuddle implementation.

Both rustfmt versions preserved a single blank line at the tested statement boundaries: bindings, conditionals, loops, explicit returns, tail expressions, break/continue, let-else boundaries, unsafe blocks, closure and async bodies, and before leading comments. Blank lines between both block and expression match arms also survived. Rustfmt removed leading/trailing blank lines in blocks, reduced repeated blank lines to one, and removed blank lines inside method chains and call arguments.

Cargo's documented integration is an executable named `cargo-uncuddle`, found through Cargo's executable search. Cargo supplies an extra `uncuddle` argument; support both `cargo uncuddle ...` and direct `cargo-uncuddle ...` invocation. Tested `cargo metadata --frozen --no-deps --format-version 1` with an unavailable dependency: project discovery succeeded without compilation or dependency resolution.

## Rules

Each rule has a stable identifier, actionable diagnostic, and individually configurable enable/disable setting. Fixes only add or remove whole blank lines between supported syntax nodes. Never rewrite tokens, reorder statements, or introduce explicit returns.

### Enabled by default

1. **before-control-flow**: one blank line before standalone `if`, `match`, `for`, `while`, and `loop` when preceded by a sibling statement. No blank line at block start. Do not split `else if`, `if let`, `while let`, let-chains, or `let x = if/match ...` expressions. Related setup is still separated from a following standalone control-flow block.
2. **after-control-flow**: separate a completed standalone control-flow statement from a following sibling. Never split an `if`/`else` chain; never put a blank line before a closing brace.
3. **statement-groups**: keep consecutive bindings/assignments together; distinguish setup, side effects, and local items. Permit immediately related value consumption or mutation, such as `let mut values = Vec::new(); values.push(x);`. Separate clearly unrelated transitions, such as `let x = compute(); log_unrelated();`. Use syntax and conservative local binding tracking, not method-name guesses or claimed type analysis. Opaque macro arguments are not evidence of variable use. Retain existing group boundaries unless a specific removal rule applies.
4. **before-exit**: separate `return`, `break` (including break values), and `continue` in larger immediate blocks. Short-block threshold: at most two immediate statements/expressions, including the exit. Count syntax nodes rather than physical lines, so changing rustfmt's line width does not change diagnostics.
5. **before-tail-expression**: use the same larger-block rule for the final value expression. Preserve idioms such as `let x = compute(); x` and single-expression function bodies. A tail `if`/`match` also follows before-control-flow; merge overlapping diagnostics into one edit.

### Optional rules

- **match-arm-spacing**: separate substantial adjacent match arms. Default off to avoid spreading short pattern tables.
- **after-block-value**: separate a multiline block-valued initializer, closure definition, async block, or unsafe block from subsequent independent work. Avoid separating a value from an immediate related consumer. Default off initially because these patterns need clearer examples and overlap resolution.
- **assignment-kinds**: distinguish introducing new bindings from later mutation. Default off; Rust shadowing and builder patterns often benefit from remaining together.
- **local-item-spacing**: separate local const/type/function definitions from executable statements. Default off initially; do not enforce Go-style declaration grouping.

### WSL rules that need adaptation or omission

| WSL rules | Rust treatment |
| --- | --- |
| `assign`, `expr`, `after-expr`, `assign-expr`, `assign-exclusive` | General statement grouping plus optional stricter assignment-kind rule. |
| `if`, `for`, `range`, `switch`, `type-switch`, `after-block` | Statement boundary rules for Rust if/match/for/while/loop. No special type-switch rule. |
| `return`, `branch`, `branch-max-lines` | Include tail expressions and break values; use immediate statement counts rather than line counts. No goto/fallthrough. |
| `decl`, `after-decl` | Optional local-item separation. Rust let is an executable binding, not a Go var declaration. No declaration rewrites. |
| `inc-dec` | Covered by assignment grouping for `+=`/`-=`; Rust has no ++/--. |
| `append` | General related mutation rules; no special recognition of push/extend by name. |
| `err` | Do not translate the forced adjacency rule. Rust uses `?`, let-else, and Result/Option matching. Manual if/match checks follow the chosen control-flow boundary rule. |
| `defer`, `after-defer` | Omit. RAII/Drop is not syntax equivalent to defer. Never change resource lifetime or infer guard types. |
| `go`, `after-go`, `send`, `select` | Omit dedicated rules. Spawn/send are library APIs; await is an expression. Macro select bodies remain opaque. |
| `label` | No blanket rule. Rust labels support structured loop/block control and are not Go goto labels. |
| `leading-whitespace`, `trailing-whitespace` | Leave block-edge formatting to rustfmt; no redundant default rules. |
| `cuddle-group`, `cuddle-max-statements`, `allow-first-in-block`, `allow-whole-block` | Do not carry over these options for control flow: the chosen policy separates the entire preceding setup group. |
| `case-max-lines` | Optional match-arm spacing, without Go indentation heuristics. |

Rust-specific safeguards are as important as additional rules: preserve `?` chains and `.await`; keep let-else intact; handle destructuring and shadowing conservatively; keep SAFETY comments attached to unsafe code; never insert gaps inside attributes/doc comments or macro token trees. Walk ordinary Rust blocks inside closures, async functions, and unsafe blocks without guessing ownership semantics.

## rustfmt compatibility contract

Use stable rustfmt as the supported baseline. Target Rust 1.85+ and all four Rust editions, testing edition-appropriate syntax. Respect compatible stable options including width, indentation, heuristics, and line endings. There is no honest universal promise for arbitrary contradictory nightly settings: for example `blank_lines_upper_bound = 0` explicitly removes the very spacing this tool adds. Document such settings as incompatible rather than adding a runtime formatter dependency.

Let F be rustfmt and U be uncuddle. The primary regression invariant on already formatted input is `F(U(F(source))) == U(F(source))`: rustfmt must leave every candidate byte unchanged. Also verify `U(U(source)) == U(source)`, unchanged tokens/comments/literals, and that formatting fixed unformatted examples does not reintroduce uncuddle violations. Unsupported or ambiguous syntax is an explicit diagnostic/skip, not a silent readiness claim.

No runtime rustfmt precondition, subprocess, automatic formatting, or requirement to preformat input. For v1, autofix only unambiguous boundaries between distinct source lines. If the source packs multiple statements onto a single line, report a boundary requiring formatting/manual intervention rather than changing indentation or code layout beyond blank lines.

## Implementation architecture and rationale

- Package `uncuddle-rs`, library plus `cargo-uncuddle` binary; proposed edition 2024 and MSRV 1.85, subject to dependency compatibility checks.
- `syn` 2 full AST/visitor plus `proc_macro2` span locations. Keep original source bytes and a line/byte map; use an accompanying token/trivia layer for comments, strings, and safe boundaries. This is a syntax tool, not a rustc plugin, so it can inspect code without building or fetching its application dependencies.
- Separate discovery, analysis, rule evaluation, diagnostics, and edit application. Prefer a small pure library that fixture tests can exercise without launching Cargo.
- Use frozen no-dependency Cargo metadata for target roots, editions, packages, and workspace defaults. Follow inline/out-of-line modules and statically resolvable path attributes; deduplicate shared files and handle inactive cfg branches conservatively. Cover lib/bin/tests/examples/benches/build.rs. Exclude dependency caches, target output, and generated files by default; explicitly report unsupported dynamic includes/module paths. Do not expand macros or execute build scripts.
- CLI: default check, `--fix`, `--workspace`, `-p/--package`, `--manifest-path`; human diagnostics with file/line/column and rule id. Stable exit codes: 0 clean/successful fix, 1 lint violations, 2 operational/parse/configuration error. `--fix` still fails if unresolved violations remain.
- `uncuddle.toml` project configuration for enabled/disabled rules, short-block threshold, and file exclusions; clear errors for unknown keys/rule names. Keep initial configuration small rather than copying all WSL switches.
- Plan all edits before writing. Reject overlaps, preserve CRLF/LF and file permissions, detect concurrent source changes, and use atomic replacement per file. Never claim a multi-file filesystem transaction. Ambiguous comments get an unfixed diagnostic, not a guess.
- Generated files and macro bodies are excluded by default. Skip directives/suppression comments should be documented; no invented Rust lint attributes that rustc would reject.

## Commit milestones and validation

1. **Specification and scaffold**: approved rule examples, manifest/toolchain decisions, library/binary split, basic Cargo invocation/help, initial README and CI. Commit when Cargo invocation and basic tests work.
2. **Discovery and diagnostics**: package/workspace/module discovery, stable diagnostics and exit codes, source/trivia safety. Commit after fixture coverage for workspace defaults, explicit targets/path modules, parse errors, and exclusions.
3. **Core rules**: before/after control flow, statement groups, exits and tails with conflict resolution. Commit after positive/negative fixtures for each rule, short bodies, nesting, shadowing, comments, and let-else.
4. **Fixer and rustfmt regression suite**: blank-line edits, dry analysis consistency, idempotence, line endings and safe writes; compatibility matrix on MSRV and stable rustfmt. Commit after real `cargo uncuddle --fix` then `cargo fmt --check` fixtures pass.
5. **Configuration and usability**: supported optional rules, config errors, skip behavior, documentation and end-to-end install/use. Commit after cargo test, fmt/clippy for this repository, and a consumer workspace smoke test. Refresh saved cloud setup/startup instructions after they have actually been exercised.

Use the existing checkout; do not create Git worktrees for cloud tasks. Do not create a commit merely to record installation artifacts outside the repository.

## Sources

- WSL: https://github.com/bombsimon/wsl and https://github.com/bombsimon/wsl/blob/main/CHECKS.md
- Cargo external tools: https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands
- Cargo metadata: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
- rustfmt configuration: https://github.com/rust-lang/rustfmt/blob/master/Configurations.md
- rustfmt limitations/editions: https://github.com/rust-lang/rustfmt#limitations and https://github.com/rust-lang/rustfmt#configuring-rustfmt
- Syn and spans: https://docs.rs/syn/latest/syn/ and https://docs.rs/proc-macro2/latest/proc_macro2/struct.Span.html
