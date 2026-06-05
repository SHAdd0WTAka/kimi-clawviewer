# Dim 07 – WebRTC P2P & NAT-Traversal fuer Remote Desktop

## Uebersicht

Dieses Dokument analysiert WebRTC-Implementierungsmuster fuer eine Remote-Desktop-Anwendung mit P2P-Architektur. Es behandelt libwebrtc-Internals, NAT-Traversal-Algorithmen, Signaling-Protokolle, Video-Track-Handling, DataChannel-Implementierung, Performance-Optimierung und den detaillierten P2P-Handshake.

---

## 1. libwebrtc Internals

### 1.1 RTCPeerConnection-Architektur

Die `RTCPeerConnection` ist das zentrale API-Element in WebRTC. In libwebrtc wird sie durch drei essentielle Threads angetrieben [^243^]:

| Thread | Verantwortlichkeit |
|--------|-------------------|
| **network_thread** | Schreibt die tatsaechlichen Media-Packets, minimale Verarbeitung |
| **worker_thread** | Ressourcenintensivste Verarbeitung von Video/Audio-Streams (Encoding/Decoding) |
| **signaling_thread** | Externe API-Funktionen und Callbacks der PeerConnection |

libwebrtc verwendet das Makro `RTC_DCHECK_RUN_ON` um sicherzustellen, dass Funktionen auf dem korrekten Thread ausgefuehrt werden [^243^]. Die Entry-Point-Funktion ist typischerweise `CreatePeerConnectionFactory`.

### 1.2 ICE-Agent und ICE-Transport

Der ICE-Transport verwaltet unabhaengig Candidate-Pair-Checks, Consent-Verifikation und Failure-Detection [^232^]:

**ICE Transport States:**

| State | Bedeutung |
|-------|-----------|
| `new` | Transport existiert, aber keine Checks gestartet |
| `checking` | Candidate-Pair-Checks laufen |
| `connected` | Eine brauchbare Pair wurde gefunden, weitere Checks moeglich |
| `completed` | Gathering abgeschlossen, End-of-Candidates signalisiert, finales Pair ausgewaehlt |
| `disconnected` | Voruebergehender Konnektivitaetsverlust, Wiederherstellung wird versucht |
| `failed` | Alle Checks erschoepft, keine funktionierende Pair gefunden |
| `closed` | Transport wurde heruntergefahren |

**Kritische Rueckwaertsuebergaenge (Back Edges):** [^232^]
- `connected -> checking`: Wenn Consent auf dem aktiven Pair widerrufen wird
- `completed -> checking`: Bei ICE-Restart durch Renegotiation
- `connected -> disconnected`: Transiente Netzwerkunterbrechung
- `disconnected -> checking`: Neue Candidate-Pairs werden verfuegbar

### 1.3 ConnectionState (Aggregierter Zustand)

Die `connectionState`-Eigenschaft kombiniert ICE- und DTLS-Transport-Zustaende [^232^]:

1. `closed` – wenn ICE-Aggregat closed ist
2. `failed` – wenn ICE-Aggregat failed ODER DTLS failed
3. `disconnected` – wenn ICE-Aggregat disconnected
4. `new` – wenn ICE-Aggregat new und alle DTLS new/closed
5. `connected` – wenn ICE-Aggregat connected und alle DTLS connected/closed
6. `connecting` – sonst (Catch-All)

> **Wichtig:** `connectionState = "connected"` bedeutet, dass sowohl ICE einen funktionierenden Pfad gefunden hat ALS AUCH DTLS seinen Handshake abgeschlossen hat. Es ist das definitive Signal, dass der Medienkanal vollstaendig funktioniert [^232^].

### 1.4 DTLS-Transport

Der DTLS-Transport fuehrt den Handshake ueber den von ICE etablierten Pfad durch [^284^]:

1. **SDP Fingerprint Exchange**: Jeder Peer enthaelt den SHA-256-Fingerprint seines DTLS-Zertifikats im `a=fingerprint`-Attribut
2. **DTLS ClientHello/ServerHello**: Nach ICE-Connectivity-Checks beginnt der DTLS-Handshake
3. **Zertifikatsaustausch und Verifikation**: Jeder Peer verifiziert das Zertifikat gegen den SDP-Fingerprint
4. **Schluesselableitung via `use_srtp`-Extension**: Der Master-Secret wird durch die DTLS-SRTP-Key-Derivation zu SRTP-Master-Keys abgeleitet
5. **SRTP-Context-Initialisierung**: Alle nachfolgenden Medienpakete werden mit AES-128-CTR verschluesselt und mit 80-bit HMAC-SHA1 authentifiziert [^284^]

Die WebRTC-Oekosystem migriert aktiv von DTLS 1.2 zu DTLS 1.3 (RFC 9147), wobei DTLS 1.3 den Handshake von zwei auf einen Round-Trip reduziert [^284^].

---

## 2. NAT-Traversal-Algorithmen

### 2.1 ICE (Interactive Connectivity Establishment)

ICE ist das Protokoll, das WebRTC fuer NAT-Traversal verwendet. Es sammelt verschiedene Adressen, die das Geraet nutzen kann [^235^]:

- **Host Candidates** – Lokale IP-Adressen des Geraets
- **Server Reflexive Candidates** – Oeffentliche IP-Adressen (via STUN-Server)
- **Relay Candidates** – Relay-IP-Adressen (via TURN-Server)

### 2.2 STUN (Session Traversal Utilities for NAT)

