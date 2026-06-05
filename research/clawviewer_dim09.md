# Dim 09 - MCP-Server Protokoll & KI-Agent Integration

## Research-Dokument: Model Context Protocol (MCP) und KI-Agent-Integrationsmuster fuer Remote-Desktop

**Datum:** Juli 2025
**Recherche-Umfang:** 25+ Web-Searches mit Inline-Citations
**Zielgruppe:** Integration-Architekten fuer Tauri/Rust-Desktop-Apps mit MCP-Server

---

## 1. MCP-Server Spezifikation

### 1.1 Uebersicht

Das Model Context Protocol (MCP) ist ein offenes Protokoll, das die Integration zwischen LLM-Anwendungen und externen Datenquellen sowie Tools standardisiert. Es wurde im November 2024 von Anthropic als Open Source eingefuehrt und wird von der Linux Foundation betreut. [^232^] [^233^]

> "MCP provides a standardized way for applications to: Share contextual information with language models, Expose tools and capabilities to AI systems, Build composposable integrations and workflows" [^233^]

### 1.2 JSON-RPC 2.0 als Message-Format

MCP verwendet **JSON-RPC 2.0** fuer alle Nachrichten zwischen Client und Server. [^256^] [^233^]

#### Drei Message-Typen:

**1. Requests** - Initiieren Operationen und erwarten Antwort:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "database_search",
    "arguments": {
      "table": "products",
      "query": "laptop",
      "limit": 10
    }
  }
}
```

- MUESSEN einen String oder Integer ID enthalten
- ID darf NICHT null sein
- ID darf nicht in derselben Session wiederverwendet werden [^256^]

**2. Responses** - Antworten auf Requests:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "matches": [
      {"id": 1, "name": "MacBook Pro", "price": 1299}
    ],
    "total": 1
  }
}
```

Fehler-Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params"
  }
}
```

**3. Notifications** - Einweg-Nachrichten ohne Antwort:
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/tools/list_changed",
  "params": {}
}
```

- Notifications duerfen KEINE ID enthalten
- Der Empfaenger darf NICHT antworten [^256^] [^281^]

### 1.3 Standard MCP Error Codes

| Code | Bedeutung | Beschreibung |
|------|-----------|-------------|
| -32700 | Parse error | Ungueltiges JSON |
| -32600 | Invalid request | Keine gueltige JSON-RPC Anfrage |
| -32601 | Method not found | Unbekannte Methode |
| -32602 | Invalid params | Ungueltige Parameter |
| -32603 | Internal error | Interner Server-Fehler |
| -32000 bis -32099 | Server error | MCP-spezifische Fehler [^268^] |

### 1.4 MCP Message-Typen Referenz

#### Client -> Server Requests

| Methode | Schema-Typ | Zweck |
|---------|-----------|-------|
| `initialize` | InitializeRequestSchema | Verbindung etablieren, Capabilities verhandeln |
| `ping` | PingRequestSchema | Health-Check |
| `tools/list` | ListToolsRequestSchema | Verfuegbare Tools entdecken |
| `tools/call` | CallToolRequestSchema | Tool ausfuehren |
| `resources/list` | ListResourcesRequestSchema | Verfuegbare Ressourcen entdecken |
| `resources/read` | ReadResourceRequestSchema | Ressource lesen |
| `resources/subscribe` | SubscribeRequestSchema | Ressourcen-Updates abonnieren |
| `resources/unsubscribe` | UnsubscribeRequestSchema | Abo beenden |
| `resources/templates/list` | ListResourceTemplatesRequestSchema | Ressourcen-Templates auflisten |
| `prompts/list` | ListPromptsRequestSchema | Prompts entdecken |
| `prompts/get` | GetPromptRequestSchema | Prompt-Details abrufen |
| `logging/setLevel` | SetLevelRequestSchema | Logging-Level konfigurieren |
| `roots/list` | ListRootsRequestSchema | Filesystem-Roots auflisten [^281^] |

#### Server -> Client Requests

| Methode | Schema-Typ | Zweck |
|---------|-----------|-------|
| `ping` | PingRequestSchema | Server-initiierter Health-Check |
| `sampling/createMessage` | CreateMessageRequestSchema | LLM-Textgenerierung anfordern |
| `elicitation/create` | ElicitRequestSchema | Benutzereingabe anfordern |
| `completion/complete` | CompleteRequestSchema | Textvervollstaendigung anfordern [^281^] |

### 1.5 Protokoll-Versionen

| Version | Datum | Status | Highlights |
|---------|-------|--------|-----------|
| 2025-11-25 | Nov 2025 | Latest Stable | OpenID Connect Discovery, Icons Metadata, Incremental Scope Consent, URL Mode Elicitation, Sampling Tool Calling, Experimental Tasks |
| 2025-06-18 | Jun 2025 | Stable | Entfernt JSON-RPC Batching, Structured Tool Output, OAuth Resource Server, Resource Indicators (RFC 8707), Elicitation |
| 2024-11-05 | Nov 2024 | Legacy | Initiale stabile Version [^238^] |

### 1.6 Architecture-Komponenten

MCP definiert drei Hauptkomponenten: [^233^] [^221^]

```
+------------------+    MCP (JSON-RPC 2.0)    +------------------+
|     Host         | <----------------------> |    Client        |
| (LLM-Anwendung)  |                          | (Connector im    |
|                  |                          |  Host-App)       |
+------------------+                          +--------+---------+
                                                       |
                                              +--------+---------+
                                              |    Server        |
                                              | (Daten/Tools/    |
                                              |  Kontext)        |
                                              +------------------+
```

- **Hosts**: LLM-Anwendungen, die Verbindungen initiieren (z.B. Claude Desktop, Cursor)
- **Clients**: Connectors innerhalb der Host-Anwendung
- **Servers**: Services, die Kontext und Faehigkeiten bereitstellen

---

## 2. MCP Lifecycle

### 2.1 Drei-Phasen-Lifecycle

