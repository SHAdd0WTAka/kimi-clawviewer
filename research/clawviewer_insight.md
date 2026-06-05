# ClawViewer – Cross-Dimension Insights (Phase 6)

## Insight 1: RustDesk's Protobuf + tokio-Architektur als Master-Blueprint
- **Insight**: Die Kombination aus Google Protobuf v3 fur Wire-Protokoll, tokio fur async-Netzwerk und sodiumoxide fur Ed25519-Krypto in RustDesk bildet ein vollstaendiges, produktionsreifes Muster, das sich 1:1 auf ClawViewer uebertragen laesst. Alle drei Komponenten (hbbs/hbbr/Client) sind in Rust und decken exakt die Anforderungen ab.
- **Derived From**: Dim01 (RustDesk Server), Dim02 (RustDesk Client), Dim10 (Sicherheit)
- **Rationale**: RustDesk hat exakt die gleiche Problemstellung geloest (P2P-Remote-Desktop mit Auth) und alle Schichten (Netzwerk, Krypto, Codec, Input) in Rust implementiert. Die Crate-Kombination ist bewaehrt.
- **Implications**: ClawViewer kann RustDesk als Referenz-Implementierung nutzen und muss keine experimentellen Architekturen erfinden. Die hbbs/hbbr-Struktur passt direkt auf ClawViewer's Signaling/Relay-Server.
- **Confidence**: high

## Insight 2: Plugin-Architektur = Multi-Controller-Architektur
- **Insight**: Remminas Plugin-Service-Architektur (RemminaPluginService-Struct mit Funktionszeigern) und xrdps dynamisches Modul-System (.so Loading) demonstrieren das gleiche Pattern, das fuer ClawViewer's Multi-Controller-Support (Human + AI) benoetigt wird: Ein Core-System delegiert Aktionen an registrierte Plugins/Controller, die sich gegenseitig nicht kennen muessen.
- **Derived From**: Dim06 (Remmina), Dim05 (xrdp), Dim11 (Bidirektionale Steuerung)
- **Rationale**: Beide Projekte loesen das Problem "Wie integriert man multiple unabhaengige Aktionsträger in eine gemeinsame Session?" – Remmina via Protokoll-Plugins, xrdp via Backend-Module. ClawViewer kann dieses Pattern auf Human-Controller und AI-Controller anwenden.
- **Implications**: Die Event-Pipeline von ClawViewer sollte als Plugin-System konzipiert werden, bei dem Human-Input und AI-Input als zwei "Input-Plugins" registriert sind, die ueber die gleiche RemminaPluginService-aehnliche API agieren.
- **Confidence**: high

## Insight 3: Tauri + webrtc-rs Hybrid als optimale Stack-Entscheidung
- **Insight**: Tauri v2's WebView unterstuetzt auf Linux keine vollstaendige WebRTC-Implementierung, waehrend der Rust-Backend-Teil mit webrtc-rs oder str0m eine produktionsreife P2P-Loesung bietet. Die optimale Architektur ist ein Hybrid: WebRTC-Signaling und Video-Decoding im Frontend (Web APIs), P2P-Transport und Screen-Capture im Rust-Backend (webrtc-rs + native APIs).
- **Derived From**: Dim07 (WebRTC), Dim08 (Tauri), Dim02 (Screen-Capture)
- **Rationale**: WebRTC im Browser-Kontext hat Limitationen (kein System-Level-Capture, kein System-Level-Input), waehrend Rust mit webrtc-rs volle Kontrolle ueber den WebRTC-Stack hat. Tauri's IPC-System (invoke/state) erlaubt die saubere Trennung.
- **Implications**: ClawViewer sollte WebRTC-Daten (Video-Frames, DataChannel-Nachrichten) zwischen Rust-Backend und Web-Frontend ueber Tauri's IPC-System austauschen, statt alles im Browser-Kontext zu versuchen.
- **Confidence**: high

