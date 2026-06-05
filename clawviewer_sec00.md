# ClawViewer – Technische Architektur-Dokumentation

## KI-gestutzte Remote-Desktop-Control-App mit P2P-Architektur und MCP-Server-Integration

**Projekt**: ClawViewer
**Version**: 1.0
**Datum**: 5. Juni 2026
**Autoren**: Multi-Agent Research & Writing Pipeline

---

## Executive Summary

### Projektziel und Kerninnovation

ClawViewer ist eine moderne, Open-Source-Remote-Desktop-Control-Anwendung, die drei bisher getrennte Technologiebereiche integriert: echte Peer-to-Peer-Remote-Desktop-Verbindungen, KI-gestutzte Co-Pilot-Funktionalitat und bidirektionale Human-plus-AI-Steuerung. Im Gegensatz zu klassischen Remote-Desktop-Tools, die entweder auf Client-Server-Architekturen oder einfache VNC-Pipes setzen, ermoglicht ClawViewer es beliebigen Nutzern (Mensch oder KI-Agent), sich direkt miteinander zu verbinden und gleichzeitig denselben Desktop zu steuern – wobei ein Event-Prioritatssystem sicherstellt, dass menschliche Eingaben stets Vorrang haben.

Die Kerninnovation liegt in der Architektur: Ein Tauri-v2-Desktop-Client (Rust-Backend, React-Frontend, ~5–15 MB Bundle) nutzt WebRTC fur P2P-Verbindungen mit automatischem NAT-Traversal, wahrend ein integrierter MCP-Server (Model Context Protocol) KI-Agenten den kontrollierten Zugriff auf den Remote-Desktop ermoglicht. Die Anwendung unterstutzt drei Einklink-Modi – Observer (KI sieht nur zu), Shared (gleichzeitige Steuerung mit Ghost-Cursor) und Full-Control (KI mit Sicherheits-Safeguards) – und implementiert eine lokale Text-to-Speech-Engine fur KI-Sprachausgabe.

### Vier Deliverables im Uberblick

Diese Dokumentation liefert vier integrierte Deliverables, die aus einer 12-dimensionalen Deep-Research-Phase mit uber 280 Web-Searches und der Analyse von sechs Open-Source-Projekten hervorgegangen sind:

**Deliverable 1 – Technische Architektur-Dokumentation** (Kapitel 1): Beschreibt die vollstandige Systemarchitektur mit Komponenten-Diagrammen, P2P-Handshake-Protokoll, Screen-Capture-Pipeline (DXGI/PipeWire), Event-Priorisierungssystem (P0–P3), MCP-Server-Integration und Chat-Viewer. Die Architektur basiert auf dem bewahrten Blueprint von RustDesk (hbbs/hbbr, Ed25519-Auth) und erganzt diesen mit WebRTC-NAT-Traversal und KI-Agent-Integration.

**Deliverable 2 – Sicherheitskonzept** (Kapitel 2): Definiert eine vierlagige Defense-in-Depth-Architektur: Ed25519-Kryptografie fur Transportsicherheit, sessionspezifische Einmalpassworter (6-stellige alphanumerische Worter oder 12-stellige Tokens) mit automatischer Rotation, OS-Keyring-Integration fur API-Key-Speicherung (BYOK: Bring Your Own Key) und eine dreischichtige KI-Sandbox mit Risk-Level-Klassifizierung und Human-in-the-Loop-Bestatigung fur alle High-Risk-Aktionen.

**Deliverable 3 – Code-Analyse-Report** (Kapitel 3): Analysiert sechs Open-Source-Projekte auf Quellcode-Ebene – RustDesk (P2P-Server und Client), FreeRDP (RDP-Protokoll-Implementierung), UltraVNC/TightVNC/LibVNCServer (RFB-Protokoll), xrdp (Linux RDP-Server mit Session-Management) und Remmina (Multi-Protokoll-Client mit Plugin-System). Extrahiert 113 konkrete Code-Referenzen und sechs wiederverwendbare Design-Patterns als direkte Blueprints fur die ClawViewer-Implementierung.

