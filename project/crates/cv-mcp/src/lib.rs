//! `cv-mcp` — Model Context Protocol (MCP) server for AI agent integration.
//!
//! This crate provides a simplified MCP server implementation that uses
//! JSON-RPC 2.0 over stdio (or other transports) to expose desktop control
//! tools to AI agents.
//!
//! # Architecture
//!
//! ```
//! AI Agent (Client)     cv-mcp (Server)
//!      |                        |
//!      |-- initialize --------->|  server.rs
//!      |<-- capabilities -------|
//!      |                        |
//!      |-- tools/list --------->|  server.rs
//!      |<-- tool definitions ---|
//!      |                        |
//!      |-- tools/call --------->|  server.rs -> tools.rs
//!      |<-- result/response ----|
//! ```
//!
//! # Modules
//!
//! - [`server`] — `McpServer`, `McpTool` trait, request dispatch.
//! - [`tools`] — Concrete tool implementations (`ScreenshotTool`, `MouseClickTool`, ...).
//! - [`transport`] — `McpTransport` trait, `StdioTransport`, `MockTransport`.
//! - [`types`] — MCP request/response types, JSON-RPC envelopes.
//!
//! # Quick Start
//!
//! ```no_run
//! use std::sync::Arc;
//! use cv_mcp::server::McpServer;
//! use cv_mcp::transport::StdioTransport;
//! use cv_mcp::tools;
//!
//! #[tokio::main]
//! async fn main() {
//!     let transport = Arc::new(StdioTransport::new());
//!     let mut server = McpServer::new(transport);
//!
//!     // Register all built-in tools
//!     for tool in tools::all_tools() {
//!         server.register_tool(tool);
//!     }
//!
//!     // Run the server loop
//!     server.run().await.expect("server run failed");
//! }
//! ```

pub mod server;
pub mod tools;
pub mod transport;
pub mod types;

// Re-export commonly used items for convenience.
pub use server::{McpServer, McpTool};
pub use transport::{McpTransport, StdioTransport};
pub use types::{
    ContentItem, JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpRequest, McpResponse,
    ToolDefinition,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use transport::MockTransport;

    /// End-to-end test: full initialize -> tools/list -> tools/call flow.
    #[tokio::test]
    async fn end_to_end_initialize_tools_list_and_call() {
        // Set up mock transport with a sequence of requests
        let requests = vec![
            // 1. Initialize
            types::McpRequest::Initialize {
                params: types::InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                },
            },
            // 2. List tools
            types::McpRequest::ToolsList,
            // 3. Call screenshot tool
            types::McpRequest::ToolsCall {
                params: types::ToolsCallParams {
                    name: "screenshot".to_string(),
                    arguments: serde_json::json!({}),
                },
            },
            // 4. Call mouse_click tool
            types::McpRequest::ToolsCall {
                params: types::ToolsCallParams {
                    name: "mouse_click".to_string(),
                    arguments: serde_json::json!({"x": 100, "y": 200, "button": "left"}),
                },
            },
            // 5. Call keyboard_type tool
            types::McpRequest::ToolsCall {
                params: types::ToolsCallParams {
                    name: "keyboard_type".to_string(),
                    arguments: serde_json::json!({"text": "Hello from MCP!"}),
                },
            },
        ];

        let mock_transport = Arc::new(MockTransport::new(requests));
        let mut server = server::McpServer::new(mock_transport.clone());

        // Register all tools
        for tool in tools::all_tools() {
            server.register_tool(tool);
        }

        assert_eq!(server.tool_count(), 6);

        // Run the server (processes all requests, then exits on empty queue)
        server.run().await.unwrap();

        // Verify responses
        let responses = mock_transport.take_responses().await;
        assert_eq!(
            responses.len(),
            5,
            "Expected 5 responses for 5 requests, got {}",
            responses.len()
        );

        // Response 1: initialize - should contain server info
        let resp1 = responses[0].as_object().expect("response 1 should be object");
        assert!(resp1.contains_key("content"));

        // Response 2: tools/list - should contain 6 tools
        let resp2_text = responses[1]["content"][0]["text"]
            .as_str()
            .unwrap_or("[]");
        let tool_defs: Vec<types::ToolDefinition> =
            serde_json::from_str(resp2_text).expect("parse tool definitions");
        assert_eq!(tool_defs.len(), 6);

        // Response 3: screenshot call
        let resp3_text = responses[2]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert!(resp3_text.contains("Screenshot"), "Response 3 should contain screenshot result");

        // Response 4: mouse_click call
        let resp4_text = responses[3]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert!(
            resp4_text.contains("left clicked at (100, 200)"),
            "Response 4 should contain click result, got: {}",
            resp4_text
        );

        // Response 5: keyboard_type call
        let resp5_text = responses[4]["content"][0]["text"]
            .as_str()
            .unwrap_or("");
        assert!(
            resp5_text.contains("Typed text: 'Hello from MCP!'"),
            "Response 5 should contain type result, got: {}",
            resp5_text
        );
    }

    /// Test that all tools have unique names.
    #[test]
    fn all_tools_have_unique_names() {
        let tools = tools::all_tools();
        let mut names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            6,
            "Expected 6 unique tool names, found duplicates"
        );
    }

    /// Test that all tools have non-empty descriptions and schemas.
    #[test]
    fn all_tools_have_metadata() {
        for tool in tools::all_tools() {
            assert!(
                !tool.name().is_empty(),
                "Tool name should not be empty"
            );
            assert!(
                !tool.description().is_empty(),
                "Tool '{}' description should not be empty",
                tool.name()
            );
            assert!(
                !tool.input_schema().is_null(),
                "Tool '{}' schema should not be null",
                tool.name()
            );
            assert_eq!(
                tool.input_schema()["type"],
                "object",
                "Tool '{}' schema should be an object",
                tool.name()
            );
        }
    }

    /// Compile-time Send + Sync assertion for McpServer.
    #[allow(dead_code)]
    fn _assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<McpServer>();
        assert_send_sync::<MockTransport>();
        assert_send_sync::<types::McpRequest>();
        assert_send_sync::<types::McpResponse>();
        assert_send_sync::<types::ToolDefinition>();
    }
}
