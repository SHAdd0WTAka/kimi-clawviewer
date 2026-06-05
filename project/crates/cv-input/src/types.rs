//! Input event type definitions for `cv-input`.
//!
//! Re-exports from `cv-shared::types` plus `cv-input`-specific extensions
//! (platform stub traits, helper conversions, etc.).

pub use cv_shared::types::{EventSource, EventType, InputEvent, MouseButton, Priority};

/// Trait for platform-specific input injection.
///
/// Windows: implemented by [`super::windows::InputInjector`].
/// Linux / macOS: stub implementations that panic with `todo!()`.
pub trait InputInject {
    /// Move the mouse cursor to the given screen coordinates.
    fn move_mouse(&self, x: i32, y: i32) -> cv_shared::CvResult<()>;

    /// Send a mouse button down/up event.
    fn click(&self, button: MouseButton, down: bool) -> cv_shared::CvResult<()>;

    /// Send a mouse-wheel scroll event.
    fn scroll(&self, delta: i32) -> cv_shared::CvResult<()>;

    /// Send a single key press / release.
    fn key_press(&self, keycode: u16, down: bool) -> cv_shared::CvResult<()>;

    /// Type a string of Unicode text.
    fn type_text(&self, text: &str) -> cv_shared::CvResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_button_equality() {
        assert_eq!(MouseButton::Left, MouseButton::Left);
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_ne!(MouseButton::Left, MouseButton::Middle);
    }

    #[test]
    fn priority_ordering() {
        use cv_shared::types::Priority::*;
        // P0 has highest priority, so it should be "greater" in the ordering
        // (BinaryHeap pops max first, and we want P0 first)
        assert!(P0_Emergency > P1_Human);
        assert!(P1_Human > P2_AI_Confirmed);
        assert!(P2_AI_Confirmed > P3_AI_Autonomous);
    }

    #[test]
    fn input_event_construction() {
        let ev = InputEvent {
            source: EventSource::Human,
            event_type: EventType::MouseMove { x: 100, y: 200 },
            priority: Priority::P1_Human,
            payload: cv_shared::types::EventPayload::empty(),
            timestamp: 0,
            sequence: 42,
        };
        assert!(matches!(ev.source, EventSource::Human));
        assert!(matches!(ev.event_type, EventType::MouseMove { x: 100, y: 200 }));
        assert!(matches!(ev.priority, Priority::P1_Human));
        assert_eq!(ev.sequence, 42);
    }
}
