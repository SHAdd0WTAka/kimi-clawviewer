# ClawViewer Dim 08 – Tauri v2 + Rust Backend-Architektur

## Zusammenfassung

Dieses Dokument analysiert Tauri v2 als Framework fuer eine Remote-Desktop-App mit Rust-Backend und React/Vue-Frontend. Es deckt Architektur, Command-System, FFI, Multi-Window, Plugin-System, OS-Integration, Build/Distribution und Performance ab. Die Recherche basiert auf 23+ Web-Searches und verarbeitet Ergebnisse aus der offiziellen Tauri-Dokumentation, GitHub-Diskussionen, Blog-Posts und Community-Quellen.

---

## 1. Tauri v2 Architecture: Core → Shell → IPC → WebView

### 1.1 Grundlegende Architektur

Tauri v2 folgt einer mehrschichtigen Architektur, die Web-Technologien mit nativem Rust-Backend verbindet [^246^]:

```
┌─────────────────────────────────────────────────────────────┐
│                    FRONTEND (WebView)                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   React/    │  │  @tauri-    │  │   window.__TAURI__  │ │
│  │   Vue/Svelte│  │  apps/api   │  │   (global API)      │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                         IPC Layer                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   invoke()  │  │   Events    │  │   Channels          │ │
│  │   (req/res) │  │   (emit)    │  │   (streaming)       │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    TAURI CORE (Rust)                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Command   │  │   Plugin    │  │   State Manager     │ │
│  │   Router    │  │   Manager   │  │   (Managed State)   │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                      OS ABSTRACTION                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   WRY       │  │    TAO      │  │   Native APIs       │ │
│  │  (WebView)  │  │  (Window)   │  │   (DXGI, etc.)      │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Schluesselkomponenten** [^246^] [^250^]:
- **TAO (Tauri App Object)**: Plattform-uebergreifende Window-Management und Event-Handling
- **WRY (Webview Render Yahoo)**: WebView-Komponente fuer HTML/CSS/JS-Rendering
- **Tauri Core**: Verbindet Frontend und Backend via IPC, bietet Plugin-System, State-Management
- **WRY WebView-Engines pro Plattform**: WebKit (macOS), WebKitGTK (Linux), WebView2 (Windows)

### 1.2 IPC-Kommunikation

Tauri verwendet ein stark typisiertes IPC-System [^208^]:

- **Frontend → Backend**: `invoke(commandName, args)` - Promise-basierte Requests
- **Backend → Frontend**: `window.emit(eventName, payload)` - Event-basiertes Streaming
- **Channel-API**: Fuer bidirektionale Streaming-Kommunikation mit Progress-Updates

### 1.3 Architektur-Vergleich: Tauri vs Electron

| Aspekt | Tauri v2 | Electron |
|--------|----------|----------|
| Backend | Rust | Node.js (gebundelt) |
| Frontend Renderer | OS WebView (WKWebView/WebView2/WebKitGTK) | Chromium (gebundelt) |
| Bundle Size | 3-10 MB | 120-200 MB |
| RAM at idle | 40-80 MB | 150-400 MB |
| Startup Time | < 200ms - 380ms | 2-5s |
| Memory Safety | Rust Borrow Checker | Node.js Heap Management |
| Cross-Platform Consistency | Variiert by OS WebView | Identisch (same Chromium) |
| Mobile Support | iOS + Android | Nicht verfuegbar |

Quellen: [^259^] [^260^] [^264^] [^248^]

---

## 2. Command-System: Tauri Commands, Invoke-Handler, State-Management

### 2.1 Command-Definition in Rust

Commands sind das Kernstueck der Frontend-Backend-Kommunikation in Tauri v2 [^208^]:

```rust
// src-tauri/src/commands.rs
use tauri::{State, AppHandle, ipc::Response};

// Einfacher Command ohne Argumente
#[tauri::command]
pub fn greet() -> String {
    "Hello from Rust!".into()
}

// Command mit Argumenten (camelCase aus Frontend)
#[tauri::command]
pub fn greet_by_name(name: String) -> String {
    format!("Hello, {}!", name)
}

// Async Command mit AppHandle-Zugriff
#[tauri::command]
pub async fn async_task(app_handle: AppHandle) -> Result<String, String> {
    let app_dir = app_handle.path().app_dir()
        .map_err(|e| e.to_string())?;
    Ok(format!("App dir: {:?}", app_dir))
}

// Command mit State-Zugriff
#[tauri::command]
pub fn get_counter(state: State<'_, CounterState>) -> i32 {
    state.0.lock().unwrap().clone()
}

// Command mit Binary-Response (optimiert)
#[tauri::command]
pub fn read_file() -> Response {
    let data = std::fs::read("/path/to/file").unwrap();
    Response::new(data)
}
```

### 2.2 Command-Registrierung in lib.rs

```rust
// src-tauri/src/lib.rs
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(CounterState(std::sync::Mutex::new(0)))
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::greet_by_name,
            commands::async_task,
            commands::get_counter,
            commands::read_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 2.3 Frontend-Aufruf via invoke()

```typescript
// React/Vue/TypeScript Frontend
import { invoke } from '@tauri-apps/api/core';

// Einfacher Aufruf
const message = await invoke('greet');
console.log(message); // "Hello from Rust!"

// Mit Argumenten (camelCase!)
const greeting = await invoke('greet_by_name', { name: 'ClawViewer' });
console.log(greeting); // "Hello, ClawViewer!"

// Fehlerbehandlung
invoke('login', { user: 'admin', password: 'secret' })
  .then((token) => console.log(token))
  .catch((error) => console.error(error));
```

### 2.4 State Management

Tauri bietet ein eingebautes State-Management-System [^208^] [^258^]:

```rust
// State-Definition
struct AppState {
    db_connection: Database,
    config: AppConfig,
    active_sessions: Arc<Mutex<HashMap<String, Session>>>,
}

// State registrieren
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            db_connection: Database::new(),
            config: AppConfig::default(),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            get_active_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// State in Commands verwenden
#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.clone())
}

#[tauri::command]
async fn get_active_sessions(
    state: State<'_, AppState>
) -> Result<Vec<Session>, String> {
    let sessions = state.active_sessions.lock().await;
    Ok(sessions.values().cloned().collect())
}
```

