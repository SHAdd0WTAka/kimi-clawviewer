# Dim 11 – Bidirektionale Steuerung & Event-Priorisierung

## Research Report: Human + AI Shared Control Architecture

**Datum:** 2025-01-28
**Scope:** Architektur-Muster fur gleichzeitige bidirektionale Steuerung (Human + AI)
**Searches durchgefuhrt:** 24+

---

## 1. Event-Queue mit Prioritaten (P0-P3)

### 1.1 Grundlagen: Event-Queue-Implementierungen

Event-Queues sind fundamentale Datenstrukturen fur asynchrone, nicht-blockierende Event-Verarbeitung [^427^]. In Echtzeit-Systemen kommt typischerweise eine Priority-Queue zum Einsatz, da sie O(log n) fur Insertion und Deletion bietet [^432^] [^433^].

**Drei grundlegende Architektur-Ansatze fur Priorisierung:**

| Ansatz | Beschreibung | Latenz | Komplexitat |
|--------|-------------|--------|-------------|
| Source Separation | Separate Queues pro Prioritatsstufe | Niedrig | Hoch |
| In-Memory Priority Queue | Single Queue mit Comparator | Mittel | Mittel |
| Database-gestutzt | Persistent mit SQL-Priorisierung | Hoch | Niedrig |

**Source Separation** (dedizierte Queues pro Prioritat) ist die sauberste Losung fur Echtzeit-Systeme: separate Input-Streams fur P0 (Emergency), P1 (Human-Override), P2 (AI-Actions), P3 (Background) ermoglichen unabhangiges Scaling und garantierte Verarbeitungsreihenfolge [^432^].

### 1.2 Rust-spezifische Implementierungen

Fur Rust existieren mehrere Event-Queue-Crates [^466^] [^469^]:

- **`rc_event_queue`**: Lock-freie FIFO-Event-Queue mit Multi-Consumer-Support. Chunk-basierte Speicherverwaltung (ahnlich C++ std::deque). Read-Counter pro Chunk – wenn alle Reader den Chunk verlassen haben, wird er freigegeben [^469^].
- **`crossbeam-channel`**: MPMC-Channels mit Prioritats-Select
- **`tokio::sync::mpsc`**: Async-fahige Multi-Producer-Single-Consumer-Queues

```rust
// Beispiel: Priorisierte Event-Queue in Rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClawEvent {
    pub id: uuid::Uuid,
    pub source: EventSource,       // "human" | "ai"
    pub priority: PriorityLevel,   // P0-P3
    pub event_type: EventType,
    pub payload: EventPayload,
    pub timestamp: u64,            // Epoch millis
    pub sequence: u64,             // Monotonically increasing
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityLevel {
    P0 = 0,  // Emergency-Stop, Hard-Abort
    P1 = 1,  // Human-Override, Direct-Input
    P2 = 2,  // AI-Actions, Autonome Operationen
    P3 = 3,  // Background, Logging, Heartbeat
}

// Reverse-Ordering fur BinaryHeap (hochste Prioritat zuerst)
impl Ord for ClawEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| self.timestamp.cmp(&other.timestamp))
            .then_with(|| self.sequence.cmp(&other.sequence))
            .reverse()
    }
}
```

### 1.3 Game Programming Patterns: Event Queue

Das "Event Queue"-Pattern aus dem Game Programming Patterns-Buch beschreibt eine Queue, die Benachrichtigungen im FIFO-Prinzip speichert und Sender vom Empfanger entkoppelt [^434^]. Wichtige Erkenntnisse:

- **Decoupling in time**: Der Sender kann eine Request enqueuen und sofort returnieren; der Empfanger verarbeitet sie, wenn es ihm passt
- **Pull vs. Push**: Queues geben Kontrolle an den Puller (Empfanger), der Verarbeitung verzogern, aggregieren oder verwerfen kann
- **Keine Response**: Queues sind ungeeignet, wenn der Sender eine Antwort braucht

### 1.4 Tauri Event System

Tauri v2 bietet ein bidirektionales Event-System fur Rust-Frontend-Kommunikation [^514^] [^208^]:

**Charakteristiken:**
- JSON-Payloads (nicht fur grosse Datenmengen geeignet)
- Global (alle Listener) oder Webview-spezifisch
- Kein Strong Typing (im Gegensatz zu Commands)
- Nicht fur Low-Latency/High-Throughput gedacht

```rust
// Tauri Event Emission aus Rust
use tauri::{AppHandle, Emitter};

#[tauri::command]
fn emit_control_event(app: AppHandle, event: ClawEvent) {
    app.emit("claw-control-event", &event).unwrap();
}

// Channel-API fur hohere Durchsatzanforderungen
use tauri::{AppHandle, ipc::Channel};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
enum ControlEvent {
    HumanInput { x: f64, y: f64 },
    AiInput { x: f64, y: f64 },
    EmergencyStop,
    ModeChange { mode: ControlMode },
}
```

