# Development

## Rust toolchain

Install Rust with [rustup](https://rustup.rs/). The project uses the Rust edition declared in each crate's `Cargo.toml`.

## Commands

Run these from the repository root:

```bash
# Format check
cargo fmt --all -- --check

# Compile-check the entire workspace
cargo check --workspace --all-targets

# Run tests
cargo test --workspace

# Run Clippy with warnings visible
cargo clippy --workspace --all-targets --all-features

# CI-style lint gate: fail on any warning
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

To apply formatting locally:

```bash
cargo fmt --all
```