### 2.5 Async Runtime (Tokio)

Tauri v2 nutzt Tokio als Async-Runtime [^283^] [^284^]:

```rust
// Tokio-Mutex fuer State across await points
use tokio::sync::Mutex;

struct AsyncState {
    connections: Mutex<Vec<WebSocketConnection>>,
}

#[tauri::command]
async fn broadcast_message(
    state: State<'_, AsyncState>,
    message: String,
) -> Result<(), String> {
    let connections = state.connections.lock().await;
    for conn in connections.iter() {
        conn.send(message.clone()).await?;
    }
    Ok(())
}
```

**Wichtige Best Practice**: `tokio::sync::Mutex` statt `std::sync::Mutex` verwenden, wenn Locks ueber `.await`-Punkte gehalten werden muessen [^284^].

### 2.6 Backend → Frontend Events

Fuer Streaming-Updates oder Push-Benachrichtigungen vom Backend [^212^]:

```rust
// Event vom Backend an Frontend senden
#[tauri::command]
async fn long_running_task(
    window: tauri::Window,
    app_handle: AppHandle,
) -> Result<String, String> {
    for i in 0..100 {
        // Fortschritt an Frontend senden
        window.emit("progress", i).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok("Complete".into())
}
```

```typescript
// Frontend: Event-Listener
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<number>('progress', (event) => {
  console.log(`Progress: ${event.payload}%`);
  setProgress(event.payload);
});

// Cleanup bei Component Unmount
unlisten();
```

---

## 3. FFI zu nativem Code: Rust → OS APIs

### 3.1 FFI-Patterns in Tauri

Tauri ermoeglicht direkten Zugriff auf OS-native APIs via Rust FFI. Die haefigsten Patterns fuer Remote-Desktop-Apps:

**Pattern 1: Direct OS API Calls in Rust**
```rust
// Windows: DXGI Screen Capture
#[cfg(target_os = "windows")]
mod capture {
    use windows::Win32::Graphics::Dxgi::*;
    
    pub fn capture_screen() -> Vec<u8> {
        // DXGI Desktop Duplication API
        unsafe {
            // IDXGIOutputDuplication::AcquireNextFrame()
            // ... Frame-Capture-Logik
        }
    }
}

// Linux: PipeWire via D-Bus
#[cfg(target_os = "linux")]
mod capture {
    pub fn capture_screen() -> Vec<u8> {
        // PipeWire xdg-desktop-portal
        // portal::ScreenCast::create_session()
    }
}

// macOS: CGDisplayStream
#[cfg(target_os = "macos")]
mod capture {
    use core_graphics::display::*;
    
    pub fn capture_screen() -> Vec<u8> {
        // CGDisplayCreateImage()
    }
}
```

**Pattern 2: Tauri Commands als FFI-Bridge**
```rust
#[tauri::command]
pub async fn capture_frame(
    display_id: u32,
) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "windows")]
    {
        crate::capture::dxgi::capture_display(display_id)
            .await
            .map_err(|e| e.to_string())
    }
    
    #[cfg(target_os = "linux")]
    {
        crate::capture::pipewire::capture_display(display_id)
            .await
            .map_err(|e| e.to_string())
    }
    
    #[cfg(target_os = "macos")]
    {
        crate::capture::cgdisplay::capture_display(display_id)
            .await
            .map_err(|e| e.to_string())
    }
}
```

### 3.2 Platform-abstraction Layer

Fuer eine Remote-Desktop-App sollte eine Platform-Abstraktion implementiert werden:

```rust
// src-tauri/src/platform/mod.rs
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

pub trait ScreenCapture {
    async fn capture_frame(&self, display_id: u32) -> Result<Vec<u8>, CaptureError>;
    fn list_displays(&self) -> Vec<DisplayInfo>;
}

pub trait InputInjection {
    async fn send_mouse_move(&self, x: i32, y: i32) -> Result<(), InputError>;
    async fn send_mouse_click(&self, button: MouseButton, down: bool) -> Result<(), InputError>;
    async fn send_key(&self, key: KeyCode, down: bool) -> Result<(), InputError>;
}
```

### 3.3 Windows-Spezifisch: DXGI + SendInput

```rust
// src-tauri/src/platform/windows/capture.rs
use windows::Win32::Graphics::Dxgi::{
    IDXGIOutputDuplication, DXGI_OUTDUPL_FRAME_INFO,
    DXGI_OUTPUT_DESC, CreateDXGIFactory1,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_MOUSE, MOUSEINPUT,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_LEFTDOWN,
};

pub struct WindowsScreenCapture {
    duplication: IDXGIOutputDuplication,
}

impl ScreenCapture for WindowsScreenCapture {
    async fn capture_frame(&self, _display_id: u32) -> Result<Vec<u8>, CaptureError> {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut desktop_resource = None;
        
        unsafe {
            self.duplication.AcquireNextFrame(
                100, // timeout ms
                &mut frame_info,
                &mut desktop_resource,
            )?;
            // ... Frame zu Vec<u8> konvertieren
        }
    }
}

pub fn inject_mouse_move(x: i32, y: i32) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: x,
                    dy: y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                }
            }
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}
```

---

## 4. Multi-Window: Separate Fenster fuer Chat, Settings, Screen-View

### 4.1 Fenster-Erstellung in Tauri v2

Tauri v2 unterstuetzt mehrere Fenster mit unterschiedlichen Faehigkeiten [^211^] [^214^] [^219^]:

```rust
use tauri::{
    WebviewWindowBuilder, WebviewUrl, Manager, AppHandle
};

#[tauri::command]
fn create_chat_overlay(app: AppHandle) -> tauri::Result<()> {
    let chat_window = WebviewWindowBuilder::new(
        &app,
        "chat", // eindeutiges Label
        WebviewUrl::App("/chat.html".into()),
    )
    .title("Chat Overlay")
    .inner_size(400.0, 600.0)
    .position(1000.0, 100.0)
    .always_on_top(true)
    .transparent(true)
    .decorations(false) // Keine Fensterdekorationen
    .skip_taskbar(true)
    .build()?;
    
    Ok(())
}

#[tauri::command]
fn create_settings_window(app: AppHandle) -> tauri::Result<()> {
    let settings_window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("/settings.html".into()),
    )
    .title("Einstellungen")
    .inner_size(800.0, 600.0)
    .center()
    .resizable(false)
    .maximizable(false)
    .build()?;
    
    Ok(())
}
```