MCP definiert einen strengen dreiphasigen Lifecycle: [^231^] [^221^] [^224^]

```
Phase 1: INITIALISIERUNG          Phase 2: OPERATION              Phase 3: SHUTDOWN
+-------------------+             +-------------------+          +-------------------+
| Client sendet     |             | Tool-Aufrufe      |          | Verbindung        |
| initialize req    |------------>| Ressourcen-Zugriff|--------->| wird geschlossen  |
| Server antwortet  |             | Prompt-Abruf      |          | Keine spezifische |
| Client sendet     |             | Progress Updates  |          | Protokollmeldung  |
| initialized note  |             | Cancellation      |          | erforderlich      |
+-------------------+             +-------------------+          +-------------------+
```

### 2.2 Initialisierung - Detail

Die Initialisierung ist die ERSTE Pflicht-Interaktion zwischen Client und Server. [^231^]

**Schritt 1 - Client sendet Initialize Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-11-25",
    "capabilities": {
      "roots": { "listChanged": true },
      "sampling": {},
      "elicitation": { "form": {}, "url": {} }
    },
    "clientInfo": {
      "name": "ExampleClient",
      "version": "1.0.0"
    }
  }
}
```

**Schritt 2 - Server antwortet mit Capabilities:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-11-25",
    "capabilities": {
      "tools": { "listChanged": true },
      "resources": { "subscribe": true, "listChanged": true },
      "prompts": { "listChanged": true },
      "logging": {}
    },
    "serverInfo": {
      "name": "example-server",
      "version": "1.0.0"
    },
    "instructions": "Optionale Anweisungen fuer den Client"
  }
}
```

**Schritt 3 - Client sendet Initialized Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/initialized",
  "params": {}
}
```

### 2.3 Tool Discovery

Nach der Initialisierung entdeckt der Client die Server-Capabilities: [^221^]

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}
```

**Response mit Tool-Metadaten:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "screenshot",
        "title": "Screen Capture",
        "description": "Captures the current screen",
        "inputSchema": {
          "type": "object",
          "properties": {
            "scale": { "type": "number", "description": "Scale factor" }
          }
        }
      }
    ]
  }
}
```

### 2.4 Notifications waehrend des Betriebs

- **Progress Notifications**: Fortschritt bei langlaufenden Operationen
- **Cancellation**: Beide Seiten koennen laufende Requests abbrechen
- **List-Changed Notifications**: Server benachrichtigt ueber Aenderungen
- **Tool-List-Changed**: Server informiert ueber neue/entfernte Tools [^223^] [^268^]

---

## 3. Tool-Use Pattern

### 3.1 Tool-Definition

Jedes Tool wird mit einem strukturierten Schema definiert: [^259^] [^266^]

```json
{
  "name": "send_email",
  "title": "Email Sender",
  "description": "Sends an email to a specified address.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "recipient": { "type": "string" },
      "subject": { "type": "string" },
      "body": { "type": "string" }
    },
    "required": ["recipient", "subject", "body"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "status": { "type": "string" },
      "messageId": { "type": "string" }
    }
  },
  "annotations": {
    "readOnlyHint": false,
    "destructiveHint": false,
    "idempotentHint": false,
    "openWorldHint": false
  }
}
```

**Tool-Definition Felder:**

| Feld | Pflicht | Beschreibung |
|------|---------|-------------|
| `name` | Ja | Eindeutiger Identifier |
| `title` | Nein | Menschenlesbarer Anzeigename |
| `description` | Nein | Funktionsbeschreibung |
| `inputSchema` | Ja | JSON Schema fuer Parameter |
| `outputSchema` | Nein | JSON Schema fuer Rueckgabe |
| `annotations` | Nein | Verhaltens-Metadaten (Sicherheit) |

### 3.2 Tool-Call (Aufruf)

Der Client ruft ein Tool ueber `tools/call` auf: [^281^] [^259^]

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_weather",
    "arguments": {
      "location": "New York"
    }
  }
}
```

### 3.3 Tool-Result (Ergebnis)

