# Dim 02: RustDesk Client-Implementierung - Deep Analysis Blueprint

## Zusammenfassung

Dieses Dokument analysiert das RustDesk-Client-Repository (rustdesk/rustdesk) und dokumentiert konkrete Implementierungsmuster fuer Screen-Capture, Video-Codec-Pipeline, Input-Injection, P2P-Client-Logik und UI-Architektur. Alle Code-Referenzen basieren auf dem aktuellen master-Branch des GitHub-Repositorys.

---

## 1. Repository-Struktur und Module

### 1.1 Gesamtstruktur [^6^]

```
rustdesk/
├── libs/
│   ├── hbb_common/        # Video codec config, TCP/UDP wrapper, Protobuf, FS utils
│   ├── scrap/             # Screen capture library (plattform-spezifisch)
│   ├── enigo/             # Platform-spezifische Keyboard/Mouse Control
│   ├── clipboard/         # File copy/paste fuer Windows, Linux, macOS
│   └── hwcodec/           # Hardware codec library (extern: rustdesk-org/hwcodec)
├── src/
│   ├── client.rs          # Startet Peer-Verbindung
│   ├── rendezvous_mediator.rs  # Kommunikation mit rustdesk-server, P2P hole punching
│   ├── server/            # Audio/Clipboard/Input/Video Services + Netzwerk
│   │   ├── video_service.rs
│   │   ├── input_service.rs
│   │   ├── audio_service.rs
│   │   ├── clipboard_service.rs
│   │   └── connection.rs
│   ├── platform/          # Platform-spezifischer Code
│   └── ui/                # Veraltete Sciter UI
├── flutter/               # Flutter Code fuer Desktop und Mobile
└── flutter/web/js/        # JavaScript fuer Flutter Web Client
```

### 1.2 Kern-Module und Verantwortlichkeiten

| Modul | Pfad | Verantwortlichkeit |
|-------|------|-------------------|
| `scrap` | `libs/scrap/src/` | Screen-Capture fuer alle Plattformen |
| `enigo` | `libs/enigo/src/` | Input-Injection (Maus/Tastatur) |
| `client` | `src/client.rs` | Verbindungsaufbau und Peer-Logik |
| `rendezvous_mediator` | `src/rendezvous_mediator.rs` | P2P NAT Traversal |
| `video_service` | `src/server/video_service.rs` | Capture-Encode-Send Loop |
| `input_service` | `src/server/input_service.rs` | Input-Event-Verarbeitung |
| `codec` | `libs/scrap/src/common/codec.rs` | Video Encoder/Decoder Abstraktion |

---

## 2. Screen-Capture Implementierung

### 2.1 Windows: DXGI Desktop Duplication API

**Quelle:** `libs/scrap/src/dxgi/mod.rs` [^34^]

#### 2.1.1 Capturer-Struktur

```rust
// libs/scrap/src/dxgi/mod.rs, Zeile 44-66
pub struct Capturer {
    device: ComPtr<ID3D11Device>,          // Direct3D 11 Device
    display: Display,
    context: ComPtr<ID3D11DeviceContext>,  // D3D11 Device Context
    duplication: ComPtr<IDXGIOutputDuplication>,  // Desktop Duplication
    fastlane: bool,
    surface: ComPtr<IDXGISurface>,
    texture: ComPtr<ID3D11Texture2D>,
    width: usize,
    height: usize,
    rotated: Vec<u8>,
    gdi_capturer: Option<CapturerGDI>,    // GDI Fallback
    gdi_buffer: Vec<u8>,
    saved_raw_data: Vec<u8>,
    output_texture: bool,
    adapter_desc1: DXGI_ADAPTER_DESC1,
    rotate: Rotate,
}
```

#### 2.1.2 Initialisierung (DXGI + GDI Fallback)

```rust
// libs/scrap/src/dxgi/mod.rs, Zeile 69-173
pub fn new(display: Display) -> io::Result<Capturer> {
    let mut device = ptr::null_mut();
    let mut context = ptr::null_mut();
    let mut duplication = ptr::null_mut();
    
    // 1. D3D11 Device erstellen
    let res = wrap_hresult(unsafe {
        D3D11CreateDevice(
            display.adapter.0 as *mut _,
            D3D_DRIVER_TYPE_UNKNOWN,
            ptr::null_mut(),
            0,  // No device flags
            ptr::null_mut(),  // Feature levels
            0,
            D3D11_SDK_VERSION,
            &mut device,
            ptr::null_mut(),
            &mut context,
        )
    });
    
    // 2. Bei Fehler -> GDI Fallback
    if res.is_err() {
        gdi_capturer = display.create_gdi();
        println!("Fallback to GDI");
    } else {
        // 3. Desktop Duplication starten
        res = wrap_hresult(unsafe {
            let hres = (*display.inner.0).DuplicateOutput(
                device.0 as *mut _, &mut duplication
            );
            if hres != S_OK {
                // DXGI nicht verfuegbar -> GDI Fallback
                gdi_capturer = display.create_gdi();
                println!("Fallback to GDI");
                ...
            }
        });
    }
}
```

