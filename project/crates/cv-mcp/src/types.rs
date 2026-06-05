//! MCP (Model Context Protocol) types for JSON-RPC 2.0 message exchange.
//!
//! This module defines the request/response types used by the MCP server
//! to communicate with AI agents. The protocol is a simplified JSON-RPC 2.0
//! over stdio (or other transports).
//!
//! # Message Flow
//!
//! 1. **Initialize** — Client sends `initialize`, server responds with capabilities.
//! 2. **Tools/List** — Client queries available tools.
//! 3. **Tools/Call** — Client invokes a tool by name with JSON arguments.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Requests (client -> server)
// ---------------------------------------------------------------------------

/// An incoming MCP request message.
///
/// Uses internally-tagged deserialization on the `method` field.
/// Methods are serialized in camelCase with slash separators for namespaced
/// methods (`tools/list`, `tools/call`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum McpRequest {
    /// Initialize the MCP session.
    #[serde(rename = "initialize")]
    Initialize {
        /// Protocol version requested by the client.
        params: InitializeParams,
    },
    /// List all available tools.
    #[serde(rename = "tools/list")]
    ToolsList,
    /// Call a specific tool by name with arguments.
    #[serde(rename = "tools/call")]
    ToolsCall {
        /// Tool name and arguments.
        params: ToolsCallParams,
    },
}

/// Parameters for the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    /// The MCP protocol version requested by the client.
    pub protocol_version: String,
}

/// Parameters for the `tools/call` request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolsCallParams {
    /// Name of the tool to invoke.
    pub name: String,
    /// JSON arguments passed to the tool.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Responses (server -> client)
// ---------------------------------------------------------------------------

/// An outgoing MCP response message.
///
/// Responses contain a list of content items (text or images).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpResponse {
    /// Content items returned by the tool or server.
    pub content: Vec<ContentItem>,
}

impl McpResponse {
    /// Create a new response with a single text content item.
    pub fn text<T: Into<String>>(text: T) -> Self {
        McpResponse {
            content: vec![ContentItem::Text { text: text.into() }],
        }
    }

    /// Create a new response with a single image content item.
    pub fn image(data: String, mime_type: String) -> Self {
        McpResponse {
            content: vec![ContentItem::Image { data, mime_type }],
        }
    }

    /// Create an empty response.
    pub fn empty() -> Self {
        McpResponse { content: vec![] }
    }

    /// Create an error response with a message.
    pub fn error<T: Into<String>>(message: T) -> Self {
        McpResponse {
            content: vec![ContentItem::Text {
                text: format!("Error: {}", message.into()),
            }],
        }
    }
}

/// A single content item within an [`McpResponse`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentItem {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// The text payload.
        text: String,
    },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image (e.g. `image/png`).
        mime_type: String,
    },
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