**Deliverable 4 – Proof-of-Concept-Plan** (Kapitel 4): Spezifiziert einen 12-wochigen Entwicklungsplan mit sechs zweiwochentlichen Meilensteinen, der von Tauri-Projekt-Setup (Woche 1–2) uber P2P-Verbindungsaufbau (Woche 3–4), Windows-Screen-Capture (Woche 5–6), Input-Injection (Woche 7–8), MCP-Server-Integration (Woche 9–10) bis zur Cross-Platform-Testphase (Woche 11–12) fuhrt. Definiert acht technische Risiken mit Mitigationen und quantifizierte Erfolgskriterien (<50 ms Video-Latenz, <10 ms Input-Latenz, >95% P2P-Stabilitat).

### Kernerkenntnisse aus der Recherche

Die 12-dimensionale Recherchephase, die uber 280 Web-Searches und die Analyse von sechs GitHub-Repositories umfasste, hat zehn cross-dimensionale Insights hervorgebracht, die die Architekturentscheidungen von ClawViewer pragen:

1. **RustDesk als Master-Blueprint**: Die Kombination aus Google Protobuf v3 fur das Wire-Protokoll, tokio fur asynchrone Netzwerkoperationen und ed25519-dalek fur Kryptografie in RustDesk bildet ein vollstandiges, produktionsreifes Muster, das sich direkt auf ClawViewer ubertragt.

2. **Plugin-Architektur = Multi-Controller-Architektur**: Remminas Plugin-Service-Pattern und xrdps dynamisches Modul-System demonstrieren das gleiche Pattern, das fur Human-plus-AI-Steuerung benotigt wird – ein Core-System delegiert Aktionen an registrierte Controller.

3. **Tauri + webrtc-rs als optimaler Stack**: Tauri v2's IPC-System ermoglicht die saubere Trennung zwischen Web-Frontend (UI) und Rust-Backend (P2P, Capture, Input). WebRTC-Signaling im Frontend, P2P-Transport im Backend via webrtc-rs.

4. **MCP-Server als universelle KI-Integrations-Schicht**: Das Model Context Protocol standardisiert die KI-Agenten-Anbindung und decoupled Sicherheitsentscheidungen von der UI-Implementierung.

5. **Event-Priorisierung als Differenzierungsfaktor**: Keines der analysierten Open-Source-Projekte implementiert echte bidirektionale gleichzeitige Steuerung mit Priorisierung. Forschung zu Shared Autonomy zeigt, dass Level-2-Interleaving die beste Success Rate (80%) erzielt.

6. **Cross-Platform-Input als FFI-Schichten-Problem**: Enigo-ahnliche Abstraktion uber Tauri's FFI mit plattformspezifischen Rust-Modulen (SendInput, uinput, CGEvent) ist das bewahrte Pattern.

7. **TTS lokal, nicht remote**: Piper TTS (~120 ms Latenz auf Intel i5) als lokale Engine mit direkter Audio-Wiedergabe via rodio/cpal, nicht als WebRTC-Stream.

8. **Screen-Capture-Codec-Pipeline vollstandig in Rust**: Capture -> Encode -> Packetize als Rust-interne Pipeline, Frontend empfangt nur WebRTC-Video-Frames.

9. **Chat als DataChannel-Overlay**: Chat-Nachrichten uber denselben WebRTC-DataChannel wie Input-Events, mit separatem Message-Type, keine zusatzliche Verbindung notig.

10. **Security-First durch Schichtentrennung**: Ed25519 (Transport) + Session-Passworter (Auth) + OS-Keyring (Credentials) + MCP-Sandbox (KI-Sicherheit) als vier unabhangige, auditierbare Schichten.

---

