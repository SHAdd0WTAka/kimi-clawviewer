//! Frame – BGRA pixel buffer with capture metadata
//!
//! Represents a single captured screen frame including pixel data, dimensions,
//! pitch (bytes per row), capture timestamp, and dirty regions (changed areas).

use std::time::Instant;

/// A captured screen frame in BGRA32 format.
///
/// Each pixel occupies 4 bytes in order: `[B, G, R, A]`.
/// The `pitch` may be larger than `width * 4` due to row alignment.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Raw BGRA pixel data (4 bytes per pixel).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Number of bytes per row (may include padding).
    pub pitch: u32,
    /// Timestamp when the frame was captured.
    pub timestamp: Instant,
    /// List of regions that changed since the previous frame.
    /// When empty, the entire frame is considered new.
    pub dirty_regions: Vec<cv_shared::Rect>,
}

impl Frame {
    /// Create a new frame from raw BGRA data.
    ///
    /// # Arguments
    /// * `data`     – Vec of BGRA pixel bytes (len = pitch * height)
    /// * `width`    – Width in pixels
    /// * `height`   – Height in pixels
    /// * `pitch`    – Bytes per row (>= width * 4)
    /// * `dirty`    – Changed regions since last frame
    ///
    /// # Panics
    /// Panics in debug mode if `data.len()` does not equal `pitch * height`.
    pub fn new(data: Vec<u8>, width: u32, height: u32, pitch: u32, dirty: Vec<cv_shared::Rect>) -> Self {
        debug_assert!(
            data.len() as u32 >= pitch * height,
            "Frame data length ({}) must be >= pitch * height ({})",
            data.len(),
            pitch * height
        );
        Self {
            data,
            width,
            height,
            pitch,
            timestamp: Instant::now(),
            dirty_regions: dirty,
        }
    }

    /// Frame size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Number of pixels.
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Bytes per pixel (always 4 for BGRA).
    pub const fn bpp() -> u32 {
        4
    }

    /// Check if this frame has dirty-region information.
    pub fn has_dirty_regions(&self) -> bool {
        !self.dirty_regions.is_empty()
    }

    /// Return the total area covered by dirty regions in pixels.
    pub fn dirty_area_pixels(&self) -> i32 {
        self.dirty_regions
            .iter()
            .map(|r| r.width() * r.height())
            .sum()
    }

    /// Returns true if the frame data is entirely zero (black screen).
    /// Useful for tests and sanity checks.
    pub fn is_black(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }

    /// Get the age of this frame relative to `now`.
    pub fn age(&self, now: Instant) -> std::time::Duration {
        now.duration_since(self.timestamp)
    }

    /// Convert a pixel coordinate to the byte offset in `data`.
    ///
    /// Returns `None` if `(x, y)` is out of bounds.
    pub fn pixel_offset(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y * self.pitch + x * Self::bpp()) as usize)
    }

    /// Read the BGRA colour of a single pixel.
    ///
    /// Returns `None` if the coordinate is out of bounds.
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        let off = self.pixel_offset(x, y)?;
        if off + 4 > self.data.len() {
            return None;
        }
        Some((
            self.data[off],     // B
            self.data[off + 1], // G
            self.data[off + 2], // R
            self.data[off + 3], // A
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rect(l: i32, t: i32, r: i32, b: i32) -> cv_shared::Rect {
        cv_shared::Rect {
            left: l,
            top: t,
            right: r,
            bottom: b,
        }
    }

    #[test]
    fn frame_creation() {
        let data = vec![0u8; 1920 * 1080 * 4];
        let frame = Frame::new(data.clone(), 1920, 1080, 1920 * 4, vec![]);
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.pitch, 1920 * 4);
        assert_eq!(frame.size_bytes(), 1920 * 1080 * 4);
        assert_eq!(frame.pixel_count(), 1920 * 1080);
        assert!(frame.is_black());
        assert!(!frame.has_dirty_regions());
    }

    #[test]
    fn frame_with_dirty_regions() {
        let dirty = vec![test_rect(0, 0, 100, 100), test_rect(200, 200, 300, 300)]
            .into_iter()
            .map(|r| cv_shared::Rect {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            })
            .collect();
        let data = vec![0u8; 1920 * 1080 * 4];
        let frame = Frame::new(data, 1920, 1080, 1920 * 4, dirty);
        assert!(frame.has_dirty_regions());
        assert_eq!(frame.dirty_regions.len(), 2);
        assert_eq!(frame.dirty_area_pixels(), 100 * 100 + 100 * 100);
    }

    #[test]
    fn frame_pixel_access() {
        let mut data = vec![0u8; 8 * 8 * 4];
        // Set pixel (3, 4) to known BGRA values
        let off = (4 * 8 + 3) * 4;
        data[off] = 10;
        data[off + 1] = 20;
        data[off + 2] = 30;
        data[off + 3] = 40;

        let frame = Frame::new(data, 8, 8, 8 * 4, vec![]);
        assert_eq!(frame.pixel_at(3, 4), Some((10, 20, 30, 40)));
        assert_eq!(frame.pixel_at(8, 4), None); // out of bounds
        assert_eq!(frame.pixel_at(3, 8), None); // out of bounds
    }

    #[test]
    fn frame_timestamp_and_age() {
        let data = vec![0u8; 16];
        let frame = Frame::new(data, 2, 2, 8, vec![]);
        // Age should be very small (just created)
        let age = frame.age(Instant::now());
        assert!(age.as_secs() < 1);
    }

    #[test]
    fn frame_pitch_larger_than_width() {
        // Simulate 4-byte aligned pitch: width=5*4=20, pitch padded to 24
        let data = vec![0u8; 24 * 10];
        let frame = Frame::new(data, 5, 10, 24, vec![]);
        assert_eq!(frame.pitch, 24);
        assert_eq!(frame.pixel_count(), 50);
    }
}
