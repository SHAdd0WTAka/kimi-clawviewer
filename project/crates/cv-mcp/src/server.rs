//! MCP (Model Context Protocol) server implementation.
//!
//! The [`McpServer`] is the central component that:
//! 1. Maintains a registry of available [`McpTool`]s.
//! 2. Reads requests from an [`McpTransport`](crate::transport::McpTransport).
//! 3. Dispatches `tools/call` requests to the appropriate tool.
//! 4. Returns JSON-RPC responses.
//!
//! # Usage
//!
//! ```
//! use std::sync::Arc;
//! use cv_mcp::server::McpServer;
//! use cv_mcp::transport::StdioTransport;
//! use cv_mcp::tools;
//!
//! async fn example() {
//!     let transport = Arc::new(StdioTransport::new());
//!     let mut server = McpServer::new(transport);
//!
//!     // Register all built-in tools
//!     for tool in tools::all_tools() {
//!         server.register_tool(tool);
//!     }
//!
//!     // Run the server (reads requests until EOF)
//!     // server.run().await.unwrap();
//! }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::{
    ContentItem, InitializeParams, JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpRequest,
    McpResponse, ToolDefinition,
};
use cv_shared::CvResult;
use serde_json::json;

// ---------------------------------------------------------------------------
// McpTool trait
// ---------------------------------------------------------------------------

/// Trait for implementors of MCP tools.
///
/// Each tool has a name, description, JSON input schema, and an async
/// execute method that performs the actual work.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Return the unique tool name.
    ///
    /// This is used as the identifier in `tools/call` requests.
    fn name(&self) -> &str;

    /// Return a human-readable description of what the tool does.
    ///
    /// This is exposed to AI agents so they can decide which tool to use.
    fn description(&self) -> &str;

    /// Return a JSON Schema describing the tool's expected input.
    ///
    /// The schema should be a valid JSON Schema object that describes
    /// the shape of the `arguments` field in `tools/call` requests.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given JSON input.
    ///
    /// # Errors
    ///
    /// Returns [`cv_shared::CvError`] if the tool execution fails.
    /// Tool execution errors should generally be returned as error
    /// [`McpResponse`]s rather than `Err`, but fatal errors may
    /// propagate as `Err`.
    async fn execute(&self, input: serde_json::Value) -> CvResult<McpResponse>;
}

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// MCP server that manages a registry of tools and handles requests.
///
/// The server runs a request-handling loop that reads [`McpRequest`]s from
/// the transport, dispatches them to the appropriate handler, and writes
/// [`JsonRpcResponse`]s back.
pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
    transport: Arc<dyn crate::transport::McpTransport>,
}

impl McpServer {
    /// Create a new [`McpServer`] with the given transport.
    ///
    /// Tools must be registered separately via [`register_tool`](Self::register_tool).
    pub fn new(transport: Arc<dyn crate::transport::McpTransport>) -> Self {
        McpServer {
            tools: HashMap::new(),
            transport,
        }
    }

