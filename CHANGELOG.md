# Changelog

All notable changes to ClawViewer Enterprise will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Enterprise-grade security module with Argon2, JWT, RBAC
- Configuration management with hot-reload support
- Monitoring and audit logging with Prometheus metrics
- Health check endpoints
- Rate limiting for authentication endpoints
- Multi-platform CI/CD pipeline
- Docker and docker-compose support
- Structured JSON logging
- Session management with automatic cleanup

### Security
- Password strength validation (12+ chars, mixed case, numbers, special)
- Session timeout and invalidation
- Account lockout after failed attempts
- Audit logging for all auth events

## [0.1.0] - 2024-01-15

### Added
- Initial release
- Screen capture with WebRTC streaming
- Basic Tauri desktop application
- Signaling server for peer discovery
- Multi-monitor support
