# ClawViewer – Technische Architektur-Dokumentation: KI-gestutzte Remote-Desktop-Control-App

## Executive Summary
### Projektziel und Kerninnovation
#### ClawViewer als Open-Source-Remote-Desktop-App mit integriertem KI-Co-Pilot, bidirektionaler Human+AI-Steuerung und P2P-Architektur auf Basis von RustDesk-Referenzmustern
### Vier Deliverables im Uberblick
#### Technische Architektur-Dokumentation, Sicherheitskonzept, Code-Analyse-Report von 6 Open-Source-Projekten, Proof-of-Concept-Plan mit erstem Meilenstein
### Kernerkenntnisse aus der Recherche
#### RustDesk als bewahrter Blueprint, Tauri v2 + webrtc-rs als optimaler Stack, MCP-Server als KI-Standard, Event-Priorisierung als Differenzierungsfaktor

## 1. Technische Architektur-Dokumentation (~4500 Worter, 5 Tabellen, 3 Diagramme)
### 1.1 Systemubersicht und Komponenten-Architektur
#### 1.1.1 Gesamtarchitektur: Tauri v2 Desktop-App mit Rust-Backend und React-Frontend, 3-15MB Bundle
#### 1.1.2 Kernkomponenten: Screen-Capture-Engine, Video-Codec-Pipeline, P2P-Netzwerk-Stack, Input-Injection-Layer, MCP-Server, TTS-Engine
#### 1.1.3 Komponenten-Diagramm: Module und ihre Schnittstellen (Tabelle: Komponente, Technologie, Verantwortlichkeit)
#### 1.1.4 Prozess-Architektur: Main-Process (Tauri Core), Renderer-Process (UI), Rust-Sidecar (Screen-Capture, P2P, Input)
### 1.2 P2P-Architektur und NAT-Traversal
#### 1.2.1 RustDesk-ahnliche P2P-Struktur: Rendezvous-Server (Signaling), optionaler Relay-Server, direkte Peer-Verbindung
#### 1.2.2 NAT-Traversal: STUN/TURN/ICE mit webrtc-rs oder str0m, UDP-Hole-Punching, Fallback-Relay
#### 1.2.3 Verbindungsaufbau: 6-Schritt-Handshake (Register -> PunchHoleRequest -> PunchHole -> PunchHoleSent -> PunchHoleResponse -> Direct Connect)
#### 1.2.4 Protokoll-Stack: Google Protobuf v3 fur Wire-Format, WebRTC fur Transport, DTLS-SRTP fur Verschlusselung
#### 1.2.5 Port-Konfiguration und Netzwerk-Topologie (Tabelle: Komponente, Port, Protokoll, Funktion)
### 1.3 Screen-Capture und Video-Pipeline
#### 1.3.1 Windows: DXGI Desktop Duplication API mit Hardware-beschleunigtem Capture (1-3ms Latenz fur 1080p)
#### 1.3.2 Linux: PipeWire / DMA-BUF (Wayland) und X11 SHM (Fallback), GStreamer-Pipeline
#### 1.3.3 macOS: CGDisplayStream als Capture-Methode
#### 1.3.4 Codec-Pipeline: H.264/VP9/AV1 mit Hardware-Encoder-Auto-Selektion, Frame-Pacing fur <50ms End-to-End-Latenz
#### 1.3.5 Video-Streaming uber WebRTC Video-Track mit RTCP-Feedback (NACK/PLI, Bandwidth Estimation)
### 1.4 Input-Weiterleitung und Event-System
#### 1.4.1 OS-native Input-Injection: Windows (SendInput), Linux (uinput/XTest), macOS (CGEvent) uber Enigo-ahnliche Abstraktion
#### 1.4.2 JSON-Event-Schema: {source: "human"|"ai", type: "mouse"|"keyboard"|"scroll", priority: P0-P3, payload, timestamp}
#### 1.4.3 Event-Prioritats-System: P0 Emergency Stop, P1 Human Input (immer Vorrang), P2 AI mit Bestatigung, P3 AI autonom
#### 1.4.4 Event-Merging: KI-Cursor als Ghost-Overlay, Human-Cursor als primarer Zeiger, Input-Koaleszenz-Strategien
#### 1.4.5 Ubertragung via WebRTC DataChannel (SCTP/DTLS), ordered + reliable fur Input-Events
### 1.5 KI-Agent-Integration und MCP-Server
#### 1.5.1 Einklink-Modi: Observer (nur sehen), Shared (gleichzeitig), Full-Control (mit Safeguards)
#### 1.5.2 MCP-Server-Architektur: JSON-RPC 2.0 uber stdio/SSE, Tool-Definition (inputSchema/outputSchema), 40+ Tools
#### 1.5.3 MCP-Tool-Ubersicht: screenshot, mouse_click, mouse_drag, keyboard_type, clipboard_get/set, ocr_recognize, ui_element_detect
#### 1.5.4 Safety-Safeguards: KI darf NIEMALS ohne explizite Bestatigung: Dateien loschen, System-Einstellungen andern, Passworter eingeben, Transaktionen bestatigen, sudo-Commands
#### 1.5.5 BYOK-Modell: User bringt eigene AI-API-Keys (OpenAI, Anthropic, Google), gespeichert im OS-Keyring
### 1.6 Chat-Viewer und Real-Time-Kommunikation
#### 1.6.1 MS-Teams-ahnliches Chat-Fenster als separates Tauri-WebviewWindow
#### 1.6.2 Nachrichten-Format: JSON mit Typen chat, system, ai-action, human-override uber WebRTC DataChannel
#### 1.6.3 UI-Komponenten: Nachrichten-Thread, KI-Status-Indikator, Session-Info, Passwort-Display
#### 1.6.4 Real-Time-Chat-Architektur: WebRTC DataChannel primar, WebSocket-Fallback
### 1.7 Tech-Stack-Ubersicht
#### 1.7.1 Vollstandiger Stack-Tabelle: Layer, Primartechnologie, Alternative, Begrundung
#### 1.7.2 Rust-Crate-Okosystem: tokio, webrtc-rs, ed25519-dalek, keyring, piper-rs, rodio, enigo, protobuf
#### 1.7.3 Frontend-Stack: React/Vite, WebRTC-APIs, Tauri-IPC (invoke/state)

