//! Concrete MCP tool implementations for AI agent integration.
//!
//! This module provides six tools that allow AI agents to interact with the
//! remote desktop:
//!
//! | Tool | Purpose |
//! |------|---------|
//! | [`ScreenshotTool`] | Capture the current screen state |
//! | [`MouseClickTool`] | Click at specific screen coordinates |
//! | [`MouseMoveTool`] | Move the cursor to specific coordinates |
//! | [`KeyboardTypeTool`] | Type a text string |
//! | [`GetClipboardTool`] | Read the clipboard contents |
//! | [`GetUiStateTool`] | Query the current UI state |
//!
//! Each tool implements the [`McpTool`] trait and provides a JSON Schema
//! describing its expected input.

use async_trait::async_trait;
use serde_json::json;

use crate::server::McpTool;
use crate::types::McpResponse;
use cv_shared::{CvResult, EventSource, EventType, InputEvent, MouseButton, Priority};

// ---------------------------------------------------------------------------
// ScreenshotTool
// ---------------------------------------------------------------------------

/// Capture a screenshot of the current screen.
///
/// Returns a placeholder description. In production this would capture
/// the actual screen via DXGI and return base64-encoded image data.
pub struct ScreenshotTool;

impl ScreenshotTool {
    /// Create a new [`ScreenshotTool`].
    pub fn new() -> Self {
        ScreenshotTool
    }
}

impl Default for ScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for ScreenshotTool {
    fn name(&self) -> &str {
        "screenshot"
    }

    fn description(&self) -> &str {
        "Take a screenshot of the current screen and return it as a base64-encoded PNG image. \
         Use this to observe the current state of the remote desktop."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "description": "No input parameters required."
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> CvResult<McpResponse> {
        // Placeholder: in production, this would call cv-capture to get
        // the actual frame data and encode it as base64 PNG.
        Ok(McpResponse::text(
            "[Screenshot placeholder] Screen capture would return a 1920x1080 image here. \
             In production mode this contains actual base64-encoded PNG data.",
        ))
    }
}

// ---------------------------------------------------------------------------
// MouseClickTool
// ---------------------------------------------------------------------------

/// Click the mouse at specific screen coordinates.
///
/// Accepts `{ x: i32, y: i32, button: "left" | "right" | "middle" }`.
pub struct MouseClickTool;

impl MouseClickTool {
    /// Create a new [`MouseClickTool`].
    pub fn new() -> Self {
        MouseClickTool
    }
}

impl Default for MouseClickTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for MouseClickTool {
    fn name(&self) -> &str {
        "mouse_click"
    }

    fn description(&self) -> &str {
        "Click the mouse at the specified screen coordinates. \
         Use this to interact with buttons, links, and other clickable UI elements."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "integer",
                    "description": "Absolute X coordinate in screen pixels."
                },
                "y": {
                    "type": "integer",
                    "description": "Absolute Y coordinate in screen pixels."
                },
                "button": {
                    "type": "string",
                    "enum": ["left", "right", "middle"],
                    "description": "Mouse button to click."
                }
            },
            "required": ["x", "y", "button"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> CvResult<McpResponse> {
        let x = input["x"]
            .as_i64()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'x' parameter".to_string()))?;
        let y = input["y"]
            .as_i64()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'y' parameter".to_string()))?;
        let button_str = input["button"]
            .as_str()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'button' parameter".to_string()))?;

        let button = match button_str {
            "left" => MouseButton::Left,
            "right" => MouseButton::Right,
            "middle" => MouseButton::Middle,
            other => {
                return Ok(McpResponse::error(format!(
                    "Invalid button '{}'. Use 'left', 'right', or 'middle'.",
                    other
                )))
            }
        };

        // Placeholder: In production this would call cv-input to inject the event.
        let _event = InputEvent {
            source: EventSource::AI,
            event_type: EventType::MouseClick {
                button,
                down: true,
            },
            priority: Priority::P2_AI_Confirmed,
            payload: cv_shared::EventPayload::empty(),
            timestamp: cv_shared::utils::now_millis(),
            sequence: 0,
        };

        // Also inject mouse-up (full click)
        let _up_event = InputEvent {
            source: EventSource::AI,
            event_type: EventType::MouseClick {
                button,
                down: false,
            },
            priority: Priority::P2_AI_Confirmed,
            payload: cv_shared::EventPayload::empty(),
            timestamp: cv_shared::utils::now_millis(),
            sequence: 1,
        };

        Ok(McpResponse::text(format!(
            "Mouse {} clicked at ({}, {}).",
            button_str, x, y
        )))
    }
}

// ---------------------------------------------------------------------------
// MouseMoveTool
// ---------------------------------------------------------------------------

/// Move the mouse cursor to specific screen coordinates.
///
/// Accepts `{ x: i32, y: i32 }`.
pub struct MouseMoveTool;

impl MouseMoveTool {
    /// Create a new [`MouseMoveTool`].
    pub fn new() -> Self {
        MouseMoveTool
    }
}

impl Default for MouseMoveTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for MouseMoveTool {
    fn name(&self) -> &str {
        "mouse_move"
    }