---

## 2. Ghost-Cursor Overlay

### 2.1 Stand der Technik: Multi-Cursor-Systeme

**MouseMux** (kommerzielle Windows-Software) ist das fortschrittlichste Beispiel fur Multi-Cursor-Systeme auf einem Desktop [^448^]:

- Jeder Benutzer erhalt einen eigenen Cursor auf dem gleichen Windows-Desktop
- Unabhangige Konfiguration pro Pointer-Device (Beschleunigung, Theme, Button-Verhalten)
- Kollaboration: gleichzeitiges Arbeiten, Annotieren, Fenster verschieben
- **Cursor Overlays**: Visuelle Unterscheidung verschiedener Cursor
- RustDesk-Integration fur Multi-User Remote Desktop
- SDK fur SCADA-Integration mit Supervisor-Override

**Pluralinput** (Legacy) bot ahnliche Funktionalitat: ein Cursor pro angeschlossener Maus mit individuellen Farben und Einstellungen [^496^].

### 2.2 Technische Implementierung eines Ghost-Cursors

**Rendering-Ansatze:**

| Methode | Technologie | Performance | Komplexitat |
|---------|------------|-------------|-------------|
| Direct2D Overlay | Windows Direct2D + UpdateLayeredWindow | Hoch | Mittel |
| GPU-Accelerated | OpenGL/Vulkan Overlay | Sehr hoch | Hoch |
| OS Cursor API | SetSystemCursor / LoadCursor | Mittel | Niedrig |
| Web-Overlay | HTML/CSS Cursor im Frontend | Niedrig | Sehr niedrig |

**Beispiel: Direct2D Ghost-Cursor Overlay (Windows)**

```rust
// Ghost Cursor Overlay mit Direct2D
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Gdi::UpdateLayeredWindow;

pub struct GhostCursor {
    id: String,              // z.B. "ai-cursor-1"
    x: f32,
    y: f32,
    color: (u8, u8, u8, u8), // RGBA – z.B. (255, 100, 0, 230) fur Orange AI
    label: String,           // z.B. "AI Agent"
    visible: bool,
    animation_state: CursorAnimation,
}

impl GhostCursor {
    pub fn render(&self, render_target: &ID2D1HwndRenderTarget) {
        // 1. Cursor-Bitmap zeichnen (gepunktet/transparent)
        let brush = create_solid_color_brush(
            render_target,
            self.color.0, self.color.1, self.color.2, self.color.3
        );
        
        // 2. Cursor-Pfeil oder Kreis zeichnen
        render_target.DrawEllipse(
            &D2D1_ELLIPSE {
                point: D2D_POINT_2F { x: self.x, y: self.y },
                radiusX: 12.0,
                radiusY: 12.0,
            },
            &brush,
            2.0,
            None,
        );
        
        // 3. Label-Text zeichnen ("AI")
        render_target.DrawText(
            &self.label,
            text_format,
            D2D_RECT_F {
                left: self.x + 16.0,
                top: self.y - 8.0,
                right: self.x + 80.0,
                bottom: self.y + 16.0,
            },
            &brush,
        );
    }
}
```

### 2.3 Visuelle Unterscheidung Human vs. AI

**Empfohlene Design-Prinzipien:**