## 2. Sicherheitskonzept (~3500 Worter, 4 Tabellen, 2 Diagramme)
### 2.1 Authentifizierungs-Architektur
#### 2.1.1 Ed25519-Key-Pairs fur jede Installation: Generierung, Key-Derivation, Persistenz in id_ed25519/id_ed25519.pub
#### 2.1.2 Trust-On-First-Use (TOFU): Public-Key-Fingerprint-Verifikation bei erster Verbindung, SSH-Stil
#### 2.1.3 Challenge-Response-Auth: Ed25519-Signing uber Protobuf-Nachrichten, UUID-Validierung
#### 2.1.4 Multi-Faktor-Auth: Session-Passwort + Ed25519-Key + optional 2FA/TOTP
### 2.2 Session-basierte Authentifizierung und Passwort-Generierung
#### 2.2.1 Session-Passwort-Generierung: 6-stelliges alphanumerisches Wort oder 12-stelliges Token via OsRng
#### 2.2.2 Passwort-Rotation: Neues Passwort bei jeder Session, keine persistenten Credentials
#### 2.2.3 Session-Lifecycle: Erstellung -> Aktiv -> Idle-Timeout (5 Min) -> Expired -> Cleanup
#### 2.2.4 Rust-Implementierung: rand::OsRng, chbs fur Diceware-Worter, zeroize fur Memory-Clearing
### 2.3 API-Key-Management mit OS-Keyring
#### 2.3.1 BYOK-Architektur: User bringt eigene Keys, App speichert nur lokal, nie Server-Kontakt fur Keys
#### 2.3.2 OS-Keyring-Integration: Windows DPAPI, Linux Secret Service (D-Bus), macOS Keychain
#### 2.3.3 Rust-Implementierung: keyring v4 Crate, Service-Name "clawviewer", Account-Name per Provider
#### 2.3.4 Key-Rotation und Revocation: 30-90 Tage Rotation empfohlen, One-Click-Revocation
### 2.4 KI-Sandbox und Safety-Safeguards
#### 2.4.1 Drei-Schichten-Sandbox: Environment (Was sieht die KI?), Permissions (Was darf die KI?), Runtime (Was tut die KI?)
#### 2.4.2 Risk-Level-Klassifizierung: Low (Text eingeben), Medium (Datei offnen), High (System-Command, Loschung)
#### 2.4.3 Human-in-the-Loop: Bestatigungsdialoge fur alle High-Risk-Aktionen, MCP-Elicitation-Pattern
#### 2.4.4 Action-Whitelist: Explizit erlaubte Aktionen, Default-Deny fur alle nicht gelisteten Operationen
#### 2.4.5 Audit-Trail: Logging aller KI-Aktionen mit Zeitstempel, Nutzer-Bestatigung, Ergebnis
### 2.5 Transport-Sicherheit und Verschlusselung
#### 2.5.1 DTLS-SRTP: WebRTC-mandatory End-to-End-Verschlusselung fur alle P2P-Daten
#### 2.5.2 TLS 1.3: Fur Signaling-Server-Verbindung (Rendezvous), rustls-Implementierung
#### 2.5.3 Rust-Crypto-Stack: ed25519-dalek + x25519-dalek + crypto_box + rustls + zeroize
#### 2.5.4 Security-Header und Hardening: Certificate-Pinning, Perfect Forward Secrecy, Anti-Replay

