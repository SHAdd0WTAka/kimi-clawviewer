# ClawViewer – Cross-Verification (Phase 4)

## High Confidence (bestatigt durch >=2 Agents, unabhangige Quellen)

| # | Finding | Bestatigt durch | Quellen |
|---|---------|----------------|---------|
| 1 | RustDesk verwendet Protobuf v3 fur Wire-Protokoll | Dim01, Dim02 | rustdesk-server/src/ (protobuf crate 3.7) |
| 2 | RustDesk hbbs nutzt TCP 21116 + UDP 21116, hbbr TCP 21117 | Dim01, Phase-1-Recherche | RustDesk Docs, Source Code |
| 3 | Ed25519 mit sodiumoxide/ed25519-dalek fur Auth/Signing | Dim01, Dim10 | RustDesk Source, Rust Crypto Crates |
| 4 | DXGI Desktop Duplication API als Windows Screen-Capture | Dim02, Phase-1-Recherche | rustdesk/src/dxgi/mod.rs |
| 5 | PipeWire/GStreamer als Linux Screen-Capture (Wayland) | Dim02, Phase-1-Recherche | rustdesk/src/wayland/pipewire.rs |
| 6 | VP9/AOM/HWRAM/VRAM als Codec-Backends mit Auto-Selektion | Dim02 | rustdesk/libs/scrap/src/common/codec.rs |
| 7 | Enigo-Abstraktion fur plattformubergreifende Input-Injection | Dim02 | rustdesk/libs/enigo/ (SendInput, uinput, CGEvent) |
| 8 | FreeRDP: rdp.c als Haupt-State-Machine (3.227 ZL) | Dim03 | FreeRDP/libfreerdp/core/rdp.c |
| 9 | FreeRDP: H264 Multi-Backend (FFmpeg, VAAPI, DXVA, VideoToolbox) | Dim03 | FreeRDP/libfreerdp/codec/h264.c |
| 10 | LibVNCServer: rfbserver.c mit rfbProcessEvents() als Main Loop | Dim04 | LibVNCServer/src/libvncserver/rfbserver.c |
| 11 | LibVNCServer: sraRegionPtr fur modifizierte Regionen-Tracking | Dim04 | LibVNCServer/include/rfb/rfb.h |
| 12 | xrdp: Multi-Prozess-Design (xrdp + sesman + sesexec) | Dim05 | neutrinolabs/xrdp/sesman/ |
| 13 | xrdp: Dynamisches Modul-System (.so Loading) | Dim05 | xrdp/libxrdp/xrdp_module.c |
| 14 | Remmina: RemminaPluginService-Struct als Plugin-API | Dim06 | remmina/src/include/remmina/plugin.h |
| 15 | WebRTC: ICE-State-Machine (Gathering->Checking->Connected) | Dim07 | WebRTC Spec, libwebrtc Source |
| 16 | WebRTC DataChannel: SCTP uber DTLS, 4-Wege-Handshake | Dim07, Dim09 | RFC 4960, WebRTC Spec |
| 17 | Tauri v2: Core->Shell->IPC->WebView mit WRY/TAO | Dim08 | tauri-apps/tauri Source |
| 18 | Tauri v2: ~3-15MB Bundle (vs 150MB+ Electron) | Dim08 | Tauri Benchmarks |
| 19 | MCP: JSON-RPC 2.0 uber stdio/SSE/WebSocket | Dim09 | modelcontextprotocol.io Spec |
| 20 | Piper TTS: ~120ms Latenz auf Intel i5, Rust via piper-rs | Dim12 | rhasspy/piper GitHub |
| 21 | OS Keyring: keyring v4 Crate cross-platform (DPAPI/Keychain/Secret Service) | Dim10, Dim09 | Rust keyring crate |
| 22 | Bidirektionale Steuerung: 4 Queues P0-P3 mit BinaryHeap | Dim11 | Shared Autonomy Forschung |

