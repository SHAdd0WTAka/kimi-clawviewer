//! DXGI Desktop Duplication API screen capture (Windows)
//!
//! Provides [`DxgiCapturer`] which wraps the DXGI Desktop Duplication API
//! to capture the screen efficiently with GPU-assisted texture read-back.
//!
//! # Platform support
//! * **Windows** – full DXGI Desktop Duplication implementation.
//! * **Linux / macOS** – stub that compiles but always returns [`None`] from
//!   [`DxgiCapturer::capture_frame`].

use crate::frame::Frame;
use std::time::{Duration, Instant};

// Re-export Rect so consumers can build dirty regions without importing cv_shared directly.
pub use cv_shared::Rect;

// ===========================================================================
// Windows implementation
// ===========================================================================

#[cfg(target_os = "windows")]
mod platform {
    pub use super::*;
    pub use std::sync::Arc;
    pub use tokio::sync::mpsc;
    pub use tracing::{debug, error, trace, warn};
    pub use windows::core::ComInterface;
    pub use windows::Win32::Foundation::RECT;
    pub use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG, D3D11_MAPPED_SUBRESOURCE,
        D3D11_RESOURCE_MISC_GDI_COMPATIBLE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    pub use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
    };
    pub use windows::Win32::Graphics::Dxgi::{
        IDXGIAdapter, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_INVALID_CALL, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE,
        DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO,
        DXGI_OUTPUT_DESC, DXGI_RESOURCE_PRIORITY_MAXIMUM,
    };
    pub use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;

    /// Hardware-accelerated screen capturer using DXGI Desktop Duplication.
    ///
    /// # Example
    /// ```no_run
    /// # use cv_capture::dxgi::DxgiCapturer;
    /// let mut cap = DxgiCapturer::new(0).unwrap();
    /// let frame = cap.capture_frame().unwrap();
    /// ```
    pub struct DxgiCapturer {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        duplication: Option<IDXGIOutputDuplication>,
        staging_texture: Option<ID3D11Texture2D>,
        width: u32,
        height: u32,
        pitch: u32,
        display_index: u32,
        frame_count: u64,
    }

    // SAFETY: COM interfaces are thread-safe via reference counting.
    unsafe impl Send for DxgiCapturer {}
    unsafe impl Sync for DxgiCapturer {}

    impl DxgiCapturer {
        const ACQUIRE_TIMEOUT_MS: u32 = 500;

        /// Create a new capturer for the given display index.
        ///
        /// # Arguments
        /// * `display_index` – Zero-based display number (0 = primary).
        ///
        /// # Errors
        /// Returns `CvError::Capture` if the D3D11 device cannot be created,
        /// the display index is invalid, or Desktop Duplication is unavailable.
        pub fn new(display_index: u32) -> cv_shared::CvResult<Self> {
            debug!(display_index, "Creating DxgiCapturer");

            let mut device = None;
            let mut feature_level = Default::default();

            unsafe {
                D3D11CreateDevice(
                    None, // adapter – let D3D pick the best one
                    D3D_DRIVER_TYPE_HARDWARE,
                    None, // software rasterizer
                    D3D11_CREATE_DEVICE_FLAG(0),
                    None, // default feature levels
                    &mut device,
                    Some(&mut feature_level),
                    None, // immediate context created automatically
                )
                .map_err(|e| {
                    cv_shared::CvError::Capture(format!(
                        "D3D11CreateDevice failed: {e}"
                    ))
                })?
            };

            let device: ID3D11Device = device.ok_or_else(|| {
                cv_shared::CvError::Capture(
                    "D3D11CreateDevice returned None device".into(),
                )
            })?;

            let context = unsafe { device.GetImmediateContext() }.map_err(|e| {
                cv_shared::CvError::Capture(format!(
                    "GetImmediateContext failed: {e}"
                ))
            })?;

            let dxgi_device: IDXGIDevice = device
                .cast()
                .map_err(|e| cv_shared::CvError::Capture(format!("IDXGIDevice cast: {e}")))?;

            let adapter = unsafe { dxgi_device.GetAdapter() }.map_err(|e| {
                cv_shared::CvError::Capture(format!("GetAdapter failed: {e}"))
            })?;

            let output = unsafe { adapter.EnumOutputs(display_index) }.map_err(|e| {
                cv_shared::CvError::Capture(format!(
                    "Invalid display index {display_index}: {e}"
                ))
            })?;

            let output_desc = unsafe { output.GetDesc() }.map_err(|e| {
                cv_shared::CvError::Capture(format!("GetDesc failed: {e}"))
            })?;

            let width = (output_desc.DesktopCoordinates.right
                - output_desc.DesktopCoordinates.left) as u32;
            let height = (output_desc.DesktopCoordinates.bottom
                - output_desc.DesktopCoordinates.top) as u32;

            let output1: IDXGIOutput1 = output
                .cast()
                .map_err(|e| cv_shared::CvError::Capture(format!("IDXGIOutput1 cast: {e}")))?;

            let duplication = unsafe {
                output1.DuplicateOutput(&device).map_err(|e| {
                    cv_shared::CvError::Capture(format!(
                        "DuplicateOutput failed (is UAC prompt active or session locked?): {e}"
                    ))
                })?
            };

            // Create staging texture for CPU read-back
            let staging_texture = create_staging_texture(&device, width, height)?;

            debug!(
                display_index,
                width, height, "DxgiCapturer initialised successfully"
            );

            Ok(Self {
                device,
                context,
                duplication: Some(duplication),
                staging_texture: Some(staging_texture),
                width,
                height,
                pitch: width * 4,
                display_index,
                frame_count: 0,
            })
        }

        /// Capture a single frame.
        ///
        /// Blocks up to 500 ms waiting for the desktop to change.
        /// Returns `Ok(None)` on timeout (no visual changes).
        ///
        /// # Latency
        /// Typical 1–3 ms for 1080p on modern hardware (GPU → CPU copy).
        pub fn capture_frame(&mut self) -> cv_shared::CvResult<Option<Frame>> {
            let duplication = self.duplication.as_ref().ok_or_else(|| {
                cv_shared::CvError::Capture(
                    "Duplication already released".into(),
                )
            })?;

            let mut frame_info: DXGI_OUTDUPL_FRAME_INFO = unsafe { std::mem::zeroed() };
            let mut desktop_resource = None;

            let hr = unsafe {
                duplication.AcquireNextFrame(
                    Self::ACQUIRE_TIMEOUT_MS,
                    &mut frame_info,
                    &mut desktop_resource,
                )
            };

            if hr == DXGI_ERROR_WAIT_TIMEOUT {
                trace!("AcquireNextFrame timed out – no desktop changes");
                return Ok(None);
            }

            if let Err(e) = hr {
                return Err(cv_shared::CvError::Capture(format!(
                    "AcquireNextFrame failed: {e}"
                )));
            }

            let resource = desktop_resource.ok_or_else(|| {
                cv_shared::CvError::Capture(
                    "AcquireNextFrame returned None resource".into(),
                )
            })?;

            let texture: ID3D11Texture2D = resource.cast().map_err(|e| {
                cv_shared::CvError::Capture(format!(
                    "Desktop resource cast to texture failed: {e}"
                ))
            })?;

            // Copy from GPU desktop texture to CPU-accessible staging texture
            let staging = self.staging_texture.as_ref().unwrap();
            unsafe { self.context.CopyResource(staging, &texture) };

            // Map staging texture to read pixels on CPU
            let mut mapped: D3D11_MAPPED_SUBRESOURCE = unsafe { std::mem::zeroed() };
            unsafe {
                self.context
                    .Map(
                        staging,
                        0,
                        windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| {
                        cv_shared::CvError::Capture(format!("Map failed: {e}"))
                    })?;
            }

            let pitch = mapped.RowPitch;
            let size_bytes = (pitch * self.height) as usize;

            // Copy pixel data from mapped memory into a Vec<u8>
            let data = unsafe {
                let src = std::slice::from_raw_parts(
                    mapped.pData as *const u8,
                    size_bytes,
                );
                let owned = src.to_vec();
                owned
            };

            // Unmap
            unsafe {
                self.context.Unmap(staging, 0);
            }

            // Release the frame back to DXGI
            unsafe {
                let _ = duplication.ReleaseFrame();
            }

            // Build dirty regions
            let dirty_regions = extract_dirty_regions(&frame_info);

            self.frame_count += 1;
            let frame = Frame {
                data,
                width: self.width,
                height: self.height,
                pitch,
                timestamp: Instant::now(),
                dirty_regions,
            };

            trace!(
                frame = self.frame_count,
                bytes = frame.size_bytes(),
                "Frame captured"
            );

            Ok(Some(frame))
        }

        /// Query the capture resolution in pixels.
        pub fn get_resolution(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        /// Number of frames captured so far.
        pub fn frame_count(&self) -> u64 {
            self.frame_count
        }

        /// Release all COM resources (explicit teardown).
        pub fn release(&mut self) {
            trace!("Releasing DxgiCapturer COM resources");
            self.staging_texture = None;
            self.duplication = None;
            // device / context dropped by Drop
        }
    }

    impl Drop for DxgiCapturer {
        fn drop(&mut self) {
            self.release();
            debug!(display_index = self.display_index, "DxgiCapturer dropped");
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn create_staging_texture(
        device: &ID3D11Device,
        width: u32,
        height: u32,
    ) -> cv_shared::CvResult<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(|e| {
                    cv_shared::CvError::Capture(format!(
                        "CreateTexture2D failed: {e}"
                    ))
                })?;
        }

        texture.ok_or_else(|| {
            cv_shared::CvError::Capture(
                "CreateTexture2D returned None".into(),
            )
        })
    }

    fn extract_dirty_regions(
        _info: &DXGI_OUTDUPL_FRAME_INFO,
    ) -> Vec<cv_shared::Rect> {
        // TODO: Use LastPresentTime / AccumulatedFrames to derive
        // move rectangles via GetFrameDirtyRects / GetFrameMoveRects.
        // For now we return empty (full-frame) to keep the hot path simple.
        vec![]
    }

    // -----------------------------------------------------------------------
    // Async capture stream
    // -----------------------------------------------------------------------

    /// Start an asynchronous capture stream at the given frame rate.
    ///
    /// Returns a [`tokio::sync::mpsc::Receiver`] that yields [`Frame`]s.
    /// The task automatically stops when the receiver is dropped.
    ///
    /// # Panics
    /// Panics if called outside the Tokio runtime.
    pub fn capture_stream(
        display_index: u32,
        fps: u32,
    ) -> tokio::sync::mpsc::Receiver<Frame> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Frame>(4);
        let frame_interval = Duration::from_millis(1000 / fps.max(1) as u64);

        tokio::spawn(async move {
            let mut capturer = match DxgiCapturer::new(display_index) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create capturer: {e}");
                    return;
                }
            };

            let mut interval = tokio::time::interval(frame_interval);
            interval.set_missed_tick_behavior(
                tokio::time::MissedTickBehavior::Skip,
            );

            loop {
                interval.tick().await;

                match capturer.capture_frame() {
                    Ok(Some(frame)) => {
                        if tx.send(frame).await.is_err() {
                            debug!("capture_stream receiver dropped – stopping");
                            break;
                        }
                    }
                    Ok(None) => {
                        // Timeout – no desktop changes, skip
                    }
                    Err(e) => {
                        error!("capture_frame error: {e}");
                        // Brief backoff to avoid spamming errors
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            capturer.release();
        });

        rx
    }
}