STUN-Server ermoeglichen es Peers, ihre oeffentlichen Netzwerkadressen zu entdecken [^267^]. Wenn ein Peer einen STUN-Server kontaktiert, erhaelt er Informationen darueber, wie seine Pakete im oeffentlichen Internet erscheinen, einschliesslich der externen IP-Adresse und Port-Mappings, die von NAT-Geraeten erstellt werden.

**STUN-Tests:** Ein STUN-Server funktioniert korrekt, wenn er Candidates vom Typ `srflx` (server reflexive) generieren kann [^263^].

### 2.3 TURN (Traversal Using Relays around NAT)

TURN-Server bieten einen Fallback-Mechanismus, wenn direkte P2P-Verbindungen nicht moeglich sind [^294^]:

1. **Client Authentication**: WebRTC-Client authentifiziert sich beim TURN-Server
2. **Allocation Request**: Client fordert Ressourcen-Allokation an
3. **Relay Address Assignment**: TURN-Server weist eine oeffentliche Relay-Adresse zu
4. **Media Relay**: Der gesamte Medien-Traffic wird ueber den TURN-Server geleitet

TURN kann verschiedene Transportprotokolle verwenden [^294^]:
- **TURN/UDP** – Bevorzugt fuer Echtzeit-Medien
- **TURN/TCP** – Fallback wenn UDP nicht erreichbar
- **TURN/TLS** – Worst-Case-Szenario

### 2.4 ICE Candidate Gathering

Der Gathering-Prozess laeuft ueber die `iceGatheringState`-Maschine [^232^]:

```
new -> gathering -> complete
```

Mit Trickle ICE erreicht der Gathering-State oft `complete`, bevor der ICE-Connectivity-State alle Pairs geprueft hat [^232^].

**Candidate-Typen nach Prioritaet (hoechste zuerst):** [^264^]
1. Host candidates (direkte Verbindung)
2. Server reflexive (STUN)
3. Relay (TURN)

### 2.5 ICE Nomination

Es gibt zwei Nomination-Strategien [^324^]:

**Regular Nomination:**
- Der Controlling Agent wartet, bis alle Checks abgeschlossen sind
- Waehlt die beste valide Pair und sendet einen zweiten STUN-Request mit `USE-CANDIDATE`-Flag
- Flexibler aber langsamer (zusätzlicher Round-Trip)

**Aggressive Nomination:**
- Das `USE-CANDIDATE`-Flag wird in jedem STUN-Request mitgesendet
- Sobald der erste Check erfolgreich ist, ist ICE fuer diesen Media-Stream abgeschlossen
- Schneller aber kann zu transienten Selektionswechseln fuehren [^324^]

**libwebrtc's "Continuous Nomination":** [^323^]
- Die erste erfolgreiche Pair wird sofort als `selected_connection` gewaehlt
- Wenn eine hoeher-priorisierte Pair spaeter erfolgreich ist, wird dynamisch umgeschaltet
- Eine Stabilisierungsperiode verhindert uebermaessiges Umschalten

### 2.6 ICE Check-Prioritaeten

Die Prioritaet einer Candidate-Pair berechnet sich aus [^326^]:
- **Type Preference**: Host (126) > Server Reflexive (100) > Relay (0)
- **Local Preference**: Geraet-abhaengig
- **Component ID**: RTP (1) > RTCP (2)

### 2.7 STUN/TURN-Server-Libraries fuer Rust

#### turn-rs (mycrl/turn-rs)
- **Reine Rust-Implementierung** eines TURN-Servers [^313^]
- **Performance**: Einzelner Thread dekodiert mit bis zu 5 GiB/s, Forwarding-Latenz unter 35 Mikrosekunden [^307^]
- **Unterstuetzte RFCs**: RFC 3489, RFC 5389, RFC 5769, RFC 5766, RFC 6062, RFC 6156
- **Features**: TCP/UDP-Transport, Multiple-Interface-Binding, gRPC-API, Prometheus-Metrics
- **Unterschied zu coturn**: Fokus auf Kernfunktionalitaet, keine DB-Speicherung, kein Transport-Layer-Encryption [^313^]

#### coturn (C-Implementierung)
- **Reifste und funktionsreichste TURN-Server-Implementierung** [^332^]
- Bietet die umfassendste RFC-Unterstuetzung
- Herausforderer: Eturnal, Pion Turn

#### webrtc-rs/rtc Sans-I/O Stack
- **Sans-I/O WebRTC-Implementierung** in Rust [^233^]
- **Core API** (8 Methoden): `poll_write()`, `poll_event()`, `poll_read()`, `poll_timeout()`, `handle_read()`, `handle_timeout()`, `handle_write()`, `handle_event()`
- Vollstaendiger Protokoll-Stack: ICE, DTLS, SRTP/SRTCP, SCTP, RTP/RTCP
- Runtime-unabhaengig, testbar ohne Netzwerk-I/O [^233^]

#### str0m
- **Sans-I/O WebRTC-Implementierung** in Rust [^302^]
- Keine internen Threads, keine async-Tasks
- Frame-Level API: `Event::MediaData` fuer vollstaendige Frames
- RTP-Level API verfuegbar fuer SFU-Anwendungen
- **Features**: SDP, ICE, DataChannels, Simulcast, NACK, Transport-Wide-CC, BWE [^302^]
- **Fehlende Features**: Adaptive Jitter Buffer, Video/Audio Capture, Encode/Decode, TURN, Network Interface Enumeration [^302^]

---

## 3. Signaling-Protokoll

### 3.1 SDP Offer/Answer Handshake

WebRTC verwendet das **JavaScript Session Establishment Protocol (JSEP)** (RFC 8829) fuer die SDP-Erzeugung und Verarbeitung [^285^].