    /// Register a tool with the server.
    ///
    /// If a tool with the same name is already registered, it will be replaced.
    pub fn register_tool(&mut self, tool: Box<dyn McpTool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get the number of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Check if a tool with the given name is registered.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get a tool definition by name.
    pub fn get_tool(&self, name: &str) -> Option<&dyn McpTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Run the server's request-handling loop.
    ///
    /// Reads requests from the transport one at a time, dispatches them,
    /// and writes responses. The loop exits when the transport returns
    /// `None` (EOF) or an unrecoverable error occurs.
    pub async fn run(&self) -> CvResult<()> {
        tracing::info!("MCP server starting, {} tools registered", self.tool_count());

        loop {
            match self.transport.read_request().await {
                Ok(None) => {
                    tracing::info!("Transport closed (EOF), shutting down");
                    break;
                }
                Ok(Some(request)) => {
                    let response = self.handle_request(request).await;
                    let json_value =
                        serde_json::to_value(&response).map_err(|e| cv_shared::CvError::Mcp(format!("JSON serialize: {e}")))?;
                    self.transport.write_response(&json_value).await?;
                }
                Err(e) => {
                    tracing::error!("Transport read error: {}", e);
                    // Write an error response
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32603,
                            message: format!("Transport error: {e}"),
                            data: None,
                        }),
                    };
                    let json_value = serde_json::to_value(&error_response)
                        .unwrap_or_else(|_| json!({"jsonrpc": "2.0", "error": {"code": -32603, "message": "Internal error"}}));
                    self.transport.write_response(&json_value).await?;
                }
            }
        }

        tracing::info!("MCP server shut down");
        Ok(())
    }

    /// Handle a single MCP request and return a JSON-RPC response envelope.
    ///
    /// This method is public so it can be used in unit tests without
    /// needing to set up a full transport.
    pub async fn handle_request(&self, request: McpRequest) -> JsonRpcResponse {
        let (result, error) = match request {
            McpRequest::Initialize { params } => {
                let response = self.handle_initialize(params).await;
                (Some(response), None)
            }
            McpRequest::ToolsList => {
                let response = self.handle_tools_list().await;
                (Some(response), None)
            }
            McpRequest::ToolsCall { params } => {
                let response = self.handle_tools_call(params).await;
                (Some(response), None)
            }
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result,
            error,
        }
    }

    /// Handle an `initialize` request.
    ///
    /// Returns server capabilities and protocol information.
    async fn handle_initialize(&self, _params: InitializeParams) -> McpResponse {
        tracing::debug!("Handling initialize request");

        let capabilities = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "clawviewer-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        McpResponse::text(capabilities.to_string())
    }

    /// Handle a `tools/list` request.
    ///
    /// Returns metadata for all registered tools.
    async fn handle_tools_list(&self) -> McpResponse {
        tracing::debug!("Handling tools/list request ({} tools)", self.tool_count());

        let tools: Vec<ToolDefinition> = self
            .tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect();

        let tools_json =
            serde_json::to_string(&tools).unwrap_or_else(|_| "[]".to_string());

        McpResponse::text(tools_json)
    }

    /// Handle a `tools/call` request.
    ///
    /// Looks up the tool by name and invokes it with the provided arguments.
    async fn handle_tools_call(
        &self,
        params: crate::types::ToolsCallParams,
    ) -> McpResponse {
        tracing::debug!("Handling tools/call request: tool='{}'", params.name);

        let tool = match self.tools.get(&params.name) {
            Some(tool) => tool,
            None => {
                tracing::warn!("Tool '{}' not found", params.name);
                return McpResponse::error(format!("Tool '{}' not found", params.name));
            }
        };

        match tool.execute(params.arguments).await {
            Ok(response) => {
                tracing::debug!("Tool '{}' executed successfully", params.name);
                response
            }
            Err(e) => {
                tracing::error!("Tool '{}' execution failed: {}", params.name, e);
                McpResponse::error(format!("Tool execution failed: {e}"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{KeyboardTypeTool, MouseClickTool, MouseMoveTool, ScreenshotTool};
    use crate::transport::MockTransport;
    use crate::types::ToolsCallParams;

    fn create_test_server() -> McpServer {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);

        server.register_tool(Box::new(ScreenshotTool::new()));
        server.register_tool(Box::new(MouseClickTool::new()));
        server.register_tool(Box::new(MouseMoveTool::new()));
        server.register_tool(Box::new(KeyboardTypeTool::new()));

        server
    }

    // ===================================================================
    // Tool registration
    // ===================================================================

    #[test]
    fn server_starts_with_no_tools() {
        let transport = Arc::new(MockTransport::empty());
        let server = McpServer::new(transport);
        assert_eq!(server.tool_count(), 0);
    }

    #[test]
    fn server_register_tool_increments_count() {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);
        server.register_tool(Box::new(ScreenshotTool::new()));
        assert_eq!(server.tool_count(), 1);
    }

    #[test]
    fn server_register_multiple_tools() {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);
        server.register_tool(Box::new(ScreenshotTool::new()));
        server.register_tool(Box::new(MouseClickTool::new()));
        server.register_tool(Box::new(MouseMoveTool::new()));
        assert_eq!(server.tool_count(), 3);
    }

    #[test]
    fn server_has_tool_lookup() {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);
        server.register_tool(Box::new(ScreenshotTool::new()));

        assert!(server.has_tool("screenshot"));
        assert!(!server.has_tool("nonexistent"));
    }

    #[test]
    fn server_get_tool_returns_tool() {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);
        server.register_tool(Box::new(ScreenshotTool::new()));

        let tool = server.get_tool("screenshot");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "screenshot");
    }

    #[test]
    fn server_register_tool_replaces_existing() {
        let transport = Arc::new(MockTransport::empty());
        let mut server = McpServer::new(transport);
        server.register_tool(Box::new(ScreenshotTool::new()));
        server.register_tool(Box::new(ScreenshotTool::new()));
        assert_eq!(server.tool_count(), 1);
    }

    // ===================================================================
    // Initialize request
    // ===================================================================

    #[tokio::test]
    async fn handle_initialize_returns_capabilities() {
        let server = create_test_server();
        let response = server
            .handle_request(McpRequest::Initialize {
                params: InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                },
            })
            .await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    // ===================================================================
    // Tools/List request
    // ===================================================================

    #[tokio::test]
    async fn handle_tools_list_returns_all_tools() {
        let server = create_test_server();
        let response = server.handle_request(McpRequest::ToolsList).await;

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert_eq!(result.content.len(), 1);

        let text = match &result.content[0] {
            ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };

        // Should contain JSON with all 4 registered tools
        let tools: Vec<ToolDefinition> = serde_json::from_str(text).expect("parse tools JSON");
        assert_eq!(tools.len(), 4);

        let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"screenshot".to_string()));
        assert!(names.contains(&"mouse_click".to_string()));
        assert!(names.contains(&"mouse_move".to_string()));
        assert!(names.contains(&"keyboard_type".to_string()));
    }

    // ===================================================================
    // Tools/Call request
    // ===================================================================

    #[tokio::test]
    async fn handle_tools_call_screenshot() {
        let server = create_test_server();
        let response = server
            .handle_request(McpRequest::ToolsCall {
                params: ToolsCallParams {
                    name: "screenshot".to_string(),
                    arguments: serde_json::json!({}),
                },
            })
            .await;

        assert!(response.result.is_some());
        let text = match &response.result.unwrap().content[0] {
            ContentItem::Text { text } => text.clone(),
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Screenshot"));
    }

    #[tokio::test]
    async fn handle_tools_call_mouse_click() {
        let server = create_test_server();
        let response = server
            .handle_request(McpRequest::ToolsCall {
                params: ToolsCallParams {
                    name: "mouse_click".to_string(),
                    arguments: serde_json::json!({"x": 100, "y": 200, "button": "left"}),
                },
            })
            .await;

        assert!(response.result.is_some());
        let text = match &response.result.unwrap().content[0] {
            ContentItem::Text { text } => text.clone(),
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("left clicked at (100, 200)"));
    }

    #[tokio::test]
    async fn handle_tools_call_unknown_tool() {
        let server = create_test_server();
        let response = server
            .handle_request(McpRequest::ToolsCall {
                params: ToolsCallParams {
                    name: "nonexistent_tool".to_string(),
                    arguments: serde_json::json!({}),
                },
            })
            .await;

        assert!(response.result.is_some());
        let text = match &response.result.unwrap().content[0] {
            ContentItem::Text { text } => text.clone(),
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("not found"));
    }

    #[tokio::test]
    async fn handle_tools_call_mouse_move() {
        let server = create_test_server();
        let response = server
            .handle_request(McpRequest::ToolsCall {
                params: ToolsCallParams {
                    name: "mouse_move".to_string(),
                    arguments: serde_json::json!({"x": 500, "y": 300}),
                },
            })
            .await;

        assert!(response.result.is_some());
        let text = match &response.result.unwrap().content[0] {
            ContentItem::Text { text } => text.clone(),
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Mouse moved to (500, 300)"));
    }

    // ===================================================================
    // End-to-end: server.run() with MockTransport
    // ===================================================================

    #[tokio::test]
    async fn server_run_processes_all_requests() {
        let requests = vec![
            McpRequest::ToolsList,
            McpRequest::Initialize {
                params: InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                },
            },
        ];

        let transport = Arc::new(MockTransport::new(requests));
        let mut server = McpServer::new(transport.clone());
        server.register_tool(Box::new(ScreenshotTool::new()));

        // run() will process both requests then exit on None
        // We need to add a None terminator - but MockTransport just returns None when empty
        server.run().await.unwrap();

        let responses = transport.take_responses().await;
        assert_eq!(responses.len(), 2, "Expected 2 responses for 2 requests");
    }

    // ===================================================================
    // Send + Sync assertions
    // ===================================================================

    #[allow(dead_code)]
    fn _assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<McpServer>();
    }
}
