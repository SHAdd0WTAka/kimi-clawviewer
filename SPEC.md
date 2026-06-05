# SPEC.md – ClawViewer Proof-of-Concept

## 1. Projektuebersicht

**ClawViewer** ist eine KI-gestuetzte Remote-Desktop-Control-App als PoC.
Ziel: P2P-Handshake + Windows Screen-Capture (DXGI) + Input-Injection (SendInput) + KI-Observer via MCP.

**Tech-Stack**: Tauri v2 + Rust-Backend + React-Frontend
**Platform**: Windows 10/11 (primary), spaeter Linux/macOS

## 2. Workspace-Struktur

```
clawviewer/
├── Cargo.toml                    # Workspace
├── crates/
│   ├── cv-shared/                # Typen, Protobuf, Errors, Utils
│   ├── cv-security/              # Ed25519, Session-Passwort
│   ├── cv-capture/               # DXGI Desktop Duplication
│   ├── cv-input/                 # SendInput Maus/Tastatur
│   ├── cv-network/               # WebRTC P2P + Signaling
│   └── cv-mcp/                   # MCP-Server Grundgeruest
├── src-tauri/                    # Tauri v2 App
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs           # Alle Tauri Commands
│   │   ├── state.rs              # Tauri State
│   │   └── webrtc_bridge.rs      # WebRTC <-> Rust Bridge
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
└── src/                          # React-Frontend
    ├── App.tsx
    ├── main.tsx
    ├── components/
    │   ├── ScreenViewer.tsx      # Remote-Screen-Anzeige
    │   ├── ChatPanel.tsx         # Chat-Overlay
    │   ├── ControlBar.tsx        # Steuerungsleiste
    │   ├── ConnectDialog.tsx     # Verbindungsdialog
    │   └── AIStatus.tsx          # KI-Status-Indikator
    └── hooks/
        └── useWebRTC.ts          # WebRTC-Hook
```

## 3. Crate-Spezifikationen

### 3.1 cv-shared

**Zweck**: Gemeinsame Typen, Protobuf-Definitionen, Error-Handling, Utility-Funktionen

**Exports**:
```rust
// lib.rs
pub mod proto;          // Protobuf-Module (rendezvous.rs, message.rs)
pub mod types;          // Gemeinsame Typen
pub mod error;          // Error-Typen
pub mod utils;          // Hilfsfunktionen (Password-Gen, Timestamp)

// types.rs
pub struct SessionId(pub String);
pub struct PeerId(pub String);
pub struct Password(pub String);    // Zeroize-on-Drop

pub enum Platform {
    Windows,
    Linux,
    MacOS,
}

pub enum VideoCodec {
    H264,
    VP9,
    AV1,
}

// Input Event Types (fuer WebRTC DataChannel)
pub struct InputEvent {
    pub source: EventSource,        // Human | AI
    pub event_type: EventType,
    pub priority: Priority,         // P0 | P1 | P2 | P3
    pub payload: EventPayload,
    pub timestamp: u64,             // Unix millis
    pub sequence: u64,
}

pub enum EventSource {
    Human,
    AI,
    System,
}

pub enum EventType {
    MouseMove { x: i32, y: i32 },
    MouseClick { button: MouseButton, down: bool },
    MouseScroll { delta: i32 },
    KeyPress { keycode: u16, down: bool },
    KeyType { text: String },
}

pub enum Priority {
    P0_Emergency,       // Emergency Stop
    P1_Human,           // Human Input (immer Vorrang)
    P2_AI_Confirmed,    // AI mit Bestaetigung
    P3_AI_Autonomous,   // AI autonom
}

pub enum Priority {
    P0_Emergency,       // Emergency Stop
    P1_Human,           // Human Input (immer Vorrang)
    P2_AI_Confirmed,    // AI mit Bestaetigung
    P3_AI_Autonomous,   // AI autonom
}

// error.rs
#[derive(Debug, thiserror::Error)]
pub enum CvError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Capture error: {0}")]
    Capture(String),
    #[error("Input error: {0}")]
    Input(String),
    #[error("Security error: {0}")]
    Security(String),
    #[error("Codec error: {0}")]
    Codec(String),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type CvResult<T> = Result<T, CvError>;
```

**Dependencies**: tokio, prost (Protobuf), thiserror, serde, serde_json, rand, zeroize, chrono

### 3.2 cv-security