**Der vollstaendige Signaling-Flow:** [^256^][^262^]

```
1. Media Preparation
   - Beide Peers rufen getDisplayMedia() auf
   - Tracks werden an RTCPeerConnection angehaengt

2. Offer Creation (Caller)
   - caller.createOffer() -> SDP-Offer
   - caller.setLocalDescription(offer)
   - Offer wird ueber Signaling-Channel an Callee gesendet

3. Answer Creation (Callee)
   - callee.setRemoteDescription(offer)
   - callee.createAnswer() -> SDP-Answer
   - callee.setLocalDescription(answer)
   - Answer wird ueber Signaling-Channel an Caller gesendet

4. ICE Candidate Exchange
   - Beide Peers sammeln ICE Candidates (onicecandidate)
   - Candidates werden ueber Signaling-Channel ausgetauscht
   - callee.addIceCandidate() / caller.addIceCandidate()

5. Direct Connection Established
   - ICE-Negotiation abgeschlossen
   - DTLS-Handshake ueber den gewaehlten Pfad
   - Medien fliessen P2P
```

**Signaling-State-Maschine:** [^232^]
```
stable -> have-local-offer -> stable (nach Answer)
```

### 3.2 SDP-Struktur fuer WebRTC

Eine SDP enthaelt folgende kritische Elemente [^285^]:

| SDP-Element | Beschreibung |
|-------------|-------------|
| `m=` lines + `a=rtpmap` | Codecs und Parameter (z.B. Opus, VP8, H264) |
| `a=mid`, `a=group:BUNDLE` | Transport-Multiplexing |
| `a=rtcp-mux` | RTCP-Multiplexing |
| `a=candidate` | ICE Candidates (IP/Port-Endpunkte) |
| `a=ice-ufrag`, `a=ice-pwd` | ICE-Credentials fuer STUN-Check-Authentifizierung |
| `a=fingerprint` | DTLS-Zertifikat-Fingerprint (SHA-256) |
| `a=setup` | DTLS-Rolle (actpass/active/passive) |
| `a=sendrecv` / `a=sendonly` / `a=recvonly` | Media-Richtung |
| `a=msid` | Korrelation mit MediaStream-Track-IDs |

### 3.3 Beispiel: SDP Offer fuer Remote Desktop

```sdp
v=0
o=- 1234567890 2 IN IP4 127.0.0.1
s=-
t=0 0
a=group:BUNDLE 0 1
a=extmap-allow-mixed
m=video 9 UDP/TLS/RTP/SAVPF 96 97 98
a=mid:0
a=sendrecv
a=rtpmap:96 VP9/90000
a=rtpmap:97 H264/90000
a=fmtp:97 profile-level-id=42e01f;level-asymmetry-allowed=1
a=rtpmap:98 VP8/90000
a=rtcp-fb:* nack
a=rtcp-fb:* nack pli
a=rtcp-fb:* goog-remb
a=rtcp-fb:* transport-cc
a=extmap:1 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01
a=extmap:2 urn:ietf:params:rtp-hdrext:sdes:mid
a=setup:actpass
a=ice-ufrag:abc123
a=ice-pwd:def456ghi789
a=fingerprint:sha-256 AB:CD:EF:...
a=candidate:1 1 UDP 2122252543 192.168.1.100 5000 typ host
a=candidate:2 1 UDP 1685855999 203.0.113.1 5000 typ srflx raddr 192.168.1.100 rport 5000
m=application 9 UDP/DTLS/SCTP webrtc-datachannel
a=mid:1
a=sctp-port:5000
a=max-message-size:262144
```

### 3.4 ICE Candidate Exchange (Trickle ICE)

```javascript
// Sender-Seite: Candidates an den Remote-Peer senden
pc.onicecandidate = event => {
  if (event.candidate) {
    signalingChannel.send(JSON.stringify({
      type: 'candidate',
      candidate: event.candidate
    }));
  }
};

// Empfaenger-Seite: Candidates hinzufuegen
signalingChannel.onmessage = async msg => {
  if (msg.type === 'candidate') {
    await pc.addIceCandidate(new RTCIceCandidate(msg.candidate));
  }
};
```

---

## 4. Video-Track-Handling

### 4.1 getDisplayMedia fuer Screen Capture

Die `getDisplayMedia()` API ermoeglicht die Erfassung von Bildschirminhalten [^257^][^260^]:

```javascript
const displayMediaOptions = {
  video: {
    cursor: "always",           // "always" | "motion" | "never"
    displaySurface: "monitor",   // "monitor" | "window" | "browser"
    logicalSurface: false,
    width: { ideal: 1920 },
    height: { ideal: 1080 },
    frameRate: { ideal: 30 }
  },
  audio: false
};

const stream = await navigator.mediaDevices.getDisplayMedia(displayMediaOptions);
```

**Wichtige Constraints fuer Remote Desktop:** [^296^][^298^]
- `cursor: "always"` – Mauszeimmer muss immer sichtbar sein
- `displaySurface: "monitor"` – Gesamter Bildschirm wird geteilt
- `degradationPreference: "maintain-resolution"` – Resolution wird gegenueber Frame-Rate bevorzugt

### 4.2 Track zum PeerConnection hinzufuegen

```javascript
const videoTrack = stream.getVideoTracks()[0];
const sender = pc.addTrack(videoTrack, stream);

// Degradation-Preference auf Aufloesung setzen (wichtig fuer Remote Desktop!)
const params = sender.getParameters();
params.degradationPreference = "maintain-resolution";
await sender.setParameters(params);
```