**Text-Ergebnis:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Temperature: 22C, Partly cloudy"
      }
    ],
    "isError": false
  }
}
```

**Image-Ergebnis:**
```json
{
  "content": [
    {
      "type": "image",
      "data": "base64-encoded-data",
      "mimeType": "image/png",
      "annotations": {
        "audience": ["user"],
        "priority": 0.9
      }
    }
  ]
}
```

**Audio-Ergebnis:**
```json
{
  "type": "audio",
  "data": "base64-encoded-audio-data",
  "mimeType": "audio/wav"
}
```

**Resource-Link:**
```json
{
  "type": "resource_link",
  "uri": "file:///project/src/main.rs",
  "name": "main.rs",
  "mimeType": "text/x-rust"
}
```

**Strukturiertes Ergebnis:**
```json
{
  "content": [...],
  "structuredContent": {
    "temperature": 22.5,
    "conditions": "Partly cloudy",
    "humidity": 65
  }
}
```

### 3.4 MCP Tool Annotations (Sicherheit)

Tool Annotations beschreiben das Verhalten eines Tools fuer Trust & Safety: [^266^]

| Annotation | Beschreibung |
|-----------|-------------|
| `readOnlyHint` | Tool aendert keinen Zustand |
| `destructiveHint` | Tool kann Daten loeschen |
| `idempotentHint` | Mehrfache Aufrufe sind sicher |
| `openWorldHint` | Tool interagiert mit externen Systemen |

> "For trust & safety and security, clients MUST consider tool annotations to be untrusted unless they come from trusted servers." [^266^]

---

## 4. Transport-Methoden

### 4.1 Vergleich der Transport-Methoden

| Feature | stdio | SSE (HTTP) | WebSocket |
|---------|-------|-----------|-----------|
| Deployment | Lokaler Prozess | HTTP-Server | HTTP-Server |
| Richtung | Bidirektional | Server->Client | Bidirektional |
| Client->Server | stdin | HTTP POST | WS Messages |
| Gleichzeitige Clients | 1 | Viele | Viele |
| Firewall-freundlich | N/A (lokal) | Ja (HTTP) | Meistens |
| Browser-kompatibel | Nein | Ja | Ja |
| Verbindungs-Overhead | Prozess-Spawn | HTTP Request | WS Handshake |
| Beste fuer | CLI Tools | Web Clients | Real-time Apps [^239^] |

### 4.2 stdio Transport

Der einfachste Transport - Client startet Server als Subprozess: [^239^]

```
Client -> Server: stdin (JSON-RPC Messages + newline)
Server -> Client: stdout (JSON-RPC Responses)
Server Logs: stderr
```

**Best Practices:**
- NIE Nicht-JSON-RPC-Daten auf stdout schreiben
- Logging nur ueber stderr
- Prozess-Isolation bietet perfekte Trennung zwischen Clients
- Keine Netzwerk-Konfiguration noetig [^239^]

### 4.3 HTTP/SSE Transport

Server-Sent Events (SSE) streamen Daten ueber HTTP: [^239^]

```
Client -> Server: HTTP POST /messages
Server -> Client: SSE Stream (text/event-stream)
Health Check: GET /health
```

**Vorteile:**
- Arbeitet durch Firewalls und Load Balancer
- Gut fuer Web-Clients
- POST fuer Requests, SSE fuer Responses
- Skalierbar hinter Load Balancern [^239^]

### 4.4 WebSocket Transport

Bidirektionale Kommunikation fuer Echtzeit-Anwendungen: [^239^]

```
Client <-> Server: WS bidirektional
Ping/Pong: Alle 30 Sekunden (Keep-Alive)
```

**Vorteile:**
- Vollduplex-Kommunikation
- Geringe Latenz
- Native Browser-Unterstuetzung

**Nachteile:**
- Erhoehte Komplexitaet
- Schlechter hinter Load Balancern als SSE [^239^]

### 4.5 Transport-Auswahl Entscheidungsmatrix

```
Lokale CLI/Desktop-Integration -> stdio
Web-erreichbare MCP-Server -> SSE
Echtzeit-Bidirektional-Streaming -> WebSocket
Mehrere gleichzeitige Clients -> SSE oder WebSocket
Einfachheit bevorzugt -> stdio
```

### 4.6 QuickDesk Dual-Transport

QuickDesk unterstuetzt beide Transport-Modi: [^235^]

- **stdio Mode**: AI-Client startet den quickdesk-mcp Prozess (lokal)
- **HTTP/SSE Mode**: QuickDesk hostet den MCP-Server fuer Multi-Client-Zugriff
- **Persistent**: Der gewaehlte Modus wird ueber Restarts gespeichert

```
AI Agent (Claude/GPT/Cursor)
    |
    |-- stdio (JSON-RPC) --> quickdesk-mcp (Rust Bridge)
    |                          |
    |-- HTTP/SSE -->           |-- WebSocket --> QuickDesk GUI (Qt 6)
                               |                    |
                               |-- Native Messaging --> quickdesk-host
                                                         (Chromium Remoting)
```

---

## 5. QuickDesk MCP-Implementierung

### 5.1 QuickDesk - Der erste AI-Native Remote Desktop

QuickDesk ist die erste AI-native Remote Desktop Anwendung mit eingebautem MCP-Server. Sie ermoeglicht es jedem AI-Agenten, Remote-Computer zu sehen und zu steuern. [^235^] [^237^]

**Projekt-Struktur:**
```
QuickDesk/
|-- QuickDesk/                 # Qt GUI Client (C++)
|   |-- src/api/               # WebSocket API Server
|   |-- qml/                   # QML Views
|
|-- quickdesk-mcp/             # Rust MCP Bridge
|   |-- src/main.rs            # Entry point
|   |-- src/server.rs          # MCP tools, prompts, resources
|   |-- src/ws_client.rs       # WebSocket client fuer Qt API
|
|-- quickdesk-skill-host/      # Rust host-side skill host
|   |-- agent/                 # Skill host binary
|   |-- mcp-server-common/     # Shared MCP framework
|   |-- skills/
|       |-- sys-info/          # System info skill
|       |-- file-ops/          # File operations skill
|       |-- shell-runner/      # Shell execution skill
|
|-- SignalingServer/           # Go signaling server
```

### 5.2 MCP Tools in QuickDesk

QuickDesk bietet **40+ MCP Tools**: [^235^] [^270^]

**Input/Control Tools:**

| Tool | Beschreibung | Parameter |
|------|-------------|-----------|
| `screenshot` | Bildschirm aufnehmen | scale (optional) |
| `mouse_click` | Maus-Klick ausfuehren | x, y, button, double |
| `mouse_move` | Maus bewegen | x, y |
| `mouse_drag` | Maus ziehen | start_x, start_y, end_x, end_y |
| `mouse_scroll` | Scrollen | direction, amount |
| `keyboard_type` | Text tippen | text |
| `keyboard_hotkey` | Hotkey senden | keys[] |
| `clipboard_read` | Zwischenablage lesen | - |
| `clipboard_write` | Zwischenablage schreiben | text |

**UI-Analyse Tools (OCR-basiert):**

| Tool | Beschreibung |
|------|-------------|
| `get_ui_state` | Aktueller UI-Zustand |
| `find_element` | UI-Element finden |
| `screen_diff_summary` | Bildschirmdiff-Analyse |
| `screen_verify` | Bildschirm-Verifikation |

**Event-Driven Tools:**

| Tool | Beschreibung |
|------|-------------|
| `wait_for_event` | Auf Ereignis warten |
| `wait_for_connection_state` | Auf Verbindungsstatus warten |
| `wait_for_clipboard_change` | Auf Zwischenablage-Aenderung warten |
| `wait_for_screen_change` | Auf Bildschirmaenderung warten |

**Host-Side Skills:**

| Skill | Beschreibung |
|-------|-------------|
| `sys-info` | Systeminformationen |
| `file-ops` | Dateioperationen |
| `shell-runner` | Shell-Ausfuehrung |

### 5.3 MCP Resources in QuickDesk

QuickDesk stellt folgende Ressourcen bereit: [^235^]

- **Realtime device status**: Aktueller Geraete-Status
- **Connection info**: Verbindungsinformationen
- **Host details**: Host-Maschinen-Details

### 5.4 MCP Prompts in QuickDesk

9 eingebaute MCP-Prompt-Templates: [^235^]

1. **Remote Operation Guide**: Grundlegende Fernsteuerung
2. **Server Health Check**: Systemdiagnose
3. **Batch Automation**: Massenautomatisierung
4. **System Diagnosis**: Erweiterte Diagnose
5. **Screen Analysis**: Bildschirmanalyse
6. **Multi-Device Orchestration**: Multi-Geraete-Orchestrierung
7. **SOP Documentation**: Standard Operating Procedures

### 5.5 Real-Time Event Streaming

QuickDesk unterstuetzt Echtzeit-Event-Streaming: [^235^]

- Connection state changes
- Clipboard updates
- Screen changes
- Performance statistics

### 5.6 Architektur-Diagramm QuickDesk

```
+---------------------+         +---------------------+
|  AI Agent           |         |  AI Agent           |
|  (Claude/GPT/)      |         |  (Cursor/VS Code)   |
+----------+----------+         +----------+----------+
           | stdio (JSON-RPC)              | HTTP/SSE
           v                               v