**Claim:** RustDesk verwendet DXGI Desktop Duplication als primaeren Capture-Mechanismus auf Windows, mit automatischem Fallback auf GDI.
**Evidence:** `libs/scrap/src/dxgi/mod.rs`, Zeile 69-173, `DuplicateOutput()` Aufruf mit GDI-Fallback bei Fehler.

#### 2.1.3 Frame-Erfassung (GPU -> System Memory)

```rust
// libs/scrap/src/dxgi/mod.rs, Zeile 282-310
unsafe fn load_frame(&mut self, timeout: UINT) -> io::Result<(*const u8, i32)> {
    let mut frame = ptr::null_mut();
    let mut info = mem::MaybeUninit::uninit().assume_init();
    
    // 1. Naechsten Frame von DXGI abrufen
    wrap_hresult((*self.duplication.0).AcquireNextFrame(timeout, &mut info, &mut frame))?;
    let frame = ComPtr(frame);
    
    if *info.LastPresentTime.QuadPart() == 0 {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    
    // 2. Fastlane: Direkter Speicherzugriff ODER GPU->CPU Copy
    let mut rect = mem::MaybeUninit::uninit().assume_init();
    if self.fastlane {
        wrap_hresult((*self.duplication.0).MapDesktopSurface(&mut rect))?;
    } else {
        self.surface = ComPtr(self.ohgodwhat(frame.0)?);
        wrap_hresult((*self.surface.0).Map(&mut rect, DXGI_MAP_READ))?;
    }
    Ok((rect.pBits, rect.Pitch))
}

// GPU Texture -> CPU-lesbarer Speicher (Staging Texture)
// libs/scrap/src/dxgi/mod.rs, Zeile 313-340
unsafe fn ohgodwhat(&mut self, frame: *mut IDXGIResource) -> io::Result<*mut IDXGISurface> {
    let mut texture: *mut ID3D11Texture2D = ptr::null_mut();
    (*frame).QueryInterface(&IID_ID3D11Texture2D, &mut texture as *mut *mut _ as *mut *mut _);
    
    let mut texture_desc = mem::MaybeUninit::uninit().assume_init();
    (*texture.0).GetDesc(&mut texture_desc);
    
    // Staging Texture fuer CPU-Lesezugriff erstellen
    texture_desc.Usage = D3D11_USAGE_STAGING;
    texture_desc.BindFlags = 0;
    texture_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    texture_desc.MiscFlags = 0;
    
    let mut readable = ptr::null_mut();
    (*self.device.0).CreateTexture2D(&mut texture_desc, ptr::null(), &mut readable)?;
    (*readable).SetEvictionPriority(DXGI_RESOURCE_PRIORITY_MAXIMUM);
    
    // GPU -> GPU Copy (zu Staging Texture)
    (*self.context.0).CopyResource(readable.0 as *mut _, texture.0 as *mut _);
    ...
}
```

**Claim:** Frame-Daten werden ueber `AcquireNextFrame()` abgerufen und bei Bedarf von GPU-Speicher in CPU-lesbaren Staging-Speicher kopiert.
**Evidence:** `libs/scrap/src/dxgi/mod.rs`, Zeile 282-340, `ohgodwhat()` Funktion fuer GPU->CPU Transfer.

#### 2.1.4 Hardware-Rotation (D3D11 VideoProcessor)

```rust
// libs/scrap/src/dxgi/mod.rs, Zeile 175-279
fn create_rotations(...) -> Rotate {
    let processor_rotation = match display.rotation() {
        DXGI_MODE_ROTATION_ROTATE90 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_90),
        DXGI_MODE_ROTATION_ROTATE180 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_180),
        DXGI_MODE_ROTATION_ROTATE270 => Some(D3D11_VIDEO_PROCESSOR_ROTATION_270),
        _ => None,
    };
    // D3D11 VideoProcessor fuer hardwarebeschleunigte Rotation erstellen
    (*video_device).CreateVideoProcessor(video_processor_enum, 0, &mut video_processor);
    (*video_context).VideoProcessorSetStreamRotation(video_processor, 0, TRUE, processor_rotation);
}
```

---

### 2.2 Linux: PipeWire / X11 Integration

#### 2.2.1 Wayland: PipeWire via xdg-desktop-portal

**Quelle:** `libs/scrap/src/wayland/pipewire.rs` [^40^]

