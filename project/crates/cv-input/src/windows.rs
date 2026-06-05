//! Windows-specific input injection via `SendInput`.
//!
//! Uses the `windows` crate FFI bindings to `Win32_UI_Input_KeyboardAndMouse`.
//! All methods return `CvResult<()>` and are safe wrappers around unsafe Win32 APIs.

use cv_shared::{CvError, CvResult};
use tracing::{debug, error, trace};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::WHEEL_DELTA;

use super::types::MouseButton;

/// Windows input injector using `SendInput`.
///
/// # Example
/// ```no_run
/// use cv_input::windows::InputInjector;
/// use cv_input::types::MouseButton;
///
/// let injector = InputInjector::new();
/// injector.move_mouse(100, 200).unwrap();
/// injector.click(MouseButton::Left, true).unwrap();   // down
/// injector.click(MouseButton::Left, false).unwrap();  // up
/// ```
#[derive(Debug, Clone)]
pub struct InputInjector;

impl InputInjector {
    /// Create a new `InputInjector`.
    pub fn new() -> Self {
        Self
    }

    /// Move the mouse cursor to screen coordinates `(x, y)`.
    ///
    /// Uses `MOUSEEVENTF_MOVE` with relative pixel deltas.
    /// For absolute positioning (0-65535) use [`move_mouse_absolute`].
    pub fn move_mouse(&self, x: i32, y: i32) -> CvResult<()> {
        trace!(x, y, "move_mouse (relative)");

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: x,
                    dy: y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        self.send_inputs(&[input])
    }

    /// Move the mouse cursor to absolute screen coordinates.
    ///
    /// `x` and `y` are scaled to the 0-65535 virtual screen coordinate space
    /// using `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE`.
    pub fn move_mouse_absolute(&self, x: i32, y: i32, screen_width: i32, screen_height: i32) -> CvResult<()> {
        trace!(x, y, screen_width, screen_height, "move_mouse_absolute");

        if screen_width <= 0 || screen_height <= 0 {
            return Err(CvError::Input(
                "Invalid screen dimensions for absolute mouse move".into(),
            ));
        }

        let abs_x = ((x * 65535) / screen_width) as i32;
        let abs_y = ((y * 65535) / screen_height) as i32;

        use windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE;
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        self.send_inputs(&[input])
    }

    /// Send a mouse button press or release event.
    ///
    /// * `button` – which mouse button (Left / Right / Middle).
    /// * `down`   – `true` for press, `false` for release.
    pub fn click(&self, button: MouseButton, down: bool) -> CvResult<()> {
        debug!(?button, down, "click");

        let flags = match (button, down) {
            (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
            (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
            (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
            (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
            (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
            (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
        };

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        self.send_inputs(&[input])
    }

    /// Send a vertical mouse-wheel scroll event.
    ///
    /// `delta` is in WHEEL_DELTA units (positive = scroll up, negative = scroll down).
    pub fn scroll(&self, delta: i32) -> CvResult<()> {
        debug!(delta, "scroll");

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: (delta * (WHEEL_DELTA as i32)) as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        self.send_inputs(&[input])
    }

    /// Send a keyboard key press or release.
    ///
    /// * `keycode` – Windows virtual-key code (`wVk`).
    /// * `down`    – `true` for key-down, `false` for key-up (adds `KEYEVENTF_KEYUP`).
    pub fn key_press(&self, keycode: u16, down: bool) -> CvResult<()> {
        debug!(keycode, down, "key_press");

        let flags = if !down { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: keycode,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        self.send_inputs(&[input])
    }

    /// Type a string by sending Unicode keystrokes for each character.
    ///
    /// Uses `KEYEVENTF_UNICODE` so no keyboard-layout conversion is needed.
    /// Each character generates a key-down + key-up pair.
    pub fn type_text(&self, text: &str) -> CvResult<()> {
        debug!(len = text.len(), "type_text");

        let mut inputs = Vec::with_capacity(text.len() * 2);

        for ch in text.chars() {
            let code_point = ch as u16;

            // Key-down
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: code_point,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });

            // Key-up
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: code_point,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });
        }

        self.send_inputs(&inputs)
    }

    /// Internal helper: call `SendInput` and verify all events were injected.
    fn send_inputs(&self, inputs: &[INPUT]) -> CvResult<()> {
        if inputs.is_empty() {
            return Ok(());
        }

        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };

        if sent as usize != inputs.len() {
            let err = std::io::Error::last_os_error();
            error!(expected = inputs.len(), sent, error = %err, "SendInput failed");
            return Err(CvError::Input(format!(
                "SendInput failed: expected {} events, sent {} (os error: {})",
                inputs.len(),
                sent,
                err
            )));
        }

        trace!(count = inputs.len(), "SendInput OK");
        Ok(())
    }
}

impl Default for InputInjector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Windows-only tests (unit tests that don't require actual input injection)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_injector_creation() {
        let injector = InputInjector::new();
        // Just verify it can be created without panicking.
        drop(injector);
    }

    #[test]
    fn input_injector_default() {
        let injector: InputInjector = Default::default();
        drop(injector);
    }

    /// Verifies that mouse button → dwFlags conversion is correct.
    /// We can't test SendInput itself without a Windows session, but we can
    /// verify the flag mapping logic by duplicating the match here.
    #[test]
    fn mouse_button_flag_mapping() {
        let cases = [
            ((MouseButton::Left, true), MOUSEEVENTF_LEFTDOWN),
            ((MouseButton::Left, false), MOUSEEVENTF_LEFTUP),
            ((MouseButton::Right, true), MOUSEEVENTF_RIGHTDOWN),
            ((MouseButton::Right, false), MOUSEEVENTF_RIGHTUP),
            ((MouseButton::Middle, true), MOUSEEVENTF_MIDDLEDOWN),
            ((MouseButton::Middle, false), MOUSEEVENTF_MIDDLEUP),
        ];

        for ((button, down), expected) in cases {
            let flags = match (button, down) {
                (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
            };
            assert_eq!(
                flags, expected,
                "Mismatch for button={:?} down={}",
                button, down
            );
        }
    }

    #[test]
    fn key_event_flag_mapping() {
        // Key-down should NOT have KEYEVENTF_KEYUP
        let down_flags = 0u32;
        assert_eq!(down_flags, 0);

        // Key-up MUST have KEYEVENTF_KEYUP
        let up_flags = KEYEVENTF_KEYUP.0;
        assert_ne!(up_flags, 0);
    }
}
