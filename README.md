# ClawViewer Enterprise

> Enterprise-grade Screen Capture & Remote Desktop Platform

[![CI/CD](https://github.com/SHAdd0WTAka/kimi-clawviewer/actions/workflows/enterprise-ci.yml/badge.svg)](https://github.com/SHAdd0WTAka/kimi-clawviewer/actions/workflows/enterprise-ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

## Features

- **High-Performance Screen Capture**: Hardware-accelerated capture with configurable FPS and resolution
- **WebRTC Real-Time Streaming**: Low-latency peer-to-peer connections with STUN/TURN support
- **Enterprise Security**: Argon2 password hashing, JWT authentication, RBAC, audit logging
- **Multi-Platform**: Native builds for Windows, macOS, and Linux
- **Monitoring & Observability**: Prometheus metrics, health checks, structured logging
- **Headless Mode**: Server deployment without UI for remote monitoring
- **Configuration Management**: YAML/JSON config with validation and hot-reload

## Architecture

```
clawviewer/
├── crates/
│   ├── clawviewer-config      # Configuration management
│   ├── clawviewer-security    # Authentication, authorization, audit
│   ├── clawviewer-monitoring  # Metrics, health checks, logging
│   ├── clawviewer-network     # Signaling, WebSocket management
│   ├── clawviewer-capture     # Screen capture engine
│   ├── clawviewer-webrtc      # WebRTC peer connection
│   ├── clawviewer-ui          # Tauri desktop application
│   └── clawviewer-lib         # Shared utilities
├── src-tauri/                 # Tauri desktop entry point
├── .github/workflows/          # CI/CD pipelines
├── docker-compose.yml          # Container orchestration
└── Dockerfile                 # Multi-stage build
```

## Quick Start

### Prerequisites

- Rust 1.75+
- Node.js 20+
- Tauri CLI: `cargo install tauri-cli`

### Development

```bash
# Clone repository
git clone https://github.com/SHAdd0WTAka/kimi-clawviewer.git
cd kimi-clawviewer/project

# Install frontend dependencies
npm install

# Run development server
npm run tauri dev
```

### Building

```bash
# Desktop application (all platforms)
cargo tauri build

# Headless server
cargo build --release --features headless

# Docker deployment
docker-compose up -d
```

### Configuration

Configuration is loaded from (in order of priority):
1. Environment variables (`CLAWVIEWER_*`)
2. `~/.config/clawviewer/config.json`
3. Default values

Example configuration:
```json
{
  "app": {
    "name": "ClawViewer Enterprise",
    "environment": "production"
  },
  "security": {
    "auth_enabled": true,
    "session_timeout_seconds": 3600,
    "require_mfa": false
  },
  "network": {
    "signaling_server_url": "wss://signal.example.com",
    "max_connections": 10
  },
  "capture": {
    "default_fps": 30,
    "hardware_acceleration": true
  }
}
```

## Security

- **Password Policy**: Minimum 12 characters, uppercase, lowercase, number, special character
- **Session Management**: JWT tokens with configurable expiration, automatic cleanup
- **Rate Limiting**: Configurable per-endpoint rate limiting
- **Audit Logging**: All authentication and authorization events logged
- **RBAC**: Role-based access control (admin, operator, viewer)

## Monitoring

- **Metrics Endpoint**: `http://localhost:9090/metrics` (Prometheus format)
- **Health Check**: `http://localhost:9090/health`
- **Structured Logging**: JSON format with configurable levels

## Docker Deployment

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f clawviewer

# Scale monitoring
docker-compose up -d --scale clawviewer=3
```

## Contributing

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Support

- Documentation: https://docs.clawviewer.dev
- Issues: https://github.com/SHAdd0WTAka/kimi-clawviewer/issues
- Enterprise Support: support@clawviewer.dev