Die `degradationPreference` hat drei Modi [^334^]:
- `balanced` – Standard, balanciert Framerate und Resolution
- `maintain-framerate` – Resolution wird reduziert, Framerate beibehalten
- `maintain-resolution` – Framerate wird reduziert, Resolution beibehalten
- `maintain-framerate-and-resolution` – Beides wird beibehalten (kann zu Frame-Drops fuehren)

### 4.3 Video Frame Encoding Pipeline

Die Latenz der Video-Pipeline setzt sich zusammen aus [^321^][^327^]:

| Komponente | Typische Latenz |
|------------|-----------------|
| Kamera/Bildschirm-Capture | ~50-100ms (dominierender Faktor) |
| H264 Encoding | ~10ms |
| WebRTC Transport (Jitter Buffer) | ~7-10ms lokal, ~10ms remote |
| H264 Decoding | ~10ms |
| Display Render | ~17ms (60Hz) |

**Wichtige Erkenntnis:** WebRTC selbst fuegt nur etwa **10ms Latenz** hinzu, selbst bei Remote-Verbindungen mit TURN-Relay [^321^].

### 4.4 Codec-Negotiation

WebRTC unterstuetzt folgende Video-Codecs [^285^]:

| Codec | Profil | Beschreibung |
|-------|--------|-------------|
| **VP8** | - | RFC 7742, royalty-free, weit verbreitet |
| **H.264** | Baseline Profile | RFC 7742, Hardware-Acceleration verfuegbar |
| **VP9** | - | Bessere Kompression als VP8 |
| **AV1** | - | Neueste Generation, beste Kompression |

**H.264 SDP Parameter:**
```sdp
a=rtpmap:97 H264/90000
a=fmtp:97 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1
```

- `profile-level-id=42e01f` = Baseline Profile, Level 3.1
- `packetization-mode=1` = Non-interleaved Mode (NAL-Units in beliebiger Reihenfolge)

### 4.5 Playout-Delay fuer Latenz-Reduktion

Ein kritischer Optimierungsparameter fuer Remote Desktop ist der **Playout-Delay** (Jitter Buffer) [^298^]:

```javascript
// SDP-Header-Extension fuer Playout-Delay
a=extmap:3 http://www.webrtc.org/experiments/rtp-hdrext/playout-delay
```

Standardmaessig ist der `max_playout_delay` auf 10 Sekunden gesetzt. Fuer Remote Desktop sollte er auf **0** gesetzt werden:

```cpp
// In libwebrtc (C++)
- max_playout_delay_(TimeDelta::Seconds(10))
+ max_playout_delay_(TimeDelta::Zero())
```

Dies fuehrte in Tests zu einer **~90ms Latenz-Reduktion** – die groesste Einzelverbesserung [^298^].

### 4.6 Quantisierungsparameter (QP) Optimierung

Der Quantization Parameter beeinflusst die Kompressionsstaerke und damit die Bildqualitaet [^296^][^298^]:

```cpp
// libwebrtc VP9 Encoder Defaults (Screen Sharing)
- config_->rc_min_quantizer = 8;
- config_->rc_max_quantizer = 52;
+ config_->rc_min_quantizer = 4;
+ config_->rc_max_quantizer = 36;
```

Zusaetzliche Rate-Control-Anpassungen [^296^]:
```cpp
+ config_->rc_undershoot_pct = 100;  // Erlaubt aggressive Bitrate-Reduktion fuer statischen Content
+ config_->rc_overshoot_pct = 15;    // Reduziert Bitrate-Spikes
```

### 4.7 RTCRtpTransceiver und setCodecPreferences

Codecs koennen bevorzugt werden:

```javascript
const transceiver = pc.addTransceiver(videoTrack, {
  direction: 'sendonly',
  streams: [stream],
  sendEncodings: [
    { maxBitrate: 5000000 }  // 5 Mbps fuer Full-HD Screen Sharing
  ]
});

// Codec-Präferenz setzen
const capabilities = RTCRtpSender.getCapabilities('video');
const preferredCodecs = capabilities.codecs.filter(codec =>
  codec.mimeType === 'video/VP9' || codec.mimeType === 'video/H264'
);
transceiver.setCodecPreferences(preferredCodecs);
```

---

## 5. DataChannel-Implementierung

### 5.1 SCTP ueber DTLS

WebRTC DataChannels verwenden **SCTP (Stream Control Transmission Protocol, RFC 4960)** ueber DTLS [^234^][^244^]:

```
Anwendung
    |
DataChannel (WebRTC API)
    |
DCEP (Data Channel Establishment Protocol, RFC 8832)
    |
SCTP (Stream Control Transmission Protocol)
    |
DTLS (Datagram Transport Layer Security)
    |
ICE/UDP (oder ICE/TCP)
```

### 5.2 SCTP-Schluesselmerkmale

SCTP bietet [^234^][^244^]:
- **Optionale Zuverlaessigkeit** – kann wie UDP unzuverlaessig konfiguriert werden
- **Optionale Ordering** – Nachrichten koennen out-of-order geliefert werden
- **Message-oriented** – Jede Nachricht wird als Einheit geparsed
- **Flow Control** – Wie TCP, aber fuer Echtzeit optimiert
- **Multi-streaming** – Bis zu 65.534 parallele Streams pro Association

### 5.3 SCTP-Association-Aufbau

SCTP verwendet einen **4-Wege-Handshake** [^246^]:

