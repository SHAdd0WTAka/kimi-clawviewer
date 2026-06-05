//! Linux input injection via evdev (uinput)
//!
//! Provides real input injection on Linux using the uinput kernel interface.
//! This allows simulating mouse and keyboard events without X11.

use cv_shared::CvResult;
use cv_shared::types::MouseButton;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use tracing::{debug, warn};

// evdev/uinput constants
const UI_DEV_CREATE: u64 = 0x5501;
const UI_DEV_DESTROY: u64 = 0x5502;
const UI_SET_EVBIT: u64 = 0x40045564;
const UI_SET_KEYBIT: u64 = 0x40045565;
const UI_SET_RELBIT: u64 = 0x40045566;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_WHEEL: u16 = 0x08;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const SYN_REPORT: u16 = 0x00;

#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

/// Linux input injector using uinput.
#[derive(Debug)]
pub struct InputInjector {
    fd: File,
}

impl InputInjector {
    /// Create a new uinput device.
    pub fn new() -> CvResult<Self> {
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| cv_shared::CvError::Input(format!("Failed to open /dev/uinput: {e}")))?;

        unsafe {
            // Enable event types
            Self::ioctl(&fd, UI_SET_EVBIT, EV_KEY as u64)?;
            Self::ioctl(&fd, UI_SET_EVBIT, EV_REL as u64)?;
            Self::ioctl(&fd, UI_SET_EVBIT, EV_SYN as u64)?;

            // Enable relative axes
            Self::ioctl(&fd, UI_SET_RELBIT, REL_X as u64)?;
            Self::ioctl(&fd, UI_SET_RELBIT, REL_Y as u64)?;
            Self::ioctl(&fd, UI_SET_RELBIT, REL_WHEEL as u64)?;

            // Enable mouse buttons
            Self::ioctl(&fd, UI_SET_KEYBIT, BTN_LEFT as u64)?;
            Self::ioctl(&fd, UI_SET_KEYBIT, BTN_RIGHT as u64)?;
            Self::ioctl(&fd, UI_SET_KEYBIT, BTN_MIDDLE as u64)?;

            // Enable common keys (0-255)
            for key in 0..256u16 {
                let _ = Self::ioctl(&fd, UI_SET_KEYBIT, key as u64);
            }

            // Setup device
            let setup = UinputSetup {
                id: InputId {
                    bustype: 0x03, // BUS_USB
                    vendor: 0x1234,
                    product: 0x5678,
                    version: 1,
                },
                name: Self::str_to_name("ClawViewer Input"),
                ff_effects_max: 0,
            };

            let ret = libc::write(
                fd.as_raw_fd(),
                &setup as *const _ as *const libc::c_void,
                std::mem::size_of::<UinputSetup>(),
            );
            if ret < 0 {
                return Err(cv_shared::CvError::Input("uinput setup failed".into()));
            }

            // Create device
            let ret = libc::ioctl(fd.as_raw_fd(), UI_DEV_CREATE, 0);
            if ret < 0 {
                return Err(cv_shared::CvError::Input("uinput create failed".into()));
            }
        }

        debug!("Linux InputInjector created via uinput");
        Ok(Self { fd })
    }

    /// Move mouse relative to current position.
    pub fn move_mouse(&self, x: i32, y: i32) -> CvResult<()> {
        self.send_event(EV_REL, REL_X, x)?;
        self.send_event(EV_REL, REL_Y, y)?;
        self.sync()?;
        debug!("Mouse moved: x={}, y={}", x, y);
        Ok(())
    }

    /// Send mouse button press/release.
    pub fn click(&self, button: MouseButton, down: bool) -> CvResult<()> {
        let code = match button {
            MouseButton::Left => BTN_LEFT,
            MouseButton::Right => BTN_RIGHT,
            MouseButton::Middle => BTN_MIDDLE,
        };
        let value = if down { 1 } else { 0 };
        self.send_event(EV_KEY, code, value)?;
        self.sync()?;
        debug!("Mouse click: button={:?}, down={}", button, down);
        Ok(())
    }

    /// Scroll wheel.
    pub fn scroll(&self, delta: i32) -> CvResult<()> {
        self.send_event(EV_REL, REL_WHEEL, delta)?;
        self.sync()?;
        debug!("Mouse scroll: delta={}", delta);
        Ok(())
    }

    /// Send key press/release.
    pub fn key_press(&self, keycode: u16, down: bool) -> CvResult<()> {
        let value = if down { 1 } else { 0 };
        self.send_event(EV_KEY, keycode, value)?;
        self.sync()?;
        debug!("Key press: keycode={}, down={}", keycode, down);
        Ok(())
    }

    /// Type text (basic ASCII only).
    pub fn type_text(&self, text: &str) -> CvResult<()> {
        for ch in text.chars() {
            if let Some(keycode) = Self::char_to_keycode(ch) {
                self.key_press(keycode, true)?;
                self.key_press(keycode, false)?;
            }
        }
        debug!("Typed text: {}", text);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn send_event(&self, type_: u16, code: u16, value: i32) -> CvResult<()> {
        let ev = InputEvent {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        unsafe {
            let ret = libc::write(
                self.fd.as_raw_fd(),
                &ev as *const _ as *const libc::c_void,
                std::mem::size_of::<InputEvent>(),
            );
            if ret < 0 {
                return Err(cv_shared::CvError::Input("Failed to write event".into()));
            }
        }
        Ok(())
    }

    fn sync(&self) -> CvResult<()> {
        self.send_event(EV_SYN, SYN_REPORT, 0)
    }

    unsafe fn ioctl(fd: &File, request: u64, arg: u64) -> CvResult<()> {
        let ret = libc::ioctl(fd.as_raw_fd(), request, arg);
        if ret < 0 {
            return Err(cv_shared::CvError::Input(format!(
                "ioctl failed: request={:x}, arg={:x}",
                request, arg
            )));
        }
        Ok(())
    }

    fn str_to_name(s: &str) -> [u8; 80] {
        let mut name = [0u8; 80];
        let bytes = s.as_bytes();
        let len = bytes.len().min(79);
        name[..len].copy_from_slice(&bytes[..len]);
        name
    }

    fn char_to_keycode(ch: char) -> Option<u16> {
        match ch {
            'a'..='z' => Some((ch as u16 - 'a' as u16) + 30),
            'A'..='Z' => Some((ch as u16 - 'A' as u16) + 30),
            '1'..='9' => Some((ch as u16 - '1' as u16) + 2),
            '0' => Some(11),
            ' ' => Some(57),
            '\n' => Some(28),
            _ => None,
        }
    }
}

impl Drop for InputInjector {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::ioctl(self.fd.as_raw_fd(), UI_DEV_DESTROY, 0);
        }
        debug!("Linux InputInjector destroyed");
    }
}

impl Default for InputInjector {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            warn!("Failed to create InputInjector: {}, using dummy", e);
            panic!("uinput not available");
        })
    }
}
