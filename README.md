# uncuddle-rs

A Rust statement-spacing linter inspired by [WSL](https://github.com/bombsimon/wsl).
The executable is named `cargo-uncuddle`, so Cargo exposes it as `cargo uncuddle`.

```sh
cargo install --path . --locked
cargo uncuddle            # check; nonzero on violations
cargo uncuddle --fix      # apply safe blank-line edits
```

Implementation is in progress. See [the design](docs/design.md) for the agreed rules
and compatibility requirements. Rust 1.85 or later is required to build the tool.
Rustfmt is used in development tests only and is never invoked by the executable.
