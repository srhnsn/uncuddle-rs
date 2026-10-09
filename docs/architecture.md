# Architecture

The library exposes analysis independently from the CLI and file writes. Its public
entry points are `analysis::analyze` (modern syntax), `analysis::analyze_with_edition`,
`fix::apply` (in-memory edits), and `runner::run` (the Cargo project workflow).

| Component | Responsibility |
| --- | --- |
| `main.rs` | Cargo argument normalization, CLI parsing, rendering, exit status. |
| `runner.rs` | Load configuration, plan all analyses/fixes, preflight, then write. |
| `discovery/mod.rs` | Cargo metadata and package/target selection. |
| `discovery/modules.rs` | Traverse module source files, deduplicate, report skips/errors. |
| `discovery/paths.rs` | Resolve static module path alternatives without evaluating cfg. |
| `parsing.rs` | AST parsing with source byte offsets and edition-2015 keyword support. |
| `source.rs` | Recognize opt-out markers in leading comments. |
| `analysis/mod.rs` | Visit normal Rust AST blocks and item lists, deduplicate boundary diagnostics. |
| `analysis/items.rs` | Identify function definitions in file, impl, and trait item lists. |
| `analysis/rules.rs` | Pure boundary predicates, with explicit overlap precedence. |
| `analysis/statements.rs` | Classify statements and conservatively recognize related use. |
| `analysis/trivia.rs` | Find safe gaps without splitting attached comments. |
| `analysis/lines.rs` | Build a line index once for efficient diagnostic locations. |
| `config.rs` | Strict TOML configuration and exclusions. |
| `diagnostic.rs` | Structured diagnostics and human-readable presentation. |
| `fix.rs` | Apply blank-line insertions and atomically replace unchanged regular files. |

To add a statement boundary rule, register its identifier/default in `config.rs`,
add a pure predicate to `analysis/rules.rs`, and add positive/negative rule fixtures
plus a representative case to the rustfmt suite. The first matching enabled rule
wins at a boundary. Syntax constructs such as match-arm boundaries belong in the
AST visitor and reuse the same gap checker. Rules never write files or invoke tools.

Keep syntax and trivia analysis conservative. Do not interpret macro token trees as
ordinary Rust, infer ownership from method names, or expand unsupported code just to
make it parse. New fixes must preserve every non-blank-line source byte and pass the
formatter/idempotence checks. Source byte offsets must refer to the original file,
including Unicode, BOMs, and shebangs.

Only discovery invokes Cargo; only tests invoke rustfmt. Runtime source analysis
has no compiler or formatter subprocess. The workflow returns a structured report,
so a future JSON or editor renderer need not change the rules or write pipeline.
