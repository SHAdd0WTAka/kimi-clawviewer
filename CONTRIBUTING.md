# Contributing to ClawViewer Enterprise

## Development Setup

1. Install Rust 1.75+ via [rustup](https://rustup.rs/)
2. Install Node.js 20+ via [nvm](https://github.com/nvm-sh/nvm)
3. Install Tauri prerequisites: https://tauri.app/start/prerequisites/

## Code Standards

- Rust code must pass `cargo fmt` and `cargo clippy`
- All new features require tests
- Documentation comments for public APIs
- Error handling with `thiserror` or `anyhow`

## Testing

```bash
# Run all tests
cargo test --all --all-features

# Run with coverage
cargo tarpaulin --out Xml

# Lint check
cargo clippy --all-targets --all-features -- -D warnings
```

## Pull Request Process

1. Update CHANGELOG.md with your changes
2. Ensure CI passes (lint, test, security audit)
3. Request review from maintainers
4. Squash commits before merge

## Security

Please report security vulnerabilities to security@clawviewer.dev