// ===========================================================================
// Non-Windows stub implementation
// ===========================================================================

#[cfg(not(target_os = "windows"))]
mod platform {
    pub use super::*;
    pub use tokio::sync::mpsc;
    pub use tracing::{debug, error, trace};

    /// Stub capturer for non-Windows platforms.
    ///
    /// The struct compiles but [`capture_frame`](DxgiCapturer::capture_frame)
    /// always returns `Ok(None)`.
    pub struct DxgiCapturer {
        width: u32,
        height: u32,
        display_index: u32,
        frame_count: u64,
    }

    impl DxgiCapturer {
        /// Create a stub capturer.
        ///
        /// The `display_index` is stored but never used.
        pub fn new(display_index: u32) -> cv_shared::CvResult<Self> {
            trace!(
                display_index,
                "DxgiCapturer stub created on non-Windows platform"
            );
            Ok(Self {
                width: 1920,
                height: 1080,
                display_index,
                frame_count: 0,
            })
        }

        /// Always returns `Ok(None)` on non-Windows.
        pub fn capture_frame(&mut self) -> cv_shared::CvResult<Option<Frame>> {
            trace!("capture_frame stub – non-Windows platform");
            Ok(None)
        }

        /// Returns the hard-coded stub resolution (1920×1080).
        pub fn get_resolution(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        /// Number of frames captured so far (always 0 on stub).
        pub fn frame_count(&self) -> u64 {
            self.frame_count
        }

        /// No-op on stub.
        pub fn release(&mut self) {}
    }

