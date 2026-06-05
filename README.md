# Kimi Agent ClawViewer

Entpackt aus `Kimi_Agent_ClawViewer 100代理.zip` (26 MB)

## Inhalt

| Ordner/Datei | Beschreibung |
|-------------|--------------|
| `project/` | Rust + TypeScript/Tauri Projekt (Screen Capture, WebRTC, MCP) |
| `research/` | Analyse-Dokumente (12 Dimensionen) |
| `SPEC.md` | Spezifikation |
| `clawviewer_sec*.md` | Sektionen der technischen Architektur |
| `*.docx` | Word-Dokumente |

## Projekt-Struktur (project/)

```
project/
├── Cargo.toml          # Rust Workspace
├── package.json        # Node.js Dependencies
├── src/                # React/TypeScript Frontend
│   ├── App.tsx
│   ├── components/     # UI-Komponenten
│   └── hooks/          # WebRTC Hook
├── src-tauri/          # Tauri Desktop-Wrapper
└── crates/             # Rust Crates
    ├── cv-capture/     # Screen Capture (DXGI)
    ├── cv-input/       # Input Simulation
    ├── cv-mcp/         # MCP Server
    ├── cv-network/     # WebRTC/Signaling
    ├── cv-security/    # Auth/Session
    └── cv-shared/      # Protobuf/Types
```

## Erstellt
- Datum: 2026-06-05
- Quelle: Downloads-Ordner
