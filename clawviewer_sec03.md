## 3. Code-Analyse-Report der Open-Source-Projekte

Dieses Kapitel analysiert sechs etablierte Open-Source-Remote-Desktop-Projekte auf Quellcode-Ebene. Ziel ist die Extraktion konkreter Implementierungsmuster, Architektur-Patterns und Code-Strukturen, die als direkter Blueprint fuer die ClawViewer-Implementierung dienen koennen. Jede Analyse umfasst Repository-Struktur, Kernkomponenten mit Dateipfaden und Funktionsnamen, identifizierte Design-Patterns und eine Abbildung auf ClawViewer-spezifische Anforderungen. Die Analyse basiert auf dem jeweils aktuellen master-Branch der untersuchten Repositorys (Stand Juni 2026).

### 3.1 RustDesk Server (hbbs/hbbr) – P2P-Architektur und Relay

#### 3.1.1 Repository-Struktur: rustdesk/rustdesk-server, 6 Hauptmodule, tokio-async-Runtime

Das Repository `rustdesk/rustdesk-server` (https://github.com/rustdesk/rustdesk-server) implementiert zwei Server-Komponenten in Rust: den Rendezvous-Server hbbs (ID-Registrierung, Peer-Discovery, NAT-Traversal-Koordination) und den Relay-Server hbbr (Datenweiterleitung bei P2P-Fehlschlag). Die Codebasis umfasst sechs Hauptmodule, die ueber eine gemeinsame tokio-Async-Runtime (Version 1.44) orchestriert werden [^1^].

Die Repository-Struktur gliedert sich wie folgt:

```
rustdesk-server/
├── Cargo.toml              # Workspace-Manifest
├── src/
│   ├── main.rs             # hbbs-Entrypoint (RendezvousServer::start)
│   ├── hbbr.rs             # hbbr-Entrypoint (relay_server::start)
│   ├── lib.rs              # Library-Exports
│   ├── common.rs           # CLI-Argumente, Ed25519-Key-Gen (gen_sk)
│   ├── rendezvous_server.rs # Kern-hbbs: Peer-Registrierung, Punch-Hole-Koordination
│   ├── relay_server.rs     # Kern-hbbr: UUID-basiertes Pairing, Bandbreiten-Limiting
│   ├── peer.rs             # Peer-Datenmodell: HashMap + SQLite-Persistenz
│   └── database.rs         # SQLite-Schema, async-Queries via sqlx + deadpool
├── libs/
│   └── hbb_common/         # Git-Submodule: Protobuf-Defs, TCP/UDP-Wrapper
│       ├── protos/
│       │   ├── rendezvous.proto   # Signaling-Protokoll (RendezvousMessage oneof)
│       │   └── message.proto      # Datenkanal-Protokoll (Video, Input, Audio)
│       └── src/
```

Die tokio-Runtime wird in `src/main.rs` ueber `#[tokio::main]` initialisiert; der RendezvousServer wird durch den Aufruf `RendezvousServer::start(port, serial, &key, rmem)` gestartet [^5^]. Der hbbr-Entrypoint in `src/hbbr.rs` parsed CLI-Argumente via clap und ruft `relay_server::start()` mit Port- und Schluesselparametern auf [^6^].

#### 3.1.2 Rendezvous-Server (hbbs): RegisterPeer, RegisterPk, SQLite-Persistenz, In-Memory-Cache

Der Rendezvous-Server koordiniert den gesamten P2P-Verbindungsaufbau. In `src/rendezvous_server.rs` wird in der Funktion `handle_udp()` ein zentraler Dispatch auf alle eingehenden Protobuf-Nachrichten durchgefuehrt [^7^]. Die zwei zentralen Registrierungsoperationen sind:

**RegisterPeer** (Code-Referenz: `src/rendezvous_server.rs::handle_udp()`, RegisterPeer-Arm): Ein Client sendet eine UDP-Nachricht `RegisterPeer { id, serial }`. Der Server speichert die Socket-Adresse via `self.update_addr(rp.id, addr, socket)` und prueft, ob eine Konfigurationsaktualisierung erforderlich ist (`if self.inner.serial > rp.serial`). Die Antwort ist eine `RegisterPeerResponse`, die gegebenenfalls die Public-Key-Registrierung anfordert [^7^].

**RegisterPk** (Code-Referenz: `src/rendezvous_server.rs::handle_udp()`, RegisterPk-Arm sowie `src/peer.rs::PeerMap::update_pk()`): Der Client sendet `RegisterPk { id, uuid, pk }` mit seinem Ed25519-Public-Key. Der Server validiert die UUID, fuehrt Rate-Limiting durch (maximal 2 Versuche pro 6 Sekunden) und persistiert den Schluessel. Die Antwort ist `RegisterPkResponse::OK` bei erfolgreicher Registrierung [^7^].

Die Persistenzschicht in `src/peer.rs` implementiert ein Dual-Storage-Pattern: eine In-Memory-Struktur `PeerMap { map: Arc<RwLock<HashMap<String, LockPeer>>>, db: Database }` kombiniert schnelle Lesezugriffe mit SQLite-Persistenz. Das SQLite-Schema in `src/database.rs::create_tables()` definiert die Tabelle `peer` mit den Feldern `guid` (UUIDv4, Primaerschluessel), `id`, `uuid`, `pk`, `created_at`, `user`, `status`, `note` und `info` (JSON) [^11^]. Die Datenbankverbindung wird ueber sqlx mit deadpool-Connection-Pooling (Default: 1 Verbindung) verwaltet.

Die nachfolgende Tabelle fasst die API-Endpunkte des hbbs-Protokolls zusammen.

| Nachrichtentyp | Richtung | Zweck | Code-Referenz |
|---|---|---|---|
| `RegisterPeer` | Client → hbbs | ID-Registrierung mit Socket-Adresse | `rendezvous_server.rs::handle_udp()` |
| `RegisterPeerResponse` | hbbs → Client | Konfig-Update oder PK-Anforderung | `rendezvous_server.rs::handle_udp()` |
| `RegisterPk` | Client → hbbs | Ed25519-Public-Key-Registrierung | `rendezvous_server.rs::handle_udp()` + `peer.rs::update_pk()` |
| `RegisterPkResponse` | hbbs → Client | OK, UUID_MISMATCH, TOO_FREQUENT | `rendezvous_server.rs::handle_udp()` |
| `PunchHoleRequest` | Client A → hbbs | Verbindungsanfrage an Peer B | `rendezvous_server.rs::handle_punch_hole_request()` |
| `PunchHole` | hbbs → Client B | Weiterleitung von A's Adresse an B | `rendezvous_server.rs::handle_punch_hole_request()` |
| `PunchHoleSent` | Client B → hbbs | B bereit fuer Direktverbindung | `rendezvous_server.rs::handle_hole_sent()` |
| `PunchHoleResponse` | hbbs → Client A | B's Adresse fuer direkten Verbindungsversuch | `rendezvous_server.rs::handle_hole_sent()` |
| `RequestRelay` | Client → hbbs | Relay-Fallback-Anfrage | `rendezvous_server.rs::handle_tcp()` |
| `RelayResponse` | hbbs → Client | Relay-Server-Zuweisung | `rendezvous_server.rs::handle_tcp()` |
| `TestNatRequest` | Client → hbbs | NAT-Typ-Bestimmung (Port 21115) | `rendezvous_server.rs::handle_listener2()` |
| `ConfigUpdate` | hbbs → Client | Server-Konfigurationsaktualisierung | `rendezvous_server.rs::handle_udp()` |

Die Tabelle dokumentiert 12 Nachrichtentypen fuer die vollstaendige P2P-Koordination. Alle Nachrichten verwenden das `RendezvousMessage`-Protobuf-Envelope mit einem `oneof union`-Pattern, das in `libs/hbb_common/protos/rendezvous.proto` definiert ist [^3^]. Dieses Entwurfsmuster ermoeglicht die Erweiterung des Protokolls um neue Nachrichtentypen ohne Breaking Changes, da unbekannte Union-Arme ignoriert werden. Die Kombination aus In-Memory-Cache (`HashMap` unter `RwLock`) und SQLite-Persistenz stellt sicher, dass der Server auch nach einem Neustart die Peer-Registrierungen wiederherstellen kann, waehrend Hot-Path-Lookups im Arbeitsspeicher erfolgen.

#### 3.1.3 Relay-Server (hbbr): UUID-basiertes Peer-Pairing, bidirektionale tokio::select!-Weiterleitung, Bandbreiten-Limiting

Der Relay-Server in `src/relay_server.rs` uebernimmt die Datenweiterleitung, wenn direkte P2P-Verbindungen aufgrund symmetrischer NAT-Typen oder Firewall-Restriktionen nicht moeglich sind. Das zentrale Paarungsverfahren basiert auf UUIDs [^8^].

Die Funktion `make_pair_()` in `src/relay_server.rs` implementiert ein Warte-Pattern: Der erste Peer, der eine `RequestRelay`-Nachricht mit einer bestimmten UUID sendet, wird in einer globalen HashMap `PEERS.lock().await.insert(rf.uuid.clone(), Box::new(stream))` eingetragen und wartet bis zu 30 Sekunden. Wenn ein zweiter Peer mit identischer UUID eintrifft, werden beide Streams gepaart und bidirektional verbunden [^8^].

Die eigentliche Datenweiterleitung erfolgt in der Funktion `relay()`, die ein `tokio::select!`-Pattern fuer gleichzeitiges Lesen von beiden Streams einsetzt:

```rust
async fn relay(...) -> ResultType<()> {
    let limiter = <Limiter>::new(sb);
    loop {
        tokio::select! {
            res = peer.recv() => { /* Weiterleitung an stream */ },
            res = stream.recv() => { /* Weiterleitung an peer */ },
            _ = timer.tick() => { /* Timeout-Check (30s) */ }
        }
    }
}
```

Diese Implementierung nutzt `async_speed_limit::Limiter` fuer Bandbreiten-Limiting (Default: 128 Mbps pro Verbindung, 1 Gbps Gesamt, 32 Mbps Blacklist-Limit) [^8^]. Die `tokio::select!`-Makro ermoeglicht gleichzeitiges Polling beider Streams in einem einzigen async-Task, was die Ressourceneffizienz maximiert. WebSocket-Verbindungen werden auf Port 21119 via `tokio-tungstenite` akzeptiert und separat behandelt.

#### 3.1.4 P2P-Handshake: 6-Schritt-Protokoll mit TCP/UDP-Hole-Punching, jittered retries, Relay-Fallback

Der P2P-Verbindungsaufbau zwischen zwei RustDesk-Clients folgt einem sechsstufigen Protokoll, das TCP- und UDP-Hole-Punching kombiniert [^7^][^14^].

**Schritt 1 – Registrierung:** Beide Peers registrieren sich ueber UDP bei hbbs via `RegisterPeer` und `RegisterPk`. Der Server speichert ihre Socket-Adressen und Public Keys.

**Schritt 2 – PunchHoleRequest:** Der initiierende Peer A sendet `PunchHoleRequest { id: B's_id, nat_type, licence_key, conn_type }` an hbbs. Der Server validiert den Lizenzschluessel (`if !key.is_empty() && ph.licence_key != key { return LICENSE_MISMATCH; }`) und prueft, ob Peer B online ist (Timeout: 30 Sekunden) [^7^].

**Schritt 3 – PunchHole-Weiterleitung:** hbbs sendet `PunchHole { socket_addr: A's_addr, relay_server, nat_type }` an Peer B. Falls beide Peers im gleichen LAN sind, wird stattdessen `FetchLocalAddr` verwendet.

**Schritt 4 – TCP-Hole-Punching:** Peer B empfaengt `PunchHole` und fuehrt in `src/rendezvous_mediator.rs::handle_punch_hole()` ein simultanes TCP-Connect durch: Zuerst wird eine Verbindung zu hbbs aufgebaut (`connect_tcp(&host, timeout)`), dann wird die lokale Adresse ermittelt (`socket.local_addr()`) und von derselben lokalen Portnummer aus ein direkter Verbindungsversuch zu Peer A gestartet (`connect_tcp_local(peer_addr, Some(local_addr), 30)`) [^14^].

**Schritt 5 – UDP-Hole-Punching mit Jittered Retries:** Wenn UDP aktiviert ist, fuehrt Peer B in `punch_udp_hole()` zusaetzlich ein UDP-Hole-Punching durch. Dabei werden bis zu 3 Pakete mit jittered Delays von 10-30 ms gesendet (`hbb_common::time_based_rand() % 20 + 10`) [^14^].

**Schritt 6 – Relay-Fallback:** Wenn alle Hole-Punching-Versuche fehlschlagen (erkannt an SYMMETRIC-NAT-Typ, Timeout oder explizitem `force_relay`-Flag), sendet Peer A `RequestRelay` an hbbs, das hbbr mit der UUID informiert. Die Verbindung wird ueber den Relay-Server aufgebaut [^7^].

#### 3.1.5 Ed25519-Auth: sodiumoxide::crypto::sign, TOFU mit UUID, License-Key-Parameter

Die kryptographische Authentifizierung basiert auf Ed25519-Signaturen via `sodiumoxide::crypto::sign` [^17^]. In `src/common.rs::gen_sk()` werden Schluesselpaare generiert: Der 64-Byte-Secret-Key wird in `id_ed25519` (Base64) gespeichert, der 32-Byte-Public-Key in `id_ed25519.pub`. Der Public-Key entspricht dabei den letzten 32 Bytes des Secret-Keys (sodiumoxide-Konvention) [^9^].

Der Server signiert in `src/rendezvous_server.rs::get_pk()` das Tupel `IdPk { id, pk }` mit seinem eigenen Secret-Key. Die Signatur wird dem anfragenden Peer waehrend des Punch-Hole-Prozesses uebermittelt, sodass dieser die Authentizitaet des Gegenuebers verifizieren kann [^7^].

Das Trust-on-First-Use (TOFU)-Modell implementiert folgende Regeln: Die erste `RegisterPk`-Registrierung fuer eine ID wird akzeptiert. Spaetere Registrierungen mit derselben UUID aber geaendertem IP/PK werden mit Rate-Limiting erlaubt. Eine abweichende UUID fuehrt zu `UUID_MISMATCH`, was auf Client-Seite eine ID-Neugenerierung ausloest [^7^]. Der optionale Lizenzschluessel wird ueber den CLI-Parameter `-k` / `--key` konfiguriert und in `PunchHoleRequest.licence_key` geprueft.

#### 3.1.6 Blueprint fuer ClawViewer: Protobuf-Protokoll, tokio-Runtime, Ed25519-Auth-Modul

Aus der RustDesk-Server-Analyse lassen sich drei direkt uebernehmbare Architekturkomponenten ableiten. Erstens das Protobuf-basierte Wire-Protokoll mit `oneof union`-Pattern als Nachrichten-Envelope, das eine saubere Protokollversionierung ermoeglicht. Zweitens die tokio-basierte Async-Runtime mit `tokio::select!` fuer Multi-Listener-Architektur (UDP + TCP + WebSocket simultan). Drittens das Ed25519-Auth-Modul mit TOFU-Semantik, das sich als separates Rust-Crate abspalten laesst. Die hbbs/hbbr-Trennung in Signaling- und Relay-Server bildet zudem die direkte Vorlage fuer ClawViewer's entsprechende Komponenten. Die Port-Belegung (21115-21119) und das NAT-Typ-Enum (`UNKNOWN_NAT=0`, `ASYMMETRIC=1`, `SYMMETRIC=2`) aus `protos/rendezvous.proto` koennen weitgehend unveraendert uebernommen werden.

### 3.2 RustDesk Client – Screen-Capture und Input

#### 3.2.1 Repository: rustdesk/rustdesk, Workspace-Struktur mit libs/scrap, libs/enigo

Das Client-Repository `rustdesk/rustdesk` organisiert die Plattformabstraktion in separaten Workspace-Crates. Die beiden zentralen Bibliotheken fuer ClawViewer-relevante Funktionalitaet sind `libs/scrap` (Screen-Capture) und `libs/enigo` (Input-Injection) [^6^].

```
rustdesk/
├── libs/
│   ├── scrap/              # Screen-Capture-Library
│   │   ├── src/dxgi/mod.rs     # Windows: DXGI Desktop Duplication
│   │   ├── src/x11/            # Linux/X11: MIT-SHM
│   │   ├── src/wayland/        # Linux/Wayland: PipeWire
│   │   ├── src/quartz/         # macOS: CoreGraphics
│   │   └── src/common/codec.rs # Encoder-Abstraktion
│   └── enigo/              # Input-Injection-Library
│       ├── src/win/win_impl.rs # Windows: SendInput
│       ├── src/linux/          # Linux: uinput/XTest
│       └── src/macos/          # macOS: CGEvent
├── src/server/
│   ├── video_service.rs    # Capture-Encode-Send Loop
│   └── input_service.rs    # Input-Event-Verarbeitung
```

#### 3.2.2 DXGI-Capture: Capturer-Struktur mit ID3D11Device + IDXGIOutputDuplication, GDI-Fallback

Die Windows-Screen-Capture-Implementierung in `libs/scrap/src/dxgi/mod.rs` definiert die `Capturer`-Struktur mit Direct3D 11-Objekten [^34^]:

```rust
pub struct Capturer {
    device: ComPtr<ID3D11Device>,
    context: ComPtr<ID3D11DeviceContext>,
    duplication: ComPtr<IDXGIOutputDuplication>,
    fastlane: bool,
    gdi_capturer: Option<CapturerGDI>,  // GDI-Fallback
    // ...
}
```

Die Initialisierung in `Capturer::new()` (Zeilen 69-173) versucht zunaechst, ein D3D11-Device zu erstellen (`D3D11CreateDevice`) und die Desktop Duplication zu starten (`DuplicateOutput`). Bei Fehler wird automatisch auf den GDI-Capturer (`display.create_gdi()`) zurueckgegriffen [^34^]. Die Frame-Erfassung in `load_frame()` ruft `AcquireNextFrame(timeout, &mut info, &mut frame)` auf. Bei `fastlane=true` erfolgt direkter Speicherzugriff via `MapDesktopSurface`; bei `fastlane=false` wird eine Staging-Texture erstellt und `CopyResource` fuer den GPU-zu-CPU-Transfer verwendet [^34^]. Die Funktion `ohgodwhat()` (Zeilen 313-340) implementiert diesen Transfer durch Erstellen einer CPU-lesbaren Staging-Texture mit `D3D11_USAGE_STAGING` und `D3D11_CPU_ACCESS_READ`.

#### 3.2.3 PipeWire-Integration: GStreamer-Pipeline, DBus xdg-desktop-portal, Restore-Token

Die Wayland-Capture auf Linux verwendet PipeWire via xdg-desktop-portal. Die Struktur `PipeWireRecorder` in `libs/scrap/src/wayland/pipewire.rs` implementiert eine GStreamer-Pipeline (`pipewiresrc -> videoconvert -> appsink`) [^40^]. Der Portal-Flow in `request_remote_desktop()` fuehrt sequentiell DBus-Aufrufe durch: Session erstellen (`screencast_portal::create_session`), Quellen auswaehlen (`select_sources`), Capture starten (`screencast_portal::start`), und PipeWire-FD oeffnen (`open_pipe_wire_remote`). Der Rueckgabewert enthaelt einen Restore-Token fuer wiederholte Sitzungen [^40^].

#### 3.2.4 Codec-Pipeline: 4 Encoder-Backends (VPX, AOM, HWRAM, VRAM), Auto-Selektion H265>H264>AV1>VP9>VP8

Die Codec-Abstraktion in `libs/scrap/src/common/codec.rs` definiert den Enum `EncoderCfg` mit vier Backends: `VPX(VpxEncoderConfig)` fuer VP8/VP9 via libvpx, `AOM(AomEncoderConfig)` fuer AV1 via aom, `HWRAM(HwRamEncoderConfig)` fuer Hardware-Encoding via FFmpeg (NVENC, VAAPI, QSV, VideoToolbox), und `VRAM(VRamEncoderConfig)` fuer Direct GPU Texture Encoding auf Windows [^204^].

Die Auto-Selektion in `codec.rs` (Zeilen 167-260) implementiert eine Prioritaetskaskade: H265 wird bevorzugt, wenn `h265_useable` wahr ist, ansonsten H264, ansonsten AV1 (falls `av1_useable && av1_test`), ansonsten VP9, mit VP8 als ultimativen Fallback [^204^]. Der `Decoder` haelt simultan Instanzen aller moeglichen Decoder (vp8, vp9, av1, h264_ram, h265_ram, h264_vram, h265_vram) und waehlt zur Laufzeit basierend auf dem empfangenen Codec-Format.

#### 3.2.5 Input-Injection: Enigo-Abstraktion mit SendInput/uinput/CGEvent, serverseitige input_service.rs

Die Enigo-Bibliothek abstrahiert Input-Injection ueber drei Plattform-Backends. Auf Windows verwendet `libs/enigo/src/win/win_impl.rs` die Win32-API `SendInput()` mit `MOUSEINPUT`- und `KEYBDINPUT`-Strukturen [^114^]. Die Funktion `mouse_event()` setzt `dwExtraInfo = ENIGO_INPUT_EXTRA_VALUE` (100), um injizierte Events zu markieren. Absolute Mauspositionen werden mittels `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK` mit Normalisierung auf 65535x65535-Koordinaten gesetzt [^114^].

Auf Linux werden drei Modi unterstuetzt: uinput (Kernel-Level, fuer Wayland), XTest (X11), und RemoteDesktop Portal (Wayland ohne Root) [^148^]. Die Funktion `handle_mouse_()` in `src/server/input_service.rs` (Zeilen 700-800) dispatched Events nach Typ: `MOUSE_TYPE_MOVE` fuer absolute Positionierung, `MOUSE_TYPE_MOVE_RELATIVE` fuer relative Bewegungen (mit Clamp auf +/-10000), `MOUSE_TYPE_DOWN` fuer Button-Presses, und `MOUSE_TYPE_WHEEL` fuer Scroll-Events [^148^].

Die nachfolgende Tabelle fasst die Platform-Implementierungen von Capture und Input zusammen.

| Plattform | Capture-API | Input-API | Quelldatei (Capture) | Quelldatei (Input) |
|---|---|---|---|---|
| Windows | DXGI Desktop Duplication, GDI-Fallback | SendInput (winuser) | `libs/scrap/src/dxgi/mod.rs` | `libs/enigo/src/win/win_impl.rs` |
| Linux/X11 | MIT-SHM (Shared Memory) | XTest (xdo) | `libs/scrap/src/x11/` | `libs/enigo/src/linux/` |
| Linux/Wayland | PipeWire (xdg-desktop-portal) | uinput (Kernel) | `libs/scrap/src/wayland/pipewire.rs` | `src/server/uinput.rs` |
| Linux/Wayland (alt.) | — | RemoteDesktop Portal | — | `src/server/rdp_input.rs` |
| macOS | Quartz Display Services | CGEvent (VirtualInput) | `libs/scrap/src/quartz/` | `libs/enigo/src/macos/` |

Die Tabelle zeigt, dass RustDesk fuer jede Plattform separate Capture- und Input-Implementierungen bereithaelt, die ueber eine gemeinsame Rust-API abstrahiert werden. Der GDI-Fallback auf Windows stellt sicher, dass die Anwendung auch auf Systemen funktioniert, auf denen die Desktop Duplication API nicht verfuegbar ist (z.B. aeltere GPUs oder Remote-Sessions). Die Linux-Unterstuetzung ist mit drei alternativen Input-Pfaeden die komplexeste, da Wayland aus Sicherheitsgruenden keine globalen Input-Injection ohne Portal oder Root erlaubt. Fuer ClawViewer ist diese Plattform-Matrix die direkte Referenz, welche OS-APIs auf welchem System verwendet werden muessen.

#### 3.2.6 Blueprint fuer ClawViewer: scrap-Crate-Struktur, Enigo-Abstraktion, Codec-Auto-Selektion

Der RustDesk-Client liefert drei uebernehmbare Architekturmuster. Die Crate-Struktur von `scrap` (plattformspezifische Module unter `src/dxgi/`, `src/x11/`, `src/wayland/`, `src/quartz/` mit gemeinsamer Codec-Abstraktion in `src/common/`) bildet das Template fuer ClawViewer's Capture-Crate. Die Enigo-Abstraktion mit plattformspezifischen Implementierungsdateien und einheitlicher API (`mouse_move_to()`, `key_down()`, etc.) wird direkt auf ClawViewer's Input-System uebertragen. Die Codec-Auto-Selektionslogik (H265 > H264 > AV1 > VP9 > VP8) mit simultaner Multi-Decoder-Instanziierung minimiert Codec-Negotiation-Overhead. Der `video_service.rs`-Loop (Capture → Encode → Send) mit QoS-Anpassung und FPS-Limiting bildet zudem die Referenz fuer ClawViewer's Haupt-Streaming-Thread.

### 3.3 FreeRDP – RDP-Protokoll-Implementierung

#### 3.3.1 Repository: FreeRDP/FreeRDP, CMake-Build, libfreerdp + winpr + client

Das Repository `FreeRDP/FreeRDP` (13.3k Stars, 432+ Contributors) enthaelt ca. 87.6% C-Code und implementiert das vollstaendige RDP-Protokoll in einer geschichteten Architektur [^34^]. Die drei Hauptkomponenten sind `libfreerdp/` (Core-Bibliothek mit Protokoll, Codecs, GDI, Krypto), `winpr/` (Windows Portable Runtime: SSPI, Krypto, Threads, I/O), und `channels/` (30+ virtuelle Kanaele).

```
FreeRDP/
├── libfreerdp/
│   ├── core/           # RDP-State-Machine: rdp.c (3.227 ZL), connection.c (2.259 ZL)
│   ├── codec/          # Alle Grafikcodecs: h264.c, rfx.c, progressive.c, nsc.c
│   ├── gdi/            # GDI-Grafikengine: gdi.c (1.544 ZL), gfx.c (2.114 ZL)
│   ├── cache/          # GDI-Objekt-Cache: Bitmap, Brush, Glyph, Pointer
│   └── crypto/         # TLS, Zertifikate
├── winpr/libwinpr/
│   ├── sspi/           # NTLM, Kerberos, Negotiate
│   ├── crypto/         # AES, DES, RC4, SHA
│   └── utils/          # wStream, WLog, ASN.1
├── channels/
│   ├── drdynvc/        # Dynamic Virtual Channel Manager
│   ├── rdpgfx/         # Graphics Pipeline (H.264, RemoteFX)
│   ├── cliprdr/        # Clipboard
│   ├── rdpsnd/         # Audio
│   └── rdpdr/          # Device Redirection
├── client/
│   ├── SDL/            # SDL2/3-Client (neuer Standard)
│   ├── X11/            # xfreerdp
│   └── Wayland/        # wlfreerdp
```

#### 3.3.2 RDP-State-Machine: libfreerdp/core/rdp.c (3.227 ZL), connection.c (2.259 ZL), state.h

Die zentrale Protokoll-State-Machine ist in `libfreerdp/core/rdp.c` (3.227 Zeilen) implementiert [^59^]. Der Enum `state_run_t` in `libfreerdp/core/state.h` definiert die Zustaende: `STATE_RUN_ACTIVE` (Verbindung aktiv), `STATE_RUN_REDIRECT` (Weiterleitung), `STATE_RUN_SUCCESS` (erfolgreich abgeschlossen), `STATE_RUN_FAILED` (fehlgeschlagen) und `STATE_RUN_TRY_AGAIN` (Wiederholen) [^61^].

Die Connection Sequence in `libfreerdp/core/connection.c` (2.259 Zeilen) implementiert den vollstaendigen RDP-Verbindungsaufbau als State-Machine: X.224 Connection Request/Confirm, MCS Connect-Initial mit GCC Conference Create Request, MCS Channel Join, Security Negotiation, Licensing, Capability Exchange und Session Activation [^42^]. Die Client-Connection-States in `connection.h` umfassen `CLIENT_STATE_INITIAL`, `CLIENT_STATE_PRECONNECT_PASSED` und `CLIENT_STATE_POSTCONNECT_PASSED` [^118^].

#### 3.3.3 H264-Codec: Multi-Backend (FFmpeg, VAAPI, DXVA, VideoToolbox, MediaCodec), AVC444-Modus

Der H264-Codec-Container in `include/freerdp/codecs.h` definiert `struct rdp_codecs` mit Zeigern auf alle unterstuetzten Codecs: `RFX_CONTEXT* rfx`, `NSC_CONTEXT* nsc`, `H264_CONTEXT* h264`, `CLEAR_CONTEXT* clear`, `PROGRESSIVE_CONTEXT* progressive` [^30^]. Codec-Flags ermoeglichen Feature-Detection: `FREERDP_CODEC_AVC420` (0x80) fuer H.264 4:2:0 und `FREERDP_CODEC_AVC444` (0x100) fuer H.264 4:4:4.

Die H264-Implementierung in `libfreerdp/codec/h264.c` (894 Zeilen) abstrahiert ueber ein Backend-Interface, das mehrere Implementierungen ermoeglicht [^43^]: `h264_ffmpeg.c` (869 Zeilen, FFmpeg mit VAAPI/VideoToolbox/DXVA-Unterstuetzung) [^193^], `h264_openh264.c` (Cisco OpenH264), `h264_mediacodec.c` (Android MediaCodec), und `h264_mf.c` (Windows Media Foundation). Der AVC444-Modus fuer Windows 8.1+ wird in `h264.c` durch die Funktion `avc444_ensure_buffer()` implementiert.

#### 3.3.4 Virtual-Channel-System: 30+ Kanaele, DVC ueber channels/drdynvc/, GFX ueber channels/rdpgfx/

FreeRDP's virtuelles Kanaelsystem unterscheidet statische Kanaele (waehrend Connection Sequence eingerichtet) und Dynamic Virtual Channels (DVC), die ueber den `drdynvc`-Kanal zur Laufzeit erstellt werden [^58^]. Der GFX-Kanal `channels/rdpgfx/` implementiert die RDP 8.1+ Graphics Pipeline mit Surface Management, Frame Acknowledgement und WireToSurface-Komprimierung [^199^].

Kanaele sind als separate CMake-Projekte mit `ChannelOptions.cmake` organisiert und koennen als Shared Libraries geladen werden. Das PubSub-Event-System in `winpr/utils/pubsub.h` verbindet Kanaele mit dem Core ueber `ChannelConnectedEventArgs` und `ChannelDisconnectedEventArgs` [^44^].

#### 3.3.5 Security: NLA in nla.c (2.475 ZL), CredSSP, TLS, RDP-Encryption in security.c

Die Sicherheitsarchitektur umfasst mehrere Schichten. Die Security Negotiation in `libfreerdp/core/nego.c` handelt zwischen RDP (RC4, deprecated), TLS (1.0-1.3 via OpenSSL/mbedTLS), NLA (CredSSP/NTLM/Kerberos) und Azure AD Authentication [^36^]. NLA ist in `libfreerdp/core/nla.c` (2.475 Zeilen) mit CredSSP-Implementierung umgesetzt [^65^]. Die RDP-Verschluesselung in `libfreerdp/core/security.c` (1.004 Zeilen) unterstuetzt RC4 mit 40/56/128-Bit-Schluesseln, FIPS 140-1 (3DES/SHA-1) und SHA-256-basierte Key-Derivation [^162^].

Die nachfolgende Tabelle fasst die Kernkomponenten von FreeRDP zusammen.

| Komponente | Datei | Zeilen | Verantwortlichkeit |
|---|---|---|---|
| Haupt-State-Machine | `libfreerdp/core/rdp.c` | 3.227 | PDU-Verarbeitung, Daten-PDUs, Verbindungszustand |
| Connection Sequence | `libfreerdp/core/connection.c` | 2.259 | X.224, MCS, GCC, Activation, Capability Exchange |
| NLA/CredSSP | `libfreerdp/core/nla.c` | 2.475 | Network Level Authentication, SSPI, NTLM, Kerberos |
| RDP-Verschluesselung | `libfreerdp/core/security.c` | 1.004 | RC4, FIPS, Key-Derivation, Client/Server Random |
| Instanzverwaltung | `libfreerdp/core/freerdp.c` | 1.615 | FreeRDP-Instanz, Event-Loop, Callbacks |
| Capability Exchange | `libfreerdp/core/capabilities.c` | ~2.000 | BitmapCodec, VirtualChannel, Input, GFX Capabilities |
| Fast-Path I/O | `libfreerdp/core/fastpath.c` | 1.441 | Optimierter Input/Output-Pfad |
| H264-Codec-Core | `libfreerdp/codec/h264.c` | 894 | H264-Context, AVC444, YUV-Puffer |
| FFmpeg-Backend | `libfreerdp/codec/h264_ffmpeg.c` | 869 | libavcodec-Integration, HW-Decoding |
| RemoteFX | `libfreerdp/codec/rfx.c` | 2.508 | Wavelet-Codec, 64x64-Tiles, DWT |
| GFX-Integration | `libfreerdp/gdi/gfx.c` | 2.114 | Surface Management, Compositing |
| GDI-Engine | `libfreerdp/gdi/gdi.c` | 1.544 | Software-Rendering, Bitmap, Clipping |

Die Tabelle zeigt die Verteilung der Code-Komplexitaet in FreeRDP. Die groessten Einzeldateien sind `rdp.c` (3.227 Zeilen fuer die Haupt-State-Machine), `connection.c` (2.259 Zeilen fuer die Verbindungssequenz), `nla.c` (2.475 Zeilen fuer die Authentifizierung) und `rfx.c` (2.508 Zeilen fuer den RemoteFX-Codec). Diese Verteilung spiegelt die Architektur-Prioritaeten wider: Protokoll-Korrektheit (rdp.c + connection.c), Sicherheit (nla.c + security.c) und Grafik-Performance (rfx.c + gfx.c + gdi.c). Die Capability Exchange mit ~2.000 Zeilen zeigt zudem die Protokollkomplexitaet von RDP, die bei der Interoperabilitaet mit verschiedenen Clients und Servern beruecksichtigt werden muss.

#### 3.3.6 Blueprint fuer ClawViewer: State-Machine-Pattern, Multi-Backend-Codec, Channel-System

FreeRDP liefert drei zentrale Architekturmuster. Die State-Machine-Implementierung (`state.h` + `rdp.c` + `connection.c`) mit expliziten Zustandsuebergaengen und Retry-Logik bildet die Referenz fuer ClawViewer's Verbindungslebenszyklus-Management. Die Multi-Backend-Codec-Architektur (`codecs.h` + `h264.c` + `h264_ffmpeg.c`) mit Backend-Interface und Laufzeit-Selektion laesst sich auf ClawViewer's Codec-Pipeline uebertragen. Das Channel-System mit statischen und dynamischen Kanaelen, PubSub-Events und Plugin-basiertem Loading bildet das Muster fuer ClawViewer's Feature-Erweiterungsmechanismus. Insbesondere das `wStream`-API aus WinPR fuer PDU-Handling mit eingebautem Bounds-Checking ist ein Pattern, das ClawViewer's Protokoll-Layer uebernehmen sollte.

### 3.4 VNC-Ecosystem – RFB-Protokoll und Framebuffer

#### 3.4.1 LibVNCServer: rfbserver.c (Main Loop rfbProcessEvents), sraRegionPtr (modifizierte Regionen)

Das Repository `LibVNC/libvncserver` (0.9.15, Dezember 2024) implementiert das RFB-Protokoll in C (96%) unter GPL-2.0+-Lizenz [^31^]. Die zentrale Server-Implementierung in `src/libvncserver/rfbserver.c` (4.251 Zeilen) enthaelt die Haupt-Ereignisschleife `rfbProcessEvents()`, die eingehende Client-Nachrichten dispatched und Framebuffer-Updates sendet [^41^].

Das Per-Client-Tracking in `include/rfb/rfb.h` definiert fuer jeden verbundenen Client (`rfbClientRec`) separate Regionen: `copyRegion` (Zielbereich von Kopieroperationen), `modifiedRegion` (vom Server geaenderte Bereiche) und `requestedRegion` (vom Client angeforderte Bereiche) [^43^]. Das Makro `FB_UPDATE_PENDING()` prueft, ob ungesendete Updates, Cursor-Aenderungen oder Groessenaenderungen vorliegen. Der Update-Versand erfolgt in `rfbSendFramebufferUpdate(rfbClientPtr cl, sraRegionPtr updateRegion)`, die ueber die modifizierten Regionen iteriert und den jeweils besten Encoding-Handler aufruft [^41^].

#### 3.4.2 Encoding-Handler: Hextile (hextile.c), Tight (tight.c mit JPEG/Zlib), ZRLE (zrle.c), Raw

LibVNCServer implementiert mehrere Encoding-Verfahren in separaten Quelldateien. Hextile in `src/libvncserver/hextile.c` unterteilt den Bildschirm in 16x16-Kacheln und wendet pro Kachel Subencodings an (Raw, Solid, RRE, Hextile-Subrects) [^38^]. Tight in `src/libvncserver/tight.c` ist der effizienteste verlustfreie Codec und kombiniert zlib-Kompression, JPEG fuer fotorealistische Inhalte (via turbojpeg) und Gradient-Filter [^36^]. ZRLE in `src/libvncserver/zrle.c` verwendet zlib-komprimierte kachelbasierte Codierung (64x64 Pixel) mit Run-Length-Encoding fuer einfarbige Bereiche [^36^]. Raw in `src/libvncserver/rfbserver.c::rfbSendRectEncodingRaw()` sendet unkomprimierte Pixeldaten zeilenweise mit Formatkonvertierung.

#### 3.4.3 UltraVNC: Video Hook Driver, Desktop Duplication API, DSM-Encryption-Plugin

UltraVNC (Repository `ultravnc/UltraVNC`, C++ 72.4%) erweitert die VNC-Architektur um Windows-spezifische Capture-Methoden [^34^]. Die `DeskdupEngine.cpp` implementiert die Desktop Duplication API (DXGI) fuer moderne Windows-Versionen. Das Hook-DLL-System in `winvnc/vnchooks/` injiziert sich in die Display-Driver-Kette, um Aenderungsbenachrichtigungen zu erhalten. Der DSM-Encryption-Plugin-Mechanismus in `DSMPlugin/` ermoeglicht Ende-zu-Ende-Verschluesselung via austauschbarer Plugins [^34^].

#### 3.4.4 Input-Handling: rfbPointerEventMsg (Typ 5), rfbKeyEventMsg (Typ 4), Client-Callbacks

Das RFB-Protokoll definiert in `include/rfb/rfbproto.h` zwei Input-Nachrichtentypen [^30^]: `rfbPointerEventMsg` (Message-Type 5) mit `buttonMask` (Bit 0=links, 1=mittel, 2=rechts, 3=scrollUp, 4=scrollDown) und 16-Bit-X/Y-Koordinaten. `rfbKeyEventMsg` (Message-Type 4) enthaelt `down` (1=gedrueckt, 0=losgelassen) und einen 32-Bit-Keysym-Wert. LibVNCServer verarbeitet diese ueber Callbacks: `screen->ptrAddEvent` fuer Maus-Events und `screen->kbdAddEvent` fuer Tastatur-Events [^43^].

Die nachfolgende Tabelle vergleicht die verfuegbaren Encodings und ihre Eigenschaften.

| Encoding | Datei | Nummer | Methode | Beste Anwendung |
|---|---|---|---|---|
| Raw | `rfbserver.c` | 0 | Unkomprimierte Pixel, zeilenweise | Kompatibilitaet, Debugging |
| RRE | `rre.c` | 2 | Background + Subrects mit anderer Farbe | Monochrome Bereiche |
| Hextile | `hextile.c` | 5 | 16x16-Kacheln mit Subencoding | Kleine verteilte Aenderungen |
| Zlib | `zlib.c` | 6 | zlib-Kompression auf Raw-Daten | Grosse Flaechen |
| Tight | `tight.c` | 7 | zlib + JPEG + Gradient-Filter | Fotos/UI kombiniert |
| ZRLE | `zrle.c` | 16 | zlib + RLE in 64x64-Tiles | Einfarbige Bereiche |
| Ultra | `ultra.c` | 9 | LZO-Kompression | Schnelle Kompression/Decompression |
| CopyRect | `rfbserver.c` | 1 | Kopiere bestehende Flaeche | Fensterverschiebungen |

Die Tabelle zeigt, dass LibVNCServer acht Encoding-Verfahren implementiert, die jeweils fuer unterschiedliche Inhaltstypen optimiert sind. Tight (Encoding 7) gilt als effizientester Allround-Codec, da er Inhalte analysiert und automatisch zwischen Fills (einheitliche Farbe), JPEG (fotorealistisch) und zlib-Kompression (Text/UI) waehlt. Hextile ist fuer VNC-Szenarien mit kleinen, verteilten Aenderungen (z.B. Cursor-Bewegungen, Text-Eingabe) besonders geeignet. CopyRect (Encoding 1) uebertraegt keinerlei Pixeldaten fuer Fensterverschiebungen, sondern nur Quell-Rechteck und Zielkoordinaten. Die Encoding-Verhandlung findet waehrend des RFB-Handshakes statt: Der Client sendet `SetEncodings` mit einer Prioritaetsliste, der Server verwendet `cl->preferredEncoding` pro Client.

#### 3.4.5 Blueprint fuer ClawViewer: Region-basiertes Update-Tracking, Encoding-Verhandlung, Callback-Architektur

Das VNC-Ecosystem liefert drei wesentliche Architekturmuster. Das region-basierte Update-Tracking mit `sraRegionPtr` minimiert die zu uebertragende Datenmenge, da nur tatsaechlich geaenderte Bereiche encodiert werden. Dieses Pattern ist fuer ClawViewer's Bandbreitenoptimierung direkt relevant. Die Encoding-Verhandlung, bei der Client und Server gemeinsam den besten Codec auswaehlen (SetEncodings-Nachricht vom Client, preferredEncoding pro Client auf Serverseite), bildet das Muster fuer ClawViewer's Codec-Negotiation. Die Callback-Architektur (`GotFrameBufferUpdateProc`, `MallocFrameBufferProc`, `kbdAddEvent`, `ptrAddEvent`) ermoeglicht die vollstaendige Entkopplung von Protokoll und Anwendungslogik. Das Deferral-Mechanismus von LibVNCServer (Standard: 5 ms) zum Batching kleiner Updates ist zudem ein Pattern, das ClawViewer's Frame-Sender uebernehmen sollte, um Netzwerk-Overhead zu reduzieren.

### 3.5 xrdp – Linux RDP-Server und Session-Management

#### 3.5.1 Multi-Prozess-Architektur: xrdp (Listener) + sesman (Session Manager) + sesexec (Executor)

Das Repository `neutrinolabs/xrdp` (6.6k Stars, Apache-2.0, C 96.3%) implementiert einen RDP-Server fuer Linux mit einer strikt mehrprozessigen Architektur [^147^]. Drei Hauptprozesse bilden das System: xrdp (RDP-Protokoll-Listener auf Port 3389), sesman (Session Manager auf Port 3350), und sesexec (Session Executor, der per fork/exec Sessions startet).

```
xrdp/
├── xrdp/                   # Hauptdaemon: RDP-Protokoll-Stack
├── libxrdp/                # Core-RDP-Bibliothek
│   ├── xrdp_iso.c          # X.224 Transport
│   ├── xrdp_mcs.c          # MCS (T.125) Multiplexing
│   ├── xrdp_sec.c          # Security Layer
│   ├── xrdp_rdp.c          # Hauptprotokoll-Logik
│   └── xrdp_channel.c      # Virtual Channel Management
├── sesman/                 # Session Manager
│   ├── sesman.c            # Hauptprogramm, Event-Loop
│   ├── scp_process.c       # SCP (Sesman Control Protocol)
│   ├── session_list.c      # Session-Tracking
│   ├── sesexec/sesexec.c   # Session Executor (PAM, fork/exec)
│   └── chansrv/            # Channel Server (Clipboard, Audio)
├── xup/                    # Xorgxrdp-Modul (libxup.so)
└── vnc/                    # VNC-Modul (libvnc.so)
```

#### 3.5.2 Session-Management: SCP-Protokoll, Session-Listen, Policies (UBC/UBD/UBI), EICP/ERCP

Der Session-Manager in `sesman/sesman.c` kommuniziert mit dem xrdp-Daemon ueber das SCP-Protokoll (Sesman Control Protocol) via Unix Domain Sockets [^136^]. Die Session-Liste in `sesman/session_list.c` verwaltet Eintraege mit username, display, ip_addr, pid, status, type, bpp, width, height und start_time [^195^]. Session-Allocations-Policies in `sesman.ini` steuern das Multi-Session-Verhalten: `UBD` (User, BPP, DisplaySize), `UBI` (User, BPP, IPAddr), `UBC` (User, BPP, Connection, immer neue Session) [^130^].

In Version 0.10.x wurde die Authentifizierung in einen separaten sesexec-Prozess ausgelagert, um Blocking des Haupt-sesman-Prozesses zu vermeiden. Der Ablauf: sesman forked sesexec fuer jeden Auth-Vorgang, sesexec fuehrt PAM-Authentifizierung durch, bei Erfolg fork von X-Server + Window Manager + chansrv [^136^].

#### 3.5.3 X11-Integration: xorgxrdp (bevorzugtes Backend), Xvnc (Alternative), SHM-Framebuffer

xrdp unterstuetzt zwei X11-Backends. Das bevorzugte Backend xorgxrdp (separates Repository) verwendet den xrdpdev-Treiber, der X11-Drawing-Operations in Shared-Memory-Framebuffer uebertraegt, den xrdp via `libxup.so` liest [^139^]. Die Xvnc-Alternative startet einen Xvnc-Server und verbindet sich als VNC-Client ueber `libvnc.so` [^143^]. Der Vergleich zeigt: xorgxrdp bietet bessere Performance, dynamisches Resizing, GPU-Acceleration via glamor und H.264/RemoteFX-Unterstuetzung, waehrend Xvnc als Legacy-Option fungiert.

#### 3.5.4 Modul-System: Dynamisches .so-Loading (libxup.so, libvnc.so), Modul-API

xrdp laedt Backend-Module zur Laufzeit als Shared Libraries, konfiguriert in `xrdp.ini` [^154^]. Das Xorg-Modul `libxup.so` (Quelle: `xup/xup.c`) implementiert die Modul-API: `mod_init()`, `mod_connect()`, `mod_start()`, `mod_event()` (Input-Events), `mod_get_event()` (Screen-Updates), `mod_end()` [^138^]. Das VNC-Modul `libvnc.so` (Quelle: `vnc/vnc.c`) implementiert `lib_mod_connect()`, `lib_mod_event()`, `lib_mod_check_wait_objs()` und unterstuetzt die VNC-Encodings Raw, RRE, CopyRect, Cursor und Hextile [^135^]. Das NeutrinoRDP-Modul `libxrdpneutrinordp.so` ermoeglicht RDP-zu-RDP-Proxying [^166^].

Die nachfolgende Tabelle fasst die xrdp-Komponenten und ihre Verantwortlichkeiten zusammen.

| Komponente | Quellcode-Pfad | Zweck | Interprozess-Protokoll |
|---|---|---|---|
| xrdp (Daemon) | `xrdp/` | RDP-Protokoll-Listener (Port 3389) | — |
| libxrdp | `libxrdp/` | Core RDP: ISO, MCS, Security, Channels | — |
| sesman | `sesman/sesman.c` | Session Manager (Port 3350) | SCP (Sesman Control Protocol) |
| sesexec | `sesman/sesexec/sesexec.c` | Session Executor (PAM, fork/exec) | EICP/ERCP |
| chansrv | `sesman/chansrv/` | Channel Server (Clipboard, Audio, Drives) | Intern (Session-Prozess) |
| xup (Xorg-Modul) | `xup/xup.c` | xorgxrdp-Backend (libxup.so) | Modul-API (mod_init/connect/event/end) |
| vnc (VNC-Modul) | `vnc/vnc.c` | Xvnc-Backend (libvnc.so) | Modul-API + VNC-Protokoll |
| session_list | `sesman/session_list.c` | Session-Tracking mit Policies | SCP |

Die Tabelle zeigt die klare Trennung von Verantwortlichkeiten in xrdp's mehrprozessiger Architektur. Der xrdp-Daemon konzentriert sich ausschliesslich auf das RDP-Protokoll, waehrend sesman alle Session-bezogenen Aufgaben uebernimmt. Die Interprozess-Kommunikation laeuft ueber wohldefinierte Protokolle (SCP fuer xrdp↔sesman, EICP/ERCP fuer sesman↔sesexec). Die Modul-API ermoeglicht die Integration beliebiger Backends (Xorg, VNC, RDP-Proxy) ohne Aenderung am Core. Diese Architektur ist direkt auf ClawViewer uebertragbar: Ein zentraler Server-Prozess fuer das P2P-Protokoll, ein separater Session-Manager fuer Benutzer-Sessions, und ein Modul-System fuer verschiedene Capture-Backends.

#### 3.5.5 Blueprint fuer ClawViewer: Multi-Prozess-Isolation, Session-Manager-Pattern, Modul-API

xrdp liefert drei uebernehmbare Architekturmuster. Die Multi-Prozess-Isolation mit separatem Session-Manager und Executor-Prozess stellt sicher, dass ein Crash einer Session keine Auswirkungen auf andere hat. Dieses Pattern ist fuer ClawViewer's Session-Management direkt uebertragbar. Der Session-Manager mit zentraler Session-Liste, Policies und SCP-Protokoll bildet die Referenz fuer ClawViewer's Multi-User-Szenarien. Das Modul-API mit definierten Lebenszyklus-Funktionen (`init`, `connect`, `start`, `event`, `get_event`, `end`) laesst sich auf ClawViewer's Backend-Plugin-System uebertragen. Die PAM-Integration als abstrahierte Authentifizierungsschicht ist zudem ein Pattern, das ClawViewer fuer die Integration verschiedener Auth-Provider uebernehmen kann.

### 3.6 Remmina – Multi-Protokoll-Client-Architektur

#### 3.6.1 Plugin-System: RemminaPluginService-Struct (100+ Funktionen), 7 Plugin-Typen, GModule-Loading

Das Repository `Remmina/Remmina` (GPL-2.0+, C/GTK3) implementiert einen Multi-Protokoll-Remote-Desktop-Client ueber ein dynamisches Plugin-System basierend auf GModule (GLib's Wrapper fuer dlopen) [^196^]. Der Plugin-Manager in `src/remmina_plugin_manager.c` laedt Shared Libraries aus `/usr/lib/remmina/plugins/` und speichert sie in einem `GPtrArray` [^194^].

Remmina definiert 7 Plugin-Typen im Enum `RemminaPluginType`: `REMMINA_PLUGIN_TYPE_PROTOCOL` (Protokoll-Handler), `REMMINA_PLUGIN_TYPE_ENTRY`, `REMMINA_PLUGIN_TYPE_FILE`, `REMMINA_PLUGIN_TYPE_TOOL`, `REMMINA_PLUGIN_TYPE_PREF`, `REMMINA_PLUGIN_TYPE_SECRET` und `REMMINA_PLUGIN_TYPE_LANGUAGE_WRAPPER` [^196^]. Jedes Plugin exportiert eine Entry-Funktion `RemminaPluginEntryFunc` mit Signature `gboolean (*RemminaPluginEntryFunc)(RemminaPluginService *service)`.

Das zentrale `RemminaPluginService`-Struct in `src/include/remmina/plugin.h` enthaelt ueber 100 Funktionszeiger, die Plugins kontrollierten Zugriff auf Core-Funktionalitaeten ermoeglichen: Groessen-Management (`protocol_plugin_get_width/set_width`), Fehler-Handling, Datei-Zugriff, Signal-Emission, Tunnel-Unterstuetzung, Authentifizierungs-UI, Logging und Fenster-Management [^196^].

#### 3.6.2 Protocol-Abstraction: RemminaProtocolWidget als Container, GHashTable-Settings

Die Klasse `RemminaProtocolWidget` in `src/remmina_protocol_widget.h` ist die zentrale Abstraktionsschicht, die als GTK-Container fuer alle Protokoll-Plugins dient [^202^]. Die Struktur erbt von `GtkEventBox` und enthaalt einen Verweis auf das geladene Protocol-Plugin. Der Lebenszyklus umfasst: `remmina_protocol_widget_new()` (Allokation), `remmina_protocol_widget_setup()` (Konfiguration), `remmina_protocol_widget_open_connection()` (Verbindungsaufbau) und `remmina_protocol_widget_close_connection()` (Verbindungsabbau).

Verbindungsprofile werden als `RemminaFile`-Objekte mit `GHashTable`-basierten Key-Value-Stores verwaltet (`src/remmina_file.h`). Die Serialisierung erfolgt in `.remmina`-Dateien im INI-Format. Settings-Typen sind als Enum definiert: `REMMINA_PROTOCOL_SETTING_TYPE_SERVER`, `REMMINA_PROTOCOL_SETTING_TYPE_PASSWORD`, `REMMINA_PROTOCOL_SETTING_TYPE_RESOLUTION` und 13 weitere [^201^].

#### 3.6.3 RDP-Plugin: FreeRDP-basiert, 7 Features, rdp_plugin.c

Das RDP-Plugin in `plugins/rdp/rdp_plugin.c` basiert auf FreeRDP's libfreerdp und definiert 7 Features: `REMMINA_RDP_FEATURE_TOOL_REFRESH`, `REMMINA_RDP_FEATURE_SCALE`, `REMMINA_RDP_FEATURE_UNFOCUS`, `REMMINA_RDP_FEATURE_TOOL_SENDCTRLALDEL`, `REMMINA_RDP_FEATURE_DYNRESUPDATE`, `REMMINA_RDP_FEATURE_MULTIMON` und `REMMINA_RDP_FEATURE_VIEWONLY` [^201^]. Die Plugin-Registrierung erfolgt durch ein statisches `RemminaProtocolPlugin`-Struct mit Lebenszyklus-Callbacks (`remmina_rdp_init`, `remmina_rdp_open_connection`, `remmina_rdp_close_connection`) und Feature-Handling. Die Einstellungen umfassen Server, Username, Password, Domain, Resolution, Color depth, Share folder, Sound, Security und Gateway settings.

#### 3.6.4 VNC-Plugin: libvncclient-basiert, 10 Features, vnc_plugin.c

Das VNC-Plugin in `plugins/vnc/vnc_plugin.c` basiert auf LibVNCClient und definiert 10 Features: `REMMINA_PLUGIN_VNC_FEATURE_PREF_QUALITY`, `REMMINA_PLUGIN_VNC_FEATURE_VIEWONLY`, `REMMINA_PLUGIN_VNC_FEATURE_PREF_DISABLESERVERINPUT`, `REMMINA_PLUGIN_VNC_FEATURE_TOOL_REFRESH`, `REMMINA_PLUGIN_VNC_FEATURE_TOOL_CHAT`, `REMMINA_PLUGIN_VNC_FEATURE_SCALE`, `REMMINA_PLUGIN_VNC_FEATURE_UNFOCUS`, `REMMINA_PLUGIN_VNC_FEATURE_TOOL_SENDCTRLALTDEL`, `REMMINA_PLUGIN_VNC_FEATURE_PREF_COLOR` und `REMMINA_PLUGIN_VNC_FEATURE_DYNRESUPDATE` [^200^]. Das Plugin verwendet `rfbClient` aus libvncclient, unterstuetzt Qualitaetsstufen 0-9 und bietet einen Listener-Modus fuer Reverse-VNC.

Die nachfolgende Tabelle stellt die Plugin-Typen und ihre Merkmale dar.

| Plugin-Typ | Enum-Wert | Beispiel-Plugins | Lebenszyklus-Callbacks | Anzahl Features (Bsp.) |
|---|---|---|---|---|
| Protocol | 0 | RDP, VNC, SSH, SPICE, X2Go | init, open_connection, close_connection, query_feature, call_feature | 7-10 |
| Entry | 1 | — | Entry-Funktion | — |
| File | 2 | .rdp-Import/Export | Import/Export-Handler | — |
| Tool | 3 | Hello-World | Tool-Funktion | — |
| Pref | 4 | — | Praeferenz-Handler | — |
| Secret | 5 | GNOME Keyring, KWallet | store_password, get_password, delete_password | — |
| Language Wrapper | 6 | Python-Wrapper | Sprachintegration | — |

Die Tabelle zeigt Remminas Plugin-Typ-Hierarchie. Protocol-Plugins sind die komplexeste Kategorie, da sie einen vollstaendigen Verbindungslebenszyklus mit Initialisierung, Verbindungsaufbau, Feature-Handling und Verbindungsabbau implementieren muessen. Jedes Protocol-Plugin definiert statisch seine Features als Array von `RemminaProtocolFeature`-Strukturen, die die UI dynamisch in Toolbar-Buttons und Menueeintraege umwandelt. Die Secret-Plugin-Kategorie ermoeglicht die Integration mit OS-spezifischen Credential-Stores (GNOME Keyring, KDE Wallet), sodass Passwoerter niemals im Klartext in Konfigurationsdateien gespeichert werden. Dieses Entkopplungsmuster ist fuer ClawViewer's Integration mit dem OS-Keyring (zur Speicherung von KI-API-Keys) direkt relevant.

#### 3.6.5 Blueprint fuer ClawViewer: Plugin-Service-Pattern, Protocol-Widget-Abstraktion, Feature-Registrierung

Remmina liefert drei zentrale Architekturmuster. Das Plugin-Service-Pattern (`RemminaPluginService`-Struct mit 100+ Funktionszeigern) ermoeglicht die vollstaendige Entkopplung von Protokoll-Plugins und Core-System, ohne dass Plugins direkte Abhaengigkeiten zum Core haben. Die Protocol-Widget-Abstraktion (`RemminaProtocolWidget` als generischer Container) ermoeglicht die einheitliche Behandlung aller Protokolle in der UI. Die Feature-Registrierung (statische Feature-Arrays pro Plugin, dynamische UI-Generierung) ermoeglicht es jedem Protokoll, seine Funktionen deklarativ zu beschreiben. Fuer ClawViewer bedeutet dies: Human-Input und AI-Input koennen als zwei "Input-Plugins" implementiert werden, die ueber die gleiche Service-API agieren und in einer gemeinsamen Event-Pipeline zusammenlaufen.

### 3.7 Gemeinsame Muster und Architektur-Blueprints

#### 3.7.1 Pattern-Uebersicht: 6 extrahierte Design-Patterns aus allen Projekten

Die Analyse der sechs Open-Source-Projekte extrahiert sechs uebergeordnete Design-Patterns, die in Kombination die Architekturgrundlage fuer ClawViewer bilden. Die nachfolgende Tabelle stellt jedes Pattern dar, ordnet es den Quell-Projekten zu und beschreibt die konkrete Anwendung in ClawViewer.

| Pattern | Quell-Projekt(e) | Konkrete Referenz in Quelle | Anwendung in ClawViewer |
|---|---|---|---|
| **Async-Multi-Listener** | RustDesk Server | `rendezvous_server.rs::start()`: tokio::select! ueber UDP + 3x TCP | Tauri-Backend mit tokio::select! fuer WebSocket + UDP + HTTP-API |
| **Protobuf-Oneof-Envelope** | RustDesk Server | `rendezvous.proto`: `message RendezvousMessage { oneof union { ... } }` | ClawViewer-Protokoll-Crate mit oneof fuer alle Nachrichtentypen |
| **Codec-Backend-Registry** | FreeRDP, RustDesk | `codecs.h`: `struct rdp_codecs { H264_CONTEXT* h264; ... }` + `codec.rs`: `enum EncoderCfg { VPX, AOM, HWRAM, VRAM }` | Unified Codec-Manager mit Backend-Trait und Auto-Selektion |
| **Region-basiertes Update-Tracking** | LibVNCServer | `rfb.h`: `sraRegionPtr modifiedRegion/copyRegion/requestedRegion` | Dirty-Region-Tracker fuer effiziente Frame-Delta-Uebertragung |
| **Modul-Lebenszyklus-API** | xrdp | `xup/xup.c`: `mod_init/connect/start/event/get_event/end` | Backend-Plugin-System mit definiertem Trait fuer Capture/Input-Backends |
| **Plugin-Service-Abstraktion** | Remmina | `plugin.h`: `RemminaPluginService` (100+ Funktionszeiger) | `ClawPluginService`-Struct fuer Human/AI-Controller-Integration |

Die Tabelle zeigt, dass jedes der sechs identifizierten Patterns aus einem spezifischen Projekt mit konkreter Code-Referenz extrahiert wurde und eine direkte Entsprechung in ClawViewer's Architektur findet. Das Async-Multi-Listener-Pattern und das Protobuf-Oneof-Envelope aus RustDesk Server bilden die Netzwerkschicht. Die Codec-Backend-Registry kombiniert FreeRDP's Container-Struktur mit RustDesk's Enum-basierter Auto-Selektion. Das Region-basierte Update-Tracking aus LibVNCServer optimiert die Bandbreitennutzung. Das Modul-Lebenszyklus-API aus xrdp und das Plugin-Service-Abstraktion aus Remmina ermoeglichen zusammen die Erweiterbarkeit fuer Human- und AI-Controller.

#### 3.7.2 Implementierungs-Matrix: Welche konkreten Dateien/Funktionen als direkte Referenz dienen

Die nachfolgende Matrix ordnet jeder ClawViewer-Komponente die konkreten Quelldateien und Funktionen zu, die als direkte Implementierungsreferenz dienen.

| ClawViewer-Komponente | Primaere Referenz | Sekundaere Referenz | Zu uebernehmende Funktionen/Strukturen |
|---|---|---|---|
| **Signaling-Server** | `rustdesk-server/src/rendezvous_server.rs` | `rustdesk-server/src/peer.rs`, `src/database.rs` | `RendezvousServer::start()`, `handle_udp()`, `PeerMap`, `RegisterPeer/PunchHoleRequest` |
| **Relay-Server** | `rustdesk-server/src/relay_server.rs` | — | `make_pair_()`, `relay()`, `tokio::select!`-Pattern |
| **Screen-Capture** | `rustdesk/libs/scrap/src/dxgi/mod.rs` | `rustdesk/libs/scrap/src/wayland/pipewire.rs` | `Capturer`-Struktur, `AcquireNextFrame()`, Staging-Texture-Pattern |
| **Codec-Pipeline** | `rustdesk/libs/scrap/src/common/codec.rs` | `FreeRDP/libfreerdp/codec/h264.c`, `codecs.h` | `EncoderCfg`-Enum, Auto-Selektionslogik, `Decoder`-Multi-Instanz |
| **Input-Injection** | `rustdesk/libs/enigo/src/win/win_impl.rs` | `rustdesk/src/server/input_service.rs` | `SendInput()`-Wrapper, `mouse_event()`, `keybd_event()`, `handle_mouse_()` |
| **Verbindungs-State-Machine** | `FreeRDP/libfreerdp/core/rdp.c` | `FreeRDP/libfreerdp/core/connection.c`, `state.h` | `state_run_t`-Enum, State-Machine-Dispatch, Retry-Logik |
| **Framebuffer-Update-Tracking** | `libvncserver/include/rfb/rfb.h` | `libvncserver/src/libvncserver/rfbserver.c` | `sraRegionPtr`, `modifiedRegion`, `FB_UPDATE_PENDING()`, `rfbSendFramebufferUpdate()` |
| **Session-Manager** | `xrdp/sesman/session_list.c` | `xrdp/sesman/scp_process.c` | Session-Listen-Struktur, SCP-Protokoll, Session-Policies (UBD/UBI/UBC) |
| **Backend-Modul-API** | `xrdp/xup/xup.c` | `xrdp/vnc/vnc.c` | `mod_init/connect/start/event/get_event/end`-Pattern |
| **Plugin-Service-API** | `remmina/src/include/remmina/plugin.h` | `remmina/src/remmina_plugin_manager.c` | `RemminaPluginService`-Struct, `RemminaProtocolPlugin`-Registrierung |
| **P2P-Auth** | `rustdesk-server/src/common.rs` | `rustdesk-server/src/rendezvous_server.rs::get_pk()` | `gen_sk()`, Ed25519-Signatur, TOFU-Modell |
| **Channel-Multiplexer** | `FreeRDP/channels/drdynvc/` | `FreeRDP/libfreerdp/core/channels.c` | DVC-Manager-Pattern, statische/dynamische Kanaele |

Die Matrix zeigt, dass fuer jede ClawViewer-Komponente mindestens eine konkrete Quelldatei mit spezifischen Funktionsnamen als Referenz identifiziert werden konnte. Die Primaerreferenzen stammen ueberwiegend aus RustDesk (fuer Netzwerk, Capture, Input, Auth) und FreeRDP (fuer State-Machine, Channels), waehrend LibVNCServer, xrdp und Remmina spezialisierte Patterns fuer Update-Tracking, Session-Management und Plugin-Architektur liefern. Diese verteilte Referenzstruktur reflektiert die Staerken jedes einzelnen Projekts: RustDesk fuehrt in P2P-Netzwerk und Cross-Platform-Capture, FreeRDP in Protokoll-State-Machines, LibVNCServer in Framebuffer-Management, xrdp in Session-Isolation und Remmina in Plugin-Systemen.

Die Kombination dieser sechs extrahierten Patterns mit den zwoelf spezifizierten Code-Referenzen bildet eine solide, auf produktionsreifen Implementierungen basierende Architekturgrundlage fuer ClawViewer. Die in den folgenden Kapiteln entwickelte Proof-of-Concept-Architektur greift diese Referenzen auf und spezifiziert die Adaptation jedes einzelnen Patterns fuer die spezifischen Anforderungen von ClawViewer, insbesondere die Integration von KI-Agenten-Steuerung und bidirektionaler Kontrolluebertragung.