| Aspekt | Human-Cursor | AI-Cursor (Ghost) |
|--------|-------------|-------------------|
| Farbe | Standard-WeiB/Schwarz | Orange (#FF6B35) oder Lila (#9333EA) |
| Form | Standard-Pfeil | Gepunkteter Umriss oder Diamant |
| Transparenz | 100% | 60-80% (deutlich als Overlay erkennbar) |
| Label | Keines | "AI" oder Agent-Name |
| Animation | Keine | Sanftes Pulsieren bei Aktivitat |
| Trail | Keiner | Kurzer Bewegungspfad (letzte 5 Positionen) |

**Virtual Overlay** (C++ Windows-Tool) demonstriert Direct2D-basierte Overlay-Rendering mit per-pixel Alpha-Transparenz und `UpdateLayeredWindow` [^467^]. Die Architektur umfasst:
- Message Loop mit Modifier-Key-Polling via `GetAsyncKeyState`
- Keine permanenten Low-Level-Hooks (nur wahrend Modifier gedruckt)
- Dodge Mode: Overlay bewegt sich weg, wenn der Cursor sich nahert

---

## 3. Input-Merging: Event-Koaleszenz & Deduplizierung

### 3.1 Event-Deduplizierungs-Strategien

Aus der Praxis von Event-Streaming-Systemen lassen sich mehrere Deduplizierungs-Muster ableiten [^493^] [^495^] [^497^]:

**1. Hash-basierte Deduplizierung (Fingerprint):**
```rust
// Event-Deduplizierung via MD5-Hash uber relevante Felder
pub fn compute_event_fingerprint(event: &ClawEvent) -> String {
    let mut hasher = Md5::new();
    hasher.update(event.source.as_bytes());
    hasher.update(event.event_type.as_bytes());
    hasher.update(&event.payload.canonical_bytes());
    format!("{:x}", hasher.finalize())
}
```

**2. Zeitfenster-basierte Deduplizierung:**
- Events mit identischem Fingerprint innerhalb von T ms werden als Duplikat verworfen
- Typisches Fenster: 50-100ms fur Input-Events

**3. Sequence-Number-basierte Deduplizierung:**
- Jede Quelle (Human/AI) erhalt eine monoton steigende Sequence-Nummer
- Out-of-order Events konnen erkannt und reordered werden

### 3.2 Event-Koaleszenz fur Mouse-Events

```rust
/// Koalesziert Mouse-Move-Events innerhalb eines Zeitfensters
pub fn coalesce_mouse_events(events: Vec<ClawEvent>, window_ms: u64) -> Vec<ClawEvent> {
    let mut coalesced: Vec<ClawEvent> = Vec::new();
    let mut pending_move: Option<ClawEvent> = None;
    
    for event in events {
        match event.event_type {
            EventType::MouseMove { .. } => {
                if let Some(ref mut pending) = pending_move {
                    // Ersetze durch neuestes Move-Event (nur letzte Position zahlt)
                    if event.timestamp - pending.timestamp <= window_ms {
                        *pending = event;
                    } else {
                        coalesced.push(pending.clone());
                        pending_move = Some(event);
                    }
                } else {
                    pending_move = Some(event);
                }
            }
            _ => {
                // Nicht-Move-Event: flush pending move zuerst
                if let Some(pending) = pending_move.take() {
                    coalesced.push(pending);
                }
                coalesced.push(event);
            }
        }
    }
    
    if let Some(pending) = pending_move {
        coalesced.push(pending);
    }
    
    coalesced
}
```

### 3.3 Event-Merging-Strategien: Ubersicht

| Strategie | Anwendungsfall | Implementierung |
|-----------|---------------|----------------|
| **Last-Wins** | Mouse-Move Events | Nur letztes Event im Fenster behalten |
| **Accumulate** | Scroll-Delta | Deltas addieren, ein kombiniertes Event |
| **Debounce** | Button-Clicks | Erstes Event behalten, Rest im Fenster verwerfen |
| **Throttle** | KI-High-Frequency-Updates | Max. N Events pro Sekunde durchlassen |
| **Merge-Metadata** | Status-Updates | Neuesten Status ubernehmen, History anhangen |

---

## 4. Human-Override: Emergency-Stop

### 4.1 Emergency-Stop-Systeme: Industriestandards

Emergency-Stop-Systeme in der Industrierobotik folgen strengen Standards [^446^] [^447^]:

**Stop-Kategorien (EN ISO 13850):**
- **Category 0**: Sofortiges Abschalten der Energie (unkontrollierter Stopp)
- **Category 1**: Kontrollierter Stopp mit Energie-Maintenance bis zum Halt
- **Category 2**: Kontrollierter Stopp ohne Energieabschaltung

**Kernanforderungen:**
- E-Stop hat immer Vorrang uber alle anderen Funktionen
- Muss jederzeit verfugbar sein, unabhangig vom Systemzustand
- Redundante Hardware und Fail-Safe-Mechanismen
- Klare visuelle und akustische Signalisierung

### 4.2 Software-Implementierung: Globaler Emergency-Stop

**Windows: Global Hotkey fur Emergency-Stop**

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use std::sync::atomic::{AtomicBool, Ordering};

static EMERGENCY_STOP_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Registriert einen globalen Emergency-Stop-Hotkey (z.B. Ctrl+Shift+F12)
pub fn register_emergency_stop_hotkey() -> Result<(), String> {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("EmergencyStopWindow"),
            WINDOW_STYLE(0),
            0, 0, 0, 0,
            None, None, None, None,
        ).map_err(|e| e.to_string())?;
        
        // Registriere Ctrl+Shift+F12 als globalen Hotkey
        RegisterHotKey(
            hwnd,
            1, // Hotkey-ID
            MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_NOREPEAT.0,
            VK_F12.0 as u32,
        ).map_err(|e| e.to_string())?;
        
        Ok(())
    }
}