```
Peer A                              Peer B
  | ---- INIT -----------------------> |
  | <--- INIT ACK (mit State Cookie) - |
  | ---- COOKIE ECHO ----------------> |
  | <--- COOKIE ACK ----------------- |
```

Der State-Cookie-Mechanismus schuetzt vor SYN-Flooding-Angriffen [^246^].

### 5.4 DataChannel-Konfiguration

```javascript
// Zuverlaessiger, geordneter Kanal (fuer Text-Input, Datei-Transfer)
const reliableChannel = pc.createDataChannel("input-events", {
  ordered: true,           // Pakete in Reihenfolge
  maxRetransmits: null,    // Unbegrenzte Retransmissions
  protocol: "json"
});

// Unzuverlaessiger, ungeordneter Kanal (fuer Maus-Bewegungen)
const fastChannel = pc.createDataChannel("mouse-movements", {
  ordered: false,          // Out-of-order erlaubt
  maxRetransmits: 0,       // Keine Retransmissions
  maxPacketLifeTime: 100   // 100ms TTL
});
```

### 5.5 DataChannel-Eigenschaften

| Eigenschaft | Beschreibung |
|-------------|-------------|
| `ordered` | Ob Nachrichten in Sendereihenfolge empfangen werden |
| `maxRetransmits` | Maximale Anzahl Retransmissions (0 = unzuverlaessig) |
| `maxPacketLifeTime` | Maximale Lebensdauer einer Nachricht in ms |
| `protocol` | Anwendungsspezifisches Protokoll-Label |
| `negotiated` | Ob der Kanal out-of-band (true) oder via DCEP (false) verhandelt wird |
| `id` | Stream-ID fuer negotiated channels |

### 5.6 DataChannel fuer Input-Events in Remote Desktop

Fuer Remote Desktop Input-Events bietet sich folgende Architektur an [^312^][^319^]:

```javascript
// Maus-Events
const mouseChannel = pc.createDataChannel("mouse", {
  ordered: false,          // Maus-Bewegungen duerfen verloren gehen
  maxPacketLifeTime: 50    // Max 50ms alt
});

// Tastatur-Events
const keyboardChannel = pc.createDataChannel("keyboard", {
  ordered: true,           // Tasten-Reihenfolge ist wichtig
  maxRetransmits: 5        // Zuverlaessig aber nicht unendlich
});

// Event-Format
function sendMouseEvent(type, x, y, button) {
  const event = JSON.stringify({
    type: type,        // "move", "down", "up", "click", "dblclick"
    x: x,              // Relative/absolute Koordinaten
    y: y,
    button: button,    // 0=left, 1=middle, 2=right
    timestamp: performance.now()
  });
  mouseChannel.send(event);
}
```

### 5.7 dcSCTP – Neue SCTP-Implementierung

Google migrierte von `usrsctp` zu **dcSCTP**, einer in Rust geschriebenen Implementierung [^237^][^241^]:
- Fokus auf Sicherheit und Kompatibilitaet
- In-Tree C++ Implementation (jetzt auch in Rust verfuegbar)
- Unterstuetzt alle DataChannel-Features
- `webrtc/dcsctp` Repository auf GitHub [^241^]

---

## 6. Performance-Optimierung

### 6.1 Jitter Buffer

Der Jitter Buffer ist fuer Remote Desktop kritisch. WebRTC's **NetEQ** fuer Audio bietet folgende Mechanismen [^269^]:

**Underrun Histogram:**
- Speichert relative Verzoegerungen ueber die gesamte Session
- 20ms Bucket-Groesse
- Ziel-Level wird aus einem Quantil des Histogramms abgeleitet (z.B. 0.95 Quantil)

**Reorder Optimizer:**
- Speichert umgeordnete Pakete
- Tradeoff-Funktion: `delay_ms + 20ms * loss_percent`
- Iteriert durch alle potenziellen Latenzen und waehlt das Minimum

**Fuer Video/Remote Desktop:**
- `max_playout_delay` sollte auf 0 gesetzt werden [^298^]
- Dies eliminiert den Jitter-Buffer-Puffer und rendert Frames sofort

### 6.2 NACK/PLI – Packet Loss Recovery

**NACK (Negative Acknowledgement):**
- Wird gesendet, wenn ein erwartetes RTP-Paket nicht ankommt [^276^]
- Signalisiert dem Sender, das fehlende Paket erneut zu senden (RTX)
- Funktioniert gut bei <10% Paketverlust [^276^]

**PLI (Picture Loss Indication):**
- Wird ueber RTCP gesendet, wenn ein komplettes Frame verloren geht [^267^]
- Signalisiert dem Sender, ein Keyframe (I-Frame) zu senden
- Weniger streng als FIR (Full Intra Request)

**FIR (Full Intra Request):**
- Explizite Anforderung eines vollstaendigen Keyframes
- Wird bei schwerwiegenden Fehlern oder bei Receiver-Switching verwendet

### 6.3 Bandwidth Estimation (GCC)

WebRTC verwendet **Google Congestion Control (GCC)** [^270^][^272^]:

**Zwei Estimatoren:**
1. **Delay-based Estimator** – Analysiert Paket-Verzoegerungsvariationen (Jitter)
2. **Loss-based Estimator** – Reagiert auf Paketverluste

**Finale BWE = min(delay_estimate, loss_estimate)**

**GCC-Packet-Loss-Schwellen:** [^273^]
- <2% Verlust: BWE weiter erhoehen
- 2-10% Verlust: BWE halten
- >10% Verlust: BWE reduzieren