### 4.2 Fenster-Konfiguration mit separaten Capabilities

Jedes Fenster kann eigene Berechtigungen erhalten [^211^]:

```json
// src-tauri/capabilities/main-window.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main-capability",
  "description": "Hauptfenster mit Screen-Capture-Zugriff",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "fs:default",
    "dialog:default",
    "shell:default"
  ]
}
```

```json
// src-tauri/capabilities/chat-overlay.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "chat-overlay-capability",
  "description": "Chat-Overlay minimale Berechtigungen",
  "windows": ["chat"],
  "permissions": [
    "core:default"
  ]
}
```

```json
// src-tauri/capabilities/settings-window.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "settings-capability",
  "description": "Settings-Fenster",
  "windows": ["settings"],
  "permissions": [
    "core:default",
    "fs:allow-read-text-file",
    "fs:allow-write-text-file"
  ]
}
```

### 4.3 Frontend: Fenster-Management

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getCurrentWindow } from '@tauri-apps/api/window';

// Chat-Overlay oeffnen
async function openChatOverlay() {
    const webview = new WebviewWindow('chat', {
        url: '/chat.html',
        width: 400,
        height: 600,
        x: 1000,
        y: 100,
        alwaysOnTop: true,
        transparent: true,
        decorations: false,
    });
    
    webview.once('tauri://created', () => {
        console.log('Chat overlay created');
    });
    
    webview.once('tauri://error', (e) => {
        console.error('Error creating chat overlay:', e);
    });
}

// Aktuelles Fenster minimieren
async function minimizeCurrentWindow() {
    const window = getCurrentWindow();
    await window.minimize();
}
```

**Wichtig**: Fuer das Erstellen neuer Fenster muss die Permission `core:webview:allow-create-webview-window` in den Capabilities gesetzt sein [^219^].

### 4.4 Multi-Window fuer Remote-Desktop-App

Fuer eine Remote-Desktop-App mit Tauri empfiehlt sich folgende Fenster-Struktur:

| Fenster | Label | Zweck | Besonderheiten |
|---------|-------|-------|----------------|
| Hauptfenster | `main` | Screen-View + Hauptsteuerung | Vollbild, Hardware-Beschleunigung |
| Chat-Overlay | `chat` | Team-Chat waehrend Session | Always-on-top, transparent, frameless |
| Einstellungen | `settings` | App-Konfiguration | Modal, fixed size |
| Verbindungsmanager | `connections` | Gespeicherte Verbindungen | Separater Tab |
| Datei-Transfer | `filetransfer` | Datei-Upload/Download | Fortschrittsanzeige |

---

## 5. Plugin-System: Tauri Plugins, Native Plugins

### 5.1 Offizielle Plugins

Tauri v2 bietet ein umfangreiches Plugin-Oekosystem [^209^] [^282^]:

| Plugin | Zweck | Win | Mac | Lin | iOS | Android |
|--------|-------|-----|-----|-----|-----|---------|
| fs | Dateisystem-Zugriff | ✅ | ✅ | ✅ | ? | ? |
| dialog | Native Dialoge (Open/Save) | ✅ | ✅ | ✅ | ✅ | ✅ |
| http | HTTP-Client in Rust | ✅ | ✅ | ✅ | ✅ | ✅ |
| notification | System-Notifications | ✅ | ✅ | ✅ | ✅ | ✅ |
| global-shortcut | Globale Hotkeys | ✅ | ✅ | ✅ | ? | ? |
| clipboard-manager | Zwischenablage | ✅ | ✅ | ✅ | ✅ | ✅ |
| shell | Shell-Befehle ausfuehren | ✅ | ✅ | ✅ | ❌ | ❌ |
| process | Prozess-Management | ✅ | ✅ | ✅ | ? | ? |
| log | Logging-Framework | ✅ | ✅ | ✅ | ✅ | ✅ |
| updater | Auto-Update | ✅ | ✅ | ✅ | ✅ | ✅ |
| autostart | Autostart mit System | ✅ | ✅ | ✅ | ❌ | ❌ |
| deep-link | URL-Scheme Handler | ✅ | ✅ | ✅ | ✅ | ✅ |
| positioner | Fenster-Positionierung | ✅ | ✅ | ✅ | ❌ | ❌ |
| opener | Dateien/URLs oeffnen | ✅ | ✅ | ✅ | ✅ | ✅ |

Quelle: [^282^]

### 5.2 Plugin-Installation

```bash
# Plugin zur Tauri-App hinzufuegen
npm run tauri add fs
npm run tauri add dialog
npm run tauri add notification
npm run tauri add global-shortcut
npm run tauri add updater
```

### 5.3 Eigenes Plugin entwickeln

Fuer Remote-Desktop-spezifische Funktionalitaet kann ein eigenes Plugin entwickelt werden [^209^]:

```bash
# Neues Plugin erstellen
npx @tauri-apps/cli plugin new screen-capture
```

**Plugin-Struktur:**
```
tauri-plugin-screen-capture/
├── src/
│   ├── commands.rs      # Commands fuer Webview
│   ├── desktop.rs       # Desktop-Implementierung
│   ├── error.rs         # Error-Typen
│   ├── lib.rs           # Re-exports, Setup
│   ├── mobile.rs        # Mobile-Implementierung
│   └── models.rs        # Shared Structs
├── permissions/         # Permission-Definitionen
├── android/             # Android Library (Kotlin)
├── ios/                 # Swift Package
├── guest-js/            # JavaScript API Bindings
├── Cargo.toml
└── package.json
```

**Plugin-Beispiel (lib.rs):**
```rust
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[derive(Default)]
pub struct ScreenCapturePlugin;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("screen-capture")
        .setup(|app, api| {
            // Plugin-Initialisierung
            app.manage(ScreenCaptureState::new()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::capture_frame,
            commands::list_displays,
        ])
        .on_event(|app, event| {
            // Lifecycle-Events (Window-Events, App-Exit, etc.)
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    // Cleanup
                }
                _ => {}
            }
        })
        .build()
}
```

### 5.4 Permission-System

Commands sind standardmaessig **nicht** zugaenglich. Jeder Command benoetigt eine explizite Permission [^209^] [^281^]:

```toml
# permissions/default.toml
[[permission]]
identifier = "allow-capture-frame"
description = "Erlaubt Frame-Capture vom Bildschirm"
commands.allow = ["capture_frame"]