/// Window Procedure fur WM_HOTKEY-Nachrichten
unsafe extern "system" fn emergency_stop_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            EMERGENCY_STOP_ACTIVE.store(true, Ordering::SeqCst);
            
            // Sofortige Aktionen:
            // 1. Alle Input-Queues leeren
            // 2. AI-Prozesse terminieren
            // 3. Ghost-Cursor ausblenden
            // 4. Visuelle Warnung anzeigen
            // 5. Control-Mode auf "Human-Only" setzen
            
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
```

### 4.3 Emergency-Stop-Architektur

```
┌─────────────────────────────────────────────────────┐
│                  EMERGENCY STOP LAYER                │
│  (Hochste Prioritat, umgeht alle anderen Schichten)  │
├─────────────────────────────────────────────────────┤
│  Input-Filter: Alle Events von AI-Source blockieren  │
│  Queue-Drainer: Alle pending AI-Events verwerfen     │
│  Process-Killer: Laufende AI-Automation abbrechen    │
│  Visual-Feedback: Roter Bildschirmrand + Alert       │
│  State-Reset: Control-Mode = HUMAN_EXCLUSIVE          │
└─────────────────────────────────────────────────────┘
```

**Rust-Implementierung des Emergency-Stop-Handlers:**

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlMode {
    HumanOnly,      // Nur Human-Input
    AiAssisted,     // AI vorschlage, Human bestatigt
    AiSupervised,   // AI fuhrt aus, Human kann intervenieren
    FullAi,         // Vollstandige AI-Autonomie
}

pub struct EmergencyStopSystem {
    active: AtomicBool,
    control_mode: Arc<RwLock<ControlMode>>,
    event_queue: Arc<RwLock<PriorityQueue<ClawEvent>>>,
    ai_process_handle: Option<AbortHandle>,
}

impl EmergencyStopSystem {
    pub async fn trigger_emergency_stop(&self) {
        // 1. Flag setzen (sofort, atomar)
        self.active.store(true, Ordering::SeqCst);
        
        // 2. Control-Mode auf HumanOnly setzen
        let mut mode = self.control_mode.write().await;
        *mode = ControlMode::HumanOnly;
        drop(mode);
        
        // 3. Event-Queue von AI-Events saubern
        let mut queue = self.event_queue.write().await;
        queue.retain(|event| event.source == EventSource::Human);
        drop(queue);
        
        // 4. Laufende AI-Operation abbrechen
        if let Some(handle) = &self.ai_process_handle {
            handle.abort();
        }
        
        // 5. Frontend-Event emitieren
        self.emit_stop_event().await;
    }
    
    /// Prüft ob ein Event blockiert werden soll
    pub fn should_block_event(&self, event: &ClawEvent) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            return false;
        }
        // Im E-Stop-Modus: nur Human-Events erlauben
        event.source != EventSource::Human
    }
}
```

---

## 5. Event-Schema: JSON-Format mit Source-Diskriminierung

### 5.1 Vollstandiges Event-Schema

```json
{
  "$schema": "https://clawviewer.io/schemas/control-event.json",
  "title": "ClawViewerControlEvent",
  "type": "object",
  "required": ["id", "source", "priority", "eventType", "timestamp", "sequence"],
  "properties": {
    "id": {
      "type": "string",
      "format": "uuid",
      "description": "Eindeutige Event-ID"
    },
    "source": {
      "type": "string",
      "enum": ["human", "ai", "system"],
      "description": "Ursprung des Events"
    },
    "priority": {
      "type": "string",
      "enum": ["P0", "P1", "P2", "P3"],
      "description": "P0=Emergency, P1=Human, P2=AI, P3=Background"
    },
    "eventType": {
      "type": "string",
      "enum": [
        "mouseMove", "mouseClick", "mouseScroll",
        "keyDown", "keyUp", "keyCombo",
        "modeChange", "emergencyStop", "heartbeat",
        "aiActionStart", "aiActionEnd", "aiSuggestion"
      ]
    },
    "payload": {
      "type": "object",
      "description": "Event-spezifische Daten"
    },
    "timestamp": {
      "type": "integer",
      "description": "Unix-Epoch in Millisekunden"
    },
    "sequence": {
      "type": "integer",
      "description": "Monoton steigende Sequenznummer pro Source"
    },
    "sessionId": {
      "type": "string",
      "description": "Session-ID zur Gruppierung"
    },
    "aiContext": {
      "type": "object",
      "description": "Zusatzlicher Kontext fur AI-Events",
      "properties": {
        "agentId": { "type": "string" },
        "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
        "intent": { "type": "string" },
        "reasoning": { "type": "string" }
      }
    }
  }
}
```

### 5.2 Rust-Implementierung mit Serde