**Bandwidth Probing:** [^270^]
- Senden von Extra-Paketen um verfuegbare Bandbreite zu testen
- Zwei Techniken: RTX probes (bevorzugt) und Padding probes
- Ziel: Schnelles Ramp-up bei Session-Start (von 300kbps auf mehrere Mbps)

**Transport-wide Congestion Control (TWCC):** [^272^]
- Globaler, monoton steigender Sequence Number ueber alle Media-Streams
- RTCP-Feedback mit Empfangszeitpunkten jedes Pakets
- Sender-seitige Bandwidth-Estimation

### 6.4 RTX (Retransmission)

RTX ermoeglicht die Retransmission verlorener Pakete [^293^]:
- Verwendet separaten RTP-Stream mit eigener Payload-Type
- NACK wird als RTCP-Feedback gesendet
- Verlorene Pakete werden auf dem RTX-Stream erneut gesendet

### 6.5 FEC (Forward Error Correction)

FEC fuegt Redundanz hinzu um Paketverlust zu kompensieren [^285^]:
- Opus bietet eingebaute FEC fuer Audio
- VP8/VP9 unterstuetzen Redundancy-Modes fuer Video
- ULPFEC (Uneven Level Protection FEC) nach RFC 5109

---

## 7. P2P-Handshake im Detail

### 7.1 Verbindungsaufbau Schritt fuer Schritt

Der vollstaendige Verbindungsaufbau einer WebRTC-P2P-Verbindung [^232^][^285^]:

**Phase 1: Signaling (Ueber WebSocket/HTTP)**
```
T=0ms   Caller: createOffer() -> SDP Offer
T=50ms  Caller: setLocalDescription(offer)
T=50ms  Caller: send Offer to Callee via Signaling Server
T=100ms Callee: receive Offer
T=100ms Callee: setRemoteDescription(offer)
T=150ms Callee: createAnswer() -> SDP Answer
T=200ms Callee: setLocalDescription(answer)
T=200ms Callee: send Answer to Caller via Signaling Server
T=250ms Caller: receive Answer
T=250ms Caller: setRemoteDescription(answer)
```

**Phase 2: ICE Gathering (parallel zu Phase 1)**
```
T=0ms   ICE State: new -> gathering
T=10ms  Host candidates gesammelt (lokale IPs)
T=50ms  STUN-Request an STUN-Server gesendet
T=100ms Server reflexive candidates erhalten (oeffentliche IPs)
T=150ms TURN-Allocation-Request gesendet (falls noetig)
T=300ms Relay candidates erhalten (falls TURN noetig)
T=var.  iceGatheringState -> complete
```

**Phase 3: ICE Connectivity Checks**
```
T=250ms ICE State: checking
T=250ms Candidate-Pairs werden gebildet und priorisiert
T=250ms STUN Binding Requests werden gesendet (gepaced)
T=300ms Erste Checks fuer Host-Host-Pairs
T=400ms Erste erfolgreiche Pair -> ICE connected
T=500ms Weitere Checks fuer bessere Pairs
T=700ms Beste Pair nominiert -> ICE completed
```

**Phase 4: DTLS Handshake**
```
T=400ms DTLS ClientHello (ueber ICE-verifizierten Pfad)
T=450ms DTLS ServerHello + Certificate
T=500ms DTLS ClientKeyExchange + ChangeCipherSpec
T=550ms DTLS Finished (beide Seiten)
T=550ms SRTP Keys abgeleitet
```

**Phase 5: Media & Data Flow**
```
T=550ms Connection State: connected
T=550ms Video-Frames werden ueber SRTP gesendet
T=550ms DataChannel (SCTP) ist bereit fuer Nachrichten
```

**Gesamtlatenz fuer Verbindungsaufbau:** ~500-700ms (ohne TURN), ~800-1200ms (mit TURN)

### 7.2 State-Machine-Progression

```
Signaling:      stable -> have-local-offer -> stable
ICE Gathering:  new -> gathering -> complete
ICE Transport:  new -> checking -> connected -> completed
DTLS Transport: new -> connecting -> connected
Connection:     new -> connecting -> connected
```

### 7.3 Latenz-Engpaesse und Optimierungen

| Engpass | Ursache | Optimierung |
|---------|---------|-------------|
| Kamera/USB-Capture | Hardware-Latenz | High-Speed-Kameras, GMSL statt USB [^321^] |
| Jitter Buffer | Frame-Delay fuer Glättung | `max_playout_delay = 0` [^298^] |
| Encoder-Quantizer | Zu hohe Kompression | `rc_max_quantizer = 36`, `rc_min_quantizer = 4` [^296^] |
| ICE Gathering | STUN/TURN-Abfragen | Trickle ICE, aggressive nomination [^323^] |
| ICE Checking | Sequentielle Pacing-Delays | Reduzierung von `XICE_CHECK_PACING_MS` [^323^] |
| DTLS Handshake | 2-RTT fuer DTLS 1.2 | DTLS 1.3 (1-RTT) [^284^] |
| Bandwidth Probing | Konservativer Start | Fruehere Probes nach Verbindungsaufbau [^270^] |

### 7.4 <100ms Latenz fuer Remote Desktop

Um **<100ms Latenz** fuer Remote Desktop zu erreichen [^296^][^298^]:

1. **P2P-Verbindung** ohne TURN-Relay verwenden
2. **Playout-Delay auf 0** setzen
3. **QP optimieren**: `rc_min_quantizer=4`, `rc_max_quantizer=36`
4. **Rate-Control**: `rc_undershoot_pct=100`, `rc_overshoot_pct=15`
5. **Degradation Preference**: `maintain-resolution`
6. **Screencast Mode** in WebRTC aktivieren
7. **Hardware-Encoding** verwenden (NVENC, QuickSync, VAAPI)
8. **Frame-Rate auf 30 FPS** begrenzen (statt 60 FPS)

