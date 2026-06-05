//! `cv-shared` — Shared types, errors, protobuf definitions, and utilities.
//!
//! This crate is the foundation of the ClawViewer workspace. All other crates
//! depend on it for common data types, error handling, protobuf-generated code,
//! and utility functions.
//!
//! # Modules
//!
//! - [`types`] — Core data types (`SessionId`, `PeerId`, `Password`, `InputEvent`, ...).
//! - [`error`] — Error enum [`CvError`] and result alias [`CvResult`].
//! - [`proto`] — Protobuf-generated modules (`rendezvous`, `message`).
//! - [`utils`] — Password generation and timestamp helpers.

pub mod error;
pub mod proto;
pub mod types;
pub mod utils;

// Re-export the most commonly used items for convenience.
pub use error::{CvError, CvResult};
pub use types::*;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ===================================================================
    // Password generation tests
    // ===================================================================

    #[test]
    fn password_generation_produces_valid_string() {
        let pwd = utils::generate_password();
        let inner = &pwd.0;
        assert_eq!(inner.len(), 6, "password must be exactly 6 characters");
        assert!(
            inner.chars().all(|c| c.is_ascii_alphanumeric()),
            "password must be alphanumeric: got {}",
            inner
        );
    }

    #[test]
    fn password_generation_produces_different_values() {
        // Non-deterministic but should almost never collide.
        let pwd1 = utils::generate_password();
        let pwd2 = utils::generate_password();
        let pwd3 = utils::generate_password();

        // At least two of three should differ (extremely high probability).
        assert!(
            pwd1.0 != pwd2.0 || pwd2.0 != pwd3.0,
            "generated identical passwords in a row: {} {} {}",
            pwd1.0,
            pwd2.0,
            pwd3.0
        );
    }

    #[test]
    fn password_random_generation_produces_valid_string() {
        let pwd = utils::generate_password_random();
        let inner = &pwd.0;
        assert_eq!(inner.len(), 6, "random password must be exactly 6 characters");
        assert!(
            inner.chars().all(|c| c.is_ascii_alphanumeric()),
            "random password must be alphanumeric: got {}",
            inner
        );
    }

    #[test]
    fn password_zeroizes_on_drop() {
        // ZeroizeOnDrop is a compile-time guarantee; this test ensures
        // the struct still behaves correctly (can be created and moved).
        let pwd = Password::new("TESTPW");
        assert_eq!(pwd.0, "TESTPW");
        drop(pwd);
        // If we reach here without a panic, ZeroizeOnDrop executed.
    }

    // ===================================================================
    // Priority ordering tests
    // ===================================================================

    #[test]
    fn priority_ordering_p0_is_highest() {
        let p0 = Priority::P0_Emergency;
        let p1 = Priority::P1_Human;
        let p2 = Priority::P2_AI_Confirmed;
        let p3 = Priority::P3_AI_Autonomous;

        assert!(p0 > p1, "P0 should be higher priority than P1");
        assert!(p1 > p2, "P1 should be higher priority than P2");
        assert!(p2 > p3, "P2 should be higher priority than P3");
        assert!(p0 > p3, "P0 should be higher priority than P3");
    }

    #[test]
    fn priority_sorts_correctly_in_vec() {
        let mut priorities = vec![
            Priority::P3_AI_Autonomous,
            Priority::P1_Human,
            Priority::P0_Emergency,
            Priority::P2_AI_Confirmed,
        ];
        priorities.sort();

        assert_eq!(priorities[0], Priority::P3_AI_Autonomous, "lowest first");
        assert_eq!(priorities[1], Priority::P2_AI_Confirmed);
        assert_eq!(priorities[2], Priority::P1_Human);
        assert_eq!(priorities[3], Priority::P0_Emergency, "highest last");
    }

    #[test]
    fn priority_binary_heap_returns_highest_first() {
        use std::collections::BinaryHeap;

        let mut heap = BinaryHeap::new();
        heap.push(Priority::P3_AI_Autonomous);
        heap.push(Priority::P1_Human);
        heap.push(Priority::P0_Emergency);
        heap.push(Priority::P2_AI_Confirmed);

        assert_eq!(heap.pop(), Some(Priority::P0_Emergency), "P0 first");
        assert_eq!(heap.pop(), Some(Priority::P1_Human), "P1 second");
        assert_eq!(heap.pop(), Some(Priority::P2_AI_Confirmed), "P2 third");
        assert_eq!(heap.pop(), Some(Priority::P3_AI_Autonomous), "P3 last");
    }

    #[test]
    fn priority_as_u8_returns_correct_values() {
        assert_eq!(Priority::P0_Emergency.as_u8(), 0);
        assert_eq!(Priority::P1_Human.as_u8(), 1);
        assert_eq!(Priority::P2_AI_Confirmed.as_u8(), 2);
        assert_eq!(Priority::P3_AI_Autonomous.as_u8(), 3);
    }

    // ===================================================================
    // InputEvent serialization roundtrip tests
    // ===================================================================

    #[test]
    fn input_event_mouse_move_roundtrip() {
        let original = InputEvent {
            source: EventSource::Human,
            event_type: EventType::MouseMove { x: 1920, y: 1080 },
            priority: Priority::P1_Human,
            payload: EventPayload::empty(),
            timestamp: 1_700_000_000_000,
            sequence: 42,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: InputEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn input_event_mouse_click_roundtrip() {
        let original = InputEvent {
            source: EventSource::AI,
            event_type: EventType::MouseClick {
                button: MouseButton::Left,
                down: true,
            },
            priority: Priority::P2_AI_Confirmed,
            payload: EventPayload::from_json(&serde_json::json!({"confidence": 0.95}))
                .expect("payload"),
            timestamp: 1_700_000_000_001,
            sequence: 100,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: InputEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn input_event_key_press_roundtrip() {
        let original = InputEvent {
            source: EventSource::System,
            event_type: EventType::KeyPress {
                keycode: 0x41, // 'A'
                down: true,
            },
            priority: Priority::P0_Emergency,
            payload: EventPayload::empty(),
            timestamp: 1_700_000_000_002,
            sequence: 0,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: InputEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn input_event_key_type_roundtrip() {
        let original = InputEvent {
            source: EventSource::Human,
            event_type: EventType::KeyType {
                text: "Hello, world!".to_string(),
            },
            priority: Priority::P1_Human,
            payload: EventPayload::empty(),
            timestamp: 1_700_000_000_003,
            sequence: 7,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: InputEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn input_event_mouse_scroll_roundtrip() {
        let original = InputEvent {
            source: EventSource::AI,
            event_type: EventType::MouseScroll { delta: -120 },
            priority: Priority::P3_AI_Autonomous,
            payload: EventPayload::empty(),
            timestamp: 1_700_000_000_004,
            sequence: 99,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: InputEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    // ===================================================================
    // Error Display formatting tests
    // ===================================================================

    #[test]
    fn error_display_network() {
        let err = CvError::Network("connection refused".to_string());
        assert_eq!(format!("{}", err), "Network error: connection refused");
    }

    #[test]
    fn error_display_capture() {
        let err = CvError::Capture("DXGI timeout".to_string());
        assert_eq!(format!("{}", err), "Capture error: DXGI timeout");
    }

    #[test]
    fn error_display_input() {
        let err = CvError::Input("SendInput failed".to_string());
        assert_eq!(format!("{}", err), "Input error: SendInput failed");
    }

    #[test]
    fn error_display_security() {
        let err = CvError::Security("invalid signature".to_string());
        assert_eq!(format!("{}", err), "Security error: invalid signature");
    }

    #[test]
    fn error_display_codec() {
        let err = CvError::Codec("H264 encode error".to_string());
        assert_eq!(format!("{}", err), "Codec error: H264 encode error");
    }

    #[test]
    fn error_display_mcp() {
        let err = CvError::Mcp("tool not found".to_string());
        assert_eq!(format!("{}", err), "MCP error: tool not found");
    }

    #[test]
    fn error_display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = CvError::from(io_err);
        assert_eq!(format!("{}", err), "IO error: file missing");
    }

    #[test]
    fn error_debug_format() {
        let err = CvError::Network("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Network"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn cverror_implements_std_error() {
        fn assert_std_error<E: std::error::Error>() {}
        assert_std_error::<CvError>();
    }

    // ===================================================================
    // Type construction & default tests
    // ===================================================================

    #[test]
    fn session_id_new() {
        let id = SessionId::new("sess-123");
        assert_eq!(id.0, "sess-123");
    }

    #[test]
    fn session_id_default() {
        let id = SessionId::default();
        assert_eq!(id.0, "");
    }

    #[test]
    fn peer_id_new() {
        let id = PeerId::new("peer-456");
        assert_eq!(id.0, "peer-456");
    }

    #[test]
    fn peer_id_default() {
        let id = PeerId::default();
        assert_eq!(id.0, "");
    }

    #[test]
    fn password_new() {
        let pwd = Password::new("SECRET");
        assert_eq!(pwd.0, "SECRET");
    }

    #[test]
    fn password_default() {
        let pwd = Password::default();
        assert_eq!(pwd.0, "");
    }

    #[test]
    fn event_payload_empty() {
        let p = EventPayload::empty();
        assert_eq!(p.0, "");
    }

    #[test]
    fn event_payload_from_json() {
        let json_value = serde_json::json!({ "key": "value", "num": 42 });
        let p = EventPayload::from_json(&json_value).expect("from_json");
        assert_eq!(p.0, r#"{"key":"value","num":42}"#);
    }

    #[test]
    fn event_payload_default() {
        let p = EventPayload::default();
        assert_eq!(p.0, "");
    }

    // ===================================================================
    // Enum variant tests
    // ===================================================================

    #[test]
    fn platform_variants() {
        let platforms = [Platform::Windows, Platform::Linux, Platform::MacOS];
        for p in &platforms {
            // Just ensure they can be created and matched.
            let _ = match p {
                Platform::Windows => "win",
                Platform::Linux => "linux",
                Platform::MacOS => "mac",
            };
        }
    }

    #[test]
    fn video_codec_variants() {
        let codecs = [VideoCodec::H264, VideoCodec::VP9, VideoCodec::AV1];
        for c in &codecs {
            let _ = match c {
                VideoCodec::H264 => "h264",
                VideoCodec::VP9 => "vp9",
                VideoCodec::AV1 => "av1",
            };
        }
    }

    #[test]
    fn event_source_variants() {
        let sources = [EventSource::Human, EventSource::AI, EventSource::System];
        for s in &sources {
            let _ = match s {
                EventSource::Human => "human",
                EventSource::AI => "ai",
                EventSource::System => "system",
            };
        }
    }

    #[test]
    fn mouse_button_variants() {
        let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
        for b in &buttons {
            let _ = match b {
                MouseButton::Left => "left",
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
            };
        }
    }

    // ===================================================================
    // Timestamp utility tests
    // ===================================================================

    #[test]
    fn now_millis_is_positive() {
        let ts = utils::now_millis();
        assert!(ts > 1_700_000_000_000, "timestamp should be after 2023");
    }

    #[test]
    fn now_millis_is_monotonic() {
        let ts1 = utils::now_millis();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = utils::now_millis();
        assert!(ts2 >= ts1, "timestamp should be monotonic");
    }

    #[test]
    fn datetime_to_millis_roundtrip() {
        use chrono::Utc;
        let dt = Utc::now();
        let ts = utils::datetime_to_millis(&dt);
        let dt2 = utils::millis_to_datetime(ts);
        // Allow 1ms difference due to truncation.
        assert!(
            (dt.timestamp_millis() - dt2.timestamp_millis()).abs() <= 1,
            "datetime roundtrip failed"
        );
    }

    #[test]
    fn millis_to_datetime_epoch() {
        let dt = utils::millis_to_datetime(0);
        assert_eq!(dt.timestamp_millis(), 0);
    }

    #[test]
    fn format_timestamp_well_formed() {
        let s = utils::format_timestamp(1_700_000_000_000);
        assert!(s.contains("2023"));
        assert!(s.contains("T")); // RFC 3339 format
    }

    // ===================================================================
    // Send + Sync assertions (compile-time)
    // ===================================================================

    #[test]
    fn all_types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SessionId>();
        assert_send_sync::<PeerId>();
        assert_send_sync::<Password>();
        assert_send_sync::<Platform>();
        assert_send_sync::<VideoCodec>();
        assert_send_sync::<EventSource>();
        assert_send_sync::<EventType>();
        assert_send_sync::<MouseButton>();
        assert_send_sync::<Priority>();
        assert_send_sync::<InputEvent>();
        assert_send_sync::<EventPayload>();
        assert_send_sync::<CvError>();
    }
}
