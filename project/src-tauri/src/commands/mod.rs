use rand::Rng;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    pub session: Mutex<Option<Session>>,
}

pub struct Session {
    pub peer_id: String,
    pub password: String,
    pub ai_mode: String,
    pub connected: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn generate_session_password() -> String {
    let words = [
        "ace", "act", "add", "age", "ago", "aid", "air", "all", "and", "any",
        "ape", "apt", "are", "arm", "art", "ash", "ask", "ate", "awe", "axe",
        "bad", "bag", "ban", "bar", "bat", "bay", "bed", "bet", "big", "bit",
        "bow", "box", "boy", "bug", "bus", "but", "buy", "bye", "cab", "can",
        "cap", "car", "cat", "cop", "cow", "cry", "cup", "cut", "dad", "day",
        "did", "die", "dig", "dim", "dip", "dog", "dot", "dry", "dub", "due",
        "dug", "ear", "eat", "egg", "ego", "elf", "elk", "elm", "end", "era",
        "eve", "eye", "fan", "far", "fat", "fax", "fee", "few", "fit", "fix",
        "flu", "fly", "fog", "foo", "for", "fox", "fry", "fun", "gag", "gap",
        "gas", "gem", "get", "gig", "god", "got", "gum", "gun", "guy", "gym",
    ];
    let word = words[rand::random::<usize>() % words.len()];
    let num = rand::random::<u16>() % 1000;
    format!("{}{:03}", word, num)
}

#[tauri::command]
pub fn start_host_session(
    password: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<String, String> {
    let peer_id = format!("claw-{}", generate_id());
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        *session = Some(Session {
            peer_id: peer_id.clone(),
            password,
            ai_mode: "disabled".to_string(),
            connected: true,
        });
    }
    let _ = app.emit("peer-connected", ());
    let _ = app.emit("connection-state-changed", serde_json::json!({"state": "connected"}));
    Ok(peer_id)
}

#[tauri::command]
pub fn connect_to_peer(
    peer_id: String,
    password: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        *session = Some(Session {
            peer_id: peer_id.clone(),
            password,
            ai_mode: "disabled".to_string(),
            connected: true,
        });
    }
    let _ = app.emit("peer-connected", ());
    let _ = app.emit("connection-state-changed", serde_json::json!({"state": "connected"}));
    Ok(())
}

#[tauri::command]
pub fn disconnect(state: State<'_, Arc<AppState>>, app: AppHandle) -> Result<(), String> {
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        *session = None;
    }
    let _ = app.emit("peer-disconnected", ());
    let _ = app.emit("connection-state-changed", serde_json::json!({"state": "disconnected"}));
    Ok(())
}

#[tauri::command]
pub fn emergency_stop(state: State<'_, Arc<AppState>>, app: AppHandle) -> Result<(), String> {
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        *session = None;
    }
    let _ = app.emit("peer-disconnected", ());
    let _ = app.emit("connection-state-changed", serde_json::json!({"state": "disconnected"}));
    let _ = app.emit("ai-mode-changed", serde_json::json!({"mode": "disabled"}));
    Ok(())
}

#[tauri::command]
pub fn set_ai_mode(
    mode: String,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut session = state.session.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut s) = *session {
            s.ai_mode = mode.clone();
        }
    }
    let _ = app.emit("ai-mode-changed", serde_json::json!({"mode": mode.clone()}));
    
    // If AI mode is active, start ghost cursor simulation
    if mode == "observer" || mode == "shared" || mode == "full" {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let mut counter = 0u32;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                counter += 1;
                
                // Simulate AI cursor movement
                let x = ((counter * 10) % 640) as f32;
                let y = ((counter * 7) % 480) as f32;
                
                let _ = app_clone.emit("ai-cursor-position", serde_json::json!({
                    "x": x,
                    "y": y,
                }));
                
                // Emit AI activity event
                let _ = app_clone.emit("ai-activity", serde_json::json!({
                    "isActive": true,
                    "confidence": 0.85 + (counter % 10) as f32 / 100.0,
                    "currentAction": match counter % 4 {
                        0 => "Analysiere Bildschirm",
                        1 => "Erkenne UI-Elemente",
                        2 => "Plane Aktion",
                        _ => "Führe aus",
                    },
                }));
            }
        });
    }
    
    Ok(())
}

#[tauri::command]
pub fn send_chat_message(content: String, app: AppHandle) -> Result<(), String> {
    let msg = serde_json::json!({
        "message": {
            "id": format!("msg-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()),
            "type": "chat",
            "sender": "Du",
            "content": content,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    });
    let _ = app.emit("chat-message", msg);
    Ok(())
}

#[tauri::command]
pub fn start_capture(app: AppHandle) -> Result<(), String> {
    println!("[ClawViewer] Screen capture started");
    
    // Start a background task that generates test frames
    std::thread::spawn(move || {
        let mut counter = 0u8;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            counter = counter.wrapping_add(1);
            
            // Generate a simple test pattern (gradient)
            let width = 640u32;
            let height = 480u32;
            let mut data = Vec::with_capacity((width * height * 4) as usize);
            
            for y in 0..height {
                for x in 0..width {
                    let r = ((x + counter as u32) % 256) as u8;
                    let g = ((y + counter as u32) % 256) as u8;
                    let b = ((x + y + counter as u32) % 256) as u8;
                    data.push(r);
                    data.push(g);
                    data.push(b);
                    data.push(255); // Alpha
                }
            }
            
            let _ = app.emit("video-frame", serde_json::json!({
                "data": data,
                "width": width,
                "height": height,
            }));
        }
    });
    
    Ok(())
}

#[tauri::command]
pub fn stop_capture() -> Result<(), String> {
    println!("[ClawViewer] Screen capture stopped");
    Ok(())
}

#[tauri::command]
pub fn send_input_event(event: serde_json::Value) -> Result<(), String> {
    println!("[ClawViewer] Input event: {:?}", event);
    Ok(())
}

fn generate_id() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let id: String = (0..8)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    id
}