+----------+----------+         +----------+----------+
|  quickdesk-mcp      |         |  quickdesk-mcp      |
|  (Rust Bridge)      |         |  (Rust Bridge)      |
|                     |         |  HTTP/SSE Endpoint  |
+----------+----------+         +----------+----------+
           |                               |
           +---------------+---------------+
                           | WebSocket
                           v
                  +--------+--------+
                  | QuickDesk GUI   |
                  | (Qt 6 + QML)    |
                  | WebSocket API   |
                  +--------+--------+
                           | Native Messaging
              +------------+------------+
              |                         |
              v                         v
    +---------+---------+     +---------+---------+
    | quickdesk-host    |     | quickdesk-client  |
    | (Chromium         |     | (Chromium         |
    |  Remoting)        |     |  Remoting)        |
    +---------+---------+     +---------+---------+
              |                         |
              +------------+------------+
                           | WebRTC P2P / TURN
                           v
                  +--------+--------+
                  | Signaling Server|
                  | (Go + Gin)      |
                  +-----------------+
```

---

## 6. Rust MCP-Server Implementierung

### 6.1 Offizielles Rust SDK

Das offizielle Rust SDK fuer MCP ist `rmcp`: [^223^]

**Repository:** `modelcontextprotocol/rust-sdk`

**Features:**
- Schema-Definitionen fuer MCP-Nachrichten
- Transport-Layer fuer Kommunikation
- High-Level Client und Server Implementierungen
- Notifications (Progress, Cancellation, Initialized, List-Changed)

### 6.2 Alternative: `mcpr` Crate

```rust
use mcpr::{
    server::{Server, ServerConfig},
    transport::stdio::StdioTransport,
    Tool,
};

let server_config = ServerConfig::new()
    .with_name("My MCP Server")
    .with_version("1.0.0")
    .with_tool(Tool {
        name: "my_tool".to_string(),
        description: Some("My awesome tool".to_string()),
        input_schema: mcpr::schema::common::ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some([...].into_iter().collect()),
            required: Some(vec!["param1".to_string()]),
        },
    });

let mut server: Server<StdioTransport> = Server::new(server_config);
server.register_tool_handler("my_tool", |params: Value| {
    // Tool-Logik hier
    Ok(serde_json::json!({"result": "success"}))
})?;
```

### 6.3 Rust MCP Server mit `rmcp` (vollstaendig)

**Cargo.toml:**
```toml
[dependencies]
rmcp = { version = "0.11.0", features = ["transport-io"] }
tokio = { version = "1.48.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
```

**main.rs:**
```rust
use rmcp::{
    handler::server::tool::ToolRouter,
    model::{CallToolResult, Content, ServerInfo, ProtocolVersion, 
             ServerCapabilities, Implementation},
    tool, tool_handler, tool_router, ServerHandler,
};
use rmcp::ErrorData as McpError;

#[derive(Clone)]
pub struct MyMcpServer {
    tool_router: ToolRouter<Self>
}

#[tool_router]
impl MyMcpServer {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(description = "Get all available items")]
    async fn get_all_items() -> Result<CallToolResult, McpError> {
        let items = fetch_items().await
            .map_err(|e| McpError::internal_error(
                format!("Error: {}", e), None))?;

        let content = Content::json(items)
            .map_err(|e| McpError::internal_error(
                format!("JSON Error: {}", e), None))?;

        Ok(CallToolResult::success(vec![content]))
    }
}

#[tool_handler]
impl ServerHandler for MyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Server instructions...".to_string()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = MyMcpServer::new()
        .serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

### 6.4 Notifications im Rust SDK

**Progress Notifications:**
```rust
context.peer.notify_progress(ProgressNotificationParam {
    progress_token: ProgressToken(NumberOrString::Number(i as i64)),
    progress: i as f64,
    total: Some(total_items as f64),
    message: Some(format!("Processing item {}/{}", i + 1, total_items)),
}).await?;
```

**Cancellation:**
```rust
context.peer.notify_cancelled(CancelledNotificationParam {
    request_id: the_request_id,
    reason: Some("User requested cancellation".into()),
}).await?;
```

**List-Changed Notifications:**
```rust
context.peer.notify_tool_list_changed().await?;
context.peer.notify_prompt_list_changed().await?;
context.peer.notify_resource_list_changed().await?;
```

---

## 7. KI-Agent-Architektur fuer Remote-Desktop

### 7.1 Control-Level-Modi

