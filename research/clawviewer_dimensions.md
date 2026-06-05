# ClawViewer – Dimension Decomposition (Phase 2)

## Route: D (File-Augmented Research)

## Dimensionen (12 total)

### Dim 01: RustDesk P2P-Server-Architektur (hbbs/hbbr)
- Scope: Rendezvous-Server (hbbs), Relay-Server (hbbr), P2P-Handshake, Ed25519-basierte Authentifizierung, ID-Registrierung, NAT-Traversal-Koordination
- Angle: Code-Ebene – konkrete Rust-Module, Protokoll-Handler, Datenstrukturen
- Source Types: GitHub Source Code (rustdesk/rustdesk-server), RustDesk Docs

### Dim 02: RustDesk Client-Implementierung
- Scope: Screen-Capture-Integration (Windows DXGI, Linux PipeWire), Video-Codec-Pipeline, Input-Injection, P2P-Client-Logik
- Angle: Code-Ebene – Client-Seite Architektur, OS-abstraktion, Video-Streaming
- Source Types: GitHub Source Code (rustdesk/rustdesk), RustDesk Docs

### Dim 03: FreeRDP Protokoll & Codec-Implementierung
- Scope: RDP-Core-Protokoll, H.264-Codec-Handling, Input-Redirection, Virtual-Channel-System
- Angle: Code-Ebene – C-Implementierung, Protokoll-State-Machine, Codec-Decoder
- Source Types: GitHub Source Code (FreeRDP/FreeRDP), FreeRDP Wiki

### Dim 04: VNC-Ecosystem (UltraVNC / TightVNC / LibVNCServer)
- Scope: RFB-Protokoll-Implementierung, Framebuffer-Update-Mechanismus, VNC-Viewer/Server-Architektur, Encoding-Handler
- Angle: Code-Ebene – C/C++ Implementierung, Protokoll-Handler, Pixel-Streaming
- Source Types: GitHub Source Code (ultravnc/ultravnc, LibVNC/tightvnc, LibVNC/libvncserver)

### Dim 05: xrdp Linux RDP-Server
- Scope: Session-Management, X11-Integration, RDP-Server-Implementierung, Module-System
- Angle: Code-Ebene – C-Implementierung, Session-Handler, X11-Forwarding
- Source Types: GitHub Source Code (neutrinolabs/xrdp), xrdp Wiki

### Dim 06: Remmina Multi-Protokoll-Client
- Scope: Plugin-System-Architektur, GTK-UI, Multi-Protokoll-Handler (RDP/VNC/SSH), Connection-Manager
- Angle: Code-Ebene – C-Implementierung, Plugin-API, UI-Architektur
- Source Types: GitHub Source Code (FreeRDP/Remmina), Remmina Docs

### Dim 07: WebRTC P2P & NAT-Traversal
- Scope: libwebrtc Implementierung, ICE/STUN/TURN, RTCPeerConnection, DataChannel, P2P-Handshake
- Angle: Technische Architektur – WebRTC-Interna, NAT-Traversal-Algorithmen, SDP/Offer/Answer
- Source Types: WebRTC Specs, libwebrtc Source, RFCs

### Dim 08: Tauri v2 + Rust Backend-Architektur
- Scope: Tauri v2 Command-System, FFI zu nativem Code, OS-Integration, Multi-Window, Plugin-System
- Angle: Framework-Architektur – Rust/JS Interop, native Module, Build-System
- Source Types: Tauri Docs, GitHub (tauri-apps/tauri), Beispiel-Projekte

### Dim 09: MCP-Server Protokoll & KI-Agent Integration
- Scope: Model Context Protocol Spezifikation, Tool-Use Pattern, stdio/SSE Transport, JSON-RPC
- Angle: Protokoll-Spezifikation – MCP-Server Implementierung, KI-Agent Anbindung
- Source Types: MCP Spec, QuickDesk GitHub, Beispiel-Implementierungen

### Dim 10: Sicherheitsarchitektur & Auth-Flows
- Scope: Ed25519-Key-Pairs, Session-basierte Auth, API-Key-Management (OS Keyring), KI-Sandbox
- Angle: Sicherheitsdesign – Kryptografie, Key-Derivation, Permission-Model
- Source Types: RustDesk Auth-Code, Rust crypto crates, OS Keyring APIs

### Dim 11: Bidirektionale Steuerung & Event-Priorisierung
- Scope: Event-Queue mit Prioritaten, Ghost-Cursor Overlay, Input-Merging, Human-Override
- Angle: UX/Input-Architektur – Event-System-Design, Cursor-Rendering, Conflict-Resolution
- Source Types: QuickDesk, RustDesk Input-Handling, UI-Automation Frameworks

### Dim 12: TTS & Audio-Pipeline
- Scope: Piper TTS (Rust), Coqui TTS, Edge-TTS, WebRTC Audio Track, lokale Audio-Wiedergabe
- Angle: Audio-Architektur – TTS-Integration, Audio-Streaming, asynchrone Verarbeitung
- Source Types: Piper GitHub, Coqui TTS, WebRTC Audio Docs
