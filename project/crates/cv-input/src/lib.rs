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
//! | Linux    | Stub (`todo!()`) – planned via XTest / uinput |
//! | macOS    | Stub (`todo!()`) – planned via CGEvent |
//!
//! ## Modules
//!
//! * [`types`] – Shared input event types (`MouseButton`, `InputEvent`, …).
//! * [`queue`] – [`PriorityInputQueue`] with P0-P3 priority levels and emergency stop.
//! * [`windows`] – [`windows::InputInjector`] using Win32 `SendInput` (Windows only).

pub mod queue;
pub mod types;

// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

// ---------------------------------------------------------------------------
// Re-exports for convenience
// ---------------------------------------------------------------------------

pub use cv_shared::types::{InputEvent, MouseButton, Priority};
pub use queue::PriorityInputQueue;

/// Platform-specific input injector.
///
/// * **Windows**: re-exports [`windows::InputInjector`].
/// * **Linux** / **macOS**: re-exports the corresponding stub.
#[cfg(target_os = "windows")]
pub use windows::InputInjector as PlatformInjector;

#[cfg(target_os = "linux")]
pub use linux::InputInjector as PlatformInjector;

#[cfg(target_os = "macos")]
pub use macos::InputInjector as PlatformInjector;

// ---------------------------------------------------------------------------
// Platform stubs (Linux / macOS)
// ---------------------------------------------------------------------------

/// Linux stub module – panics with `todo!()` for all operations.
#[cfg(target_os = "linux")]
pub mod linux {
    use cv_shared::CvResult;
    use super::types::MouseButton;
    use tracing::warn;

    /// Linux input injector stub.
    #[derive(Debug, Clone)]
    pub struct InputInjector;

    impl InputInjector {
        /// Create a new stub injector.
        pub fn new() -> Self {
            warn!("Linux InputInjector is a stub – real implementation needs XTest/uinput");
            Self
        }

        /// Stub: `todo!("Implement via XTest or evdev")`
        pub fn move_mouse(&self, _x: i32, _y: i32) -> CvResult<()> {
            todo!("Linux mouse move not yet implemented (needs XTest or evdev)")
        }

        /// Stub: `todo!("Implement via XTest")`
        pub fn click(&self, _button: MouseButton, _down: bool) -> CvResult<()> {
            todo!("Linux mouse click not yet implemented (needs XTest)")
        }

        /// Stub: `todo!("Implement via XTest")`
        pub fn scroll(&self, _delta: i32) -> CvResult<()> {
            todo!("Linux mouse scroll not yet implemented (needs XTest)")
        }

        /// Stub: `todo!("Implement via XTest")`
        pub fn key_press(&self, _keycode: u16, _down: bool) -> CvResult<()> {
            todo!("Linux key press not yet implemented (needs XTest)")
        }

        /// Stub: `todo!("Implement via XTest")`
        pub fn type_text(&self, _text: &str) -> CvResult<()> {
            todo!("Linux text typing not yet implemented (needs XTest)")
        }
    }

    impl Default for InputInjector {
        fn default() -> Self {
            Self::new()
        }
    }
}

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
        // Verify that InputEvent, MouseButton, Priority and PriorityInputQueue
        // can be used directly from the crate root.
        let _ev = InputEvent::new(
            EventSource::Human,
            EventType::MouseClick {
                button: MouseButton::Left,
                down: true,
            },
            Priority::P1_Human,
            0,
        );
        let _queue = PriorityInputQueue::new();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn platform_injector_is_windows() {
        // On Windows PlatformInjector == windows::InputInjector
        let _: PlatformInjector = windows::InputInjector::new();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn platform_injector_is_linux_stub() {
        // On Linux it should compile as a stub.
        let _ = linux::InputInjector::new();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn platform_injector_is_macos_stub() {
        // On macOS it should compile as a stub.
        let _ = macos::InputInjector::new();
    }
}