---

## 8. Remote Desktop Architektur

### 8.1 Komponenten-Architektur

```
+------------------+                      +------------------+
|   Host (Sender)  |                      | Remote (Empfaenger)|
|                  |                      |                  |
|  getDisplayMedia | --MediaStream------> |  <video> Element |
|  Video Encoder   |    (VP9/H264)        |  Video Decoder   |
|  (VP9/H264)      |                      |                  |
|                  | --DataChannel------> |  Input Simulator |
|  Input Receiver  |    (Mouse/Keyboard)  |  (pyautogui)     |
+------------------+                      +------------------+
         ^                                        |
         |         Signaling Server               |
         +-------- (WebSocket/SSE) ---------------+
                        |
                   +---------+
                   | STUN/TURN|
                   |  Server  |
                   +---------+
```

### 8.2 Host-Modus (Screen-Sharing-Seite)

1. Bildschirm mit `getDisplayMedia()` erfassen
2. Video-Track an `RTCPeerConnection` anhaengen
3. Auf DataChannel-Nachrichten (Input-Events) warten
4. Empfangene Events in native System-Events umwandeln

### 8.3 Remote-Modus (Viewer-Seite)

1. PeerConnection mit Offer starten
2. Video-Track vom Host empfangen und anzeigen
3. Maus/Tastatur-Events im Viewport-Fenster erfassen
4. Events ueber DataChannel an Host senden

### 8.4 Input-Event-Protokoll

```json
{
  "type": "mouse",
  "action": "move",
  "x": 1200,
  "y": 800,
  "timestamp": 1699900000000
}
```

```json
{
  "type": "keyboard",
  "action": "keydown",
  "key": "Control",
  "code": "ControlLeft",
  "timestamp": 1699900000010
}
```

---

## 9. Rust-Implementierungsoptionen

### 9.1 webrtc-rs (Async, Tokio-basiert)

- **GitHub**: `github.com/webrtc-rs/webrtc`
- **Lizenz**: MIT/Apache-2.0
- **Features**: Vollstaendige WebRTC-Implementierung, Async/Await, DataChannels
- **v0.17.x**: Letzter Tokio-basierter Release, Bugfixes nur [^308^]
- **Neu**: Sans-I/O Architektur in Entwicklung

### 9.2 str0m (Sans-I/O)

- **GitHub**: `github.com/algesten/str0m`
- **Lizenz**: MIT
- **Features**: Sans-I/O, Frame-Level API, Simulcast, NACK, BWE [^302^]
- **Limitierungen**: Kein Adaptive Jitter Buffer, kein Capture/Encode/Decode [^302^]

### 9.3 webrtc-rs/rtc (Sans-I/O Stack)

- **GitHub**: `github.com/webrtc-rs/rtc`
- **Features**: Runtime-unabhaengig, vollstaendiger Protokoll-Stack [^233^]
- **API**: 8 Methoden (poll/handle fuer write/event/read/timeout)

### 9.4 turn-rs (TURN-Server)

- **GitHub**: `github.com/mycrl/turn-rs`
- **Features**: Reiner Rust-TURN-Server, 5GiB/s Single-Thread [^313^]
- **Verwendung**: Als Crate oder Standalone-Server

---

## 10. Zusammenfassung und Empfehlungen

### Fuer Remote Desktop mit WebRTC:

1. **Verwende P2P ohne TURN** wenn moeglich (geringste Latenz)
2. **Setze max_playout_delay = 0** fuer sofortiges Rendern
3. **Verwende VP9 oder H.264** mit Hardware-Encoding
4. **DegradationPreference = maintain-resolution**
5. **Optimiere QP-Werte** fuer scharfe Textdarstellung
6. **Nutze DataChannels** fuer Input-Events (separater Kanal fuer Maus/Keyboard)
7. **Trickle ICE** fuer schnelleren Verbindungsaufbau
8. **Rust-Stack**: `str0m` oder `webrtc-rs/rtc` fuer Server, `turn-rs` fuer TURN

### Latenz-Budget fuer <100ms Remote Desktop:

| Komponente | Budget |
|------------|--------|
| Capture | 16ms (1 Frame @ 60Hz) |
| Encoding | 5-10ms (Hardware) |
| Network (P2P, lokales Netz) | 1-5ms |
| Jitter Buffer | 0ms (deaktiviert) |
| Decoding | 5-10ms (Hardware) |
| Render | 16ms (1 Frame @ 60Hz) |
| **Gesamt** | **43-57ms** |

---

## Quellenverzeichnis