Fuer KI-Agenten in Remote-Desktop-Umgebungen existieren verschiedene Kontroll-Level: [^230^] [^235^]

| Modus | Beschreibung | QuickDesk |
|-------|-------------|-----------|
| **Observer-Modus** | KI beobachtet, gibt Empfehlungen, KEINE direkte Kontrolle | Screenshot-Analyse |
| **Shared-Control** | KI fuehrt Aktionen aus, Benutzer sieht alles und kann eingreifen | QuickDesk Real-Time Visibility |
| **Full-Control** | KI hat vollstaendige Kontrolle, Benutzer kann Not-Stop ausloesen | QuickDesk mit Emergency Stop |

### 7.2 Observer-Modus

Im Observer-Modus: [^230^]
- KI analysiert den Bildschirm (Screenshots)
- Gibt Empfehlungen aus
- Fuehrt KEINE Aktionen aus
- Nutzt Tools wie `get_ui_state`, `screen_verify`
- Human-in-the-loop fuer alle Entscheidungen

### 7.3 Shared-Control (QuickDesk Modell)

QuickDesk implementiert ein Shared-Control-Modell: [^235^]

- **Real-Time Visibility**: Jede Mausbewegung und jeder Tastenanschlag ist sichtbar
- **Intervention moeglich**: Benutzer kann jederzeit eingreifen
- **Background Mode**: `show_window=false` fuer headless Batch-Operationen
- **Screenshot Scaling**: Anpassbare Aufloesung fuer schnellere AI-Verarbeitung

### 7.4 Multi-Device AI Orchestration

KI kann mehrere Remote-Maschinen gleichzeitig steuern: [^235^]

- Batch-Automation
- Cross-Device Workflows
- Fleet Management

### 7.5 Prompt Injection als zentrale Gefahr

AI-Agenten sind anfaellig fuer: [^288^]

- **Prompt Injection**: Instruktionen in Dokumenten/Webseiten kapern Agent-Verhalten
- **Tool Output Poisoning**: Tool-Antworten manipulieren nachfolgende Entscheidungen
- **Delegation-Chains**: Sub-Agenten vervielfachen die Angriffsoberflaeche

---

## 8. Safety-Safeguards

### 8.1 Bestaetigungspflichtige Aktionen

**Ansatz: Risk-Assessment vor jeder Aktion**

QuickDesk Roadmap beinhaltet: [^235^]
- Trust Layer & Safety (Risk Assessment)
- Confirmation Dialogs fuer kritische Aktionen
- Emergency Stop
- Audit Log

**MCP Elicitation fuer Bestaetigungen:**

Elicitation erlaubt Servern, strukturierte Bestaetigungen vom Benutzer anzufordern: [^273^] [^276^]

```json
{
  "method": "elicitation/requestInput",
  "params": {
    "message": "Soll der folgende Befehl ausgefuehrt werden: rm -rf /?",
    "schema": {
      "type": "object",
      "properties": {
        "confirmation": {
          "type": "string",
          "enum": ["Ja", "Nein"]
        }
      },
      "required": ["confirmation"]
    }
  }
}
```

### 8.2 Sandbox-Modell

**Permission-Modell fuer AI-Agenten:** [^284^] [^285^] [^288^]

**1. Task-Centric Access Control:**
- Policy: task -> 2^P (minimale temporaere Berechtigungen)
- 4-Tuple: (Agent, Resource, Operation, Context) -> {Allow, Deny}
- Default-Deny fuer alles
- Berechtigungen haben TTL und werden nach Task-Ende aufgehoben [^284^]

**2. RBAC (Role-Based Access Control):**
- Agenten haben Rollen (z.B. "support-agent", "data-reader")
- Vordefinierte Berechtigungen pro Rolle [^291^]

**3. ABAC (Attribute-Based Access Control):**
- Dynamische Entscheidungen basierend auf Attributen
- Agent-Typ, Datensensitivitaet, Umgebung, Risiko-Level [^285^]

**4. PBAC (Policy-Based Access Control):**
- Zentrale Policy-Enforcement
- Runtime-basierte Regelaktualisierung
- Trennung von Identity, Authorization und Execution [^285^]

### 8.3 Least Privilege fuer AI-Agenten

**Best Practices:** [^291^] [^292^]

- Nur minimale Zugriffsrechte gewaehren
- Keine unbeschraenkten Credentials
- Scoped Tokens statt Root-API-Keys
- Read-only Rollen wenn moeglich
- Short-lived Tokens mit Ablauf
- Audit Logging fuer jede Aktion
- Schnelle und praezise Revocation

### 8.4 Konkrete Sicherheitsmassnahmen fuer Remote-Desktop

**QuickDesk spezifisch:** [^235^]

1. **Privacy Screen**: Host-Display wird schwarz, lokale Eingabe blockiert
2. **Virtual Display**: IDD-Treiber fuer isolierte Sitzungen
3. **Access Code**: Temporaerer 9-stelliger Code mit Auto-Refresh
4. **Audit Trail**: Alle Operationen werden protokolliert
5. **Self-Hosted**: Eigene Infrastruktur, volle Datenkontrolle

**Allgemein fuer MCP-basierte Desktop-Automation:**

1. **Tool-Annotation Pruefung**: destructiveHint, readOnlyHint auswerten
2. **Human-in-the-loop**: Kritische Aktionen (Delete, Format, Shell) bestaetigen
3. **Scope-Begrenzung**: Nur bestimmte Verzeichnisse/Fenster zugaenglich
4. **Rate Limiting**: Maximale Anzahl Aktionen pro Minute
5. **Session-Timeout**: Automatische Beendigung nach Inaktivitaet

---

## 9. API-Key-Management

### 9.1 BYOK (Bring Your Own Key)

**Konzept:**

BYOK ermoeglicht es Benutzern, ihre eigenen API-Keys fuer AI-Provider mitzubringen: [^271^] [^274^]