```rust
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EventSource {
    Human,
    Ai,
    System,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    MouseMove { x: f64, y: f64, dx: f64, dy: f64 },
    MouseClick { button: MouseButton, x: f64, y: f64, click_count: u8 },
    MouseScroll { delta_x: f64, delta_y: f64 },
    KeyDown { key_code: u32, modifiers: Vec<ModifierKey> },
    KeyUp { key_code: u32, modifiers: Vec<ModifierKey> },
    ModeChange { new_mode: ControlMode, old_mode: ControlMode, reason: String },
    EmergencyStop { triggered_by: EventSource, reason: String },
    Heartbeat,
    AiActionStart { action_id: String, description: String },
    AiActionEnd { action_id: String, success: bool },
    AiSuggestion { suggestion_text: String, suggested_action: Box<EventType> },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClawEvent {
    pub id: Uuid,
    pub source: EventSource,
    pub priority: PriorityLevel,
    pub event_type: EventType,
    pub payload: serde_json::Value,
    pub timestamp: u64,
    pub sequence: u64,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_context: Option<AiContext>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AiContext {
    pub agent_id: String,
    pub confidence: f64,
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}
```

### 5.3 W3C UI Events als Referenz

Die W3C UI Events-Spezifikation definiert Standard-Event-Typen fur User-Agent-Interaktionen [^473^] [^520^]:

- **MouseEvent**: `screenX/Y`, `clientX/Y`, `button`, `relatedTarget`
- **KeyboardEvent**: `keydown` → `beforeinput` → `input` → `keyup` Pipeline
- **Event-Canceling**: `event.preventDefault()` stoppt Default-Action

Fur die ClawViewer-Integration wird das W3C-Schema erweitert um:
- `source`-Feld zur Ursprungs-Identifikation
- `priority`-Feld zur Echtzeit-Priorisierung
- `aiContext`-Objekt fur KI-spezifische Metadaten

---

## 6. Conflict-Resolution: Wer hat Vorrang?

### 6.1 Shared Autonomy: Forschungsstand

Die Forschung zu Shared Autonomy in der Robotik liefert fundierte Erkenntnisse fur Human-AI-Control-Sharing [^491^] [^492^] [^494^] [^499^] [^503^]:

**SARI (Shared Autonomy across Repeated Interaction)** [^494^]:
- End-to-End Imitation Learning zur Intent-Erkennung
- Discriminator misst Konfidenz der AI-Hilfe
- Robot gibt Kontrolle zuruck, wenn er sich unsicher ist
- Theoretisch beweisbare Stabilitat (uniformly ultimately bounded)

**Shared Autonomy Framework** [^491^]:
- Funktionale Trennung: Human = High-Level (Arm), AI = Low-Level (Hand/Grasp)
- Cognitive Load Reduction durch Delegation
- Residual Actions korrigieren Human-Input minimal-invasiv

**Level of Shared Autonomy** [^499^]:

| Level | Name | Success Rate | Avg. Time | Human Interaction |
|-------|------|-------------|-----------|-------------------|
| 1 | Assisted Teleoperation | 13.3% | 482.5s | 249.5s |
| 2 | Shared Autonomy | 80.0% | 424.8s | 142.9s |
| 3 | Full Automation | 66.7% | 151.1s | 17.1s |

**Kritische Erkenntnis**: Shared Autonomy (Level 2) erreicht die hochste Success Rate, wahrend Full Automation zwar schneller aber weniger robust ist.

### 6.2 Conflict-Resolution-Strategien

**Strategie 1: Priority-Based (P0-P3)**
```
P0 (Emergency): Sofortige Ausfuhrung, alles andere abbrechen
P1 (Human Override): Human-Input hat immer Vorrang vor AI
P2 (AI Action): AI darf nur ausfuhren wenn kein Human-Input aktiv
P3 (Background): Niedrigste Prioritat, kann verzogert werden
```

**Strategie 2: Temporal Interleaving**
```rust
/// Wechselt zwischen Human- und AI-Input basierend auf Zeitfenstern
pub fn temporal_interleave(
    human_events: Vec<ClawEvent>,
    ai_events: Vec<ClawEvent>,
    human_window_ms: u64,
    ai_window_ms: u64,
) -> Vec<ClawEvent> {
    let mut merged: Vec<ClawEvent> = Vec::new();
    let mut last_human_time = 0u64;
    let mut last_ai_time = 0u64;
    
    for event in human_events.into_iter().chain(ai_events) {
        match event.source {
            EventSource::Human => {
                last_human_time = event.timestamp;
                merged.push(event);
            }
            EventSource::Ai => {
                // AI-Event nur zulassen wenn Human nicht aktiv
                if event.timestamp - last_human_time > human_window_ms {
                    merged.push(event);
                    last_ai_time = event.timestamp;
                }
            }
            _ => merged.push(event),
        }
    }
    
    merged.sort_by_key(|e| e.timestamp);
    merged
}
```