    fn description(&self) -> &str {
        "Move the mouse cursor to the specified absolute screen coordinates. \
         Use this to position the cursor before clicking or to hover over elements."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "integer",
                    "description": "Absolute X coordinate in screen pixels."
                },
                "y": {
                    "type": "integer",
                    "description": "Absolute Y coordinate in screen pixels."
                }
            },
            "required": ["x", "y"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> CvResult<McpResponse> {
        let x = input["x"]
            .as_i64()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'x' parameter".to_string()))?;
        let y = input["y"]
            .as_i64()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'y' parameter".to_string()))?;

        // Placeholder: In production this would call cv-input to inject the event.
        let _event = InputEvent {
            source: EventSource::AI,
            event_type: EventType::MouseMove { x: x as i32, y: y as i32 },
            priority: Priority::P2_AI_Confirmed,
            payload: cv_shared::EventPayload::empty(),
            timestamp: cv_shared::utils::now_millis(),
            sequence: 0,
        };

        Ok(McpResponse::text(format!(
            "Mouse moved to ({}, {}).",
            x, y
        )))
    }
}

// ---------------------------------------------------------------------------
// KeyboardTypeTool
// ---------------------------------------------------------------------------

/// Type a text string on the remote keyboard.
///
/// Accepts `{ text: String }`.
pub struct KeyboardTypeTool;

impl KeyboardTypeTool {
    /// Create a new [`KeyboardTypeTool`].
    pub fn new() -> Self {
        KeyboardTypeTool
    }
}

impl Default for KeyboardTypeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for KeyboardTypeTool {
    fn name(&self) -> &str {
        "keyboard_type"
    }

    fn description(&self) -> &str {
        "Type the given text string on the remote keyboard. \
         Use this to enter text into input fields, search boxes, or documents."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The text string to type."
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> CvResult<McpResponse> {
        let text = input["text"]
            .as_str()
            .ok_or_else(|| cv_shared::CvError::Input("missing 'text' parameter".to_string()))?;

        if text.is_empty() {
            return Ok(McpResponse::error("Text parameter cannot be empty."));
        }

        // Placeholder: In production this would call cv-input to inject the event.
        let _event = InputEvent {
            source: EventSource::AI,
            event_type: EventType::KeyType {
                text: text.to_string(),
            },
            priority: Priority::P2_AI_Confirmed,
            payload: cv_shared::EventPayload::empty(),
            timestamp: cv_shared::utils::now_millis(),
            sequence: 0,
        };

        Ok(McpResponse::text(format!(
            "Typed text: '{}'",
            text
        )))
    }
}

// ---------------------------------------------------------------------------
// GetClipboardTool
// ---------------------------------------------------------------------------

/// Read the contents of the remote clipboard.
///
/// Returns the clipboard text content. No input parameters required.
pub struct GetClipboardTool;

impl GetClipboardTool {
    /// Create a new [`GetClipboardTool`].
    pub fn new() -> Self {
        GetClipboardTool
    }
}

impl Default for GetClipboardTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for GetClipboardTool {
    fn name(&self) -> &str {
        "get_clipboard"
    }