- Kein Vendor Lock-in
- Volle Kontrolle ueber Kosten
- Erhoehte Privatsphaere und Sicherheit
- API-Keys werden lokal gespeichert und nie mit dem Anbieter geteilt [^274^]

**Beispiele aus der Praxis:**
- **JetBrains IDEs**: BYOK fuer Anthropic, OpenAI, und kompatible Provider [^274^]
- **Cloudflare AI Gateway**: Sichere Speicherung im Dashboard [^271^]

### 9.2 OS-Keyring fuer API-Key-Speicherung

**Rust `keyring` Crate:**

```rust
use keyring::Entry;

// API-Key speichern
pub fn save_api_key(provider: &str, key: &str) -> Result<(), String> {
    let entry = Entry::new("clawviewer", &format!("api_key_{}", provider))
        .map_err(|e| e.to_string())?;
    entry.set_password(key).map_err(|e| e.to_string())
}

// API-Key lesen
pub fn get_api_key(provider: &str) -> Result<String, String> {
    let entry = Entry::new("clawviewer", &format!("api_key_{}", provider))
        .map_err(|e| e.to_string())?;
    entry.get_password().map_err(|e| e.to_string())
}

// API-Key loeschen
pub fn delete_api_key(provider: &str) -> Result<(), String> {
    let entry = Entry::new("clawviewer", &format!("api_key_{}", provider))
        .map_err(|e| e.to_string())?;
    entry.delete_credential().map_err(|e| e.to_string())
}
```

**Cross-Platform Support:** [^280^]

| Plattform | Credential Store |
|-----------|-----------------|
| Windows | Windows Credential Store (DPAPI) |
| macOS | macOS Keychain Services |
| Linux | D-Bus Secret Service (GNOME Keyring, KWallet) |
| iOS | Protected Data Store |
| Android | Shared Preferences |

**Cargo.toml:**
```toml
[dependencies]
keyring = { version = "3", features = [
    "apple-native", 
    "windows-native", 
    "sync-secret-service"
] }
```

### 9.3 Tauri-Spezifische Speicheroptionen

Fuer Tauri v2 Apps gibt es drei Optionen: [^258^] [^267^] [^300^]

**Option 1: OS Keyring (Empfohlen)**

```rust
use keyring::Entry;

#[tauri::command]
pub fn save_token(token: String) -> Result<(), String> {
    let entry = Entry::new("clawviewer", "token")
        .map_err(|e| e.to_string())?;
    entry.set_password(&token).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_token() -> Result<String, String> {
    let entry = Entry::new("clawviewer", "token")
        .map_err(|e| e.to_string())?;
    entry.get_password().map_err(|e| e.to_string())
}
```

**Option 2: Tauri Stronghold Plugin**

```rust
// Stronghold - Verschluesselter Vault
use tauri_plugin_stronghold::Builder;

// Initialisierung mit Argon2
app.handle().plugin(
    tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build()
)?;
```

**JavaScript/Frontend:**
```javascript
import { Stronghold, Client } from '@tauri-apps/plugin-stronghold';

const stronghold = await Stronghold.load(vaultPath, vaultPassword);
const client = await stronghold.loadClient(clientName);
const store = client.getStore();

// Speichern
await store.insert('api_key', Array.from(new TextEncoder().encode(key)));
await stronghold.save();

// Lesen
const data = await store.get('api_key');
const value = new TextDecoder().decode(new Uint8Array(data));
```

**Option 3: Tauri Keyring Plugin**

```rust
use tauri::Manager;
use tauri_plugin_keyring_store::KeyringExt;

fn example<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let svc = app.keyring().store.service();
    // API mit Sessions, Vault Locations, SLIP10/BIP39
}
```

### 9.4 MCP OAuth 2.1 fuer Autorisierung

MCP spezifiziert OAuth 2.1 fuer die Autorisierung: [^295^] [^296^] [^302^]

**Warum OAuth 2.1 statt API Keys:**

| API Keys | OAuth 2.1 |
|----------|-----------|
| Statisch, ewig gueltig | Kurzlebig, gescopte Tokens |
| Keine User-Unterscheidung | Multi-Client, Multi-User |
| Keine Nachvollziehbarkeit | Audit-Logs fuer jeden Token |
| Kein Scope | Granulare Berechtigungen |
| Manuelle Rotation | Automatische Refresh/Rotation [^296^] |

**OAuth 2.1 Flow fuer MCP:**

```
MCP Client                       MCP Server                Auth Server
     |                                 |                         |
     |-- 1. Discover Auth Server ----->|                         |
     |<-- 2. Protected Resource Meta --|                         |
     |                                 |                         |
     |-- 3. Register Client --------->|------------------------>|
     |<-- 4. client_id + client_secret-|                         |
     |                                 |                         |
     |-- 5. Auth Request (PKCE) -------------------------------->|
     |                                 |<-- 6. User Login ------|
     |                                 |     (User Consent)      |
     |<-- 7. Authorization Code ---------------------------------|
     |                                 |                         |
     |-- 8. Token Exchange (code) ------------------------------>|
     |<-- 9. Access Token + Refresh Token -----------------------|
     |                                 |                         |
     |-- 10. MCP Request (Bearer Token) ->|                     |
     |<-- 11. Response ------------------|                      |
```

**PKCE (Proof Key for Code Exchange):**
- MCP OBLIGATORISCH fuer alle Clients
- Schutz gegen Authorization Code Interception
- Code Challenge + Verifier [^301^]

### 9.5 Zusammenfassung API-Key-Management Best Practices

1. **BYOK-Modell**: Benutzer bringen eigene Keys mit, keine zentrale Speicherung
2. **OS Keyring**: Keys im Plattform-eigenen Credential Store speichern
3. **Keine Hardcoding**: NIE Keys im Source Code oder Config-Files
4. **Encryption at Rest**: Verschluesselte Speicherung (Stronghold als Alternative)
5. **Session-basiert**: Short-lived Sessions statt dauerhafter Keys
6. **OAuth 2.1**: Fuer MCP-Server-Autorisierung standardkonform implementieren
7. **Audit Logging**: Jeden Zugriff protokollieren
8. **Revocation**: Schnelle Deaktivierung bei Kompromittierung