## 3. Code-Analyse-Report der Open-Source-Projekte (~5500 Worter, 6 Tabellen, 6 Projekt-Abschnitte)
### 3.1 RustDesk Server (hbbs/hbbr) – P2P-Architektur und Relay
#### 3.1.1 Repository-Struktur: rustdesk/rustdesk-server, 6 Hauptmodule, tokio-async-Runtime
#### 3.1.2 Rendezvous-Server (hbbs): RegisterPeer, RegisterPk, SQLite-Persistenz, In-Memory-Cache (Tabelle: API-Endpunkte)
#### 3.1.3 Relay-Server (hbbr): UUID-basiertes Peer-Pairing, bidirektionale tokio::select!-Weiterleitung, Bandbreiten-Limiting
#### 3.1.4 P2P-Handshake: 6-Schritt-Protokoll mit TCP/UDP-Hole-Punching, jittered retries, Relay-Fallback
#### 3.1.5 Ed25519-Auth: sodiumoxide::crypto::sign, TOFU mit UUID, License-Key-Parameter
#### 3.1.6 Blueprint fur ClawViewer: Protobuf-Protokoll, tokio-Runtime, Ed25519-Auth-Modul
### 3.2 RustDesk Client – Screen-Capture und Input
#### 3.2.1 Repository: rustdesk/rustdesk, Workspace-Struktur mit libs/scrap, libs/enigo
#### 3.2.2 DXGI-Capture: Capturer-Struktur mit ID3D11Device + IDXGIOutputDuplication, GDI-Fallback
#### 3.2.3 PipeWire-Integration: GStreamer-Pipeline, DBus xdg-desktop-portal, Restore-Token
#### 3.2.4 Codec-Pipeline: 4 Encoder-Backends (VPX, AOM, HWRAM, VRAM), Auto-Selektion H265>H264>AV1>VP9>VP8
#### 3.2.5 Input-Injection: Enigo-Abstraktion mit SendInput/uinput/CGEvent, serverseitige input_service.rs
#### 3.2.6 Blueprint fur ClawViewer: scrap-Crate-Struktur, Enigo-Abstraktion, Codec-Auto-Selektion
### 3.3 FreeRDP – RDP-Protokoll-Implementierung
#### 3.3.1 Repository: FreeRDP/FreeRDP, CMake-Build, libfreerdp + winpr + client
#### 3.3.2 RDP-State-Machine: libfreerdp/core/rdp.c (3.227 ZL), connection.c (2.259 ZL), state.h
#### 3.3.3 H264-Codec: Multi-Backend (FFmpeg, VAAPI, DXVA, VideoToolbox, MediaCodec), AVC444-Modus
#### 3.3.4 Virtual-Channel-System: 30+ Kanale, DVC uber channels/drdynvc/, GFX uber channels/rdpgfx/
#### 3.3.5 Security: NLA in nla.c (2.475 ZL), CredSSP, TLS, RDP-Encryption in security.c
#### 3.3.6 Blueprint fur ClawViewer: State-Machine-Pattern, Multi-Backend-Codec, Channel-System
### 3.4 VNC-Ecosystem – RFB-Protokoll und Framebuffer
#### 3.4.1 LibVNCServer: rfbserver.c (Main Loop rfbProcessEvents), sraRegionPtr (modifizierte Regionen)
#### 3.4.2 Encoding-Handler: Hextile (hextile.c), Tight (tight.c mit JPEG/Zlib), ZRLE (zrle.c), Raw
#### 3.4.3 UltraVNC: Video Hook Driver, Desktop Duplication API, DSM-Encryption-Plugin
#### 3.4.4 Input-Handling: rfbPointerEventMsg (Typ 5), rfbKeyEventMsg (Typ 4), Client-Callbacks
#### 3.4.5 Blueprint fur ClawViewer: Region-basiertes Update-Tracking, Encoding-Verhandlung, Callback-Architektur
### 3.5 xrdp – Linux RDP-Server und Session-Management
#### 3.5.1 Multi-Prozess-Architektur: xrdp (Listener) + sesman (Session Manager) + sesexec (Executor)
#### 3.5.2 Session-Management: SCP-Protokoll, Session-Listen, Policies (UBC/UBD/UBI), EICP/ERCP
#### 3.5.3 X11-Integration: xorgxrdp (bevorzugtes Backend), Xvnc (Alternative), SHM-Framebuffer
#### 3.5.4 Modul-System: Dynamisches .so-Loading (libxup.so, libvnc.so), Modul-API
#### 3.5.5 Blueprint fur ClawViewer: Multi-Prozess-Isolation, Session-Manager-Pattern, Modul-API
### 3.6 Remmina – Multi-Protokoll-Client-Architektur
#### 3.6.1 Plugin-System: RemminaPluginService-Struct (100+ Funktionen), 7 Plugin-Typen, GModule-Loading
#### 3.6.2 Protocol-Abstraction: RemminaProtocolWidget als Container, GHashTable-Settings
#### 3.6.3 RDP-Plugin: FreeRDP-basiert, 7 Features, rdp_plugin.c
#### 3.6.4 VNC-Plugin: libvncclient-basiert, 10 Features, vnc_plugin.c
#### 3.6.5 Blueprint fur ClawViewer: Plugin-Service-Pattern, Protocol-Widget-Abstraktion, Feature-Registrierung
### 3.7 Gemeinsame Muster und Architektur-Blueprints
#### 3.7.1 Pattern-Ubersicht: 6 extrahierte Design-Patterns aus allen Projekten (Tabelle: Pattern, Quelle, Anwendung in ClawViewer)
#### 3.7.2 Implementierungs-Matrix: Welche konkreten Dateien/Funktionen als direkte Referenz dienen

