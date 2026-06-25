# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability within ClawViewer, please follow these steps:

1. **Do NOT open a public issue.**
2. Email **security@clawviewer.dev** with:
   - A description of the vulnerability
   - Steps to reproduce (if applicable)
   - Possible impact
   - Suggested fix (optional)
3. You will receive an acknowledgment within **24 hours**.
4. We aim to provide a fix within **72 hours** for critical vulnerabilities.

## Security Measures

- All dependencies are scanned with `cargo audit` and `cargo deny` in CI.
- Binaries are signed and SBOMs are generated for every release.
- We follow the [Rust Secure Code Guidelines](https://github.com/rust-secure-code/projects).