**Zweck**: Kryptografie, Authentifizierung, Session-Management

**Exports**:
```rust
// lib.rs
pub mod auth;
pub mod session;
pub mod password;
pub mod keyring;

// auth.rs
pub struct KeyPair {
    pub public: ed25519_dalek::VerifyingKey,
    pub secret: ed25519_dalek::SigningKey,   // Wrapped mit Zeroize
}

impl KeyPair {
    pub fn generate() -> Self;
    pub fn from_file(path: &Path) -> CvResult<Self>;
    pub fn save(&self, path: &Path) -> CvResult<()>;
    pub fn sign(&self, message: &[u8]) -> Signature;
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool;
}

pub struct AuthChallenge {
    pub peer_id: PeerId,
    pub nonce: [u8; 32],
    pub timestamp: u64,
}

impl AuthChallenge {
    pub fn generate(peer_id: &PeerId) -> Self;
    pub fn sign(&self, keypair: &KeyPair) -> Vec<u8>;
    pub fn verify(&self, public_key: &VerifyingKey, signature: &[u8]) -> bool;
}

// session.rs
pub struct Session {
    pub id: SessionId,
    pub password: Password,
    pub peer_id: PeerId,
    pub state: SessionState,
    pub created_at: Instant,
    pub expires_at: Instant,
}

pub enum SessionState {
    Created,
    Active,
    Idle,
    Expired,
}

impl Session {
    pub fn create(peer_id: PeerId) -> Self;           // Neue Session mit Passwort
    pub fn validate_password(&self, pwd: &str) -> bool;
    pub fn is_expired(&self) -> bool;
    pub fn touch(&mut self);                          // Renew idle timer
}

// password.rs
pub fn generate_password_word() -> String;      // 6-stelliges alphanumerisches Wort
pub fn generate_password_token() -> String;     // 12-stelliges Token
pub fn calculate_entropy(password: &str) -> f64;
```

**Dependencies**: cv-shared, ed25519-dalek, x25519-dalek, rand, zeroize, keyring, serde

### 3.3 cv-capture

**Zweck**: Bildschirmerfassung via DXGI Desktop Duplication API (Windows)

**Exports**:
```rust
// lib.rs
pub mod dxgi;

// dxgi.rs
pub struct DxgiCapturer {
    device: ID3D11Device,
    duplication: IDXGIOutputDuplication,
    width: u32,
    height: u32,
}

pub struct Frame {
    pub data: Vec<u8>,           // BGRA Pixel-Daten
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub timestamp: Instant,
    pub dirty_regions: Vec<Rect>, // Nur geaenderte Regionen
}

pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl DxgiCapturer {
    pub fn new(display_index: u32) -> CvResult<Self>;
    pub fn capture_frame(&mut self) -> CvResult<Option<Frame>>;
    pub fn get_resolution(&self) -> (u32, u32);
    pub fn release(&mut self);
}

// Async-Stream
pub fn capture_stream(display_index: u32, fps: u32) -> tokio::sync::mpsc::Receiver<Frame>;
```

**Dependencies**: cv-shared, tokio, windows-rs (features: Win32_Graphics_Dxgi, Win32_Graphics_Direct3D11, Win32_Foundation)

**Windows-APIs**:
- D3D11CreateDevice
- IDXGIOutput1::DuplicateOutput
- AcquireNextFrame / ReleaseFrame
- Map / Unmap fuer Textur-Upload

### 3.4 cv-input

**Zweck**: Input-Injection via SendInput (Windows)

**Exports**:
```rust
// lib.rs
pub mod windows;

// windows.rs (Windows-spezifisch)
pub struct InputInjector;

pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl InputInjector {
    pub fn new() -> Self;
    pub fn move_mouse(&self, x: i32, y: i32) -> CvResult<()>;
    pub fn click(&self, button: MouseButton, down: bool) -> CvResult<()>;
    pub fn scroll(&self, delta: i32) -> CvResult<()>;
    pub fn key_press(&self, keycode: u16, down: bool) -> CvResult<()>;
    pub fn type_text(&self, text: &str) -> CvResult<()>;
}

// Event-Queue mit Priorisierung
pub struct PriorityInputQueue {
    queue: std::collections::BinaryHeap<QueuedEvent>,
    emergency_stop: Arc<AtomicBool>,
}

struct QueuedEvent {
    priority: Reverse<u8>,  // P0=0 (highest), P3=3 (lowest)
    sequence: u64,
    event: InputEvent,
}

impl PriorityInputQueue {
    pub fn new() -> Self;
    pub fn push(&mut self, event: InputEvent);
    pub fn pop(&mut self) -> Option<InputEvent>;
    pub fn emergency_stop(&self);       // Sofortiger KI-Abbruch
    pub fn is_stopped(&self) -> bool;
}
```