## Insight 4: MCP-Server als universelle KI-Integrations-Schicht
- **Insight**: Das Model Context Protocol (MCP) ist nicht nur fuer QuickDesk relevant, sondern stellt einen emergierenden Standard dar, der die Integration von KI-Agenten in Desktop-Anwendungen standardisiert. Die Kombination aus MCP-Server (Rust) + BYOK-API-Keys (OS Keyring) + Safety-Safeguards (Permission-Modell) bildet eine schichtenbasierte Sicherheitsarchitektur.
- **Derived From**: Dim09 (MCP), Dim10 (Sicherheit), Dim11 (Bidirektionale Steuerung)
- **Rationale**: MCP decoupled den KI-Agenten von der Anwendung, das OS-Keyring decoupled die Credentials von der Anwendung, und das Permission-Modell decoupled die Sicherheitsentscheidungen von der Implementierung. Diese drei Schichten sind unabhaengig voneinander test- und auditierbar.
- **Implications**: ClawViewer sollte den MCP-Server als separate Crate/Crate-Modul implementieren, das ueber stdio mit dem KI-Client kommuniziert und ueber eine interne API mit dem Input-System. Die Sicherheitsgrenzen werden im MCP-Server enforced, nicht im UI.
- **Confidence**: high

## Insight 5: Event-Priorisierung als kritischer Innovationsfaktor
- **Insight**: Keines der analysierten Open-Source-Projekte (RustDesk, FreeRDP, VNC, xrdp, Remmina) implementiert echte bidirektionale gleichzeitige Steuerung mit Priorisierung. Dies ist der wichtigste Differenzierungsfaktor von ClawViewer. Die Forschung zu Shared Autonomy (SARI) zeigt, dass Level-2-Interleaving die beste Success Rate (80%) hat.
- **Derived From**: Dim11 (Bidirektionale Steuerung), Dim02 (Input), Dim09 (KI-Agent)
- **Rationale**: Alle bestehenden Remote-Desktop-Tools implementieren "last-write-wins" oder explizites Locking. ClawViewer's Anforderung an gleichzeitige Human+AI-Steuerung ist innovativ und erfordert neues Design.
- **Implications**: Die Event-Architektur ist der kritischste Custom-Code-Bereich. ClawViewer muss eine echtzeitfaehige Priority-Queue (BinaryHeap mit Reverse-Ordering) implementieren, die P0 (Emergency) > P1 (Human) > P2 (AI confirmed) > P3 (AI autonomous) verarbeitet.
- **Confidence**: high

## Insight 6: Cross-Platform-Input als FFI-Schichten-Problem
- **Insight**: Die Input-Injection auf allen drei Plattformen (Windows: SendInput, Linux: uinput/XTest, macOS: CGEvent) erfordert OS-spezifischen Code, der nicht direkt im Web-Frontend laufen kann. Tauri's FFI-Mechanismus (plattformspezifische Rust-Module mit #[cfg(target_os)]) kombiniert mit Enigo-aehnlicher Abstraktion loest dies elegant.
- **Derived From**: Dim02 (Input), Dim08 (Tauri), Dim11 (Event-System)
- **Rationale**: RustDesk's Enigo-Library und Tauri's cfg-basierte Kompilierung zeigen, dass eine einheitliche Rust-API mit plattformspezifischen Backends das optimale Pattern ist.
- **Implications**: ClawViewer sollte eine eigene Input-Abstraction-Crate erstellen (aehnlich Enigo), die ueber Tauri-Commands vom Frontend aufgerufen wird. Die KI sendet Input-Events ueber den MCP-Server, die in die gleiche Queue wie Human-Input eingehen.
- **Confidence**: high