---

## 10. MCP Resources und Templates

### 10.1 Resource-Definition

Ressourcen bieten read-only Zugriff auf Daten: [^272^] [^275^]

```json
{
  "uri": "file:///home/user/documents/report.pdf",
  "mimeType": "application/pdf",
  "name": "Report",
  "description": "Monthly report"
}
```

### 10.2 Resource-Templates

Templates ermoeglichen dynamische Ressourcen mit URI-Parametern: [^272^] [^275^]

```
file:///{path}        -> file:///etc/passwd
weather://{city}      -> weather://Berlin
repos://{owner}/{repo} -> repos://octocat/hello-world
```

**RFC 6570 URI Templates:**

```python
# Wildcard Parameter fuer Pfade
files://{filepath*}   # Matcht: files://docs/server/resources.mdx

# Query Parameter
data://{id}{?format}  # data://123?format=xml
```

---

## 11. MCP Sampling und Elicitation

### 11.1 Sampling

**Sampling** erlaubt Servern, LLM-generierten Text vom Client anzufordern: [^276^] [^273^]

```python
# Server-seitig
response = await ctx.sample(
    messages=analysis_prompt,
    system_prompt=("You are a data analyst..."),
    temperature=0.1,
    max_tokens=500,
)
```

**Anwendungsfall:** Server nutzt LLM fuer Zwischenanalysen ohne direkte API-Zugriffe.

### 11.2 Elicitation

**Elicitation** erlaubt Servern, strukturierte Benutzereingaben anzufordern: [^273^] [^276^]

```python
# Server-seitig
result = await ctx.elicit(
    message="Should I create the recommended index?",
    response_type=["Yes", "No"],
)
```

**Anwendungsfall:**
- Bestaetigungsdialog vor kritischen Aktionen
- Zusatzinformationen vom Benutzer anfordern
- Mehrfachauswahl bei mehreren Optionen

---

## 12. QuickDesk Integration Guide fuer MCP-Clients

### 12.1 Verbindung mit Claude Desktop

**Konfiguration `claude_desktop_config.json`:**
```json
{
  "mcpServers": {
    "quickdesk": {
      "command": "quickdesk-mcp",
      "args": ["--stdio"],
      "env": {
        "QUICKDESK_WS_URL": "ws://localhost:8080"
      }
    }
  }
}
```

### 12.2 Verbindung mit Cursor/VS Code

```json
{
  "mcpServers": {
    "quickdesk": {
      "url": "http://localhost:3000/sse"
    }
  }
}
```

### 12.3 Verfuegbare Clients

QuickDesk MCP-Server funktioniert mit: [^235^]
- Claude Desktop
- Cursor
- VS Code
- Jede MCP-kompatible Anwendung

---

## 13. Vergleich: MCP vs. Function Calling vs. Direct API

### 13.1 Entscheidungsmatrix

| Aspekt | Direct API | Function Calling | MCP |
|--------|-----------|-----------------|-----|
| Komplexitaet | Niedrig | Mittel | Hoch |
| Standardisierung | Keine | Fragmentiert (OpenAI, Anthropic...) | Einheitlich |
| Tool Discovery | Hardcoded | Hardcoded | Dynamisch zur Laufzeit |
| Multi-Client | Nein | Nein | Ja |
| Credential Isolation | Nein | Nein | Ja (Server-seitig) |
| Kontext-Overhead | Keiner | Tokens pro Tool | 40K-75K+ Tokens |
| Performance | Schnellste | Mittel | Protokoll-Overhead |
| Beste fuer | Deterministische Ops | Prototyping, 2-5 Tools | Shared integrations [^236^] |

### 13.2 MCP Kontext-Window-Tax

> "GitHub's MCP server alone can consume 40,000-55,000 tokens just for its tool definitions. A typical multi-server setup can eat 75,000+ tokens in overhead alone. On a 200K context window, that's over a third of your capacity gone before the agent does anything useful." [^236^]

**Best Practice:** Aggressive Kuratierung - Weniger Tools, bessere Beschreibungen, High-Level abstrahieren.

---

## 14. Zusammenfassung Architektur-Entscheidungen

### 14.1 Empfohlene Architektur fuer ClawViewer

Basierend auf der Recherche wird fuer eine Tauri/Rust-Desktop-App mit MCP-Server folgende Architektur empfohlen:

```
+----------------------------------------------------------+
|                    Tauri Frontend (Web)                   |
|  - React/Vue UI fuer MCP-Server-Konfiguration             |
|  - BYOK API-Key Eingabe                                   |
|  - Echtzeit-Anzeige von AI-Aktionen                       |
|  - Bestaetigungsdialoge fuer kritische Aktionen           |
+----------------------------------------------------------+
                            |
                    Tauri IPC Bridge
                            |
+----------------------------------------------------------+
|                    Tauri Rust Backend                     |
|                                                           |
|  +-----------------+  +-----------------------------+     |
|  | OS Keyring      |  | MCP Server (rmcp)           |     |
|  | (keyring crate) |  | - Tool Definitionen         |     |
|  | - API Keys      |  | - Tool Handler              |     |
|  | - Credentials   |  | - Resources                 |     |
|  +-----------------+  | - Prompts                   |     |
|                        | - Elicitation               |     |
|  +-----------------+  +-----------------------------+     |
|  | Stronghold      |              |                       |
|  | (Fallback fuer  |              | stdio / HTTP/SSE      |
|  |  OS ohne        |              |                       |
|  |  Keyring)       |              v                       |
|  +-----------------+  +-----------------------------+     |
|                        | Remote-Desktop Interface      |    |
|  +-----------------+  | - WebSocket zu RDP-Core     |     |
|  | Safety Layer    |  | - Input Injection           |     |
|  | - Risk Assessment|  | - Screenshot Capture        |     |
|  | - Confirmation  |  | - Event Streaming           |     |
|  | - Emergency Stop|  +-----------------------------+     |
|  +-----------------+                                     |
+----------------------------------------------------------+
```

