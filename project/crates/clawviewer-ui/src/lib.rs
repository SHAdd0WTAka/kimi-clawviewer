//! UI components and Tauri integration
//! 
//! This crate provides enterprise-grade ui functionality
//! for the ClawViewer platform.

use tracing::{info, debug, warn, error};

pub fn init() {
    info!("Initializing {}", env!("CARGO_PKG_NAME"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
    }
}