## Insight 7: TTS-Audio als separater P2P-Track
- **Insight**: Die TTS-Ausgabe sollte nicht als WebRTC-Video-Track gemultiplext werden, sondern als separater Audio-Track oder lokale Wiedergabe. Die Latenz von Piper TTS (~120ms) + lokale Audio-Wiedergabe ist deutlich geringer als Remote-Streaming ueber WebRTC.
- **Derived From**: Dim12 (TTS), Dim07 (WebRTC Audio)
- **Rationale**: Die KI-Sprachausgabe ist fuer den lokalen Nutzer bestimmt (nicht fuer den Remote-Peer). Lokale Wiedergabe via rodio/cpal vermeidet unnoetigen Netzwerk-Overhead.
- **Implications**: TTS wird auf dem Client lokal gerendert, der die KI steuert. Der KI-Status-Indikator im UI zeigt an, wenn TTS aktiv ist. Remote-TTS (z.B. fuer Barrierefreiheit) kann als optionaler WebRTC-Audio-Track implementiert werden.
- **Confidence**: high

## Insight 8: Screen-Capture-Codec-Pipeline als Rust-FFI-Problem
- **Insight**: Die Kombination aus OS-nativem Screen-Capture (DXGI/PipeWire) + Hardware-Encoder (H.264/VP9) + WebRTC-Video-Track erfordert eine Rust-interne Pipeline, die ohne JavaScript/Frontend auskommt. Der Frontend-Teil sollte nur den decoded Video-Frame anzeigen.
- **Derived From**: Dim02 (Screen-Capture), Dim07 (WebRTC Video), Dim08 (Tauri)
- **Rationale**: RustDesk's scrap-Crate und die Codec-Auto-Selektion zeigen, dass die gesamte Capture->Encode->Packetize-Pipeline in Rust stattfinden sollte. Das Frontend empfaengt nur WebRTC-Video-Frames.
- **Implications**: ClawViewer braucht eine Rust-Crate "capture" (aehnlich scrap), die DXGI/PipeWire/CGDisplay integriert, Hardware-Encoder automatisch selektiert, und die Frames an webrtc-rs uebergibt. Das Frontend zeigt den Stream via HTMLVideoElement an.
- **Confidence**: high

## Insight 9: Chat-Viewer als DataChannel-Overlay
- **Insight**: Der Chat-Viewer (MS-Teams-aehnlich) kann ueber denselben WebRTC-DataChannel wie Input-Events laufen, mit separatem Message-Type. Dies vermeidet zusaetzliche WebSocket-Verbindungen und nutzt die bestehende P2P-Verbindung.
- **Derived From**: Dim07 (DataChannel), Dim09 (MCP), Dim08 (Tauri Multi-Window)
- **Rationale**: WebRTC DataChannels unterstuetzen multiple Streams mit unterschiedlichen Konfigurationen (ordered/unordered, reliable/unreliable). Chat-Nachrichten (ordered, reliable) und Input-Events (ordered, reliable) koennen den gleichen SCTP-Transport nutzen.
- **Implications**: ClawViewer definiert einen einzigen DataChannel mit JSON-Nachrichten, die ein `type`-Feld haben (input, chat, system, ai-action, human-override). Die Chat-UI ist ein separates Tauri-Fenster, das ueber Tauri-Events mit dem Backend kommuniziert.
- **Confidence**: medium

## Insight 10: Security-First Design durch Schichtentrennung
- **Insight**: Die Kombination aus Ed25519 (Transportsicherheit) + Session-Passwoertern (Authentifizierung) + OS-Keyring (Credential-Speicherung) + MCP-Sandbox (KI-Sicherheit) bildet eine 4-Schichten-Sicherheitsarchitektur, bei der jede Schicht unabhaengig funktioniert.
- **Derived From**: Dim10 (Sicherheit), Dim01 (RustDesk Auth), Dim09 (MCP Safety)
- **Rationale**: RustDesk nutzt Ed25519 + TOFU, was die Transportsicherheit garantiert. Session-Passwoerter (statt persistente Credentials) minimieren das Angriffsfenster. OS-Keyring isoliert API-Keys vom Anwendungs-Code. MCP-Sandbox enforced KI-Sicherheitsgrenzen.
- **Implications**: ClawViewer sollte jede Sicherheitsschicht als separate Rust-Crate/Modul implementieren: `crypto` (Ed25519/X25519), `session` (Passwort-Generierung, Rotation), `keyring` (API-Key-Storage), `mcp-sandbox` (KI-Permission-Model).
- **Confidence**: high
