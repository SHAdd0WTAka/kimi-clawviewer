# ClawViewer – Entwicklungsplan: 100-Agent Proof-of-Concept

## Ziel
Entwicklung des ClawViewer PoC (Proof-of-Concept) – erste lauffahige Milestone mit:
- P2P-Handshake (WebRTC, Signaling-Server)
- Windows Screen-Capture (DXGI Desktop Duplication)
- Input-Loop (SendInput Maus/Tastatur)
- KI-Observer-Modus (MCP-Server Grundgerüst)
- Tauri v2 UI (React-Frontend)

## Architektur: 8 Rust-Crates + Tauri-Frontend

### Crate-Struktur:
```
clawviewer/
├── Cargo.toml                    # Workspace-Manifest
├── crates/
│   ├── cv-network/               # P2P, WebRTC, Signaling
│   ├── cv-capture/               # Screen-Capture (DXGI, PipeWire)
│   ├── cv-codec/                 # Video-Encoding (H.264, VP9)
│   ├── cv-input/                 # Input-Injection (SendInput, uinput)
│   ├── cv-mcp/                   # MCP-Server (KI-Agent Integration)
│   ├── cv-security/              # Auth, Ed25519, Session-Mgmt
│   ├── cv-tts/                   # Text-to-Speech (Piper)
│   └── cv-shared/                # Gemeinsame Typen, Protobuf, Utils
├── src-tauri/                    # Tauri v2 App (Rust-Backend)
│   ├── src/
│   │   ├── main.rs               # Tauri Entrypoint
│   │   ├── commands/             # Tauri Commands (IPC)
│   │   ├── state.rs              # App-State Management
│   │   └── webrtc_bridge.rs      # WebRTC-Rust-Bridge
│   └── Cargo.toml
└── src/                          # React-Frontend
    ├── components/
    ├── App.tsx
    └── main.tsx
```

## Stage 1: Projekt-Setup & Grundgerüst (sequential)
- Agent: project_setup – Workspace, Cargo.toml, Tauri v2 Init

## Stage 2: Core-Crates parallel (Round 1 – keine Abhängigkeiten)
- cv-shared: Gemeinsame Typen, Protobuf-Defs, Error-Handling
- cv-security: Ed25519 Key-Gen, Session-Passwort, OS-Keyring
- cv-capture: DXGI Desktop Duplication API (Windows)
- cv-input: SendInput Maus/Tastatur (Windows)

## Stage 3: Higher-Level-Crates parallel (Round 2 – hängen von Round 1 ab)
- cv-codec: Video-Encoding (bindet cv-capture an)
- cv-network: WebRTC P2P, Signaling-Server (bindet cv-security an)
- cv-mcp: MCP-Server-Grundgerüst (unabhängig)
- cv-tts: Piper TTS Integration (unabhängig)

## Stage 4: Tauri-Integration & Frontend (Round 3 – hängt von allen Rust-Crates ab)
- Tauri-Commands: IPC zwischen Rust und React
- WebRTC-Bridge: Frontend-Signaling + Rust-P2P
- React-Frontend: UI-Komponenten (Screen-View, Chat, Controls)

## Stage 5: Integration & Build
- Cargo Workspace Build
- Integration-Tests
- README + Setup-Anleitung

## Skill-Zuordnung
- **Capability**: vibecoding-general-swarm (Rust Desktop-App)
- **Artifact**: docx (Doku), aber primär Code

## Technologie-Stack
- Rust 1.80+ mit tokio async
- Tauri v2 mit React + TypeScript + Vite
- webrtc-rs oder str0m für P2P
- windows crate für DXGI/SendInput
- ed25519-dalek für Krypto
- keyring für OS-Keyring
- tauri-plugin-global-shortcut für Hotkeys