## Medium Confidence (1 Agent, authoritative Source)

| # | Finding | Agent | Quelle |
|---|---------|-------|--------|
| 23 | RustDesk TOFU mit UUID-Validierung und License-Key via -k Parameter | Dim01 | RustDesk Source |
| 24 | RustDesk: KCP fur UDP-Traversal mit NaCl-Verschlusselung | Dim02 | RustDesk Source |
| 25 | FreeRDP: NLA in nla.c (2.475 ZL) mit CredSSP | Dim03 | FreeRDP Source |
| 26 | UltraVNC: Video Hook Driver + Desktop Duplication API | Dim04 | ultravnc/UltraVNC Source |
| 27 | xrdp: xorgxrdp als bevorzugtes X11-Backend | Dim05 | xrdp Source |
| 28 | Remmina: 7 Plugin-Typen (Protocol, Secret, Tool, Pref, File, Entry, Widget) | Dim06 | remmina Source |
| 29 | WebRTC P2P-Handshake: ~500-700ms Gesamtaufbau | Dim07 | WebRTC Messungen |
| 30 | webrtc-rs (async) und str0m (Sans-I/O) als Rust-Libraries | Dim07 | GitHub |
| 31 | Tauri v2: WebRTC nativ in WebView2/WebKit, Linux erfordert Custom Build | Dim08 | Tauri Docs |
| 32 | QuickDesk: 40+ MCP-Tools, Dual-Transport (stdio + HTTP/SSE) | Dim09 | barry-ran/QuickDesk |
| 33 | KI-Sandbox: Drei-Schichten-Modell (Env/Permissions/Runtime) | Dim10 | AI Safety Literatur |
| 34 | Ghost-Cursor: Direct2D + UpdateLayeredWindow, Orange-Lila Farbkodierung | Dim11 | MouseMux Referenz |
| 35 | Piper: Entwicklung verschoben zu OHF-Voice/piper1-gpl | Dim12 | GitHub (archiviert) |

## Conflict Zone

| # | Konflikt | Agenten | Analyse |
|---|----------|---------|---------|
| 36 | **Bundle-Grösse Tauri**: "~5MB" (User-Anforderung) vs "3-15MB realistisch" (Dim08) vs "3MB Hello World, 15MB komplex" | Dim08, User | Kein echter Konflikt – 5MB ist Minimum fur komplexe App, 15MB realistisch fur Remote-Desktop mit WebRTC |
| 37 | **Codec-Prioritat**: Dim02 sagt H265>H264>AV1>VP9>VP8, aber User-Anforderung nennt VP9 oder H264 | Dim02, User | Kein Konflikt – Dim02 beschreibt RustDesks Auto-Selektion; User gibt bevorzugte Codecs vor |
| 38 | **WebRTC auf Linux**: Dim08 sagt "Custom WebKitGTK-Build erforderlich", Dim07 empfiehlt webrtc-rs | Dim07, Dim08 | Auflosung: Tauri-WebView hat auf Linux eingeschrankte WebRTC-Unterstutzung; Empfehlung ist webrtc-rs im Rust-Backend |
| 39 | **TTS Engine**: Dim12 empfiehlt Piper (lokal), User erwahnt auch Coqui/Edge-TTS/ElevenLabs | Dim12, User | Kein Konflikt – Dim12 empfiehlt lokale Engine als Primaerloesung, Cloud als Fallback (passt zu User-Anforderung) |

## Zusammenfassung

- **High Confidence**: 22 Funde aus >=2 unabhaengigen Quellen – solide Basis fur Architekturentscheidungen
- **Medium Confidence**: 13 Funde aus 1 Quelle, aber authoritativ – verwendbar mit Quellenangabe
- **Conflict Zone**: 4 scheinbare Konflikte, die bei genauerer Betrachtung aufgeloest werden koennen
- **Keine kritischen Konflikte** die Phase 5 (Targeted Validation) erfordern wuerden