```rust
// libs/scrap/src/wayland/pipewire.rs
// GStreamer-basierte PipeWire Capture
pub struct PipeWireRecorder {
    buffer: Option<gst::MappedBuffer<gst::buffer::Readable>>,
    buffer_cropped: Vec<u8>,
    pix_fmt: String,
    pipeline: gst::Pipeline,
    appsink: AppSink,
    width: usize,
    height: usize,
    saved_raw_data: Vec<u8>,
}

impl PipeWireRecorder {
    pub fn new(capturable: PipeWireCapturable) -> ResultType<Self> {
        let pipeline = gst::Pipeline::new(None);
        
        // pipewiresrc -> videoconvert -> appsink
        let src = gst::ElementFactory::make("pipewiresrc", None)?;
        src.set_property("fd", &capturable.fd.as_raw_fd())?;
        src.set_property("path", &format!("{}", capturable.path))?;
        src.set_property("always-copy", &true)?;
        
        let convert = gst::ElementFactory::make("videoconvert", None)?;
        let sink = gst::ElementFactory::make("appsink", None)?;
        sink.set_property("drop", &true)?;
        sink.set_property("max-buffers", &1u32)?;
        
        pipeline.add_many(&[&src, &convert, &sink])?;
        src.link(&convert)?;
        convert.link(&sink)?;
        
        // Caps auf BGRx/RGBx beschraenken
        let mut caps = gst::Caps::new_empty();
        caps.merge_structure(gst::structure::Structure::new("video/x-raw", 
            &[("format", &"BGRx")]));
        caps.merge_structure(gst::structure::Structure::new("video/x-raw", 
            &[("format", &"RGBx")]));
        appsink.set_caps(Some(&caps));
        
        pipeline.set_state(gst::State::Playing)?;
        ...
    }
}
```

**Claim:** Wayland-Capture erfolgt ueber das xdg-desktop-portal mit PipeWire als Backend. Ein GStreamer-Pipeline (`pipewiresrc -> videoconvert -> appsink`) wird verwendet, um Frame-Daten in BGRx/RGBx-Format zu erhalten.
**Evidence:** `libs/scrap/src/wayland/pipewire.rs`, Zeile 174-260, GStreamer Pipeline mit `pipewiresrc`.

#### 2.2.2 DBus Portal Flow (ScreenCast/RemoteDesktop)

```rust
// libs/scrap/src/wayland/pipewire.rs, Zeile 380-450
pub fn request_remote_desktop(capture_cursor: bool) -> ResultType<(...)> {
    let conn = SyncConnection::new_session()?;
    let portal = get_portal(&conn);
    
    // 1. Session erstellen (org.freedesktop.portal.ScreenCast)
    let path = screencast_portal::create_session(&portal, args)?;
    
    // 2. Quellen auswaehlen
    let path = portal.select_sources(ses.clone(), args)?;
    
    // 3. Capture starten -> liefert PipeWire File Descriptor
    let path = screencast_portal::start(&portal, session.clone(), "", args)?;
    
    // 4. PipeWire Remote oeffnen
    fd.replace(portal.open_pipe_wire_remote(session.clone(), HashMap::new())?);
    
    Ok((conn, fd, streams, session, is_support_restore_token))
}
```

#### 2.2.3 X11: MIT-SHM

**Quelle:** `libs/scrap/src/x11/` [^33^]

X11-Capture verwendet die MIT-SHM (Shared Memory) Extension fuer zero-copy Frame-Zugriff. Die Implementierung ist im `x11/` Unterordner von `libs/scrap/src/`.

---

## 3. Video-Codec-Pipeline

### 3.1 Codec-Architektur

**Quelle:** `libs/scrap/src/common/codec.rs` [^204^]

```rust
// libs/scrap/src/common/codec.rs, Zeile 43-53
#[derive(Debug, Clone)]
pub enum EncoderCfg {
    VPX(VpxEncoderConfig),      // libvpx fuer VP8/VP9
    AOM(AomEncoderConfig),      // aom fuer AV1
    #[cfg(feature = "hwcodec")]
    HWRAM(HwRamEncoderConfig),  // FFmpeg Hardware Encoder (NVENC, VAAPI, QSV, VideoToolbox)
    #[cfg(feature = "vram")]
    VRAM(VRamEncoderConfig),    // Direct GPU Texture Encoding (Windows D3D11)
}
```

### 3.2 Encoder-Selektion (Auto-Priority)