[[permission]]
identifier = "allow-list-displays"
description = "Erlaubt Auflisten der Displays"
commands.allow = ["list_displays"]
```

---

## 6. OS-Integration: System-Tray, Notifications, Global Shortcuts

### 6.1 System-Tray

Tauri v2 bietet umfassende System-Tray-Unterstuetzung [^216^] [^210^]:

```rust
use tauri::{
    menu::{MenuItemBuilder, MenuBuilder},
    tray::TrayIconBuilder,
};

pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    // Menu-Items erstellen
    let connection_status = MenuItemBuilder::new("Status: Pruefe...")
        .id("connection_status")
        .build(app)?;
    
    let hide = MenuItemBuilder::new("Ausblenden")
        .id("hide")
        .build(app)?;
    
    let show = MenuItemBuilder::new("Anzeigen")
        .id("show")
        .build(app)?;
    
    let quit = MenuItemBuilder::new("Beenden")
        .id("quit")
        .build(app)?;
    
    // Menu zusammenbauen
    let menu = MenuBuilder::new(app)
        .items(&[&connection_status, &hide, &show, &quit])
        .build()?;
    
    // Tray-Icon erstellen
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("ClawViewer Remote Desktop")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.hide().unwrap();
                }
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
            _ => {}
        })
        .build(app)?;
    
    // Dynamisches Status-Update (async)
    let app_handle = app.app_handle().clone();
    let status_item = connection_status.clone();
    tauri::async_runtime::spawn(async move {
        use tokio::time::{sleep, Duration};
        loop {
            let status = check_connection_status(&app_handle).await;
            let label = match status {
                Ok(true) => "🟢 Verbunden",
                Ok(false) => "⚪ Getrennt",
                Err(_) => "🔴 Fehler",
            };
            let _ = status_item.set_text(label);
            sleep(Duration::from_secs(5)).await;
        }
    });
    
    Ok(())
}
```

**Cargo.toml Feature:**
```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
```

### 6.2 Notifications

```typescript
// Frontend: Notification-Plugin
import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
} from '@tauri-apps/plugin-notification';

async function notifyConnectionLost() {
    let permissionGranted = await isPermissionGranted();
    
    if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === 'granted';
    }
    
    if (permissionGranted) {
        sendNotification({
            title: 'ClawViewer',
            body: 'Verbindung zum Remote-PC verloren!',
            icon: 'icons/disconnect.png',
        });
    }
}
```

### 6.3 Globale Shortcuts

Globale Shortcuts funktionieren systemweit, auch wenn die App nicht fokussiert ist [^285^] [^289^] [^292^]:

```rust
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut,
};

pub fn setup_shortcuts(app: &mut tauri::App) -> tauri::Result<()> {
    // Shortcut definieren
    let shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyC,
    );
    
    // Plugin initialisieren
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut_event, event| {
                if event.state() == ShortcutState::Pressed {
                    match shortcut_event.key {
                        Code::KeyC => {
                            // Chat-Overlay togglen
                            toggle_chat_overlay(app);
                        }
                        Code::KeyF => {
                            // Vollbild togglen
                            toggle_fullscreen(app);
                        }
                        _ => {}
                    }
                }
            })
            .build(),
    )?;
    
    // Shortcut registrieren
    app.global_shortcut().register(shortcut)?;
    
    Ok(())
}
```

**Wichtige Best Practices fuer Global Shortcuts** [^285^]:
- Shortcuts sollten **nicht hardcoded** sein - Benutzer-konfigurierbar machen
- Auf macOS Accessibility-Permissions pruefen und anfordern
- Alte Shortcuts vor Neu-Registrierung deregistrieren
- Konflikte mit anderen Apps vermeiden durch komplexe Kombinationen

**Required Capabilities:**
```json
{
  "permissions": [
    "global-shortcut:allow-is-registered",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister"
  ]
}
```

---

## 7. Build & Distribution: Cross-Compilation, Code-Signing, Updates

### 7.1 Build-Konfiguration fuer Bundle-Groesse

Tauri v2 ermoeglicht extrem kleine Bundles durch Rust-Optimierungen [^249^] [^241^]:

```toml
# Cargo.toml - Release-Optimierungen
[profile.release]
codegen-units = 1          # Bessere LLVM-Optimierung
lto = true                 # Link-Time Optimization
opt-level = "s"            "z" fuer min. Groesse, "3" fuer max. Speed
panic = "abort"            # Keine Panic-Handler (kleiner Binary)
strip = true               # Debug-Symbole entfernen

# ODER fuer maximale Performance:
[profile.release-fast]
inherits = "release"
opt-level = 3
codegen-units = 16
lto = "thin"
```

```json
// tauri.conf.json - Bundle-Konfiguration
{
  "build": {
    "removeUnusedCommands": true
  },
  "bundle": {
    "active": true,
    "category": "DeveloperTool",
    "copyright": "2026 ClawViewer",
    "targets": ["nsis", "msi", "appimage", "dmg", "deb"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      },
      "certificateThumbprint": null
    },
    "macOS": {
      "frameworks": [],
      "minimumSystemVersion": "10.13",
      "signingIdentity": null
    },
    "linux": {
      "appimage": {
        "bundleMediaFramework": false
      }
    }
  }
}
```

### 7.2 Code-Signing

```bash
# Windows: Zertifikat konfigurieren
export TAURI_SIGNING_PRIVATE_KEY="Pfad/zum/private.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="password"

# macOS: Signing Identity
export APPLE_SIGNING_IDENTITY="Developer ID Application: Team Name"
export APPLE_ID="apple@id.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAM_ID"