### 14.2 Kritische Architektur-Entscheidungen

| Entscheidung | Empfehlung | Begruendung |
|-------------|-----------|-------------|
| Transport-Modus | stdio fuer lokal, HTTP/SSE fuer remote | QuickDesk-Pattern folgen |
| API-Key-Speicherung | OS Keyring (keyring crate) | Plattform-native Sicherheit |
| Sicherheitsmodell | Shared-Control mit Bestaetigung | QuickDesk-Modell |
| Tool-Design | Atomar, fokussiert | Best Practice MCP |
| Autorisierung | OAuth 2.1 (wenn Server exposed) | MCP-Spec-konform |
| Session-Management | Short-lived mit Timeout | Sicherheits-Best-Practice |
| Audit-Logging | Alle Aktionen protokollieren | Nachvollziehbarkeit |

---

## Referenzen

- [^221^] MCP Architecture: Components, Lifecycle & Client-Server Tutorial - Obot AI
- [^222^] mcpr - Rust Crate Documentation (docs.rs)
- [^223^] modelcontextprotocol/rust-sdk - Official Rust SDK for MCP
- [^224^] Model Context Protocol (MCP) explained - Codilime
- [^225^] Building MCP Servers in Rust with rmcp - Complete Guide
- [^226^] MCP Lifecycle Explained: Client-Server Workflow - Medium
- [^227^] MCP Server Lifecycle Overview - Emergent Mind
- [^228^] Build a Weather MCP Server with Rust - Paul's Blog
- [^229^] AI Computer Control - Bytebot AI (YouTube)
- [^230^] Remote Desktop for AI Agents - Astropad Blog
- [^231^] MCP Lifecycle Specification (2025-11-25)
- [^232^] Model Context Protocol GitHub Organization
- [^233^] MCP Specification (2025-06-18) - modelcontextprotocol.io
- [^234^] What is the Model Context Protocol (MCP)? - Introduction
- [^235^] QuickDesk GitHub Repository - barry-ran/QuickDesk
- [^236^] When to use MCP vs API vs Function/Tool call - JamWithAI
- [^237^] QuickDesk MCP Server - MCPgee Directory
- [^238^] Model Context Protocol Specification - Version History
- [^239^] MCP Transport Options: stdio vs SSE vs WebSocket
- [^256^] MCP Specification Overview - JSON-RPC 2.0 Messages
- [^257^] MCP Specification Draft - modelcontextprotocol.io
- [^258^] tauri-plugin-keyring-store - Rust Documentation
- [^259^] Defining and Implementing MCP Tools - Obot AI
- [^260^] Exploring MCP Primitives - CodeSignal Learn
- [^261^] Keyring - Rust Utility (lib.rs)
- [^262^] MCP tools with dependent types
- [^263^] MCP Specification - Stainless
- [^264^] Reading the MCP Specification - Medium
- [^265^] Why Model Context Protocol uses JSON-RPC - Medium
- [^266^] MCP Tools Specification (Draft)
- [^267^] Cross-Platform Admin Desktop App with Tauri - Medium
- [^268^] Complete MCP JSON-RPC Reference Guide - Portkey
- [^269^] MCP Tools Concepts - modelcontextprotocol.info
- [^270^] QuickDesk MCP Tools - GitHub
- [^271^] BYOK (Store Keys) - Cloudflare AI Gateway
- [^272^] What Are MCP Resources? - Zuplo
- [^273^] MCP Client Concepts: Elicitation, Sampling - Medium
- [^274^] BYOK in JetBrains IDEs - JetBrains Blog
- [^275^] Resources & Templates - FastMCP
- [^276^] Memgraph MCP: Elicitation and Sampling
- [^277^] What Is Bring Your Own Key (BYOK)? - IBM
- [^278^] InCountry BYOK Documentation
- [^279^] How is JSON-RPC used in MCP? - Milvus
- [^280^] keyring - Rust Crate Documentation
- [^281^] Complete MCP JSON-RPC Reference Guide - Portkey
- [^282^] The Communication Protocol - MCP Course (HuggingFace)
- [^283^] MCP Desktop Automation Server - MCPservers.org
- [^284^] Automated AI Agent Permission Management - Emergent Mind
- [^285^] AI Agent Access Control - Codebridge
- [^286^] Debug MCP servers at JSON-RPC level - MCPjam
- [^287^] Understanding RPC and MCP in Agentic AI
- [^288^] Why AI Agents Need Their Own Permission Model - Auth0
- [^289^] Unpacking the MCP Base Protocol - dev.to
- [^290^] Why MCP uses JSON-RPC - Medium
- [^291^] AI Agent Access Control Guide - witness.ai
- [^292^] Handling AI agent permissions - Stytch
- [^295^] MCP OAuth: How OAuth 2.1 Works - Prefect
- [^296^] Migrate from API keys to MCP OAuth 2.1 - Scalekit
- [^297^] MCP Desktop Automation Server - MCPservers.org
- [^298^] @tauri-apps/plugin-stronghold - Tauri Docs
- [^299^] Stronghold Plugin - Tauri Docs
- [^300^] Token Storage in Tauri - Medium
- [^301^] MCP, OAuth 2.1, PKCE, and AI Authorization - Aembit
- [^302^] MCP Authorization Specification
- [^303^] tauri-plugin-stronghold - crates.io
- [^304^] Introduction to MCP and Authorization - Auth0

---

*Dieses Dokument wurde im Rahmen der ClawViewer-Dimensions-Recherche (Dim 09) erstellt und dient als Grundlage fuer Architektur-Entscheidungen bei der Integration von MCP-Servern und KI-Agenten in die Tauri/Rust-Desktop-Applikation.*