```rust
// libs/scrap/src/common/codec.rs, Zeile 167-260
// Auto: h265 > h264 > av1/vp9/vp8
let mut auto_codec = if av1_useable && av1_test { CodecFormat::AV1 } else { CodecFormat::VP9 };
if h264_useable {
    auto_codec = CodecFormat::H264;
}
if h265_useable {
    auto_codec = CodecFormat::H265;
}

// Codec-Selektion basierend auf Peer-Preferences und Hardware-Unterstuetzung
*format = match preference {
    PreferCodec::H264 => {
        if h264vram_encoding || h264hw_encoding.is_some() {
            CodecFormat::H264
        } else { auto_codec }
    }
    PreferCodec::H265 => {
        if h265vram_encoding || h265hw_encoding.is_some() {
            CodecFormat::H265
        } else { auto_codec }
    }
    PreferCodec::Auto => auto_codec,
    ...
};
```

**Claim:** RustDesk waehlt automatisch den besten Codec: Hardware-Encoding (H264/H265) wird bevorzugt, mit Fallback auf Software-Codecs (VP9, AV1, VP8).
**Evidence:** `libs/scrap/src/common/codec.rs`, Zeile 167-260, Codec-Prioritaet H265 > H264 > AV1 > VP9 > VP8.

### 3.3 Hardware-Codec-Implementierung

**Externe Library:** `rustdesk-org/hwcodec` [^15^]

Die Hardware-Codec-Implementierung basiert auf FFmpeg und unterstuetzt:
- **NVIDIA**: NVENC (encode) + NVDEC/D3D11VA (decode)
- **Intel**: QSV (encode/decode)
- **AMD**: VAAPI (encode/decode)
- **Apple**: VideoToolbox (encode/decode)

```rust
// Beispiel HwCodecConfig (aus Logs)
HwCodecConfig {
    ram_encode: [
        CodecInfo { name: "h264_nvenc", format: H264, ... },
        CodecInfo { name: "hevc_nvenc", format: H265, ... },
    ],
    ram_decode: [
        CodecInfo { name: "h264", format: H264, hwdevice: AV_HWDEVICE_TYPE_D3D11VA },
        CodecInfo { name: "hevc", format: H265, hwdevice: AV_HWDEVICE_TYPE_D3D11VA },
    ],
    vram_encode: [
        FeatureContext { driver: FFMPEG, vendor: NV, data_format: H264 },
        FeatureContext { driver: FFMPEG, vendor: NV, data_format: H265 },
    ],
}
```

### 3.4 Decoder-Implementierung

```rust
// libs/scrap/src/common/codec.rs, Zeile 390-500
pub struct Decoder {
    vp8: Option<VpxDecoder>,
    vp9: Option<VpxDecoder>,
    av1: Option<AomDecoder>,
    #[cfg(feature = "hwcodec")]
    h264_ram: Option<HwRamDecoder>,
    #[cfg(feature = "hwcodec")]
    h265_ram: Option<HwRamDecoder>,
    #[cfg(feature = "vram")]
    h264_vram: Option<VRamDecoder>,
    #[cfg(feature = "vram")]
    h265_vram: Option<VRamDecoder>,
    format: CodecFormat,
    valid: bool,
}
```

### 3.5 Video Service (Capture-Encode-Send Loop)

**Quelle:** `src/server/video_service.rs` [^133^]

```rust
// src/server/video_service.rs, Zeile 475-650
fn run(vs: VideoService) -> ResultType<()> {
    let mut c = get_capturer(vs.source, display_idx, ...)?;
    let (mut encoder, encoder_cfg, codec_format, use_i444, recorder) = 
        setup_encoder(&c, sp.name(), quality, ...)?;
    
    // Capture-Encode-Send Hauptloop
    while sp.ok() {
        // 1. QoS anpassen (FPS, Qualitaet)
        check_qos(&mut encoder, &mut quality, &mut spf, ...)?;
        
        // 2. Frame erfassen
        let res = match c.frame(spf) {
            Ok(frame) => {
                if frame.valid() {
                    // 3. Frame zu YUV konvertieren
                    let frame = frame.to(encoder.yuvfmt(), &mut yuv, &mut mid_data)?;
                    
                    // 4. Encodieren und senden
                    let send_conn_ids = handle_one_frame(
                        display_idx, &sp, frame, ms, &mut encoder, ...
                    )?;
                    frame_controller.set_send(now, send_conn_ids);
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // Kein neuer Frame -> letzten wiederholen (falls nicht latency-free)
                if !encoder.latency_free() && yuv.len() > 0 {
                    handle_one_frame(..., EncodeInput::YUV(&yuv), ...)?;
                }
            }
        };
        
        // 5. Auf Frame-Acknowledgment warten (fuer multiple Connections)
        frame_controller.try_wait_next(&mut fetched_conn_ids, 300);
        
        // 6. FPS-Limit einhalten
        if elapsed < spf {
            std::thread::sleep(spf - elapsed);
        }
    }
}
```

---

## 4. Input-Injection

### 4.1 Windows: SendInput / mouse_event / keybd_event

**Quelle:** `libs/enigo/src/win/win_impl.rs` [^114^]

