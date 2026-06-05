# Dim 12 – TTS & Audio-Pipeline: Recherchebericht

> **Projekt:** ClawViewer – Remote-Desktop KI-Anwendung mit TTS-Sprachausgabe
> **Autor:** Integration-Architekt
> **Datum:** 2026-01-20
> **Quellen:** 20+ Web-Searches, GitHub-Repositories, API-Dokumentationen

---

## Inhaltsverzeichnis

1. [Piper TTS – Lokale Rust-Integration](#1-piper-tts--lokale-rust-integration)
2. [Coqui TTS – Python-basiert & ONNX-Export](#2-coqui-tts--python-basiert--onnx-export)
3. [Edge TTS – Microsoft Online-TTS](#3-edge-tts--microsoft-online-tts)
4. [ElevenLabs API – Cloud-TTS](#4-elevenlabs-api--cloud-tts)
5. [OpenAI TTS API](#5-openai-tts-api)
6. [WebRTC Audio Track & Streaming](#6-webrtc-audio-track--streaming)
7. [Lokale Audio-Wiedergabe in Rust](#7-lokale-audio-wiedergabe-in-rust)
8. [Vergleich & Empfehlungen](#8-vergleich--empfehlungen)
9. [Architektur-Entscheidungen für ClawViewer](#9-architektur-entscheidungen-für-clawviewer)
10. [Quellenverzeichnis](#10-quellenverzeichnis)

---

## 1. Piper TTS – Lokale Rust-Integration

### 1.1 Übersicht

**Piper** ist ein schnelles, lokales neuronales Text-to-Speech-System, entwickelt vom Rhasspy-Team (Michael Hansen / synesthesiam). Es ist auf Raspberry Pi 4 optimiert und ermöglicht hochwertige Sprachsynthese ohne Cloud-Abhängigkeit [^461^].

**Wichtiger Hinweis:** Das Repository `rhasspy/piper` wurde am 6. Oktober 2025 archiviert. Die aktive Entwicklung wurde zu `OHF-Voice/piper1-gpl` verschoben [^527^].

### 1.2 Technische Basis

- **Modell-Architektur:** VITS (Variational Inference with adversarial learning for end-to-end Text-to-Speech) [^461^]
- **Inferenz-Format:** ONNX (Open Neural Network Exchange)
- **Qualitätsstufen:** `x_low` (16kHz) bis `high` (22.05kHz) [^461^]
- **Lizenz:** MIT License
- **Sprachen:** Multiple – über 27 Sprachmodelle verfügbar [^475^]

### 1.3 Rust-Crates für Piper

Es existieren mehrere Rust-Crates für die Piper-Integration:

#### Option 1: `piper-rs` (Empfohlen)

```toml
[dependencies]
piper-rs = "0.1.9"
```

- **Repository:** https://github.com/thewh1teagle/piper-rs [^528^]
- **Downloads:** ~13.700 all time [^511^]
- **Features:**
  - Kompatibilität mit allen Piper TTS-Modellen
  - Multi-Sprach-Support
  - Pure Rust Implementierung
  - High-Performance ONNX-Inferenz
  - Stimmen-Modelle von HuggingFace `rhasspy/piper-voices` [^475^]

**Beispiel-Code:**
```rust
use piper_rs::tts::PiperTts;

let tts = PiperTts::new("path/to/model.onnx")?;
let audio = tts.synthesize("Hello, this is a test.")?;
```

#### Option 2: `piper-tts-rs`

```toml
[dependencies]
piper-tts-rs = "0.1"
```

- **Repository:** github.com/WrldEngine/piper-tts-rs [^455^]
- **Größe:** 17.1 KiB, 86 SLoC
- **Abhängigkeit:** `piper-tts-rs-sys` (FFI-Bindings mit bindgen) [^459^]
- **Build-Requirements:** clang, cmake, CUDA-optional (Feature: `cpu`/`cuda`)

#### Option 3: `piper-tts-rs-sys` (Low-Level)

- Raw FFI-Bindings zu Piper C++
- 1.5K SLoC, 1MB
- Unterstützt CPU und CUDA Features [^459^]

### 1.4 Stimmen-Modelle

- **Offizielle Quelle:** https://huggingface.co/rhasspy/piper-voices [^475^]
- **Format:** Jedes Modell besteht aus zwei Dateien:
  - `.onnx` – Das neuronale Netzwerk
  - `.onnx.json` – Modell-Konfiguration [^467^]
- **Download:** Automatisch bei erster Verwendung oder manuell
- **Multi-Speaker:** Einige Modelle unterstützen mehrere Sprecher [^467^]

### 1.5 Performance & Latenz

| Metrik | Wert | Quelle |
|--------|------|--------|
| RTF (Real-Time Factor) auf Intel i5 | ~0.06 (1.9s Output in ~120ms) | [^488^] |
| Raspberry Pi 4 | Echtzeit (~1x RTF) | [^488^] |
| Raspberry Pi Zero 2 | ~1x RTF (gleiche Zeit wie Output) | [^488^] |
| TTFB (Time to First Byte) | Variiert je nach Modellgröße | [^480^] |
| Subprozess-Integration | Stabil, einfach zu sandboxen | [^480^] |
| Python API Integration | Geringerer Overhead | [^480^] |

**Streaming-Verhalten:** Piper TTS ist nicht wirklich streaming-fähig, aber Satz-für-Satz-Wiedergabe kann das *Gefühl* von Echtzeit vermitteln [^480^].

### 1.6 Trainingsmöglichkeiten

- Eigene Stimmen können mit nur 4 Wörtern Referenz-Audio trainiert werden [^463^]
- Training über PyTorch, Export nach ONNX
- LJSpeech-kompatibles Format erforderlich (22050Hz mono) [^463^]

---

## 2. Coqui TTS – Python-basiert & ONNX-Export

### 2.1 Übersicht

**Coqui TTS** war eines der populärsten Open-Source-TTS-Projekte. Das Unternehmen wurde Anfang 2024 geschlossen, der Code bleibt aber Open Source [^474^].

### 2.2 XTTS-v2 (Voice Cloning)

- **Fähigkeit:** Voice Cloning in verschiedene Sprachen mit nur 6 Sekunden Audio
- **Sprachen:** 17 Sprachen
- **Emotion & Style Transfer:** Repliziert nicht nur die Stimme, sondern auch emotionalen Tonfall [^474^]
- **Lizenz:** Coqui Public Model License (nicht-kommerziell) [^474^]
- **Latenz:** <150ms Streaming-Latenz mit pure PyTorch auf Consumer-GPU [^474^]

### 2.3 ONNX-Export

- Coqui-Modelle können nach ONNX exportiert werden
- **Pocket TTS ONNX** bietet INT8-quantisierte Modelle für schnelle CPU-Inferenz [^456^]
- **Features des ONNX-Exports:**
  - Zero-shot Voice Cloning
  - Multilinguale Bundles
  - Streaming-Support mit adaptivem Chunking
  - Temperatur-Control für Generation-Diversity [^456^]

### 2.4 Chatterbox-Turbo (Nachfolger)

Chatterbox-Turbo ist ein Nachfolger mit MIT-Lizenz:
- **Parameter:** 350M (distilled one-step decoder)
- **Latenz:** Sub-200ms Inferenz
- **Features:** Emotion exaggeration control, native paralinguistische Tags (`[laugh]`, `[cough]`, `[chuckle]`)
- **Vergleich:** Benchmarked favorably gegen ElevenLabs [^474^]

### 2.5 Rust-Integration

Coqui/XTTS selbst hat keine native Rust-Unterstützung. Integration erfolgt über:
1. **ONNX-Export** + `ort` Crate in Rust
2. **Python-Prozess-Aufruf** aus Rust
3. **HTTP-API** (selbst gehosteter Server)

---

## 3. Edge TTS – Microsoft Online-TTS

### 3.1 Übersicht

**Microsoft Edge TTS** ist ein kostenloser Online-TTS-Dienst, der die "Read aloud"-Funktion von Microsoft Edge nutzt. Es ist ein Drop-in-Ersatz für OpenAI TTS [^457^].

### 3.2 Features

- **Kostenlos** – Keine API-Key-Kosten
- **Stimmen:** Viele Microsoft-Stimmen in verschiedenen Sprachen
- **Docker-Support:** `docker run -d -p 5050:5050 travisvn/openai-edge-tts:latest` [^457^]
- **OpenAI-kompatibel** – Drop-in-Ersatz für OpenAI TTS API

### 3.3 Rust-Integration

```toml
[dependencies]
msedge-tts = "0.3.0"
```

- **Downloads:** 15.344 all time [^479^]
- **Lizenz:** MIT
- **Wrapper** der MS Edge Read Aloud API
- Funktioniert direkt aus Rust ohne zusätzliche Abhängigkeiten

### 3.4 Nachteile

- Keine Garantie auf Verfügbarkeit (inoffizielle API)
- Internetverbindung erforderlich
- Ratenbegrenzungen unklar
- Keine Offline-Nutzung möglich

---

## 4. ElevenLabs API – Cloud-TTS

### 4.1 Übersicht

**ElevenLabs** gilt als Goldstandard für KI-Sprachsynthese. Die Qualität ist branchenführend, aber die Kosten sind hoch [^452^][^453^].

### 4.2 Preisgestaltung (2026)

| Plan | Preis | Credits (TTS) | Concurrency | Audio-Qualität |
|------|-------|---------------|-------------|----------------|
| Free | $0 | 10k Multilingual / 20k Flash | 2 | 128 kbps |
| Starter | $6 | 30k / 60k | 3 | 128 kbps |
| Creator | $22 | 100k / 200k | 5 | 192 kbps |
| Pro | $99 | 500k / 1M | 10 | 192 kbps, 44.1kHz PCM |
| Scale | $299 | 1.8M / 4M | 15 | 192 kbps |
| Business | $990 | 6M / 11M | 15 | 192 kbps + Low-Latency |
| Enterprise | Custom | Custom | Custom | Custom [^453^][^452^] |

**Overage-Raten (TTS):**
- Creator: $0.30 / 1k chars
- Pro: $0.24 / 1k chars
- Scale: $0.18 / 1k chars
- Business: $0.12 / 1k chars [^452^]

### 4.3 Modelle

- **Multilingual v2:** Höchste Qualität, 29+ Sprachen, 192kbps
- **Flash:** Niedrigere Latenz, geringerer Credit-Verbrauch
- **Conversational AI:** Echtzeit-Dialog-Agenten [^454^]

### 4.4 Rust-Integration

```toml
[dependencies]
elevenlabs_tts = "0.2.1"
```

- **Downloads:** 3.709 all time [^479^]
- Type-safe Rust Client für ElevenLabs API

### 4.5 Nachteile

- **6-20x teurer** als OpenAI TTS [^458^]
- Internetverbindung erforderlich
- Ratenbegrenzungen bei niedrigen Plänen
- Keine echte Offline-Nutzung

---

## 5. OpenAI TTS API

### 5.1 Übersicht

OpenAI bietet eine hochwertige TTS-API mit GPT-4o mini TTS als aktuellem Modell [^466^].

### 5.2 Modelle

| Modell | Latenz | Qualität | Preis |
|--------|--------|----------|-------|
| `gpt-4o-mini-tts` | Niedrig | Hoch (empfohlen) | $0.015 / 1K chars |
| `tts-1` | Gering | Standard | $0.015 / 1K chars |
| `tts-1-hd` | Höher | Höher | $0.030 / 1K chars [^466^] |

### 5.3 Stimmen

**13 eingebaute Stimmen** [^466^]:
`alloy`, `ash`, `ballad`, `coral`, `echo`, `fable`, `nova`, `onyx`, `sage`, `shimmer`, `verse`, `marin`, `cedar`

**Empfohlen:** `marin` oder `cedar` für beste Qualität [^466^]

### 5.4 Streaming

```python
async with openai.audio.speech.with_streaming_response.create(
    model="gpt-4o-mini-tts",
    voice="coral",
    input="Today is a wonderful day!",
    response_format="pcm",
) as response:
    await LocalAudioPlayer().play(response)
```

- Chunked Transfer Encoding für Echtzeit-Wiedergabe
- PCM/WAV-Format empfohlen für schnellste Antwortzeiten
- Custom Voices verfügbar über Voice-IDs [^466^]

### 5.5 Vergleich mit ElevenLabs

| Aspekt | OpenAI TTS | ElevenLabs |
|--------|-----------|------------|
| Preis (pro 1k chars) | $0.015 - $0.030 | $0.12 - $0.30 |
| Voice-Qualität | Gut, aber weniger expressiv | Branchenführend |
| Voice-Cloning | Eingeschränkt | Professionell |
| Latenz | Niedrig | Niedrig (Flash) / Hoch (Multilingual) |
| Anpassung | Weniger Optionen | Sehr viele Optionen [^458^] |

---

## 6. WebRTC Audio Track & Streaming

### 6.1 Architektur-Übersicht

WebRTC (Web Real-Time Communication) ermöglicht P2P-Audio-Streaming ohne Plugins [^465^][^505^].

### 6.2 Kernkomponenten

| Komponente | Funktion |
|-----------|----------|
| `getUserMedia()` | Mikrofon-Zugriff, liefert MediaStream |
| `RTCPeerConnection` | P2P-Verbindung zwischen Peers |
| `RTCSessionDescription` | SDP Offer/Answer für Codec-Verhandlung |
| ICE (Interactive Connectivity Establishment) | NAT Traversal, Firewall-Penetration |
| STUN-Server | Ermittlung öffentlicher IP |
| TURN-Server | Relay für direkte Verbindungen [^465^][^505^] |

### 6.3 Audio-Codecs

**Opus** ist der Goldstandard für WebRTC-Audio [^482^][^485^]:

| Eigenschaft | Wert |
|-------------|------|
| Bitrate-Bereich | 6 kbps – 510 kbps |
| Algorithmische Latenz | 2.5ms (CELT mode) |
| Typische Frame-Größe | 20ms (~26.5ms Gesamtlatenz) |
| Browser-Support | Chrome, Firefox, Safari, Edge |
| Adaptive Bitrate | Ja – dynamische Qualitätsanpassung |
| Forward Error Correction | Ja [^482^] |

**Opus Bitrate-Empfehlungen** [^485^]:

| Medientyp | Empfohlene Bitrate |
|-----------|-------------------|
| Narrow-band Speech (NB) | 8-12 kbps |
| Wide-band Speech (WB) | 16-20 kbps |
| Full-band Speech (FB) | 28-40 kbps |
| Full-band mono Musik | 48-64 kbps |
| Full-band stereo Musik | 64-128 kbps |

### 6.4 WebRTC P2P Verbindungsaufbau

1. **Media Preparation** – `getUserMedia()` für Mikrofon
2. **Offer Creation** – SDP Offer mit Codec-Präferenzen
3. **Answer Creation** – SDP Answer vom Empfänger
4. **ICE Candidate Exchange** – STUN/TURN-Kandidaten
5. **DTLS-SRTP Handshake** – Verschlüsselte Verbindung [^505^]

### 6.5 Latenz

| Architektur | Teilnehmer | Server-Bandwidth | Latenz |
|-------------|-----------|-----------------|--------|
| P2P | 2-4 | Nur Signaling | Sub-100ms direkt |
| SFU | 4-100+ | Hoch | Sub-200ms |
| MCU | Beliebig | Hoch (Transcoding) | 200-500ms [^505^] |

### 6.6 Audio Track API (Unity/WebRTC)

```csharp
// AudioStreamTrack mit AudioSource
var track = new AudioStreamTrack(audioSource);
peerConnection.AddTrack(track, sendStream);

// Oder mit Raw Audio
var track = new AudioStreamTrack();
track.SetData(float[] data, int channels, int sampleRate); [^473^]
```

### 6.7 Web-Audio-Integration

```javascript
// WebAudio → WebRTC Audio Track
const audioContext = new AudioContext();
const dest = audioContext.createMediaStreamDestination();
const track = dest.stream.getAudioTracks()[0];
peerConnection.addTrack(track, dest.stream); [^476^]
```

### 6.8 WebRTC für ClawViewer

Für eine Remote-Desktop-KI-Anwendung bietet WebRTC:
- **Geringe Latenz** (<100ms P2P)
- **Verschlüsselte Übertragung** (DTLS-SRTP)
- **Adaptive Qualität** (Opus adaptive Bitrate)
- **Cross-Platform** (alle modernen Browser)
- **Keine Server-Infrastruktur** für Media nötig

---

## 7. Lokale Audio-Wiedergabe in Rust

### 7.1 cpal – Cross-Platform Audio Library

**cpal** ist die fundamentale Rust-Audio-Library für Low-Level-Audio-I/O [^468^].

```toml
[dependencies]
cpal = "0.15"
```

**Features:**
- Cross-Platform: Windows (WASAPI), Linux (ALSA/JACK/PipeWire), macOS (CoreAudio)
- Input & Output Streams
- Callback-basierte API
- Nicht-blockierend – dedizierter High-Priority-Thread [^468^]

**Beispiel – Output Stream:**
```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

let host = cpal::default_host();
let device = host.default_output_device().expect("no output device");
let config = device.default_output_config().unwrap();

let stream = device.build_output_stream(
    &config.into(),
    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        // Audio-Daten in den Buffer schreiben
        for sample in data.iter_mut() {
            *sample = 0.0; // Silence
        }
    },
    move |err| eprintln!("stream error: {}", err),
    None,
).unwrap();

stream.play().unwrap();
```

### 7.2 rodio – High-Level Audio Playback

**rodio** baut auf cpal auf und bietet eine einfachere API [^504^].

```toml
[dependencies]
rodio = "0.21"
```

**Stats:** 5.3M Downloads, MIT/Apache-2.0 [^507^]

**Beispiel – Audio-Datei abspielen:**
```rust
use rodio::{Decoder, DeviceSinkBuilder};
use std::fs::File;

let handle = DeviceSinkBuilder::open_default_sink()
    .expect("open default audio stream");
let player = rodio::Player::connect_new(&handle.mixer());

let file = File::open("audio.mp3").unwrap();
let source = Decoder::try_from(file).unwrap();
handle.mixer().add(source);
```

**Beispiel – Sink für Queue:**
```rust
use rodio::{OutputStream, Sink, source::SineWave};
use std::time::Duration;

let (_stream, stream_handle) = OutputStream::try_default().unwrap();
let sink = Sink::try_new(&stream_handle).unwrap();

sink.append(SineWave::new(440.0).take_duration(Duration::from_secs_f32(0.25)));
sink.sleep_until_end(); [^513^]
```

### 7.3 Symphonia – Audio-Decoding

rodio nutzt Symphonia als Standard-Decoder für:
- FLAC, MP3, MP4, Vorbis, WAV [^504^]

### 7.4 Audio-Queue für TTS-Streaming

Für eine TTS-Anwendung ist eine Audio-Queue essentiell:

```rust
use rodio::{Sink, Decoder};
use std::io::Cursor;

pub struct TtsAudioQueue {
    sink: Sink,
}

impl TtsAudioQueue {
    pub fn enqueue_pcm(&self, pcm_data: Vec<u8>) {
        let cursor = Cursor::new(pcm_data);
        let source = Decoder::new(cursor).unwrap();
        self.sink.append(source);
    }
    
    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }
    
    pub fn stop(&self) {
        self.sink.stop();
    }
}
```

### 7.5 Plattform-Support

| Plattform | cpal | rodio |
|-----------|------|-------|
| Windows (WASAPI) | ✅ | ✅ |
| Linux (ALSA) | ✅ | ✅ |
| Linux (PipeWire) | ✅ | ✅ |
| Linux (JACK) | ✅ | ✅ |
| macOS (CoreAudio) | ✅ | ✅ |
| iOS | ✅ | ✅ |
| Android | ✅ | ✅ |
| WebAssembly | ❌ | ❌ |

---

## 8. Vergleich & Empfehlungen

### 8.1 TTS-Engine-Vergleich

| Aspekt | Piper | Coqui/XTTS | Edge TTS | ElevenLabs | OpenAI |
|--------|-------|-----------|----------|-----------|--------|
| **Offline** | ✅ | ✅ | ❌ | ❌ | ❌ |
| **Kosten** | Kostenlos | Kostenlos | Gratis-Tier | $6-$990/mo | $0.015/1k |
| **Rust nativ** | ✅ (piper-rs) | ❌ (Python) | ✅ (msedge-tts) | ✅ (elevenlabs_tts) | ✅ (reqwest) |
| **Qualität** | Gut | Sehr gut | Gut | Exzellent | Sehr gut |
| **Latenz** | ~120ms (CPU) | <150ms (GPU) | Netzwerk | Netzwerk | Netzwerk |
| **Voice Cloning** | Trainierbar | ✅ 6s Sample | ❌ | ✅ Profi | Eingeschränkt |
| **Modellgröße** | ~50-100MB | ~400MB-2GB | Keine | Keine | Keine |
| **Lizenz** | MIT | Non-commercial | Proprietär | Proprietär | Proprietär |
| **Sprachen** | 27+ | 17 | Viele | 29+ | Mehrere |

### 8.2 Latenz-Vergleich

| System | Gesamtlatenz | TTS-Latenz | Quelle |
|--------|-------------|-----------|--------|
| Piper (Intel i5) | ~120ms | ~120ms | [^488^] |
| Piper (Raspberry Pi 4) | Echtzeit | ~1x RTF | [^488^] |
| XTTS-v2 (Consumer GPU) | <150ms | <150ms | [^474^] |
| Chatterbox-Turbo | <200ms | <200ms | [^474^] |
| E2E Voice Agent Pipeline | ~940ms | ~280ms | [^471^] |
| ElevenLabs (Flash) | ~400ms TTFB | ~400ms | [^489^] |
| OpenAI TTS (Streaming) | Wenige 100ms | Wenige 100ms | [^466^] |

### 8.3 Empfohlene Setup-Kombinationen

#### Szenario A: Vollständig Offline (Privacy-First)
- **TTS:** Piper via `piper-rs` Crate
- **Audio-Output:** rodio/cpal
- **Vorteile:** Keine Internetverbindung, keine Kosten, volle Kontrolle
- **Nachteile:** Weniger natürliche Stimmen, Modell-Größe

#### Szenario B: Hybrid (Offline + Cloud-Backup)
- **TTS:** Piper für Standard-Antworten, OpenAI TTS für komplexe/generierte Inhalte
- **Audio-Output:** rodio/cpal lokal, WebRTC für Remote-Streaming
- **Vorteile:** Flexibilität, Kosteneinsparung für Standard-Antworten

#### Szenario C: Cloud-First (Beste Qualität)
- **TTS:** ElevenLabs oder OpenAI TTS
- **Audio-Output:** WebRTC Audio Track für Remote-Streaming
- **Vorteile:** Beste Stimmqualität, kein Modell-Download
- **Nachteile:** Kosten, Internetabhängigkeit, Latenz

---

## 9. Architektur-Entscheidungen für ClawViewer

### 9.1 Empfohlene Architektur: Mehrstufiges TTS-System

```
┌─────────────────────────────────────────────────────────────┐
│                     ClawViewer App                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ KI-Agent     │  │ TTS-Engine   │  │ Audio-Output │      │
│  │ (MCP-Server) │──│ (Auswahl)    │──│ (rodio/cpal) │      │
│  └──────────────┘  └──────┬───────┘  └──────────────┘      │
│                           │                                  │
│              ┌────────────┼────────────┐                    │
│              ▼            ▼            ▼                    │
│         ┌────────┐  ┌──────────┐  ┌──────────┐             │
│         │ Piper  │  │ Edge TTS │  │ OpenAI   │             │
│         │(local) │  │(online)  │  │ TTS API  │             │
│         └────────┘  └──────────┘  └──────────┘             │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │          WebRTC Audio Track (optional)              │   │
│  │     Für Remote-Desktop Audio-Streaming             │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 9.2 TTS-Auswahl-Logik

```rust
pub enum TtsProvider {
    Piper(PiperConfig),      // Lokale Synthese
    Edge(EdgeConfig),        // Gratis Online
    OpenAI(OpenAiConfig),    // Pay-per-use
    ElevenLabs(ElevenConfig), // Premium
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<AudioData> {
        match self {
            TtsProvider::Piper(config) => config.synthesize(text),
            TtsProvider::Edge(config) => config.synthesize(text).await,
            TtsProvider::OpenAI(config) => config.synthesize(text).await,
            TtsProvider::ElevenLabs(config) => config.synthesize(text).await,
        }
    }
}
```

### 9.3 Audio-Pipeline-Architektur

```
KI-Antwort (Text)
    │
    ▼
┌──────────────────┐
│ Text Chunker     │ ← Satzweise Aufteilung für Streaming
│ (Sentence Split) │
└────────┬─────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐ ┌────────┐
│ TTS #1 │ │ TTS #2 │ ← Parallele Synthese
│ (Satz) │ │ (Satz) │
└───┬────┘ └───┬────┘
    │          │
    ▼          ▼
┌──────────────────┐
│ Audio Queue      │ ← rodio::Sink
│ (FIFO Puffer)    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Audio Output     │ ← cpal Output Stream
│ (Lautsprecher)   │
└──────────────────┘
```

### 9.4 WebRTC-Integration für Remote-Desktop

Für die Remote-Desktop-Komponente:

```rust
// TTS-Audio → WebRTC Audio Track
pub fn create_tts_audio_track(
    audio_data: Vec<f32>,
    sample_rate: u32,
    channels: u32,
) -> AudioStreamTrack {
    let track = AudioStreamTrack::new();
    track.set_data(&audio_data, channels, sample_rate);
    track
}

// Opus-Kodierung für WebRTC
pub fn encode_opus(
    pcm_data: &[f32],
    sample_rate: u32,
) -> Result<Vec<u8>> {
    let mut encoder = opus::Encoder::new(
        sample_rate,
        opus::Channels::Mono,
        opus::Application::Audio,
    )?;
    let mut output = vec![0u8; 4000];
    let len = encoder.encode_float(pcm_data, &mut output)?;
    output.truncate(len);
    Ok(output)
}
```

### 9.5 Empfohlene Crate-Kombination

```toml
[dependencies]
# TTS
piper-rs = "0.1.9"           # Lokale Piper-TTS
msedge-tts = "0.3.0"          # Edge Online-TTS
elevenlabs_tts = "0.2.1"      # ElevenLabs API (optional)

# Audio
rodio = { version = "0.21", default-features = false, features = ["symphonia-all"] }
cpal = "0.15"                  # Low-Level Audio I/O

# WebRTC
webrtc = "0.12"                # Rust WebRTC-Implementation
opus = "0.3"                   # Opus Codec

# Audio-Processing
fundsp = "0.18"                # Optional: Audio-DSP
hound = "3.5"                  # WAV file handling
```

### 9.6 Qualitätsanforderungen für KI-Sprachausgabe

| Anwendungsfall | Mindest-Qualität | Empfohlene Engine |
|----------------|-----------------|-------------------|
| Statusmeldungen | x_low (16kHz) | Piper |
| Benachrichtigungen | medium (22kHz) | Piper / Edge |
| Konversation | high (22kHz) | Piper / OpenAI |
| Präsentation | high + expressive | ElevenLabs / OpenAI HD |
| Echtzeit-Agent | low-latency streaming | Chatterbox / ElevenLabs Flash |

---

## 10. Quellenverzeichnis

### Piper TTS
- [^455^] https://crates.io/crates/piper-tts-rs – Piper TTS Rust Crate
- [^459^] https://lib.rs/crates/piper-tts-rs-sys – Low-Level FFI Bindings
- [^461^] https://sourceforge.net/projects/piper-tts.mirror/ – Piper TTS Mirror/Overview
- [^462^] https://docs.pipecat.ai/api-reference/server/services/tts/piper – Piper TTS API Reference
- [^463^] https://calbryant.uk/blog/training-a-new-ai-voice-for-piper-tts-with-only-4-words/ – Training Custom Voices
- [^464^] https://community.home-assistant.io/t/collections-of-pre-trained-piper-voices/915666 – Pre-trained Voices Collection
- [^467^] https://www.openhab.org/addons/voice/pipertts/ – openHAB Piper Integration
- [^475^] https://huggingface.co/rhasspy/piper-voices – Official Voice Models
- [^480^] https://medium.com/@mail2chasif/livekit-piper-tts-building-a-low-latency-local-voice-agent-with-real-time-latency-tracking-92a1008416e4 – Piper + LiveKit Latency Tracking
- [^483^] https://levelup.gitconnected.com/piper-tts-10x-faster-lightweight-real-time-offline-human-like-voice-text-to-speech-a-google-36ddcdeac8df – Piper Performance Analysis
- [^488^] https://users.rust-lang.org/t/text-to-speech-for-rust/110824 – Rust TTS Discussion (Piper ~120ms)
- [^490^] https://github.com/xd009642/xd-tts – Pure Rust TTS Engine Example
- [^511^] https://crates.io/crates/piper-rs – piper-rs Crate (13.687 Downloads)
- [^527^] https://github.com/rhasspy/piper – Piper GitHub (archiviert)
- [^528^] https://github.com/thewh1teagle/piper-rs – piper-rs GitHub

### Coqui TTS & ONNX
- [^456^] https://huggingface.co/KevinAHM/pocket-tts-onnx – Pocket TTS ONNX Export
- [^474^] https://www.bentoml.com/blog/exploring-the-world-of-open-source-text-to-speech-models – Open-Source TTS Models 2026

### Edge TTS
- [^457^] https://www.reddit.com/r/LocalLLaMA/comments/1g2ceyu/free_microsoft_edge_tts_api_endpoint_local/ – Free Edge TTS API
- [^479^] https://crates.io/keywords/text-to-speech – msedge-tts Crate (15.344 Downloads)

### ElevenLabs
- [^452^] https://flexprice.io/blog/elevenlabs-pricing-breakdown – ElevenLabs Pricing Guide 2026
- [^453^] https://elevenlabs.io/pricing – Offizielle ElevenLabs Preise
- [^454^] https://www.cekura.ai/blogs/elevenlabs-pricing – ElevenLabs Pricing Breakdown
- [^458^] https://www.reddit.com/r/ElevenLabs/comments/17pk48h/elevenlabs_vs_openai_api_pricing/ – ElevenLabs vs OpenAI Pricing
- [^460^] https://elevenlabs.io/pricing/api – ElevenLabs API Pricing

### OpenAI TTS
- [^466^] https://developers.openai.com/api/docs/guides/text-to-speech – OpenAI TTS API Docs
- [^472^] https://community.openai.com/t/your-top-picks-for-tts-api-voices/488714 – TTS Voice Discussion

### WebRTC & Audio Streaming
- [^465^] https://videosdk.live/developer-hub/webrtc/webrtc-audio-stream – WebRTC Audio Streams Guide
- [^471^] https://arxiv.org/html/2508.04721v1 – Low-Latency End-to-End Voice Agents (0.94s E2E)
- [^473^] https://docs.unity3d.com/Packages/com.unity.webrtc@2.4/manual/audiostreaming.html – Unity WebRTC Audio Streaming
- [^476^] https://livekit.com/blog/stream-music-over-webrtc-using-react-and-webaudio – WebRTC + WebAudio
- [^478^] https://softvelum.com/2026/05/opus-zero-transcode-webrtc/ – Opus in WebRTC
- [^482^] https://getstream.io/resources/projects/webrtc/advanced/codecs/ – WebRTC Codecs Guide
- [^485^] https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/WebRTC_codecs – MDN WebRTC Codecs
- [^487^] https://webrtc.ventures/2025/02/understanding-webrtc-codecs/ – Understanding WebRTC Codecs
- [^489^] https://medium.com/async-ai/build-low-latency-voice-into-your-app-with-async-4fad07700501 – Async Low-Latency TTS (<400ms)
- [^505^] https://antmedia.io/how-to-create-webrtc-peer-to-peer-communication/ – WebRTC P2P Communication 2026
- [^509^] https://getstream.io/glossary/webrtc-protocol/ – WebRTC Protocol Overview
- [^512^] https://blog.logrocket.com/webrtc-video-streaming/ – WebRTC P2P Video Streaming

### Rust Audio
- [^468^] https://docs.rs/cpal/latest/cpal/ – cpal Documentation
- [^469^] https://bekk.christmas/post/2023/19/make-some-noise-with-rust – Rust Audio Tutorial
- [^477^] https://lib.rs/crates/any-tts – any-tts Rust Crate
- [^479^] https://crates.io/keywords/text-to-speech – Rust TTS Crates Overview
- [^481^] https://docs.rs/tts – tts Crate (Cross-Platform)
- [^484^] https://huggingface.co/ThreadAbort/IndexTTS-Rust – IndexTTS Pure Rust
- [^504^] https://docs.rs/rodio – rodio Audio Library
- [^506^] https://milvus.io/ai-quick-reference/how-can-developers-integrate-tts-into-their-applications – TTS Integration Guide
- [^507^] https://generalistprogrammer.com/tutorials/rodio-rust-crate-guide – Rodio Guide 2025
- [^510^] https://crates.io/crates/rodio/0.20.1 – rodio Crate Registry
- [^513^] https://lib.rs/crates/youtui-vendored-rodio – rodio Sink Documentation

---

## Zusammenfassung

Für die **ClawViewer Remote-Desktop KI-Anwendung** ergibt sich folgende Architektur-Empfehlung:

### Primäre TTS-Engine: Piper via `piper-rs`
- **Gründe:** Kostenlos, offline, Rust-nativ, ausreichende Qualität für KI-Sprachausgabe, Latenz ~120ms auf moderner CPU
- **Integration:** Direkte Crate-Integration in Tauri/Rust-Backend

### Fallback: OpenAI TTS API
- **Gründe:** 10x günstiger als ElevenLabs, gute Qualität, Streaming-Support
- **Verwendung:** Wenn höchste Qualität benötigt wird oder für bestimmte Sprachen

### Audio-Wiedergabe: rodio + cpal
- **Gründe:** 5.3M Downloads, battle-tested, Cross-Platform, einfache Audio-Queue
- **Integration:** Rust-Backend spielt Audio lokal ab, WebRTC-Track für Remote-Streaming

### Remote-Streaming: WebRTC mit Opus
- **Gründe:** Sub-100ms Latenz P2P, adaptive Bitrate, verschlüsselt, universal unterstützt
- **Integration:** Tauri Frontend (WebView) oder Rust nativ via `webrtc` Crate

### Gesamtlatenz-Ziel
- TTS-Synthese: ~120ms (Piper)
- Audio-Queue + Playback: ~50ms
- WebRTC-Übertragung: ~50-100ms
- **Gesamt:** ~220-270ms für Remote-Sprachausgabe

Diese Architektur bietet die optimale Balance aus **Privatsphäre** (lokale TTS), **Kosteneffizienz** (keine Cloud-Kosten für Standard-Betrieb), **Qualität** (Piper high-Voices) und **Latenz** (sub-300ms Gesamtsystem).