/// Metadata describing an available MCP tool.
///
/// Each tool exposes a JSON Schema describing its expected input shape,
/// which allows AI agents to discover and correctly invoke tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    /// Unique tool name (used as the `name` field in `tools/call`).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's expected input.
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new<T: Into<String>>(
        name: T,
        description: T,
        input_schema: serde_json::Value,
    ) -> Self {
        ToolDefinition {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC envelope
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request envelope.
///
/// This wraps the MCP method-specific payload in a standard JSON-RPC envelope
/// with an `id` field for request/response correlation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,
    /// Request identifier for correlation.
    pub id: Option<serde_json::Value>,
    /// The MCP request payload.
    #[serde(flatten)]
    pub request: McpRequest,
}

/// A JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,
    /// Request identifier matching the request.
    pub id: Option<serde_json::Value>,
    /// The result payload.
    pub result: Option<McpResponse>,
    /// Error information, if the call failed.
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ===================================================================
    // McpRequest deserialization
    // ===================================================================

    #[test]
    fn deserialize_initialize_request() {
        let json = r#"{"method":"initialize","params":{"protocol_version":"2024-11-05"}}"#;
        let req: McpRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(
            req,
            McpRequest::Initialize {
                params: InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                },
            }
        );
    }

    #[test]
    fn deserialize_tools_list_request() {
        let json = r#"{"method":"tools/list"}"#;
        let req: McpRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req, McpRequest::ToolsList);
    }

    #[test]
    fn deserialize_tools_call_request() {
        let json = r#"{"method":"tools/call","params":{"name":"mouse_click","arguments":{"x":100,"y":200,"button":"left"}}}"#;
        let req: McpRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(
            req,
            McpRequest::ToolsCall {
                params: ToolsCallParams {
                    name: "mouse_click".to_string(),
                    arguments: serde_json::json!({"x": 100, "y": 200, "button": "left"}),
                },
            }
        );
    }

    // ===================================================================
    // McpResponse & ContentItem serialization
    // ===================================================================

    #[test]
    fn serialize_text_response() {
        let resp = McpResponse::text("Hello, world!");
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("Hello, world!"));
        assert!(json.contains("\"type\":\"text\""));
    }

    #[test]
    fn serialize_image_response() {
        let resp = McpResponse::image("aGVsbG8=".to_string(), "image/png".to_string());
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("aGVsbG8="));
        assert!(json.contains("image/png"));
        assert!(json.contains("\"type\":\"image\""));
    }

    #[test]
    fn content_item_text_roundtrip() {
        let item = ContentItem::Text {
            text: "test content".to_string(),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let deserialized: ContentItem = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(item, deserialized);
    }

    #[test]
    fn content_item_image_roundtrip() {
        let item = ContentItem::Image {
            data: "base64data".to_string(),
            mime_type: "image/jpeg".to_string(),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let deserialized: ContentItem = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(item, deserialized);
    }

    // ===================================================================
    // ToolDefinition
    // ===================================================================

    #[test]
    fn tool_definition_creation() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "x": { "type": "integer" },
                "y": { "type": "integer" }
            }
        });
        let tool = ToolDefinition::new("mouse_move", "Move the mouse cursor", schema.clone());
        assert_eq!(tool.name, "mouse_move");
        assert_eq!(tool.description, "Move the mouse cursor");
        assert_eq!(tool.input_schema, schema);
    }

    #[test]
    fn tool_definition_serde_roundtrip() {
        let tool = ToolDefinition::new(
            "screenshot",
            "Take a screenshot",
            serde_json::json!({"type": "object", "properties": {}}),
        );
        let json = serde_json::to_string(&tool).expect("serialize");
        let deserialized: ToolDefinition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tool, deserialized);
    }

    // ===================================================================
    // JSON-RPC envelope
    // ===================================================================

    #[test]
    fn json_rpc_request_envelope_roundtrip() {
        let envelope = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(42)),
            request: McpRequest::ToolsList,
        };
        let json = serde_json::to_string(&envelope).expect("serialize");
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":42"));

        let deserialized: JsonRpcRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope.jsonrpc, deserialized.jsonrpc);
        assert_eq!(envelope.id, deserialized.id);
    }

    #[test]
    fn json_rpc_response_with_error() {
        let envelope = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        };
        let json = serde_json::to_string(&envelope).expect("serialize");
        assert!(json.contains("-32601"));
        assert!(json.contains("Method not found"));
    }

    // ===================================================================
    // McpResponse helpers
    // ===================================================================

    #[test]
    fn mcp_response_text_helper() {
        let resp = McpResponse::text("success");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(
            resp.content[0],
            ContentItem::Text {
                text: "success".to_string()
            }
        );
    }

    #[test]
    fn mcp_response_error_helper() {
        let resp = McpResponse::error("something went wrong");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(
            resp.content[0],
            ContentItem::Text {
                text: "Error: something went wrong".to_string()
            }
        );
    }

    #[test]
    fn mcp_response_empty_helper() {
        let resp = McpResponse::empty();
        assert!(resp.content.is_empty());
    }

    // ===================================================================
    // Send + Sync assertions
    // ===================================================================

    #[allow(dead_code)]
    fn _assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<McpRequest>();
        assert_send_sync::<McpResponse>();
        assert_send_sync::<ContentItem>();
        assert_send_sync::<ToolDefinition>();
        assert_send_sync::<JsonRpcRequest>();
        assert_send_sync::<JsonRpcResponse>();
        assert_send_sync::<JsonRpcError>();
    }
}