**Strategie 3: Intent-Arbitration (SARI-basiert)**
- AI beobachtet Human-Verhalten und lernt typische Muster
- Bei bekannter Aufgabe: AI assistiert mit hohem Autonomiegrad
- Bei unbekannter Aufgabe: AI gibt Kontrolle an Human ab
- Konfidenz-Score (0-1) steuert den Ubergang

**Strategie 4: Blending (gewichtete Mischung)**
```rust
/// Mischt Human- und AI-Input mit Gewichtungsfaktor
pub fn blend_inputs(
    human: InputVector,
    ai: InputVector,
    ai_autonomy: f64, // 0.0 = Human only, 1.0 = AI only
) -> InputVector {
    InputVector {
        x: human.x * (1.0 - ai_autonomy) + ai.x * ai_autonomy,
        y: human.y * (1.0 - ai_autonomy) + ai.y * ai_autonomy,
        buttons: if ai_autonomy > 0.5 { ai.buttons } else { human.buttons },
    }
}
```

### 6.3 Handoff-Protokoll

```
┌─────────────┐      Request Handoff       ┌─────────────┐
│   HUMAN     │ ─────────────────────────> │     AI      │
│   CONTROL   │                            │   CONTROL   │
└─────────────┘                            └─────────────┘
       │                                          │
       │     <───────────────────────── Accept    │
       │     (mit confidence score)               │
       │                                          │
       │     ─────────────────────────> Grant     │
       │     (mit timeout und rollback)           │
       ▼                                          ▼
┌─────────────┐      Keep-Alive Heartbeat  ┌─────────────┐
│  OBSERVING  │ <────────────────────────> │  EXECUTING  │
│   (can      │      (alle 100ms)          │   (ai       │
│  interrupt) │                            │  performs)  │
└─────────────┘                            └─────────────┘
       │                                          │
       │     <──────────────────────── Complete   │
       │     oder ───────────────────> Revoke     │
       ▼                                          ▼
┌─────────────┐                            ┌─────────────┐
│  HUMAN      │     <────── Abort ─────    │  ROLLBACK   │
│  CONTROL    │                            │  & RESTORE  │
└─────────────┘                            └─────────────┘
```

---

## 7. UI-Indikatoren: KI-Aktivitatsanzeige

### 7.1 AI Progress Indicators (SAP Fiori Design)

SAP Fiori definiert spezifische Muster fur AI Progress Indicators [^462^]:

**Komponenten:**
- **Linear Progress Indicator**: Animierter Balken mit AI-Icon
- **Activity Indicator**: Rotierendes AI-Icon mit optionalem Label
- **Checkout Indicator**: Kompakte Variante fur begrenzten Raum
- **Button Loading State**: AI-Icon im Button bei asynchroner Aktion

**Verhaltensregeln:**
- AI-Indikator erst nach 1 Sekunde anzeigen (Vermeidung von Flickering)
- Mindestanzeigedauer: 1000ms
- Animation: Loop-Animation mit Pulsen zwischen den Loops
- Stopp-Option: Benutzer muss AI-Generation abbrechen konnen

### 7.2 Steuerungs-Modus-Display

**Empfohlene UI-Elemente:**

```
┌──────────────────────────────────────────────────────┐
│  ClawViewer                                    [_]   │
├──────────────────────────────────────────────────────┤
│                                                      │
│                    [Main Content]                    │
│                                                      │
│                                                      │
│                                                      │
├──────────────────────────────────────────────────────┤
│  [H] HUMAN ONLY  [●] AI Active  [🛑] ESC to Stop    │
│  Confidence: 85%  │  Task: "Navigating to element"   │
└──────────────────────────────────────────────────────┘
```

**Status-Bar-Komponenten:**

| Element | Zustande | Farbe |
|---------|----------|-------|
| Control Mode | Human Only / AI Assisted / AI Supervised / Full AI | Grun/Orange/Blau/Rot |
| AI Activity | Idle / Thinking / Acting / Error | Grau/Animiert/Grun/Rot |
| Emergency Stop | Ready / Active | Grun pulsiert / Rot blinkend |
| AI Confidence | 0-100% | Gradient von Rot zu Grun |

### 7.3 Fluent 2 Design System: Activity Indicators

Microsoft Fluent 2 gibt folgende Richtlinien [^465^]:
- Activity Indicators sind **indeterminate** (kein Fortschrittswert)
- Positionierung relativ zu wo neuer Content erscheint
- Nie uber UI-Elementen daruber legen (kein Blocking)
- Bei Prozessen > 3 Sekunden: HUD mit Label verwenden
- `animating`-Label fur VoiceOver-Zuganglichkeit

### 7.4 Tauri-Implementierung der Status-Anzeige

