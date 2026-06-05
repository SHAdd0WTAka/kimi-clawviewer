//! Transport layer for MCP (Model Context Protocol) communication.
//!
//! This module defines the [`McpTransport`] trait and a stdio-based implementation
//! [`StdioTransport`] that reads JSON-RPC requests line-by-line from stdin and
//! writes responses to stdout.
//!
//! # Usage
//!
//! ```
//! use std::sync::Arc;
//! use cv_mcp::transport::{McpTransport, StdioTransport};
//!
//! let transport: Arc<dyn McpTransport> = Arc::new(StdioTransport::new());
//! ```

use async_trait::async_trait;
use std::io;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};

use crate::types::McpRequest;
use cv_shared::{CvError, CvResult};

// ---------------------------------------------------------------------------
// McpTransport trait
// ---------------------------------------------------------------------------

/// Abstract transport for MCP message exchange.
///
/// Implementations handle the low-level I/O of reading requests and writing
/// responses. The [`StdioTransport`] is the primary implementation for
/// local AI agent integration.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Read the next MCP request from the transport.
    ///
    /// Returns `Ok(None)` when the transport has been closed (EOF).
    async fn read_request(&self) -> CvResult<Option<McpRequest>>;

    /// Write an MCP response to the transport.
    ///
    /// The response should be serialized as JSON-RPC by the caller.
    async fn write_response(&self, response: &serde_json::Value) -> CvResult<()>;

    /// Write a raw text line to the transport.
    ///
    /// Used for JSON-RPC line-delimited framing.
    async fn write_line(&self, line: &str) -> CvResult<()>;
}

// ---------------------------------------------------------------------------
// StdioTransport
// ---------------------------------------------------------------------------

/// Stdio-based MCP transport.
///
/// Reads JSON-RPC requests line-by-line from stdin and writes responses
/// to stdout. This is the standard transport for MCP servers communicating
/// with local AI agents.
///
/// Each message is a single JSON object on its own line (newline-delimited
/// JSON / JSON Lines format).
pub struct StdioTransport {
    stdin: tokio::sync::Mutex<BufReader<Stdin>>,
    stdout: tokio::sync::Mutex<Stdout>,
}

impl StdioTransport {
    /// Create a new [`StdioTransport`] backed by process stdio.
    pub fn new() -> Self {
        StdioTransport {
            stdin: tokio::sync::Mutex::new(BufReader::new(tokio::io::stdin())),
            stdout: tokio::sync::Mutex::new(tokio::io::stdout()),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn read_request(&self) -> CvResult<Option<McpRequest>> {
        let mut stdin = self.stdin.lock().await;
        let mut line = String::new();

        let bytes_read = stdin
            .read_line(&mut line)
            .await
            .map_err(|e| CvError::Io(e))?;

        if bytes_read == 0 {
            // EOF reached
            return Ok(None);
        }

        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        let request: McpRequest =
            serde_json::from_str(line).map_err(|e| CvError::Mcp(format!("JSON parse: {e}")))?;

        Ok(Some(request))
    }

    async fn write_response(&self, response: &serde_json::Value) -> CvResult<()> {
        let json =
            serde_json::to_string(response).map_err(|e| CvError::Mcp(format!("JSON serialize: {e}")))?;
        self.write_line(&json).await
    }

    async fn write_line(&self, line: &str) -> CvResult<()> {
        let mut stdout = self.stdout.lock().await;
        stdout
            .write_all(line.as_bytes())
            .await
            .map_err(|e| CvError::Io(e))?;
        stdout
            .write_all(b"\n")
            .await
            .map_err(|e| CvError::Io(e))?;
        stdout.flush().await.map_err(|e| CvError::Io(e))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MockTransport (for testing)
// ---------------------------------------------------------------------------

/// A mock transport for unit testing.
///
/// Pre-loads a queue of requests and captures all written responses for
/// later inspection.
pub struct MockTransport {
    requests: tokio::sync::Mutex<std::collections::VecDeque<McpRequest>>,
    responses: tokio::sync::Mutex<Vec<serde_json::Value>>,
}

impl MockTransport {
    /// Create a new [`MockTransport`] with the given pre-loaded requests.
    pub fn new(requests: Vec<McpRequest>) -> Self {
        MockTransport {
            requests: tokio::sync::Mutex::new(requests.into()),
            responses: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create an empty [`MockTransport`] with no pre-loaded requests.
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    /// Push a request to the back of the queue.
    pub async fn push_request(&self, request: McpRequest) {
        let mut queue = self.requests.lock().await;
        queue.push_back(request);
    }

    /// Take all captured responses.
    pub async fn take_responses(&self) -> Vec<serde_json::Value> {
        let mut responses = self.responses.lock().await;
        std::mem::take(&mut *responses)
    }

    /// Get the number of captured responses.
    pub async fn response_count(&self) -> usize {
        self.responses.lock().await.len()
    }
}

#[async_trait]
impl McpTransport for MockTransport {
    async fn read_request(&self) -> CvResult<Option<McpRequest>> {
        let mut queue = self.requests.lock().await;
        Ok(queue.pop_front())
    }

    async fn write_response(&self, response: &serde_json::Value) -> CvResult<()> {
        let mut responses = self.responses.lock().await;
        responses.push(response.clone());
        Ok(())
    }

    async fn write_line(&self, line: &str) -> CvResult<()> {
        let mut responses = self.responses.lock().await;
        responses.push(serde_json::Value::String(line.to_string()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InitializeParams, McpRequest, ToolsCallParams};

    // ===================================================================
    // MockTransport tests
    // ===================================================================

    #[tokio::test]
    async fn mock_transport_reads_requests_in_order() {
        let transport = MockTransport::new(vec![
            McpRequest::ToolsList,
            McpRequest::Initialize {
                params: InitializeParams {
                    protocol_version: "1.0".to_string(),
                },
            },
        ]);

        let req1 = transport.read_request().await.unwrap();
        assert_eq!(req1, Some(McpRequest::ToolsList));

        let req2 = transport.read_request().await.unwrap();
        assert_eq!(
            req2,
            Some(McpRequest::Initialize {
                params: InitializeParams {
                    protocol_version: "1.0".to_string(),
                },
            })
        );

        let req3 = transport.read_request().await.unwrap();
        assert_eq!(req3, None);
    }

    #[tokio::test]
    async fn mock_transport_captures_responses() {
        let transport = MockTransport::empty();
        let value = serde_json::json!({"result": "ok"});

        transport.write_response(&value).await.unwrap();

        let responses = transport.take_responses().await;
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], value);
    }

    #[tokio::test]
    async fn mock_transport_write_line() {
        let transport = MockTransport::empty();
        transport.write_line("hello world").await.unwrap();

        let responses = transport.take_responses().await;
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], serde_json::Value::String("hello world".to_string()));
    }

    #[tokio::test]
    async fn mock_transport_push_request() {
        let transport = MockTransport::empty();
        transport.push_request(McpRequest::ToolsList).await;

        let req = transport.read_request().await.unwrap();
        assert_eq!(req, Some(McpRequest::ToolsList));
    }

    // ===================================================================
    // StdioTransport construction
    // ===================================================================

    #[test]
    fn stdio_transport_new() {
        let _transport = StdioTransport::new();
        // Just verify it constructs without panicking.
    }

    #[test]
    fn stdio_transport_default() {
        let _transport: StdioTransport = Default::default();
    }

    // ===================================================================
    // Send + Sync assertions
    // ===================================================================

    #[allow(dead_code)]
    fn _assert_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StdioTransport>();
        assert_send_sync::<MockTransport>();
    }
}