    impl Drop for DxgiCapturer {
        fn drop(&mut self) {
            trace!("DxgiCapturer stub dropped");
        }
    }

    /// Stub stream that never produces frames on non-Windows.
    pub fn capture_stream(
        display_index: u32,
        _fps: u32,
    ) -> tokio::sync::mpsc::Receiver<Frame> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Frame>(1);

        tokio::spawn(async move {
            debug!(
                display_index,
                "capture_stream stub on non-Windows – no frames will be produced"
            );
            // Hold the sender until the receiver is dropped so the channel stays open.
            let _ = tx.closed().await;
        });

        rx
    }
}

// Re-export platform-specific types at crate::dxgi
pub use platform::*;

// ---------------------------------------------------------------------------
// Tests (platform-independent)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capturer_stub_creation() {
        let cap = DxgiCapturer::new(0);
        assert!(cap.is_ok());
    }

    #[test]
    fn capturer_stub_resolution() {
        let cap = DxgiCapturer::new(0).unwrap();
        let (w, h) = cap.get_resolution();
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn capturer_stub_capture_returns_none() {
        let mut cap = DxgiCapturer::new(0).unwrap();
        let frame = cap.capture_frame();
        assert!(frame.is_ok());
        // On non-Windows this is None; on Windows it may be Some if the
        // desktop has changed, so we only assert Ok.
    }

    #[test]
    fn capturer_release_and_drop() {
        let mut cap = DxgiCapturer::new(0).unwrap();
        cap.release();
        // Drop called automatically at end of scope
    }

    #[test]
    fn rect_re_exported() {
        let r = Rect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 100);
    }
}