    fn description(&self) -> &str {
        "Read the current contents of the remote clipboard. \
         Use this to retrieve text that was copied on the remote machine."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "description": "No input parameters required."
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> CvResult<McpResponse> {
        // Placeholder: In production this would call the Windows clipboard API
        // (GetClipboardData) to read the actual clipboard contents.
        Ok(McpResponse::text(
            "[Clipboard placeholder] Clipboard would return actual text content here. \
             In production mode this contains the real clipboard data from the remote machine.",
        ))
    }
}

// ---------------------------------------------------------------------------
// GetUiStateTool
// ---------------------------------------------------------------------------

/// Query the current UI state of the remote desktop.
///
/// Returns information about the active window, cursor position, and
/// general UI state. No input parameters required.
pub struct GetUiStateTool;

impl GetUiStateTool {
    /// Create a new [`GetUiStateTool`].
    pub fn new() -> Self {
        GetUiStateTool
    }
}

impl Default for GetUiStateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for GetUiStateTool {
    fn name(&self) -> &str {
        "get_ui_state"
    }

    fn description(&self) -> &str {
        "Query the current UI state of the remote desktop. \
         Returns information about the active window, cursor position, \
         screen resolution, and available UI elements. \
         Use this to understand the current context before taking actions."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "description": "No input parameters required."
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> CvResult<McpResponse> {
        // Placeholder: In production this would query the actual UI state
        // via accessibility APIs or screen analysis.
        let state = json!({
            "active_window": "Placeholder Window",
            "cursor_position": { "x": 960, "y": 540 },
            "screen_resolution": { "width": 1920, "height": 1080 },
            "clipboard_has_content": false,
            "timestamp": cv_shared::utils::now_millis()
        });

        Ok(McpResponse::text(state.to_string()))
    }
}

// ---------------------------------------------------------------------------
// All tools helper
// ---------------------------------------------------------------------------

/// Returns a vector of all available tool instances.
///
/// This is the canonical way to register all built-in tools with an
/// [`McpServer`](crate::server::McpServer).
pub fn all_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(ScreenshotTool::new()),
        Box::new(MouseClickTool::new()),
        Box::new(MouseMoveTool::new()),
        Box::new(KeyboardTypeTool::new()),
        Box::new(GetClipboardTool::new()),
        Box::new(GetUiStateTool::new()),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ===================================================================
    // ScreenshotTool
    // ===================================================================

    #[tokio::test]
    async fn screenshot_tool_name() {
        let tool = ScreenshotTool::new();
        assert_eq!(tool.name(), "screenshot");
    }

    #[tokio::test]
    async fn screenshot_tool_execute() {
        let tool = ScreenshotTool::new();
        let response = tool.execute(serde_json::json!({})).await.unwrap();
        assert_eq!(response.content.len(), 1);
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Screenshot"));
    }

    #[tokio::test]
    async fn screenshot_tool_schema_is_object() {
        let tool = ScreenshotTool::new();
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
    }

    // ===================================================================
    // MouseClickTool
    // ===================================================================

    #[tokio::test]
    async fn mouse_click_tool_name() {
        let tool = MouseClickTool::new();
        assert_eq!(tool.name(), "mouse_click");
    }