# Build mit Signierung
npm run tauri build -- --target universal-apple-darwin
```

### 7.3 Auto-Updater

Der Tauri Updater unterstuetzt signierte Updates von einem Server oder statischem JSON [^213^] [^287^] [^291^]:

```json
// tauri.conf.json - Updater-Konfiguration
{
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...",
      "endpoints": [
        "https://releases.clawviewer.com/{{target}}/{{arch}}/{{current_version}}",
        "https://github.com/clawviewer/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

**Frontend-Implementierung:**
```typescript
import { check } from '@tauri-apps/plugin-updater';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';

export async function checkForUpdates() {
    const update = await check();
    
    if (update?.available) {
        const yes = await ask(
            `Update auf ${update.version} verfuegbar!\n\n${update.body}`,
            {
                title: 'Update verfuegbar',
                kind: 'info',
                okLabel: 'Jetzt aktualisieren',
                cancelLabel: 'Spaeter',
            }
        );
        
        if (yes) {
            await update.downloadAndInstall();
            await relaunch();
        }
    }
}
```

**Update-Server JSON-Format:**
```json
{
  "version": "1.2.0",
  "notes": "Neue Features und Bugfixes",
  "pub_date": "2026-04-15T12:00:00Z",
  "signature": "...base64-signature...",
  "url": "https://releases.clawviewer.com/v1.2.0/ClawViewer_1.2.0_x64-setup.exe"
}
```

### 7.4 Cross-Platform Build mit GitHub Actions

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - platform: 'macos-latest'
            args: '--target universal-apple-darwin'
          - platform: 'ubuntu-22.04'
            args: ''
          - platform: 'windows-latest'
            args: ''
    
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Rust installieren
        uses: dtolnay/rust-action@stable
      
      - name: Node.js installieren
        uses: actions/setup-node@v4
        with:
          node-version: 20
      
      - name: Abhaengigkeiten installieren
        run: npm ci
      
      - name: Tauri build
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: v__VERSION__
          releaseName: 'ClawViewer v__VERSION__'
          releaseBody: 'Siehe CHANGELOG.md'
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}
```

### 7.5 Plattformspezifische Konfiguration

Tauri unterstuetzt plattformspezifische Config-Dateien [^286^]:

```
src-tauri/
├── tauri.conf.json           # Basis-Konfiguration
├── tauri.windows.conf.json    # Windows-spezifisch
├── tauri.linux.conf.json      # Linux-spezifisch
├── tauri.macos.conf.json      # macOS-spezifisch
```

```json
// tauri.windows.conf.json
{
  "bundle": {
    "windows": {
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      }
    }
  }
}
```

---

## 8. Performance: Bundle-Groesse, Memory-Footprint, Startup-Zeit

### 8.1 Benchmarks: Tauri v2 vs Electron

Aktuelle Benchmarks (2025-2026) zeigen massive Performance-Vorteile fuer Tauri [^259^] [^260^] [^248^] [^264^]:

| Metrik | Tauri 2.x | Electron 34.x | Differenz |
|--------|-----------|---------------|-----------|
| Hello World Bundle | 3.2 MB | 85 MB | **96% kleiner** |
| Complex App Bundle (6 Fenster) | 8.6 MB | 244 MB | **96% kleiner** |
| Cold Startup | 380 ms | 1,420 ms | **3.7x schneller** |
| Idle Memory (1 Fenster) | 42 MB | 168 MB | **75% weniger** |
| Active Memory (6 Fenster) | 172 MB | 409 MB | **58% weniger** |
| CPU Idle Usage | <0.5% | 1-5% | **Deutlich niedriger** |
| IPC Latenz (Round-Trip) | 0.12 ms | 0.45 ms | **3.75x schneller** |
| Datei-Lesen (100 MB) | 85 ms | 142 ms | **40% schneller** |
| Initiale Build-Zeit | 48 s | 22 s | Electron 2.2x schneller |
| Batterieverbrauch/Std | 0.4% | 2.1% | **5x effizienter** |

Quellen: [^259^] [^260^] [^248^] [^262^] [^264^]

### 8.2 Real-World Tauri-Bundle-Groessen

| App-Typ | Bundle-Groesse | Quelle |
|---------|---------------|--------|
| Einfache Utility | 3-5 MB | [^263^] |
| Notizen-App (Lokus) | ~10 MB | [^206^] |
| AI Desktop App | 8-15 MB | [^240^] |
| Komplexe App (6 Fenster) | ~8.6 MB | [^248^] |
| Git-Client (GitButler) | ~15 MB | [^260^] |

### 8.3 Bundle-Groesse optimieren

```toml
# Cargo.toml - Maximale Size-Optimierung
[profile.release]
codegen-units = 1
lto = true
opt-level = "z"
panic = "abort"
strip = true

# Zusaetzlich: UPX-Kompression (optional)
# upx --best target/release/myapp
```

**Frontend-Optimierungen** [^241^]:
- Tree Shaking aktivieren (Vite/Webpack)
- Code Splitting mit dynamischen Imports
- Images als WebP/AVIF
- Source Maps in Produktion deaktivieren
- `removeUnusedCommands: true` in tauri.conf.json

### 8.4 Memory-Footprint optimieren

```rust
// Rust-Seite: Speicher-effiziente Datenstrukturen
use std::sync::Arc;

// Shared State mit Arc statt Clone
#[derive(Default)]
struct SharedState {
    config: Arc<AppConfig>,         // Shared, nicht geklont
    connections: Arc<Mutex<Vec<Connection>>>,
}