**Dependencies**: cv-shared, tokio, windows-rs (features: Win32_UI_Input_KeyboardAndMouse)

**Windows-APIs**:
- SendInput (INPUT_MOUSE, INPUT_KEYBOARD)
- MOUSEEVENTF_MOVE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP
- KEYBDINPUT mit wVk, dwFlags

### 3.5 cv-network

**Zweck**: WebRTC P2P, Signaling, NAT-Traversal

**Exports**:
```rust
// lib.rs
pub mod signaling;
pub mod webrtc;
pub mod peer;

// signaling.rs
pub struct SignalingClient {
    ws_stream: WebSocketStream,
    peer_id: PeerId,
}

pub enum SignalingMessage {
    Register { peer_id: PeerId, public_key: Vec<u8> },
    RequestConnection { target_peer: PeerId, offer: RTCSessionDescription },
    Answer { target_peer: PeerId, answer: RTCSessionDescription },
    IceCandidate { target_peer: PeerId, candidate: RTCIceCandidate },
    KeepAlive,
}

impl SignalingClient {
    pub async fn connect(url: &str, peer_id: PeerId) -> CvResult<Self>;
    pub async fn send(&mut self, msg: SignalingMessage) -> CvResult<()>;
    pub async fn receive(&mut self) -> CvResult<Option<SignalingMessage>>;
}

// webrtc.rs
pub struct P2PConnection {
    peer_connection: RTCPeerConnection,
    data_channel: Arc<RTCDataChannel>,
    video_track: Arc<TrackLocalStaticSample>,
}

pub enum ConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl P2PConnection {
    pub async fn new(config: RTCConfiguration) -> CvResult<Self>;
    pub async fn create_offer(&self) -> CvResult<RTCSessionDescription>;
    pub async fn set_remote_description(&self, desc: RTCSessionDescription) -> CvResult<()>;
    pub async fn add_ice_candidate(&self, candidate: RTCIceCandidate) -> CvResult<()>;
    pub async fn send_video_frame(&self, frame: &[u8], width: u32, height: u32) -> CvResult<()>;
    pub async fn send_input_event(&self, event: &InputEvent) -> CvResult<()>;
    pub fn on_input_event<F: Fn(InputEvent)>(&self, callback: F);
    pub fn on_state_change<F: Fn(ConnectionState)>(&self, callback: F);
    pub async fn close(&self) -> CvResult<()>;
}

// peer.rs
pub struct PeerManager {
    local_keypair: KeyPair,
    connections: HashMap<PeerId, P2PConnection>,
}

impl PeerManager {
    pub fn new(keypair: KeyPair) -> Self;
    pub async fn connect(&mut self, peer_id: PeerId, signaling: &mut SignalingClient) -> CvResult<()>;
    pub fn disconnect(&mut self, peer_id: &PeerId);
    pub fn get_connection(&self, peer_id: &PeerId) -> Option<&P2PConnection>;
}
```

**Dependencies**: cv-shared, cv-security, webrtc-rs, tokio, tokio-tungstenite, serde, serde_json

### 3.6 cv-mcp

**Zweck**: Model Context Protocol Server fuer KI-Agent-Integration

**Exports**:
```rust
// lib.rs
pub mod server;
pub mod tools;
pub mod transport;

// server.rs
pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
    transport: Arc<dyn McpTransport>,
    session: Option<Session>,
}

pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: serde_json::Value) -> CvResult<serde_json::Value>;
}

pub enum McpRequest {
    Initialize { protocol_version: String },
    ToolsList,
    ToolsCall { name: String, input: serde_json::Value },
}

pub enum McpResponse {
    InitializeResult { capabilities: serde_json::Value },
    ToolsListResult { tools: Vec<ToolDefinition> },
    ToolsCallResult { output: serde_json::Value },
    Error { code: i32, message: String },
}

impl McpServer {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self;
    pub fn register_tool(&mut self, tool: Box<dyn McpTool>);
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse;
    pub async fn run(&self) -> CvResult<()>;
}

// tools.rs
pub struct ScreenshotTool;       // screenshot des Remote-Desktops
pub struct MouseClickTool;       // mouse_click(x, y, button)
pub struct MouseMoveTool;        // mouse_move(x, y)
pub struct KeyboardTypeTool;     // keyboard_type(text)
pub struct GetClipboardTool;     // clipboard_get()
pub struct SetClipboardTool;     // clipboard_set(text)

impl McpTool for ScreenshotTool { ... }
// etc.

// transport.rs
pub trait McpTransport: Send + Sync {
    async fn read_request(&self) -> CvResult<Option<McpRequest>>;
    async fn write_response(&self, response: &McpResponse) -> CvResult<()>;
}

pub struct StdioTransport;
pub struct HttpSseTransport;
```