```rust
// libs/enigo/src/win/win_impl.rs, Zeile 30-48
pub const ENIGO_INPUT_EXTRA_VALUE: ULONG_PTR = 100;

fn mouse_event(flags: u32, data: u32, dx: i32, dy: i32) -> DWORD {
    let mut u = INPUT_u::default();
    unsafe {
        *u.mi_mut() = MOUSEINPUT {
            dx, dy,
            mouseData: data,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: ENIGO_INPUT_EXTRA_VALUE,
        };
    }
    let mut input = INPUT { type_: INPUT_MOUSE, u };
    unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) }
}

fn keybd_event(mut flags: u32, vk: u16, scan: u16) -> DWORD {
    // Scan-Code aus Virtual Key mittels MapVirtualKeyExW ableiten
    if scan == 0 {
        if LAYOUT.is_null() {
            let current_window_thread_id =
                GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            LAYOUT = GetKeyboardLayout(current_window_thread_id);
        }
        scan = MapVirtualKeyExW(vk as _, 0, LAYOUT) as _;
    }
    
    // Extended Key Flag setzen fuer E0/E1-Scancodes
    if flags & KEYEVENTF_UNICODE == 0 {
        if scan >> 8 == 0xE0 || scan >> 8 == 0xE1 {
            flags |= winapi::um::winuser::KEYEVENTF_EXTENDEDKEY;
        }
    }
    
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    unsafe {
        *union.ki_mut() = KEYBDINPUT {
            wVk: vk, wScan: scan,
            dwFlags: flags, time: 0,
            dwExtraInfo: ENIGO_INPUT_EXTRA_VALUE,
        };
    }
    let mut inputs = [INPUT { type_: INPUT_KEYBOARD, u: union }; 1];
    unsafe { SendInput(inputs.len() as UINT, inputs.as_mut_ptr(), size_of::<INPUT>() as c_int) }
}
```

**Claim:** Windows Input-Injection verwendet die Windows-API `SendInput()` mit `MOUSEINPUT` und `KEYBDINPUT` Strukturen. Unicode-Input wird ueber `KEYEVENTF_UNICODE` unterstuetzt.
**Evidence:** `libs/enigo/src/win/win_impl.rs`, Zeile 30-113.

#### 4.1.2 Absolute Mausposition (virtueller Desktop)

```rust
// libs/enigo/src/win/win_impl.rs, Zeile 140-155
fn mouse_move_to(&mut self, x: i32, y: i32) {
    mouse_event(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        0,
        (x - unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }) * 65535
            / unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        (y - unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }) * 65535
            / unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    );
}
```

### 4.2 Linux: uinput / XTest / RemoteDesktop Portal

**Quelle:** `src/server/input_service.rs`, `src/server/uinput.rs`

```rust
// src/server/input_service.rs, Zeile 470-500
#[cfg(target_os = "linux")]
pub async fn setup_uinput(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    set_uinput_resolution(minx, maxx, miny, maxy).await?;
    
    let keyboard = super::uinput::client::UInputKeyboard::new().await?;
    let mouse = super::uinput::client::UInputMouse::new().await?;
    
    ENIGO.lock().unwrap().set_custom_keyboard(Box::new(keyboard));
    ENIGO.lock().unwrap().set_custom_mouse(Box::new(mouse));
    Ok(())
}

// Alternative: RemoteDesktop Portal fuer Wayland
#[cfg(target_os = "linux")]
pub async fn setup_rdp_input() -> ResultType<()> {
    let keyboard = RdpInputKeyboard::new(rdp_info.conn.clone(), rdp_info.session.clone())?;
    let mouse = RdpInputMouse::new(rdp_info.conn.clone(), rdp_info.session.clone(), ...)?;
    en.set_custom_keyboard(Box::new(keyboard));
    en.set_custom_mouse(Box::new(mouse));
}
```

**Claim:** Auf Linux gibt es drei Input-Wege: 1) uinput (Kernel-Level, fuer Wayland), 2) XTest (X11), 3) RemoteDesktop Portal (Wayland ohne Root).
**Evidence:** `src/server/input_service.rs`, Zeile 470-520.

### 4.3 macOS: CGEvent / VirtualInput

```rust
// src/server/input_service.rs, Zeile 430-470
#[cfg(target_os = "macos")]
lazy_static! {
    static ref QUEUE: Queue = Queue::main();
}

#[cfg(target_os = "macos")]
struct VirtualInputState {
    virtual_input: VirtualInput,
    capslock_down: bool,
}

// Input muss im Main-Thread laufen (sonst Crash auf macOS >= 10.15)
#[cfg(target_os = "macos")]
pub fn handle_key(evt: &KeyEvent) {
    let evt = evt.clone();
    QUEUE.exec_async(move || handle_key_(&evt));
    key_sleep();  // 12ms Sleep fuer Key-Event-Verarbeitung
}
```