// Streaming statt Buffering fuer grosse Daten
#[tauri::command]
async fn stream_file(path: String) -> Result<tauri::ipc::Response, String> {
    let file = tokio::fs::File::open(&path).await
        .map_err(|e| e.to_string())?;
    let stream = tokio_util::io::ReaderStream::new(file);
    tauri::ipc::Response::from_stream(stream)
        .map_err(|e| e.to_string())
}
```

**Frontend-Optimierungen** [^241^]:
- `React.memo`, `useMemo`, `useCallback` fuer teure Komponenten
- Virtual Scrolling fuer grosse Listen (`react-window`)
- Event Listener und Timer in `useEffect` Cleanup entfernen
- Lazy Loading von Routen

### 8.5 Startup-Zeit optimieren

```rust
// Lazy Loading: Nicht-kritische Initialisierung verzoegern
.setup(|app| {
    // Kritisch: Sofort ausfuehren
    app.manage(CoreState::new()?);
    
    // Nicht-kritisch: Async nach Startup
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        // Netzwerk-Checks, Update-Pruefung etc.
        check_for_updates(&app_handle).await;
    });
    
    Ok(())
})
```

---

## 9. WebRTC im Tauri-Kontext

### 9.1 WebRTC-Unterstuetzung in Tauri v2

WebRTC wird in Tauri v2 ueber die jeweilige WebView-Engine unterstuetzt [^247^]:

| Plattform | WebView-Engine | WebRTC-Support |
|-----------|---------------|----------------|
| Windows | WebView2 (Edge) | ✅ Nativ unterstuetzt |
| macOS | WebKit | ✅ Nativ unterstuetzt |
| Linux | WebKitGTK | ⚠️ Erfordert Custom Build |

**Linux-Spezialfall**: WebKitGTK benoetigt einen speziellen Build mit aktiviertem WebRTC [^247^]:
```nix
# Nix flake fuer WebRTC-faehigen WebKitGTK
webkitgtk_4_1.overrideAttrs (final: prev: {
  cmakeFlags = prev.cmakeFlags ++ [
    "-DENABLE_MEDIA_STREAM=ON"
    "-DENABLE_WEB_RTC=ON"
  ];
})
# Zusaetzlich: gst-plugins-bad fuer webrtcbin/webrtcdsp
gst_all_1.gst-plugins-bad
```

### 9.2 WebRTC-Implementierung fuer Remote-Desktop

```typescript
// Frontend: WebRTC Peer Connection
import { invoke } from '@tauri-apps/api/core';

class RemoteDesktopPeer {
    private pc: RTCPeerConnection;
    private dataChannel: RTCDataChannel;
    
    constructor() {
        this.pc = new RTCPeerConnection({
            iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
        });
        
        // DataChannel fuer Input-Events (Maus, Tastatur)
        this.dataChannel = this.pc.createDataChannel('input', {
            ordered: true,
        });
        
        this.pc.ontrack = (event) => {
            // Video-Stream vom Remote-PC empfangen
            const videoElement = document.getElementById('remote-screen');
            videoElement.srcObject = event.streams[0];
        };
    }
    
    async connect(signalUrl: string) {
        // SDP Offer erstellen
        const offer = await this.pc.createOffer();
        await this.pc.setLocalDescription(offer);
        
        // Via Tauri-Backend an Signaling-Server senden
        const answer = await invoke('webrtc_signal', {
            signalUrl,
            offer: JSON.stringify(offer),
        });
        
        await this.pc.setRemoteDescription(JSON.parse(answer as string));
    }
    
    sendMouseMove(x: number, y: number) {
        this.dataChannel.send(JSON.stringify({
            type: 'mouse_move',
            x, y,
        }));
    }
    
    sendKey(key: string, down: boolean) {
        this.dataChannel.send(JSON.stringify({
            type: 'key',
            key, down,
        }));
    }
}
```

```rust
// Backend: Signaling-Server-Proxy
#[tauri::command]
async fn webrtc_signal(
    signal_url: String,
    offer: String,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(&signal_url)
        .header("Content-Type", "application/json")
        .body(offer)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    response.text().await.map_err(|e| e.to_string())
}
```

### 9.3 Lokaler WebRTC-Server in Tauri

Fuer LAN-Verbindungen ohne externen Signaling-Server kann ein lokaler Server in Tauri laufen [^244^]:

```rust
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

#[tauri::command]
async fn start_local_signaling(
    app_handle: AppHandle,
) -> Result<u16, String> {
    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| e.to_string())?;
    
    let port = listener.local_addr()
        .map_err(|e| e.to_string())?.port();
    
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            // WebSocket-Handling fuer Signaling
            handle_signaling_connection(ws_stream, &app_handle).await;
        }
    });
    
    Ok(port)
}
```

---

## 10. File-System-Zugriff

### 10.1 FS-Plugin API

Tauri v2 bietet ein sicheres File-System-Plugin mit Scoping [^277^] [^278^]:

```typescript
import {
    readFile, readTextFile, writeFile, writeTextFile,
    exists, mkdir, remove, stat,
    BaseDirectory,
} from '@tauri-apps/plugin-fs';

// Datei lesen (text)
const config = JSON.parse(
    await readTextFile('config.json', { baseDir: BaseDirectory.AppConfig })
);

// Datei schreiben (binary)
await writeFile('screenshot.png', imageBytes, {
    baseDir: BaseDirectory.AppData,
});

// Datei streamen (grosse Dateien)
const file = await open('large-file.bin', {
    read: true,
    baseDir: BaseDirectory.AppData,
});
const stat = await file.stat();
const buffer = new Uint8Array(stat.size);
await file.read(buffer);
await file.close();

// Verzeichnis erstellen
await mkdir('downloads/session-123', {
    baseDir: BaseDirectory.AppData,
    recursive: true,
});

// Datei existiert?
const fileExists = await exists('config.json', {
    baseDir: BaseDirectory.AppConfig,
});
```

### 10.2 Capabilities fuer File-System

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main-capability",
  "windows": ["main"],
  "permissions": [
    "fs:default",
    {
      "identifier": "fs:allow-read-file",
      "allow": [{ "path": "$APPDATA/**" }]
    },
    {
      "identifier": "fs:allow-write-file",
      "allow": [{ "path": "$APPDATA/**" }]
    },
    {
      "identifier": "fs:allow-read-file",
      "allow": [{ "path": "$DOWNLOAD/**" }]
    },
    {
      "identifier": "fs:scope",
      "allow": ["$APPDATA/screenshots/**"],
      "deny": ["$APPDATA/screenshots/private/**"]
    }
  ]
}
```

---

## 11. Sicherheit

### 11.1 Capabilities-basiertes Sicherheitsmodell

Tauri v2 verwendet ein "Deny by Default"-Sicherheitsmodell [^261^] [^265^]:

- **Kein direkter Node.js-Zugriff** wie bei Electron
- Alle API-Calls muessen explizit erlaubt werden
- Scoping erlaubt feingranularen Zugriff auf Ressourcen
- Runtime-Validierung aller IPC-Commands

### 11.2 Content Security Policy (CSP)

```json
{
  "app": {
    "security": {
      "csp": {
        "default-src": "'self'",
        "connect-src": "'self' ws://localhost:*",
        "img-src": "'self' blob: data:",
        "script-src": "'self'",
        "style-src": "'self' 'unsafe-inline'"
      }
    }
  }
}
```