**Dependencies**: cv-shared, tokio, serde, serde_json, async-trait

## 4. Tauri-App Spezifikation

### 4.1 Commands (IPC-Interface)

```rust
// commands.rs

#[tauri::command]
async fn generate_session_password() -> Result<String, String>;

#[tauri::command]
async fn create_session(password: String) -> Result<SessionInfo, String>;

#[tauri::command]
async fn connect_to_peer(peer_id: String, password: String) -> Result<(), String>;

#[tauri::command]
async fn disconnect() -> Result<(), String>;

#[tauri::command]
async fn start_capture(display_index: u32) -> Result<(), String>;

#[tauri::command]
async fn stop_capture() -> Result<(), String>;

#[tauri::command]
async fn send_mouse_move(x: i32, y: i32) -> Result<(), String>;

#[tauri::command]
async fn send_mouse_click(button: String, down: bool) -> Result<(), String>;

#[tauri::command]
async fn send_key_press(keycode: u16, down: bool) -> Result<(), String>;

#[tauri::command]
async fn send_chat_message(message: String, msg_type: String) -> Result<(), String>;

#[tauri::command]
async fn get_ai_status() -> Result<AIStatus, String>;

#[tauri::command]
async fn set_ai_mode(mode: String) -> Result<(), String>;

#[tauri::command]
async fn emergency_stop() -> Result<(), String>;
```

### 4.2 State

```rust
// state.rs
pub struct AppState {
    pub peer_manager: Arc<Mutex<PeerManager>>,
    pub capture: Arc<Mutex<Option<DxgiCapturer>>>,
    pub input_queue: Arc<Mutex<PriorityInputQueue>>,
    pub mcp_server: Arc<Mutex<Option<McpServer>>>,
    pub connection_state: Arc<Mutex<ConnectionState>>,
    pub video_sender: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Frame>>>>,
}
```

### 4.3 Frontend-Komponenten

**App.tsx**: Hauptlayout mit ScreenViewer (Hauptbereich), ControlBar (unten), ChatPanel (rechts)
**ScreenViewer.tsx**: HTMLVideoElement fuer Remote-Stream + Canvas-Overlay fuer Ghost-Cursor
**ConnectDialog.tsx**: Peer-ID-Eingabe, Passwort, Verbinden-Button
**ControlBar.tsx**: Capture start/stop, AI-Modus (Observer/Shared/Full), E-Stop-Button
**ChatPanel.tsx**: Nachrichten-Thread, KI-Status, Session-Info
**AIStatus.tsx**: Visualisierung des aktuellen KI-Modus und der Confidence

### 4.4 WebRTC Hook

```typescript
// useWebRTC.ts
export function useWebRTC() {
  const [connectionState, setConnectionState] = useState<ConnectionState>('new');
  const [remoteStream, setRemoteStream] = useState<MediaStream | null>(null);
  
  const connect = async (peerId: string, password: string) => { ... };
  const disconnect = async () => { ... };
  const sendInputEvent = (event: InputEvent) => { ... };
  const sendChatMessage = (message: string) => { ... };
  
  return { connectionState, remoteStream, connect, disconnect, sendInputEvent, sendChatMessage };
}
```

## 5. Protokoll-Spezifikation

### 5.1 Protobuf-Definitionen