### 4.4 Input-Service: Event-Verarbeitung

**Quelle:** `src/server/input_service.rs` [^148^]

```rust
// src/server/input_service.rs, Zeile 700-800
pub fn handle_mouse_(evt: &MouseEvent, conn: i32, ...) {
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    let mut en = ENIGO.lock().unwrap();
    
    match evt_type {
        MOUSE_TYPE_MOVE => {
            set_relative_mouse_active(conn, false);
            en.mouse_move_to(evt.x, evt.y);
        }
        MOUSE_TYPE_MOVE_RELATIVE => {
            // Relative Mausbewegung fuer Gaming/3D
            set_relative_mouse_active(conn, true);
            const MAX_DELTA: i32 = 10000;
            en.mouse_move_relative(
                evt.x.clamp(-MAX_DELTA, MAX_DELTA),
                evt.y.clamp(-MAX_DELTA, MAX_DELTA)
            );
        }
        MOUSE_TYPE_DOWN => match buttons {
            MOUSE_BUTTON_LEFT => en.mouse_down(MouseButton::Left).ok(),
            MOUSE_BUTTON_RIGHT => en.mouse_down(MouseButton::Right).ok(),
            MOUSE_BUTTON_WHEEL => en.mouse_down(MouseButton::Middle).ok(),
            MOUSE_BUTTON_BACK => en.mouse_down(MouseButton::Back).ok(),
            MOUSE_BUTTON_FORWARD => en.mouse_down(MouseButton::Forward).ok(),
        },
        MOUSE_TYPE_WHEEL | MOUSE_TYPE_TRACKPAD => {
            en.mouse_scroll_y(y);
            en.mouse_scroll_x(x);
        }
    }
}
```

---

## 5. P2P-Client-Logik und Verbindungsaufbau

### 5.1 Verbindungsaufbau (client.rs)

**Quelle:** `src/client.rs` [^119^]

```rust
// src/client.rs, Zeile 200-350
async fn _start_inner(peer: String, key: String, ...) -> ResultType<(...)> {
    // 1. Zu Rendezvous-Server verbinden
    let mut socket = connect_tcp(&*rendezvous_server, CONNECT_TIMEOUT).await?;
    
    // 2. NAT-Typ bestimmen
    let my_nat_type = crate::get_nat_type(100).await;
    
    // 3. UDP Socket fuer NAT Traversal erstellen
    let (udp_socket, udp_port) = if crate::get_udp_punch_enabled() {
        new_direct_udp_for(&rendezvous_server).await?
    } else { (None, None) };
    
    // 4. Punch Hole Request senden
    msg_out.set_punch_hole_request(PunchHoleRequest {
        id: peer.to_owned(),
        token: token.to_owned(),
        nat_type: nat_type.into(),
        licence_key: key.to_owned(),
        udp_port: udp_nat_port as _,
        force_relay: interface.is_force_relay(),
        ...
    });
    
    // 5. Auf Rendezvous-Antwort warten (max 3 Versuche)
    for i in 1..=3 {
        socket.send(&msg_out).await?;
        if let Some(msg_in) = crate::get_next_nonkeyexchange_msg(&mut socket, Some(i * 3000)).await {
            match msg_in.union {
                Some(PunchHoleResponse(ph)) => {
                    // Peer-Adresse erhalten -> P2P-Verbindung versuchen
                    peer_addr = AddrMangle::decode(&ph.socket_addr);
                    peer_nat_type = ph.nat_type();
                    relay_server = ph.relay_server;
                    break;
                }
                Some(RelayResponse(rr)) => {
                    // Relay-Verbindung aufbauen
                    return Self::create_relay(&peer, rr.uuid, rr.relay_server, &key, ...).await;
                }
            }
        }
    }
    
    // 6. Direkte Verbindung zu Peer herstellen (TCP/UDP/IPv6 parallel)
    Self::connect(my_addr, peer_addr, &peer, signed_id_pk, &relay_server, ...).await
}
```

### 5.2 Rendezvous Mediator (NAT Traversal)

**Quelle:** `src/rendezvous_mediator.rs` [^161^]