### 11.3 Prozess-Sandboxing

- **Windows**: AppContainer-Isolation
- **macOS**: App Sandbox
- **Linux**: Seccomp-BPF

### 11.4 Supply Chain Security

```bash
# Cargo-Audit fuer Dependency-Pruefung
cargo audit

# Cargo-Deny fuer Lizenz- und Security-Policies
cargo deny check

# Dependencies vendoren fuer Reproducible Builds
cargo vendor
```

---

## 12. Best Practices & Empfehlungen

### 12.1 Projekt-Struktur fuer Remote-Desktop-App

```
clawviewer/
├── src/                          # Frontend (React/Vue)
│   ├── components/
│   │   ├── RemoteScreen/         # Video-Renderer
│   │   ├── ChatPanel/            # Chat-Overlay
│   │   ├── ConnectionBar/        # Verbindungsstatus
│   │   └── FileTransfer/         # Datei-Transfer-UI
│   ├── hooks/
│   │   ├── useWebrtc.ts          # WebRTC-Logik
│   │   useTauriCommands.ts       # Rust-Command-Wrapper
│   │   └── useGlobalShortcut.ts  # Shortcut-Handling
│   ├── services/
│   │   ├── signaling.ts          # Signaling-Server-Client
│   │   ├── inputCapture.ts       # Maus/Tastatur-Capture
│   │   └── fileTransfer.ts       # Datei-Transfer-Logik
│   ├── stores/
│   │   └── connectionStore.ts    # Zustand fuer Verbindungen
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               # App-Entry Point
│   │   ├── lib.rs                # Builder + Setup
│   │   ├── commands/
│   │   │   ├── mod.rs            # Command-Registrierung
│   │   │   ├── connection.rs     # Verbindungs-Commands
│   │   │   ├── capture.rs        # Screen-Capture-Commands
│   │   │   ├── input.rs          # Input-Injection-Commands
│   │   │   └── filetransfer.rs   # Datei-Transfer-Commands
│   │   ├── platform/
│   │   │   ├── mod.rs            # Platform-Traits
│   │   │   ├── windows/          # Windows-Implementierung
│   │   │   ├── linux/            # Linux-Implementierung
│   │   │   └── macos/            # macOS-Implementierung
│   │   ├── webrtc/
│   │   │   ├── mod.rs
│   │   │   └── signaling.rs      # Lokaler Signaling-Server
│   │   └── state.rs              # App-State-Definition
│   ├── capabilities/
│   │   ├── main.json             # Hauptfenster-Capabilities
│   │   ├── chat.json             # Chat-Overlay-Capabilities
│   │   └── settings.json         # Settings-Capabilities
│   ├── tauri.conf.json           # Haupt-Konfiguration
│   ├── tauri.windows.conf.json   # Windows-spezifisch
│   ├── tauri.linux.conf.json     # Linux-spezifisch
│   ├── tauri.macos.conf.json     # macOS-spezifisch
│   └── Cargo.toml
├── package.json
├── vite.config.ts
├── tsconfig.json
└── tailwind.config.js
```

### 12.2 Wichtige Architektur-Entscheidungen

| Entscheidung | Empfehlung | Begruendung |
|-------------|------------|-------------|
| Frontend-Framework | React 18+ mit TypeScript | Grosse Community, gute Tauri-Integration |
| State Management | Tauri Managed State (Rust) + Zustand (Frontend) | Beste Performance, Rust-seitige Wahrheit |
| Bundler | Vite | Offiziell empfohlen, schnell |
| Styling | Tailwind CSS | Utility-first, kleines Bundle |
| Icons | SVG (inline) | Keine externen Font-Dependencies |
| WebRTC Signaling | Tauri-eigener Tokio-Server + externer Fallback | LAN ohne Internet moeglich |
| Screen Capture | DXGI (Win) / PipeWire (Lin) / CGDisplay (Mac) | Native Performance, geringe Latenz |
| Input Injection | SendInput (Win) / XTest (Lin) / CGEvent (Mac) | OS-native, niedrigste Latenz |

### 12.3 Typische Fallstricke vermeiden

**1. Mutex across await points:**
```rust
// FALSCH: std::sync::Mutex ueber await
let guard = state.lock().unwrap();
some_async_fn().await; // Compiler-Fehler!

// RICHTIG: tokio::sync::Mutex verwenden
let guard = state.lock().await;
some_async_fn().await; // OK
```

**2. CORS in Production:**
```typescript
// FALSCH: fetch() direkt an localhost in Tauri-Production
const res = await fetch('http://localhost:11434/api'); // CORS-Fehler!

// RICHTIG: Via Tauri-Command proxy'en
const res = await invoke('proxy_request', { url: 'http://localhost:11434/api' });
```

**3. Blocking in async Commands:**
```rust
// FALSCH: Blockierende Operation im Tokio-Worker
#[tauri::command]
async fn slow_op() {
    std::thread::sleep(Duration::from_secs(10)); // Blockiert Worker!
}

// RICHTIG: spawn_blocking fuer CPU-intensive/blockierende Ops
#[tauri::command]
async fn slow_op() {
    tokio::task::spawn_blocking(|| {
        std::thread::sleep(Duration::from_secs(10));
    }).await.unwrap();
}
```

**4. Permissions nicht vergessen:**
```json
// Immer alle benoetigten Permissions explizit deklarieren!
{
  "permissions": [
    "fs:allow-read-file",
    "fs:allow-write-file",
    "dialog:allow-open",
    "notification:default",
    "global-shortcut:allow-register"
  ]
}
```

---

## 13. Zusammenfassung: Tauri v2 fuer Remote-Desktop-Apps

### Staerken
- **Extrem kleine Bundles** (3-10 MB vs 120-200 MB bei Electron)
- **Geringer Memory-Footprint** (40-80 MB idle vs 150-400 MB)
- **Schneller Startup** (< 400ms vs 2-5s)
- **Rust-Backend** fuer performance-kritische Operationen (Screen Capture, Input Injection)
- **Capability-basierte Sicherheit** - kein Node.js-Zugriff aus dem WebView
- **Multi-Window** mit fensterspezifischen Berechtigungen
- **Cross-Platform** - Windows, macOS, Linux (+ Mobile in v2)
- **Reifes Plugin-Oekosystem** fuer haefige Anforderungen