## 4. Proof-of-Concept-Plan (~3500 Worter, 3 Tabellen, 1 Roadmap-Diagramm)
### 4.1 Vision und Ziel des PoC
#### 4.1.1 Ziel: Erste lauffahige Milestone mit P2P-Handshake + Screen-Capture + Input-Loop
#### 4.1.2 Erfolgskriterien: <50ms Video-Latenz, <10ms Input-Latenz, stabile P2P-Verbindung, KI-Observer-Modus
### 4.2 Meilensteine und Roadmap
#### 4.2.1 Milestone 1 (Woche 1-2): Projekt-Setup, Tauri v2 + Rust-Backend, UI-Grundgerust
#### 4.2.2 Milestone 2 (Woche 3-4): P2P-Verbindung, Signaling-Server (WebSocket), WebRTC-Integration
#### 4.2.3 Milestone 3 (Woche 5-6): Windows Screen-Capture (DXGI), Video-Codec (H.264), Streaming-Loop
#### 4.2.4 Milestone 4 (Woche 7-8): Input-Injection (SendInput), Event-System, Prioritats-Queue
#### 4.2.5 Milestone 5 (Woche 9-10): MCP-Server-Grundgerust, KI-Observer-Modus, TTS-Integration
#### 4.2.6 Milestone 6 (Woche 11-12): Chat-Viewer, Sicherheitsfeatures, Cross-Platform-Tests
### 4.3 Architektur der ersten lauffahigen Version
#### 4.3.1 Modul-Struktur: 8 Rust-Crates (network, capture, codec, input, mcp, security, tts, app)
#### 4.3.2 Crate-Abhangigkeiten und Interface-Definitionen (Diagramm: Crate-Dependency-Graph)
#### 4.3.3 Frontend-Architektur: React-Komponenten, State-Management, Tauri-IPC-Wrapper
### 4.4 Technische Risiken und Mitigationen
#### 4.4.1 Risiko-Matrix: 8 identifizierte Risiken (Tabelle: Risiko, Wahrscheinlichkeit, Impact, Mitigation)
#### 4.4.2 Kritische Pfad-Analyse: Abhangigkeiten zwischen Modulen und langste Pfad
#### 4.4.3 Fallback-Strategien: WebRTC-Fallbacks, Codec-Fallbacks, OS-Fallbacks

# References
## clawviewer.agent.outline.md
- **Type**: Report outline
- **Description**: This outline file
- **Path**: /mnt/agents/output/clawviewer.agent.outline.md

## Research Artifacts
- **Type**: Deep Research Dimension Reports
- **Description**: 12 Dimension Reports, Cross-Verification, Insights
- **Path**: /mnt/agents/output/research/clawviewer_dim01.md through clawviewer_dim12.md, clawviewer_cross_verification.md, clawviewer_insight.md

## Phase-1-Recherche
- **Type**: Source file
- **Description**: Phase 1 Recherche zu Remote-Desktop-Architektur
- **Path**: /mnt/agents/upload/phase1-technical-architecture.md