    #[tokio::test]
    async fn mouse_click_tool_execute_left() {
        let tool = MouseClickTool::new();
        let response = tool
            .execute(serde_json::json!({"x": 100, "y": 200, "button": "left"}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("left clicked at (100, 200)"));
    }

    #[tokio::test]
    async fn mouse_click_tool_execute_right() {
        let tool = MouseClickTool::new();
        let response = tool
            .execute(serde_json::json!({"x": 50, "y": 75, "button": "right"}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("right clicked at (50, 75)"));
    }

    #[tokio::test]
    async fn mouse_click_tool_invalid_button() {
        let tool = MouseClickTool::new();
        let response = tool
            .execute(serde_json::json!({"x": 0, "y": 0, "button": "invalid"}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Invalid button"));
    }

    #[tokio::test]
    async fn mouse_click_tool_missing_x() {
        let tool = MouseClickTool::new();
        let result = tool.execute(serde_json::json!({"y": 200, "button": "left"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mouse_click_tool_schema_required_fields() {
        let tool = MouseClickTool::new();
        let schema = tool.input_schema();
        let required = schema["required"].as_array().unwrap();
        let fields: Vec<String> = required
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(fields.contains(&"x".to_string()));
        assert!(fields.contains(&"y".to_string()));
        assert!(fields.contains(&"button".to_string()));
    }

    // ===================================================================
    // MouseMoveTool
    // ===================================================================

    #[tokio::test]
    async fn mouse_move_tool_name() {
        let tool = MouseMoveTool::new();
        assert_eq!(tool.name(), "mouse_move");
    }

    #[tokio::test]
    async fn mouse_move_tool_execute() {
        let tool = MouseMoveTool::new();
        let response = tool
            .execute(serde_json::json!({"x": 500, "y": 300}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Mouse moved to (500, 300)"));
    }

    #[tokio::test]
    async fn mouse_move_tool_missing_params() {
        let tool = MouseMoveTool::new();
        let result = tool.execute(serde_json::json!({"x": 100})).await;
        assert!(result.is_err());
    }

    // ===================================================================
    // KeyboardTypeTool
    // ===================================================================

    #[tokio::test]
    async fn keyboard_type_tool_name() {
        let tool = KeyboardTypeTool::new();
        assert_eq!(tool.name(), "keyboard_type");
    }

    #[tokio::test]
    async fn keyboard_type_tool_execute() {
        let tool = KeyboardTypeTool::new();
        let response = tool
            .execute(serde_json::json!({"text": "Hello, World!"}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Typed text: 'Hello, World!'"));
    }

    #[tokio::test]
    async fn keyboard_type_tool_empty_text() {
        let tool = KeyboardTypeTool::new();
        let response = tool
            .execute(serde_json::json!({"text": ""}))
            .await
            .unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("cannot be empty"));
    }

    #[tokio::test]
    async fn keyboard_type_tool_missing_text() {
        let tool = KeyboardTypeTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    // ===================================================================
    // GetClipboardTool
    // ===================================================================

    #[tokio::test]
    async fn get_clipboard_tool_name() {
        let tool = GetClipboardTool::new();
        assert_eq!(tool.name(), "get_clipboard");
    }

    #[tokio::test]
    async fn get_clipboard_tool_execute() {
        let tool = GetClipboardTool::new();
        let response = tool.execute(serde_json::json!({})).await.unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("Clipboard"));
    }

    // ===================================================================
    // GetUiStateTool
    // ===================================================================

    #[tokio::test]
    async fn get_ui_state_tool_name() {
        let tool = GetUiStateTool::new();
        assert_eq!(tool.name(), "get_ui_state");
    }

    #[tokio::test]
    async fn get_ui_state_tool_execute() {
        let tool = GetUiStateTool::new();
        let response = tool.execute(serde_json::json!({})).await.unwrap();
        let text = match &response.content[0] {
            crate::types::ContentItem::Text { text } => text,
            other => panic!("Expected Text content, got {:?}", other),
        };
        assert!(text.contains("active_window"));
        assert!(text.contains("cursor_position"));
        assert!(text.contains("screen_resolution"));
    }

    // ===================================================================
    // all_tools helper
    // ===================================================================

    #[test]
    fn all_tools_returns_six_tools() {
        let tools = all_tools();
        assert_eq!(tools.len(), 6);

        let names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();
        assert!(names.contains(&"screenshot".to_string()));
        assert!(names.contains(&"mouse_click".to_string()));
        assert!(names.contains(&"mouse_move".to_string()));
        assert!(names.contains(&"keyboard_type".to_string()));
        assert!(names.contains(&"get_clipboard".to_string()));
        assert!(names.contains(&"get_ui_state".to_string()));
    }

    // ===================================================================
    // Default constructors
    // ===================================================================

    #[test]
    fn screenshot_tool_default() {
        let _tool: ScreenshotTool = Default::default();
    }

    #[test]
    fn mouse_click_tool_default() {
        let _tool: MouseClickTool = Default::default();
    }

    #[test]
    fn mouse_move_tool_default() {
        let _tool: MouseMoveTool = Default::default();
    }

    #[test]
    fn keyboard_type_tool_default() {
        let _tool: KeyboardTypeTool = Default::default();
    }

    #[test]
    fn get_clipboard_tool_default() {
        let _tool: GetClipboardTool = Default::default();
    }

    #[test]
    fn get_ui_state_tool_default() {
        let _tool: GetUiStateTool = Default::default();
    }
}
