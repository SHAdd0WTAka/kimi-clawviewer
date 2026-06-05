//! # cv-input – Input Injection and Event Prioritisation
//!
//! This crate provides platform-specific input injection (mouse, keyboard)
//! and a thread-safe priority queue for ordering input events.
//!
//! ## Platform support
//!
//! | Platform | Status |
//! |----------|--------|
//! | Windows  | Full `SendInput` implementation |
//! | Linux    | Real implementation via uinput/evdev |
//! | macOS    | Stub (`todo!()`) – planned via CGEvent |
//!
//! ## Modules
//!
//! * [`types`] – Shared input event types (`MouseButton`, `InputEvent`, …).
//! * [`queue`] – [`PriorityInputQueue`] with P0-P3 priority levels and emergency stop.
//! * [`windows`] – [`windows::InputInjector`] using Win32 `SendInput` (Windows only).
//! * [`linux`] – [`linux::InputInjector`] using uinput (Linux only).

pub mod queue;
pub mod types;

// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// ---------------------------------------------------------------------------
// Re-exports for convenience
// ---------------------------------------------------------------------------

pub use cv_shared::types::{InputEvent, MouseButton, Priority};
pub use queue::PriorityInputQueue;

/// Platform-specific input injector.
///
/// * **Windows**: re-exports [`windows::InputInjector`].
/// * **Linux**: re-exports [`linux::InputInjector`].
/// * **macOS**: re-exports the corresponding stub.
#[cfg(target_os = "windows")]
pub use windows::InputInjector as PlatformInjector;

#[cfg(target_os = "linux")]
pub use linux::InputInjector as PlatformInjector;

#[cfg(target_os = "macos")]
pub use macos::InputInjector as PlatformInjector;

// ---------------------------------------------------------------------------
// Platform stubs (macOS only)
// ---------------------------------------------------------------------------

/// macOS stub module – panics with `todo!()` for all operations.
#[cfg(target_os = "macos")]
pub mod macos {
    use cv_shared::CvResult;
    use super::types::MouseButton;
    use tracing::warn;

    /// macOS input injector stub.
    #[derive(Debug, Clone)]
    pub struct InputInjector;

    impl InputInjector {
        /// Create a new stub injector.
        pub fn new() -> Self {
            warn!("macOS InputInjector is a stub – real implementation needs CGEvent");
            Self
        }

        /// Stub: `todo!("Implement via CGEvent")`
        pub fn move_mouse(&self, _x: i32, _y: i32) -> CvResult<()> {
            todo!("macOS mouse move not yet implemented (needs CGEvent)")
        }

        /// Stub: `todo!("Implement via CGEvent")`
        pub fn click(&self, _button: MouseButton, _down: bool) -> CvResult<()> {
            todo!("macOS mouse click not yet implemented (needs CGEvent)")
        }

        /// Stub: `todo!("Implement via CGEvent")`
        pub fn scroll(&self, _delta: i32) -> CvResult<()> {
            todo!("macOS mouse scroll not yet implemented (needs CGEvent)")
        }

        /// Stub: `todo!("Implement via CGEvent")`
        pub fn key_press(&self, _keycode: u16, _down: bool) -> CvResult<()> {
            todo!("macOS key press not yet implemented (needs CGEvent)")
        }

        /// Stub: `todo!("Implement via CGEvent")`
        pub fn type_text(&self, _text: &str) -> CvResult<()> {
            todo!("macOS text typing not yet implemented (needs CGEvent)")
        }
    }

    impl Default for InputInjector {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-platform smoke tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cv_shared::types::{EventSource, EventType, InputEvent};

    #[test]
    fn reexports_are_available() {
        let _ev = InputEvent {
            source: EventSource::Human,
            event_type: EventType::MouseClick {
                button: MouseButton::Left,
                down: true,
            },
            priority: Priority::P1_Human,
            payload: cv_shared::types::EventPayload::empty(),
            timestamp: 0,
            sequence: 0,
        };
        let _queue = PriorityInputQueue::new();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn platform_injector_is_windows() {
        let _: PlatformInjector = windows::InputInjector::new();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn platform_injector_is_linux() {
        let _ = linux::InputInjector::new();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn platform_injector_is_macos_stub() {
        let _ = macos::InputInjector::new();
    }
}