```rust
// Rust: Steuerungsmodus-Events an Frontend senden
#[tauri::command]
async fn set_control_mode(
    app: AppHandle,
    mode: ControlMode,
    ai_context: Option<AiContext>,
) -> Result<(), String> {
    let event = ControlModeEvent {
        mode,
        ai_context,
        timestamp: current_timestamp(),
    };
    
    app.emit("control-mode-changed", &event)
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

// Frontend (React/Vue): Status-Anzeige
function ControlStatusBar({ mode, aiActivity, confidence }) {
  const modeColors = {
    humanOnly: '#22c55e',      // green
    aiAssisted: '#f59e0b',     // amber
    aiSupervised: '#3b82f6',   // blue
    fullAi: '#ef4444',         // red
  };

  return (
    <div className="control-status-bar">
      <div className="mode-indicator" style={{ color: modeColors[mode] }}>
        {mode === 'humanOnly' && '🎮 Human Control'}
        {mode === 'aiAssisted' && '🤖 AI Assisted'}
        {mode === 'aiSupervised' && '👁️ AI Supervised'}
        {mode === 'fullAi' && '🤖 Full AI'}
      </div>
      
      {aiActivity && (
        <div className="ai-activity">
          <div className="activity-spinner" />
          <span>{aiActivity.description}</span>
          <span className="confidence">{confidence}%</span>
        </div>
      )}
      
      <button className="emergency-stop" onClick={triggerEmergencyStop}>
        🛑 STOP
      </button>
    </div>
  );
}
```

---

## 8. Remote Desktop Referenzarchitekturen

### 8.1 MouseMux: Multi-User Remote Desktop

MouseMux demonstriert eine vollstandige Multi-Cursor-Remote-Desktop-Losung [^448^]:

- **Input Capture Device**: Remote Desktop User (TeamViewer, RustDesk) agieren als unabhangige lokale User
- **Runtime Virtualization Layer**: Isolierte Sessions pro User
- **Cursor Overlays**: Visuelle Unterscheidung der verschiedenen Cursor
- **SDK**: SCADA-Integration mit Supervisor-Override-Funktionalitat

### 8.2 Remotly: Lokaler vs. Remote-Cursor

Remotly unterscheidet zwischen lokalem und remote-cursor Rendering [^472^]:
- **Lokaler Cursor**: Besser fur normale Desktop-Aktivitaten ("lag free" feeling)
- **Remote Cursor**: Nur im Game Mode sichtbar
- Hardware Mouse Overlay vs. Software Cursor Rendering

### 8.3 RustDesk Architektur

RustDesk verwendet folgende Architektur [^396^] [^470^]:

```
libs/
  hbb_common/     → Video codec, config, TCP/UDP, protobuf
  scrap/          → Screen capture
  enigo/          → Platform-spezifische Keyboard/Mouse Control
  clipboard/      → Cross-Platform Clipboard
src/
  server/         → Audio, clipboard, input/video services
  client.rs       → Peer connection initialization
  platform/       → OS-spezifischer Code
```

**Input-Handling**: `libs/enigo` implementiert plattformubergreifende Maus/Tastatur-Steuerung fur Windows, Linux und macOS.

---

## 9. Gesamtsystem-Architektur

### 9.1 Vorgeschlagene ClawViewer-Architektur

```
┌─────────────────────────────────────────────────────────────┐
│                        FRONTEND (Tauri WebView)              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Ghost      │  │  Status     │  │  Control Panel      │  │
│  │  Cursor     │  │  Bar        │  │  (Mode/E-Stop)      │  │
│  │  Overlay    │  │  (AI State) │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                    │              │
│  ┌──────┴────────────────┴────────────────────┴──────┐      │
│  │              Event Bus (Frontend)                  │      │
│  └──────────────────────┬─────────────────────────────┘      │
└─────────────────────────┼───────────────────────────────────┘
                          │ IPC (Tauri)
┌─────────────────────────┼───────────────────────────────────┐
│  RUST BACKEND           ▼                                   │
│  ┌─────────────────────────────────────────────────────┐    │
│  │           Event Router & Priority Queue              │    │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │    │
│  │  │ P0 E-   │ │ P1 Human│ │ P2 AI   │ │ P3 BG   │   │    │
│  │  │ Stop    │ │ Input   │ │ Actions │ │ Tasks   │   │    │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘   │    │
│  │       └───────────┴───────────┴───────────┘        │    │
│  │                     │                               │    │
│  │              Conflict Resolver                      │    │
│  │  (Priority > Timestamp > Source > Sequence)         │    │
│  └─────────────────────┬───────────────────────────────┘    │
│                        │                                     │
│  ┌─────────────────────┼─────────────────────┐               │
│  │           Input Merger & Coalescer         │               │
│  │  (Debounce > Coalesce > Throttle)          │               │
│  └─────────────────────┬─────────────────────┘               │
│                        │                                     │
│  ┌─────────────────────┼─────────────────────┐               │
│  │         OS Input Injection Layer           │               │
│  │  (enigo / SendInput / evdev)               │               │
│  └─────────────────────┬─────────────────────┘               │
│                        │                                     │
│  ┌─────────────────────┴─────────────────────┐               │
│  │        Emergency Stop System               │               │
│  │  (Global Hotkey + Queue Drain + Kill)      │               │
│  └────────────────────────────────────────────┘               │
└───────────────────────────────────────────────────────────────┘
```