[^232^] Giacomo Vacca, "Understanding WebRTC State Machines", 2026
[^233^] Hacker News, "Show HN: webrtc-rs/rtc – A Sans-I/O WebRTC Stack for Rust", 2026
[^234^] BlogGeek.me, "Why was SCTP Selected for WebRTC's Data Channel?"
[^235^] BlogGeek.me, "ICE candidates and active connections in WebRTC"
[^237^] WebRTC Blog, "WebRTC's data channel uses dcSCTP instead of usrSCTP"
[^238^] MDN, "RTCPeerConnection: iceConnectionState property"
[^239^] MDN, "RTCDataChannel - Web APIs"
[^240^] WebRTC.Ventures, "Native WebRTC Development: A Guide to libWebRTC and Alternatives"
[^241^] GitHub, "webrtc/dcsctp: An SCTP implementation for WebRTC Data Channels"
[^243^] Dyte.io, "WebRTC 102: Understanding libWebRTC"
[^244^] WebRTC for the Curious, "Data Communication"
[^246^] Yoshihisa Onoue, "SCTP (rfc4960): Underlying Protocol of WebRTC DataChannel"
[^247^] GitHub, "webrtc-rs/webrtc: Async-friendly WebRTC implementation in Rust"
[^248^] Chromium Issues, "Failure to gather ICE candidates should result in 'failed' state"
[^249^] Archive Casouri, "Peer-to-peer Connection with WebRTC in Rust Using webrtc-rs"
[^250^] StackOverflow, "SCTP and WebRTC"
[^251^] RFC 8831, "WebRTC Data Channels"
[^256^] AntMedia, "WebRTC Peer-to-Peer Communication: How P2P Works"
[^257^] MDN, "Using the Screen Capture API"
[^258^] libp2p, "WebRTC with js-libp2p"
[^259^] WebRTC Course, "ICE candidate gathering"
[^260^] MDN, "MediaDevices: getDisplayMedia() method"
[^261^] W3C, "Screen Capture"
[^262^] Dave Kilian, "Setting Up WebRTC The Hard Way"
[^263^] WebRTC Samples, "Trickle ICE"
[^264^] GetStream.io, "Peer-To-Peer (P2P)"
[^266^] arXiv, "Robust Bandwidth Estimation for Real-Time Communication"
[^267^] BlogGeek.me, "PLI (Picture Loss Indication)"
[^268^] VideoSDK, "WebRTC Data Channels: A Comprehensive Guide"
[^269^] WebRTCHacks, "How WebRTC's NetEQ Jitter Buffer Provides Smooth Audio"
[^270^] WebRTCHacks, "Probing WebRTC Bandwidth Probing – why and how in GCC"
[^271^] Meta Engineering, "Optimizing RTC bandwidth estimation with machine learning"
[^272^] Meetecho, "Bandwidth Estimation (BWE) and Janus"
[^273^] TechRxiv, "Server-side Bandwidth Estimation in the WebRTC Ecosystem"
[^274^] GetStream.io, "RTCDataChannel WebRTC Tutorial"
[^275^] Stony Brook University, "Investigating WebRTC BBR as an alternative to GCC"
[^276^] GStreamer Discourse, "how webrtcbin support qos methods like pli and fir and nack"
[^283^] BlogGeek.me, "Packetization in WebRTC"
[^284^] AntMedia, "WebRTC Security: DTLS-SRTP, Encryption, and Token Authorization"
[^285^] VoIPMonitor, "Understanding the WebRTC Protocol"
[^286^] WebRTC for the Curious, "Securing"
[^287^] AudioCodes, "SRTP using DTLS Protocol"
[^288^] WebRTC, "Video Timing RTP Header Extension"
[^289^] Medium, "How we have built a sub-80ms latency open-source remote desktop application"
[^290^] RFC 8827, "WebRTC Security Architecture"
[^291^] WebRTC Security Study
[^292^] StackOverflow, "WebRTC SRTP decryption"
[^293^] WebRTC.googlesource.com, "RTP in WebRTC"
[^294^] BlogGeek.me, "TURN: Traversal Using Relays around NAT"
[^295^] LinuxLinks, "str0m is a Sans I/O WebRTC implementation"
[^296^] GetHopp.app, "Achieving <100 ms Latency for Remote Control with WebRTC"
[^298^] Multi.app, "Making Illegible, Slow WebRTC Screenshare Legible and Fast"
[^300^] GetStream.io, "WebRTC Stun vs Turn Servers"
[^301^] Medium, "What is a TURN Server?"
[^302^] GitHub, "algesten/str0m: A Sans I/O WebRTC implementation in Rust"
[^303^] Metered.ca, "WebRTC Screen Sharing with Javascript"
[^306^] wasm-bindgen Guide, "WebRTC DataChannel Example"
[^307^] Lib.rs, "mycrl-stun: TURN Server implemented by Rust"
[^308^] Docs.rs, "webrtc 0.17.1"
[^309^] GitHub, "webrtc-rs/examples"
[^312^] GitHub, "RustBuddies/desktop-sharing"
[^313^] GitHub, "mycrl/turn-rs: A pure rust implemented turn server"
[^316^] WebRTC.org, "Data channels"
[^319^] Medium, "WebRTC Remote Desktop Application"
[^321^] Transitive Robotics, "WebRTC Latency: A Breakdown"
[^322^] Medium, "Low-Latency video stream without tears"
[^323^] le0.me, "ICE Nomination Strategy Optimization"
[^324^] RFC 5245, "Interactive Connectivity Establishment (ICE)"
[^325^] StackOverflow, "h264 via WebRTC latency issue"
[^326^] Vocal.com, "Interactive Connectivity Establishment (ICE)"
[^327^] WebRTC for the Curious, "Debugging"
[^328^] StackOverflow, "Where is the nomination flag in ICE STUN request packet"
[^332^] WebRTC Developers, "Coturn, the fragile colossus"
[^333^] WebRTCHacks, "Real-Time Video Processing with WebCodecs"
[^334^] MDN, "RTCRtpSender: setParameters() method"
[^337^] W3C, "WebRTC 1.0: Real-time Communication Between Browsers"
[^338^] UDN, "Using the Screen Capture API"
[^343^] WebRTC.org, "Media capture and constraints"
[^344^] MDN, "MediaDevices: getDisplayMedia() method"
[^346^] StackOverflow, "displaySurface constraint not restricting user share screen selection"