```rust
// src/rendezvous_mediator.rs, Zeile 500-600
async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&ph.socket_addr);
    let relay = use_ws() || Config::is_proxy() || ph.force_relay;
    
    // Bei symmetrischem NAT -> Relay
    if ph.nat_type.enum_value() == Ok(NatType::SYMMETRIC)
        || Config::get_nat_type() == NatType::SYMMETRIC as i32
        || relay {
        let uuid = Uuid::new_v4().to_string();
        return self.create_relay(ph.socket_addr.into(), relay_server, uuid, server, ...).await;
    }
    
    // UDP Hole Punching
    if ph.udp_port > 0 {
        peer_addr.set_port(ph.udp_port as u16);
        self.punch_udp_hole(peer_addr, server, msg_punch, ...).await?;
        return Ok(());
    }
    
    // TCP Hole Punching
    let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let local_addr = socket.local_addr();
    allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
    ...
}

// UDP NAT Traversal mit KCP
async fn udp_nat_listen(socket: Arc<UdpSocket>, peer_addr: SocketAddr, ...) -> ResultType<()> {
    socket.connect(peer_addr).await?;
    let res = crate::punch_udp(socket.clone(), true).await?;
    let stream = KcpStream::accept(socket, Duration::from_millis(CONNECT_TIMEOUT as _), res).await?;
    crate::server::create_tcp_connection(server, stream.1, peer_addr_v4, true, ...).await?;
    Ok(())
}
```

**Claim:** RustDesk verwendet TCP/UDP Hole Punching fuer P2P-Verbindungen, mit automatischem Fallback auf Relay bei symmetrischem NAT. UDP-Traversal nutzt KCP fuer zuverlaessige Uebertragung.
**Evidence:** `src/rendezvous_mediator.rs`, Zeile 500-600, `handle_punch_hole()` mit NAT-Typ-Erkennung.

### 5.3 Sichere Verbindung (NaCl)

```rust
// src/client.rs, Zeile 500-560
async fn secure_connection(peer_id: &str, signed_id_pk: Vec<u8>, key: &str, conn: &mut Stream) 
    -> ResultType<Option<Vec<u8>>> {
    // Ed25519 Signatur-Pruefung
    let rs_pk = get_rs_pk(if key.is_empty() { config::RS_PUB_KEY } else { key });
    
    if let Some(rs_pk) = rs_pk {
        if let Ok((id, pk)) = decode_id_pk(&signed_id_pk, &rs_pk) {
            if id == peer_id {
                sign_pk = Some(sign::PublicKey(pk));
            }
        }
    }
    
    // X25519 Key Exchange
    let (asymmetric_value, symmetric_value, key) = create_symmetric_key_msg(their_pk_b);
    conn.set_key(key);  // NaCl symmetric encryption
}
```

---

## 6. UI-Architektur: Flutter/Rust Integration

### 6.1 Flutter-Dateistruktur

```
flutter/
├── lib/
│   ├── main.dart              # App-Entrypoint
│   ├── mobile/pages/          # Mobile UI Screens
│   ├── desktop/pages/         # Desktop UI Screens
│   ├── models/                # Datenmodelle
│   ├── common.dart            # Gemeinsame Utilities
│   └── ...
├── lib/desktop/
├── lib/mobile/
├── lib/web/
├── assets/
├── android/
├── ios/
├── macos/
├── windows/
└── linux/
```

### 6.2 Rust-Bridge (flutter_rust_bridge)

RustDesk verwendet `flutter_rust_bridge` fuer die Kommunikation zwischen Dart/Flutter und Rust [^202^]. Die Rust-Seite exportiert Funktionen ueber FFI, die von Dart aufgerufen werden.

**Wichtige Rust-FFI-Datei:** `src/flutter_ffi.rs`

```rust
// src/flutter_ffi.rs (aus Logs identifiziert)
// Schnittstelle zwischen Flutter UI und Rust-Backend
#[cfg(feature = "flutter")]
pub fn handle_ui_event(event: String) -> String { ... }

// Session-Management
#[cfg(feature = "flutter")]
pub fn session_start(id: String, password: String) -> ResultType<()> { ... }
```

### 6.3 Kommunikationsfluss

1. **Flutter UI** -> Dart FFI Call -> **Rust Function** (flutter_rust_bridge)
2. **Rust Backend** -> Event/Callback -> **Flutter UI** (via Platform Channel)
3. **Rust Core** -> Protobuf Messages -> **Peer** (Netzwerk)

---

## 7. Client-Server Kommunikation

### 7.1 Nachrichtenprotokoll (Protobuf)

**Quelle:** `hbb_common/src/message_proto.rs` (generiert aus .proto)

Wichtige Nachrichtentypen:
- `Message` - Container mit `union` fuer verschiedene Subtypen
- `VideoFrame` - Enkodierte Videoframes (VP8s, VP9s, H264s, H265s, Av1s)
- `KeyEvent` / `MouseEvent` - Input-Events
- `AudioFrame` / `AudioFormat` - Audio-Daten
- `ClipboardNonFile` / `ClipboardFile` - Clipboard
- `LoginRequest` / `LoginResponse` - Authentifizierung
- `Misc` - Verschiedenes (SwitchDisplay, etc.)

### 7.2 Connection (src/server/connection.rs)