### Herausforderungen
- **Rust-Lernkurve** fuer Backend-Entwicklung
- **Kleineres Oekosystem** als Electron
- **WebView-Inkonsistenzen** zwischen Plattformen (besonders Linux WebRTC)
- **Laengere Build-Zeiten** durch Rust-Kompilierung
- **Debugging** der Rust-JS-Bruecke kann komplex sein
- **Plattformspezifischer Code** fuer Screen Capture notwendig

### Bewertung fuer ClawViewer

Tauri v2 ist eine **exzellente Wahl** fuer eine Remote-Desktop-App:

1. **Performance**: Die geringe Bundle-Groesse und der niedrige Memory-Footprint sind entscheidend fuer eine App, die moeglicherweise staendig im Hintergrund laeuft.

2. **Rust-Backend**: Ermoeglicht native Performance fuer Screen Capture (DXGI/PipeWire) und Input Injection (SendInput/XTest) direkt aus dem Backend.

3. **Multi-Window**: Perfekt fuer die Anforderung - Hauptfenster fuer Screen-View, separates Overlay fuer Chat, Einstellungen als Dialog.

4. **Sicherheit**: Das Capability-System passt gut zu einer App, die sensible Operationen (Fernsteuerung, Datei-Zugriff) durchfuehrt.

5. **WebRTC**: Unterstuetzt fuer Video/Audio-Streaming, DataChannels fuer Input-Events - ideal fuer die Remote-Desktop-Uebertragung.

---

## Quellenverzeichnis

| # | Quelle | URL | Inhalt |
|---|--------|-----|--------|
| [^206^] | Reddit r/rust | reddit.com/r/rust/comments/1nvvoee | Erfahrungsbericht Tauri 2.0 - Bundle 10MB vs 100MB+ |
| [^208^] | Tauri Docs | v2.tauri.app/develop/calling-rust | Offizielle Command-System-Dokumentation |
| [^209^] | Tauri Docs | v2.tauri.app/develop/plugins | Plugin-Development-Guide |
| [^210^] | Medium | medium.com/@sjobeiri | System-Tray Implementierung Tauri v2 |
| [^211^] | Tauri Docs | v2.tauri.app/learn/security/capabilities | Multi-Window Capabilities |
| [^212^] | Reddit r/rust | reddit.com/r/rust/comments/1afyy77 | Backend → Frontend Events ohne invoke |
| [^213^] | Tauri Docs | v2.tauri.app/plugin/updater | Auto-Updater Konfiguration |
| [^214^] | Tauri Tutorials | tauritutorials.com/blog/creating-windows | WebviewWindowBuilder API |
| [^216^] | Tauri Docs | v2.tauri.app/learn/system-tray | System Tray API |
| [^219^] | GitHub Discussions | github.com/orgs/tauri-apps/discussions/9487 | Multi-Window Permission Fix |
| [^240^] | Dev.to | dev.to/purpledoubled | Tauri v2 + React 19 AI App |
| [^241^] | Oflight Blog | oflight.co.jp | Performance & Bundle Size Optimization |
| [^244^] | YouTube Shorts | youtube.com/shorts/yrEvAHNTONg | Yjs WebRTC in Tauri |
| [^246^] | Medium | medium.com/andamp | Tauri v2: One Codebase 4 All |
| [^247^] | GitHub Discussions | github.com/orgs/tauri-apps/discussions/8426 | WebRTC in WebKitGTK |
| [^248^] | Hopp Blog | gethopp.app/blog/tauri-vs-electron | Benchmark: 8.6MB vs 244MB |
| [^249^] | Tauri Docs | v2.tauri.app/concept/size | Offizielle Size-Optimierung |
| [^256^] | crates.io | tauri-plugin-notifications | Notifications Plugin Crate |
| [^258^] | docs.rs | tauri::trait.Manager | Manager Trait fuer State |
| [^259^] | Tech Insider | tech-insider.org/tauri-vs-electron-2026 | Umfassender Vergleich 2026 |
| [^260^] | Rustify | rustify.rs | Tauri vs Electron Benchmarks |
| [^261^] | Oflight Blog | oflight.co.jp | Tauri v2 Security Model |
| [^263^] | Dev.to | dev.to/ottoaria | Tauri in 2026 Cross-Platform |
| [^264^] | OpenReplay | openreplay.com | Electron vs Tauri Vergleich |
| [^265^] | Huakun Tech | huakun.tech | Tauri V2 Overview Security |
| [^277^] | Tauri Docs | v2.tauri.app/plugin/file-system | FS Plugin Dokumentation |
| [^278^] | Tauri Docs | v2.tauri.app/reference/javascript/fs | FS Plugin JS API |
| [^281^] | Tauri Docs | v2.tauri.app/security/permissions | Permission-System |
| [^282^] | GitHub | tauri-apps/plugins-workspace | Offizielle Plugin-Liste |
| [^283^] | docs.rs | tauri::async_runtime | Tokio Async Runtime |
| [^284^] | Dev.to | hiyoyok | Rust Async in Tauri v2 |
| [^285^] | Dev.to | hiyoyok | Global Shortcuts Tauri v2 |
| [^286^] | Tauri Docs | v2.tauri.app/develop/configuration-files | Konfigurationsdateien |
| [^287^] | CrabNebula | docs.crabnebula.dev | Auto-Updater Guide |
| [^289^] | Dev.to | rain9 | Global Shortcut Implementation |
| [^291^] | Ratul's Blog | ratulmaharaj.com | Tauri v2 Updater Guide |
| [^292^] | Stack Overflow | stackoverflow.com/questions/78056795 | Global Shortcuts in v2 |
| [^293^] | Stack Overflow | stackoverflow.com/questions/77670990 | Async invoke UI-Freezing |
| [^295^] | RFDonnelly Blog | rfdonnelly.github.io | Tauri + Async Rust Process |

---

*Dokument erstellt: Juni 2026*
*Recherchebasis: 23+ Web-Searches, offizielle Dokumentation, Community-Quellen*
*Gueltigkeit: Tauri v2.x (Stand: Juni 2026)*
