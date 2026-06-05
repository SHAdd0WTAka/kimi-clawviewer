//! cv-capture – Windows screen capture via DXGI Desktop Duplication API
//!
//! This crate provides hardware-accelerated screen capture on Windows
//! using the DXGI Desktop Duplication API.  It wraps D3D11 texture
//! read-back and exposes both a synchronous [`DxgiCapturer`] and an
//! asynchronous [`capture_stream`] that feeds frames through a Tokio
//! channel.
//!
//! # Quick start
//! ```no_run
//! use cv_capture::dxgi::DxgiCapturer;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut capturer = DxgiCapturer::new(0)?; // primary display
//! if let Some(frame) = capturer.capture_frame()? {
//!     println!("Captured {}x{} frame ({} bytes)",
//!              frame.width, frame.height, frame.data.len());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Platform support
//! | Platform | Behaviour |
//! |----------|-----------|
//! | Windows 10/11 | Full DXGI Desktop Duplication |
//! | Linux / macOS | Stub – compiles, returns empty frames |
//!
//! # Feature flags
//! None currently – all functionality is enabled unconditionally.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod dxgi;
pub mod frame;

// Convenience re-exports so `use cv_capture::{Frame, DxgiCapturer}` works.
pub use dxgi::{capture_stream, DxgiCapturer, Rect};
pub use frame::Frame;