```rust
// src/server/connection.rs
// Verwaltet eine einzelne Client-Verbindung
pub struct Connection {
    id: i32,
    stream: Stream,
    conn_type: ConnType,
    authorized: bool,
    // Services: Video, Audio, Input, Clipboard
    video_subscribed: bool,
    input_enabled: bool,
    clipboard_enabled: bool,
}
```

---

## 8. Zusammenfassung der Implementierungsmuster

### 8.1 Screen-Capture

| Plattform | API | Fallback | Datei |
|-----------|-----|----------|-------|
| Windows | DXGI Desktop Duplication | GDI (BitBlt) | `libs/scrap/src/dxgi/mod.rs` |
| Linux/X11 | MIT-SHM | XGetImage | `libs/scrap/src/x11/` |
| Linux/Wayland | PipeWire (xdg-desktop-portal) | - | `libs/scrap/src/wayland/pipewire.rs` |
| macOS | Quartz Display Services | - | `libs/scrap/src/quartz/` |

### 8.2 Video-Codec Pipeline

| Codec | Encoder | Hardware | Datei |
|-------|---------|----------|-------|
| VP8/VP9 | libvpx (VPXEncoder) | Nein | `libs/scrap/src/common/vpxcodec.rs` |
| AV1 | aom (AomEncoder) | Nein | `libs/scrap/src/common/aom.rs` |
| H264/H265 | hwcodec (FFmpeg) | NVENC, QSV, VAAPI, VideoToolbox | Extern: `rustdesk-org/hwcodec` |
| H264/H265 | vram (D3D11 Texture) | NVIDIA/Intel DirectX | `libs/scrap/src/common/vram.rs` |

### 8.3 Input-Injection

| Plattform | API | Modus | Datei |
|-----------|-----|-------|-------|
| Windows | SendInput (winuser) | Absolut + Relativ | `libs/enigo/src/win/win_impl.rs` |
| Linux/X11 | XTest (xdo) | Absolut | `libs/enigo/src/linux/` |
| Linux/Wayland | uinput (Kernel) | Absolut | `src/server/uinput.rs` |
| Linux/Wayland | RemoteDesktop Portal | Absolut | `src/server/rdp_input.rs` |
| macOS | CGEvent (VirtualInput) | Absolut | `libs/enigo/src/macos/` |

### 8.4 P2P-Verbindung

| Phase | Protokoll | Details |
|-------|-----------|---------|
| Registrierung | TCP/UDP | Peer-ID + Public Key beim Rendezvous-Server registrieren |
| NAT Discovery | UDP STUN | NAT-Typ (Symmetric/Asymmetric/Unknown) ermitteln |
| Hole Punching | TCP + UDP | Gleichzeitiger Verbindungsversuch von beiden Seiten |
| Fallback | TCP Relay | Bei symmetrischem NAT: Relay-Server verwenden |
| Verschluesselung | NaCl | X25519 Key Exchange + Ed25519 Signatur |

---

## 9. Referenzen

[^6^] https://github.com/rustdesk/rustdesk - GitHub Repository (README, File Structure)
[^15^] https://github.com/rustdesk-org/hwcodec - Hardware Codec Library
[^33^] https://github.com/rustdesk/rustdesk/tree/master/libs/scrap/src - Screen Capture Module
[^34^] https://github.com/rustdesk/rustdesk/blob/master/libs/scrap/src/dxgi/mod.rs - DXGI Implementation
[^37^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/libs/scrap/src/dxgi/mod.rs - Raw DXGI Code
[^38^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/libs/scrap/src/wayland.rs - Wayland Module
[^40^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/libs/scrap/src/wayland/pipewire.rs - PipeWire Implementation
[^42^] https://github.com/rustdesk/rustdesk/tree/master/libs/enigo/src - Input Library
[^114^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/libs/enigo/src/win/win_impl.rs - Windows Input
[^119^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/src/client.rs - Client Connection
[^120^] https://github.com/rustdesk/rustdesk/tree/master/src/server - Server Modules
[^133^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/src/server/video_service.rs - Video Service
[^148^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/src/server/input_service.rs - Input Service
[^161^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/src/rendezvous_mediator.rs - Rendezvous Mediator
[^202^] https://www.kore-ledger.net/blog/2024/04/26/flutter-rust-bridge - Flutter Rust Bridge
[^203^] https://github.com/rustdesk/rustdesk/discussions/10540 - hwcodec Logs
[^204^] https://raw.githubusercontent.com/rustdesk/rustdesk/refs/heads/master/libs/scrap/src/common/codec.rs - Codec Implementation

---

*Dokument erstellt am: 2025-01-09*
*Analysiertes Repository: https://github.com/rustdesk/rustdesk (master branch)*
*Searches durchgefuehrt: 20+ (Web + GitHub Code)*