### 9.2 Zusammenfassung der Architekturentscheidungen

| Entscheidung | Gewahlter Ansatz | Begrundung |
|-------------|------------------|------------|
| Event-Queue | Source Separation (4 Queues) | Unabhangige Verarbeitung, kein Head-of-Line Blocking |
| Priorisierung | P0-P3 mit strict ordering | E-Stop > Human > AI > Background |
| Ghost-Cursor | Direct2D Overlay (Windows) | Hardware-beschleunigt, per-pixel Alpha |
| Input-Merging | Last-Wins + Throttle | Einfach, performant, ausreichend fur UI-Automation |
| E-Stop | Global Hotkey + Atomic Flag | Sofortige Reaktion, kein Polling |
| Conflict-Resolution | Priority-First + Temporal Interleaving | Human hat immer Vorrang, AI wird pausiert |
| Handoff | Intent-basiert (SARI-Prinzip) | Graduelle Ubergabe statt harter Switches |
| UI-Indikatoren | SAP Fiori AI Pattern | Etabliertes Design, verstandlich fur Nutzer |

---

## 10. Quellen & Referenzen

### Event Queues & Priorisierung
- [^427^] redis.io – Event Queue Definition & Best Practices
- [^432^] dev.to – Priority Processing in Event Driven Architectures
- [^433^] UMassD – Priority Queues: Overview and Applications
- [^434^] GameProgrammingPatterns.com – Event Queue Pattern

### Multi-Cursor & Ghost Cursor
- [^448^] MouseMux.com – Multiple Mouse Cursors on Windows
- [^467^] Virtual Overlay – Direct2D Overlay Rendering
- [^496^] Pluralinput – Multiple Mice on one PC
- [^472^] Remotly Community – Mouse Cursor Rendering

### Emergency Stop & Safety
- [^445^] Reynolds-Moore – Speech Recognition Based Emergency Stop
- [^446^] eShield.pl – Emergency Stop Function: Machine Safety
- [^447^] free-barcode.com – Industrial Robot Emergency Stop Systems

### Remote Desktop & Collaboration
- [^450^] TeamViewer – Session Limit Documentation
- [^451^] SuperUser – TeamViewer Multiple Sessions
- [^396^] Medium – Deep Dive into RustDesk Forensics
- [^470^] EthicalHacksAcademy – RustDesk Architecture

### Shared Autonomy & Human-AI Control
- [^491^] arxiv.org – End-to-End Dexterous Arm-Hand VLA via Shared Autonomy
- [^492^] TTIC – Shared Autonomy in Unprepared Environments
- [^494^] ACM – SARI: Shared Autonomy across Repeated Interaction
- [^499^] PMC/NIH – Levels of Shared Autonomy in Brain-Robot Interfaces
- [^503^] RSS Proceedings – Shared Autonomy via Hindsight Optimization

### UI Events & Input Handling
- [^463^] Microsoft – UI Automation Events Overview
- [^473^] W3C – UI Events Specification
- [^474^] Microsoft – Keyboard Events Documentation

### Rust & Tauri
- [^466^] YouTube – Game Programming Patterns in Rust: Event Queue
- [^469^] docs.rs – rc_event_queue Crate
- [^514^] Tauri Docs – Calling the Frontend from Rust
- [^208^] Tauri Docs – Calling Rust from the Frontend

### Event Deduplication
- [^493^] Twilio SOCless – Event Deduplication Algorithm
- [^495^] Snowplow – Cross-batch Natural Deduplication
- [^497^] Elastic – Log Deduplication with Elasticsearch

### AI UI Patterns
- [^462^] SAP Fiori – AI Progress Indicators
- [^465^] Microsoft Fluent 2 – Activity Indicator
- [^468^] Medium – UX Design Patterns for Progress

### Global Hotkeys
- [^522^] StackOverflow – Global Hotkey to Stop Windows Script
- [^524^] LostInDetails.com – Global HotKeys for Windows Applications

---

*Dieses Dokument wurde als Teil der ClawViewer-Dimensions-Recherche (Dim 11) erstellt. Alle Architekturentscheidungen sind auf dem aktuellen Stand der Forschung und Industriepraxis basiert.*
