mod commands;

use std::sync::{Arc, Mutex};
use tauri::Manager;

pub use commands::*;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Arc::new(commands::AppState::new()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generate_session_password,
            commands::start_host_session,
            commands::connect_to_peer,
            commands::disconnect,
            commands::emergency_stop,
            commands::set_ai_mode,
            commands::send_chat_message,
            commands::start_capture,
            commands::stop_capture,
            commands::send_input_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