```protobuf
// rendezvous.proto
syntax = "proto3";

message RendezvousMessage {
  oneof union {
    RegisterPeer register_peer = 1;
    RegisterPk register_pk = 2;
    PunchHoleRequest punch_hole_request = 3;
    PunchHole punch_hole = 4;
    PunchHoleSent punch_hole_sent = 5;
    PunchHoleResponse punch_hole_response = 6;
    RequestRelay request_relay = 7;
    RelayResponse relay_response = 8;
    KeepAlive keep_alive = 9;
  }
}

message RegisterPeer {
  string peer_id = 1;
  bytes public_key = 2;
  string password = 3;
}

message RegisterPk {
  string peer_id = 1;
  bytes public_key = 2;
}

message PunchHoleRequest {
  string peer_id = 1;
  string target_peer = 2;
  int32 nat_type = 3;
}

message PunchHole {
  string peer_id = 1;
  bytes ip = 2;
  int32 port = 3;
  int32 nat_type = 4;
}

message PunchHoleSent {
  string peer_id = 1;
  string target_peer = 2;
}

message PunchHoleResponse {
  string peer_id = 1;
  bytes ip = 2;
  int32 port = 3;
  int32 nat_type = 4;
}

message RequestRelay {
  string peer_id = 1;
  string target_peer = 2;
}

message RelayResponse {
  string relay_id = 1;
  string peer_id = 2;
}

message KeepAlive {}

// message.proto (fuer DataChannel)
message DataMessage {
  oneof union {
    InputEvent input_event = 1;
    ChatMessage chat_message = 2;
    VideoFrame video_frame = 3;
    SystemMessage system_message = 4;
  }
}

message InputEvent {
  string source = 1;          // "human" | "ai" | "system"
  int32 priority = 2;         // 0-3
  int32 event_type = 3;       // Enum
  bytes payload = 4;          // JSON-serialisiert
  uint64 timestamp = 5;
  uint64 sequence = 6;
}

message ChatMessage {
  string msg_type = 1;        // "chat" | "system" | "ai-action" | "human-override"
  string sender = 2;
  string content = 3;
  uint64 timestamp = 4;
}

message VideoFrame {
  bytes data = 1;
  uint32 width = 2;
  uint32 height = 3;
  uint32 pitch = 4;
  uint64 timestamp = 5;
  repeated Rect dirty_regions = 6;
}

message Rect {
  int32 left = 1;
  int32 top = 2;
  int32 right = 3;
  int32 bottom = 4;
}

message SystemMessage {
  string msg_type = 1;
  string content = 2;
}
```

## 6. Build & Distribution

### Cargo.toml (Workspace)
```toml
[workspace]
members = ["crates/*", "src-tauri"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.44", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
anyhow = "1.0"
prost = "0.13"
bytes = "1.7"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### src-tauri/Cargo.toml
```toml
[package]
name = "clawviewer-tauri"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2.0", features = [] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
cv-shared = { path = "../crates/cv-shared" }
cv-security = { path = "../crates/cv-security" }
cv-capture = { path = "../crates/cv-capture" }
cv-input = { path = "../crates/cv-input" }
cv-network = { path = "../crates/cv-network" }
cv-mcp = { path = "../crates/cv-mcp" }
```

## 7. Test-Strategie

### Unit-Tests (pro Crate)
- cv-shared: Typ-Konvertierung, Serde-Roundtrip, Password-Gen-Entropie
- cv-security: Ed25519 Sign/Verify, Challenge-Response, Session-Lifecycle
- cv-capture: DXGI Init (Mock), Frame-Format-Validierung
- cv-input: Event-Queue Ordering, Priority-Sortierung, E-Stop
- cv-network: Signaling-Msg Serde, ICE-Candidate-Parsing
- cv-mcp: Tool-Registration, Request/Response Roundtrip

### Integration-Tests
- P2P-Handshake lokal (Loopback)
- Capture -> Encode -> Packetize Pipeline
- Input-Event von DataChannel bis SendInput
- MCP-Tool-Aufruf via stdio Transport

## 8. Entwicklungs-Reihenfolge

### Phase 1 (Runde 1 – parallel)
1. cv-shared: Typen + Protobuf-Build
2. cv-security: Ed25519 + Passwort + Session
3. cv-capture: DXGI Desktop Duplication
4. cv-input: SendInput + Priority Queue

### Phase 2 (Runde 2 – parallel, haengt von Runde 1 ab)
5. cv-network: WebRTC P2P + Signaling
6. cv-mcp: MCP-Server + Tools

### Phase 3 (Runde 3 – haengt von allen ab)
7. Tauri-App: Commands + State + Frontend
8. Integration: Alles zusammenfuehren
