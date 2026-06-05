# Dim 10 - Sicherheitsarchitektur & Auth-Flows

## Umfassende Recherche fuer P2P-Remote-Desktop-Anwendung

**Datum:** 2025-07-17
**Schritte:** 25+ Web-Searches durchgefuehrt
**Quellen:** 50+ Referenzen mit Inline-Citations

---

## 1. Ed25519-Key-Pairs: Generierung, Signing, Verification in Rust

### 1.1 Ueberblick

Ed25519 ist ein modernes elliptische-Kurven-Signaturverfahren basierend auf Curve25519. Es bietet:
- **Schnelle Verifikation** (~10x schneller als ECDSA)
- **Kompakte Signaturen** (64 Bytes)
- **Kompakte Public Keys** (32 Bytes)
- **Deterministische Signaturen** (kein Zufallszahlengenerator noetig fuer Signing)
- **Keine Nonce-Kollisionen** (im Gegensatz zu ECDSA) [^97^]

### 1.2 Implementierung mit ed25519-dalek

Das `ed25519-dalek` Crate ist die bevorzugte Implementierung in Rust mit ueber 20 Millionen Downloads pro Monat und wird in 9.562+ Crates verwendet [^379^].

#### Cargo.toml

```toml
[dependencies]
ed25519-dalek = "3"
rand_core = "0.6"  # fuer OsRng
```

#### Key-Pair-Generierung

```rust
use rand_core::OsRng;
use ed25519_dalek::{SigningKey, VerifyingKey};

// CSPRNG aus dem Betriebssystem
let mut csprng = OsRng;

// SigningKey generieren (enthaelt Public + Secret Key)
let signing_key: SigningKey = SigningKey::generate(&mut csprng);

// Public Key extrahieren
let verifying_key: VerifyingKey = signing_key.verifying_key();

// Serialisierung fuer Uebertragung/Speicherung
let public_key_bytes: [u8; 32] = verifying_key.to_bytes();
let secret_key_bytes: [u8; 32] = signing_key.to_bytes();
```
[Quelle: docs.rs/ed25519-dalek ^97^]

#### Signieren einer Nachricht

```rust
use ed25519_dalek::{Signer, Signature};

let message: &[u8] = b"Session-Auth-Challenge: 12345";
let signature: Signature = signing_key.sign(message);

// Signature serialisieren (64 Bytes)
let signature_bytes: [u8; 64] = signature.to_bytes();
```
[Quelle: docs.rs/ed25519-dalek ^97^]

#### Verifikation

```rust
use ed25519_dalek::Verifier;

// Verifikation mit dem Public Key
assert!(verifying_key.verify(message, &signature).is_ok());

// Jeder andere, der den Public Key hat, kann verifizieren
let decoded_verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
assert!(decoded_verifying_key.verify(message, &signature).is_ok());
```
[Quelle: docs.rs/ed25519-dalek ^97^]

### 1.3 Alternative: ring Crate

Das `ring` Crate (basiert auf BoringSSL) bietet eine alternative Implementierung [^390^]:

```rust
use ring::{rand, signature::{self, KeyPair}};

let rng = rand::SystemRandom::new();
let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();

let public_key_bytes = key_pair.public_key().as_ref();
let signature = key_pair.sign(message);

// Verifikation
let peer_public_key = signature::UnparsedPublicKey::new(
    &signature::ED25519, 
    public_key_bytes
);
assert!(peer_public_key.verify(message, signature.as_ref()).is_ok());
```
[Quelle: asecuritysite.com/rust_ed25519 ^390^]

### 1.4 Feature-Flags und Konfiguration

| Feature | Beschreibung |
|---------|-------------|
| `fast` (default) | Vorberechnete Tabellen fuer schnellere Operationen |
| `zeroize` (default) | `ZeroizeOnDrop` fuer sicheres Loeschen des Secret Keys |
| `rand_core` | `SigningKey::generate()` Methode |
| `serde` | Serde-Serialisierung fuer Keys und Signaturen |
| `batch` | Batch-Verifikation fuer hoeheren Durchsatz |
| `pkcs8` | PKCS#8 Import/Export |

[Quelle: lib.rs/crates/ed25519-dalek ^379^]

### 1.5 Ed25519 zu X25519 Konversion

Fuer P2P-Anwendungen ist es oft nuetzlich, das gleiche Key-Pair sowohl fuer Signaturen (Ed25519) als auch fuer Key-Exchange (X25519) zu verwenden. Dies ist kryptographisch sicher moeglich [^407^][^411^].

```rust
// Ed25519 Key-Pair zu X25519 Key-Pair konvertieren
use ed25519_to_curve25519::{ed25519_pk_to_curve25519, ed25519_sk_to_curve25519};

let ed25519_public_key: [u8; 32] = verifying_key.to_bytes();
let ed25519_secret_key: [u8; 32] = signing_key.to_bytes();

let x25519_public_key = ed25519_pk_to_curve25519(&ed25519_public_key)?;
let x25519_secret_key = ed25519_sk_to_curve25519(&ed25519_secret_key)?;
```
[Quelle: docs.rs/ed25519_to_curve25519 ^407^]

Die Konversion basiert auf der mathematischen Beziehung zwischen der Montgomery Curve (Curve25519 fuer X25519) und der twisted Edwards Curve (Ed25519) wie in RFC 7748 spezifiziert [^421^].

### 1.6 X25519 Diffie-Hellman Key Exchange

```rust
use x25519_dalek::{EphemeralSecret, PublicKey};

// Alice
let alice_secret = EphemeralSecret::random();
let alice_public = PublicKey::from(&alice_secret);

// Bob
let bob_secret = EphemeralSecret::random();
let bob_public = PublicKey::from(&bob_secret);

// Shared Secret berechnen
let alice_shared = alice_secret.diffie_hellman(&bob_public);
let bob_shared = bob_secret.diffie_hellman(&alice_public);

assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
```
[Quelle: docs.rs/x25519-dalek ^373^]

### 1.7 NaCl crypto_box fuer E2EE

Das `crypto_box` Crate implementiert NaCl's public-key authenticated encryption mit X25519 + XSalsa20Poly1305 [^394^]:

```rust
use crypto_box::{SalsaBox, PublicKey, SecretKey, Nonce};
use rand_core::OsRng;

let alice_secret = SecretKey::generate(&mut OsRng);
let alice_public = alice_secret.public_key();

let bob_secret = SecretKey::generate(&mut OsRng);
let bob_public = bob_secret.public_key();

// Encryption
let alice_box = SalsaBox::new(&bob_public, &alice_secret);
let nonce = Nonce::from_slice(b"unique nonce 12");
let ciphertext = alice_box.encrypt(&nonce, b"Hello Bob".as_ref()).unwrap();

// Decryption
let bob_box = SalsaBox::new(&alice_public, &bob_secret);
let plaintext = bob_box.decrypt(&nonce, &ciphertext).unwrap();
```
[Quelle: docs.rs/crypto_box ^394^]

**Sicherheitsaudit:** Cure53 hat `crypto_box` v0.7.1 geprueft und keine signifikanten Schwachstellen gefunden [^398^].

---

## 2. Session-basierte Authentifizierung

### 2.1 Zufaellige Passwoerter pro Session

RustDesk verwendet ein One-Time-Password (OTP) System fuer jede Session [^348^][^396^]:

- **One-Time Password:** Jedes Mal wenn RustDesk gestartet wird, wird ein neues zufaelliges Passwort generiert
- **Permanent Password:** Optional konfigurierbar fuer unbeaufsichtigten Zugriff
- **Numeric OTP Option:** Seit v1.4.1 koennen rein numerische OTPs aktiviert werden [^89^]
- **Laengenkonfiguration:** Die Passwortlaenge ist konfigurierbar (Standard: z.B. 8-12 Zeichen)

### 2.2 Secure Password Generation in Rust

#### Mit OsRng (getrandom)

```rust
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;

// Alphanumerisches Passwort (z.B. fuer Session-OTP)
fn generate_session_password(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

// Passphrase (Diceware-Stil)
fn generate_passphrase(word_count: usize) -> String {
    // Verwendet OsRng intern fuer kryptographisch sichere Zufallswerte
    let wordlist = include_str!("eff_wordlist.txt");
    let words: Vec<&str> = wordlist.lines().collect();
    
    let mut rng = thread_rng();
    (0..word_count)
        .map(|_| words[rng.gen_range(0..words.len())])
        .collect::<Vec<_>>()
        .join("-")
}
```

#### Mit passgenr Crate

```rust
use passgenr;

// 20-Zeichen Passwort mit allen ASCII-Druckzeichen
let password = passgenr::random_password(
    passgenr::charsets::ASCII, 
    20, 
    ""
).unwrap();
```

`passgenr` verwendet `OsRng` als Zufallsquelle, die direkt vom OS-CSPRNG liest (z.B. `getrandom(2)` auf Linux, `RtlGenRandom` auf Windows). Die Auswahl erfolgt uniform (kein "mod N"-Problem) [^371^].

#### Mit chbs (Diceware)

```rust
use chbs::prelude::*;

// Standard-Passphrase generieren
let passphrase = passphrase();
println!("{}", passphrase);

// Konfigurierte Passphrase
let mut config = BasicConfig::default();
config.words = 6;
config.word_separator = "-".to_string();
let scheme = config.to_scheme();
let passphrase = scheme.generate();
```
[Quelle: docs.rs/chbs ^395^]

**Empfohlene Entropie:**
| Passworttyp | Laenge | Entropie | Brute-Force-Widerstand |
|-------------|--------|----------|----------------------|
| Numerisch (6 Ziffern) | 6 | ~20 Bits | Sehr schwach |
| Numerisch (8 Ziffern) | 8 | ~27 Bits | Schwach |
| Alphanumerisch | 8 | ~48 Bits | Moderat |
| Alphanumerisch | 12 | ~71 Bits | Stark |
| Diceware (6 Woerter) | 6 | ~78 Bits | Sehr stark |
| Alphanumerisch | 16 | ~95 Bits | Exzellent |

### 2.3 Session-Rotation

```rust
use std::time::{Duration, Instant};

struct SessionManager {
    password: String,
    created_at: Instant,
    ttl: Duration,  // Time-To-Live
}

impl SessionManager {
    fn new(ttl_seconds: u64) -> Self {
        Self {
            password: generate_session_password(12),
            created_at: Instant::now(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }
    
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
    
    fn rotate(&mut self) -> String {
        self.password = generate_session_password(12);
        self.created_at = Instant::now();
        self.password.clone()
    }
}
```

---

## 3. Trust-On-First-Use (TOFU)

### 3.1 Konzept

TOFU ist ein Sicherheitsmodell bei dem Vertrauen beim ersten Verbindungsaufbau zwischen Client und Server/Peer etabliert wird [^352^][^359^]:

1. **Erste Verbindung:** Der Client speichert den Public-Key-Fingerprint des Servers
2. **Nachfolgende Verbindungen:** Der Client vergleicht den gespeicherten Fingerprint mit dem aktuellen
3. **Mismatch:** Wenn sich der Key aendert, wird eine Warnung angezeigt

### 3.2 TOFU bei RustDesk

RustDesk implementiert TOFU durch:

- **Relay/Server ID Pinning:** Die Server-ID und API-Key koennen in der Netzwerkkonfiguration festgepinnt werden [^348^]
- **Public-Key-Fingerprint:** Der Client speichert den Fingerprint des Relay-Servers
- **Vertrauensentscheidung:** Beim ersten Verbinden wird der Benutzer gefragt, ob er dem Server vertraut

```
RustDesk Client → Verbindet zu Relay-Server
                → Speichert Server Public Key Fingerprint
                → Bei spaeterer Verbindung: Vergleich mit gespeichertem Fingerprint
                → Bei Aenderung: WARNUNG - Moeglicher Man-in-the-Middle!
```

### 3.3 TOFU Implementierung (SSH-Stil)

```rust
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex;

struct TofuTrustStore {
    known_hosts: HashMap<String, String>, // hostname -> fingerprint
}

impl TofuTrustStore {
    fn compute_fingerprint(public_key: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let result = hasher.finalize();
        hex::encode(&result[..16]) // Erste 128 Bits als Fingerprint
    }
    
    fn verify_or_ask(&mut self, host: &str, public_key: &[u8]) -> TofuResult {
        let fingerprint = Self::compute_fingerprint(public_key);
        
        match self.known_hosts.get(host) {
            Some(known_fp) if known_fp == &fingerprint => {
                TofuResult::Trusted
            }
            Some(known_fp) => {
                TofuResult::Mismatch {
                    expected: known_fp.clone(),
                    actual: fingerprint,
                }
            }
            None => {
                // Erste Verbindung - speichern und Benutzer fragen
                self.known_hosts.insert(host.to_string(), fingerprint.clone());
                TofuResult::FirstConnection { fingerprint }
            }
        }
    }
}

enum TofuResult {
    Trusted,
    Mismatch { expected: String, actual: String },
    FirstConnection { fingerprint: String },
}
```

### 3.4 Staerken und Schwaechen von TOFU

**Staerken [^355^][^359^]:**
- Einfach zu implementieren
- Keine zentrale CA noetig
- Menschliche Validierung jeder Interaktion
- Weniger Overhead als Web of Trust
- Funktioniert ohne Internet-Infrastruktur (ideal fuer P2P)

**Schwaechen:**
- **Erste Verbindung ist angreifbar** (Man-in-the-Middle bei erstem Kontakt)
- Skaliert schlecht fuer grosse Netzwerke
- Benutzer koennen Warnungen ignorieren ("Warning Fatigue")
- Keine automatische Widerrufsmoeglichkeit

**Best Practices [^350^]:**
- Fingerprint Out-of-Band verifizieren (z.B. QR-Code scannen, Telefon)
- Certificate Pinning ergaenzen
- Mutual TLS (mTLS) fuer zusaetzliche Sicherheit
- Regelmaessige Audits des Trust Store

---

## 4. API-Key-Management mit OS Keyring

### 4.1 Architektur

Das `keyring` Crate in Rust bietet einen cross-platform Ansatz fuer sichere Credential-Speicherung [^393^][^397^]:

| OS | Backend | Technologie |
|----|---------|-------------|
| Windows | Windows Credential Manager | DPAPI (Data Protection API) |
| macOS | Keychain | Security Framework |
| Linux | Secret Service | D-Bus (gnome-keyring, kwallet) |
| Linux (alternativ) | keyutils | Linux Kernel Key Retention Service |

### 4.2 keyring v4 API

```rust
// Cargo.toml
[dependencies]
keyring = "4.0"
keyring-core = "0.7"
```

#### macOS Keychain

```rust
fn main() -> keyring_core::Result<()> {
    // 1) Store auswaehlen
    keyring::use_apple_keychain_store(&std::collections::HashMap::new())?;

    // 2) Entry API fuer set/get/delete
    let entry = keyring_core::Entry::new("clawviewer", "api-key")?;
    entry.set_password("sk-abc123...")?;

    let password = entry.get_password()?;
    println!("API Key = {password}");

    entry.delete_credential()?;
    
    // 3) Store explizit freigeben
    keyring::release_store();
    Ok(())
}
```
[Quelle: Qiita Rust Keyring Tutorial ^397^]

#### Windows (DPAPI)

```rust
fn main() -> keyring_core::Result<()> {
    // Windows Native Credential Store
    keyring::use_windows_native_store(&std::collections::HashMap::new())?;

    let entry = keyring_core::Entry::new("clawviewer", "api-key")?;
    entry.set_password("sk-abc123...")?;
    
    let password = entry.get_password()?;
    println!("Gespeicherter Key: {password}");
    
    Ok(())
}
```

#### Linux (Secret Service)

```rust
fn main() -> keyring_core::Result<()> {
    // D-Bus Secret Service (GNOME Keyring, KWallet)
    keyring::use_dbus_secret_service_store(&std::collections::HashMap::new())?;

    let entry = keyring_core::Entry::new("clawviewer", "api-key")?;
    entry.set_password("sk-abc123...")?;
    
    Ok(())
}
```

### 4.3 Windows DPAPI direkt

Fuer spezifische Anforderungen kann `windows-dpapi` direkt verwendet werden [^347^]:

```rust
use windows_dpapi::{encrypt_data, decrypt_data, Scope};

fn main() -> anyhow::Result<()> {
    let secret = b"my secret api key";
    
    // Verschluesseln fuer aktuellen User
    let encrypted = encrypt_data(secret, Scope::User, None)?;
    
    // Entschluesseln
    let decrypted = decrypt_data(&encrypted, Scope::User, None)?;
    assert_eq!(secret, decrypted.as_slice());
    
    Ok(())
}
```

**Sicherheitshinweise [^347^]:**
- `Scope::User`: Nur der gleiche User auf dem gleichen Rechner kann entschluesseln
- `Scope::Machine`: Jeder User auf dem gleichen Rechner kann entschluesseln (weniger sicher)
- Verschluesselte Daten koennen NICHT auf einem anderen Rechner entschluesselt werden

### 4.4 Linux Secret Service (oo7)

`oo7` ist eine moderne Rust-Implementierung fuer Linux Secret Service [^363^][^367^]:

```rust
use oo7::{Keyring, Secret};

async fn store_api_key(service: &str, api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Automatisch den besten Backend auswaehlen
    let keyring = Keyring::new().await?;
    
    let secret = Secret::new(api_key.as_bytes());
    keyring.create_item(
        &format!("{service} API Key"),
        &[("service", service)],
        &secret,
        true,  // replace existing
    ).await?;
    
    Ok(())
}
```

### 4.5 API-Key Storage Best Practices

1. **Nie rohe API-Keys speichern** - Nur den Hash speichern, den rohen Key nur einmalig zurueckgeben [^438^]
2. **Key-Praefix** - Erste paar Zeichen fuer Identifikation speichern
3. **Scopes** - Granulare Berechtigungen pro Key
4. **Rotation** - 30-90 Tage Rotation fuer Produktion
5. **Audit Logging** - Jede Nutzung loggen
6. **Rate Limiting** - Anfragen pro Key begrenzen

```rust
struct ApiKeyEntry {
    key_id: String,          // Zufaellige ID
    prefix: String,          // z.B. "sk_abc" fuer Identifikation
    key_hash: String,        // SHA-256 Hash
    scopes: Vec<String>,     // Berechtigungen
    created_at: u64,         // Unix timestamp
    expires_at: Option<u64>, // Optional
    is_active: bool,         // Status
}
```
[Quelle: oneuptime.com ^438^]

---

## 5. KI-Sandbox-Architektur: Permission-Modell

### 5.1 Drei-Schichten-Sicherheitsmodell

Ein robustes KI-Agent-Sicherheitsframework besteht aus drei Schichten [^377^]:

| Schicht | Controls | Tools |
|---------|----------|-------|
| **Environment** | Sandboxing, Network Segmentation, Read-Only Mirrors | VMs, Containers |
| **Permissions** | Principle of Least Privilege, File-Tree Allowlists | Policy Enforcers |
| **Runtime Enforcement** | Real-time Monitoring, Diff Approval | Git Hooks, CI Rules |

### 5.2 Permission-Modell nach Risikostufe

#### Vellum Permission Model [^400^]

Jede Aktion wird nach Risikostufe klassifiziert:

- **Low (Gruen)** - Read-Only Operationen (Dateien lesen, Websuche, Erinnerungen abrufen) -> Automatisch erlaubt
- **Medium (Gelb)** - Zustandsaendernde Operationen (Dateien schreiben, API-Calls, Shell-Befehle) -> Abhaengig von Risikotoleranz
- **High (Rot)** - Destruktive/Sensitive Operationen (Dateien loeschen, Source Code aendern, sudo) -> Immer Bestaetigung

**Kritischer Sicherheitsdetail:** Unknown Tools defaulten zu DESTRUCTIVE. Wenn ein Agent einen unbekannten Tool-Namen halluziniert, wird dies auf hoechster Einschraenkung ausgefuehrt [^410^].

### 5.3 Sandbox Boundary Pattern

```
+------------------+        +-------------------+
|   AI Sandbox     |        |   Host Machine    |
|   (isoliert)     |        |                   |
|                  |        |  host_file_read   |
| ~/.workspace/    |<------>|  host_file_write  |
| - read/write/    |   API  |  host_bash        |
|   edit freely    |        |  -> immer Prompt  |
| - shell sandboxed|        |                   |
| - build apps     |        |  OS-Level Enforcement |
| - save memories  |        |  - sandbox-exec (macOS) |
+------------------+        |  - bubblewrap (Linux)  |
                            +-------------------+
```
[Quelle: vellum.ai/docs ^400^]

### 5.4 Action Confirmation Dialog Pattern

```rust
#[derive(Debug, Clone)]
enum RiskLevel {
    Low,      // Read-Only: Auto-approve
    Medium,   // Write: Log + ggf. Bestaetigung
    High,     // Destructive: Immer Bestaetigung
}

struct ActionRequest {
    tool_name: String,
    description: String,
    risk_level: RiskLevel,
    parameters: serde_json::Value,
}

impl ActionRequest {
    async fn execute(&self) -> Result<ActionResult, ActionError> {
        match self.risk_level {
            RiskLevel::Low => self.execute_direct().await,
            RiskLevel::Medium => {
                log_action(self);
                if requires_confirmation(&self.tool_name) {
                    self.wait_for_approval().await
                } else {
                    self.execute_direct().await
                }
            }
            RiskLevel::High => {
                log_action(self);
                self.wait_for_approval().await
            }
        }
    }
    
    async fn wait_for_approval(&self) -> Result<ActionResult, ActionError> {
        // UI-Dialog anzeigen mit:
        // - Beschreibung der Aktion
        // - Color-coded Risk Badge
        // - "Show details" mit vollstaendigem Input
        // - Approve / Reject Buttons
        
        let approval = ui::show_confirmation_dialog(
            &self.description,
            &self.risk_level,
            &self.parameters,
        ).await?;
        
        match approval {
            Approval::Approved => self.execute_direct().await,
            Approval::Rejected => Err(ActionError::UserRejected),
        }
    }
}
```
[Quelle: dev.to/thedailyagent ^410^]

### 5.5 File-System-Permission-Modell

```rust
struct FileSystemPermissions {
    // Whitelist von erlaubten Pfaden
    allowed_paths: Vec<PathBuf>,
    
    // Read-Only Pfade
    read_only_paths: Vec<PathBuf>,
    
    // Verbotene Pfade (ueberschreibt Whitelist)
    blocked_paths: Vec<PathBuf>,
    
    // Max Dateigroesse
    max_file_size: usize,
    
    // Operationen erlauben/verbieten
    allow_read: bool,
    allow_write: bool,
    allow_delete: bool,
    allow_execute: bool,
}

impl FileSystemPermissions {
    fn can_access(&self, path: &Path, operation: FileOperation) -> bool {
        // 1. Pruefe Blocked Paths
        if self.blocked_paths.iter().any(|p| path.starts_with(p)) {
            return false;
        }
        
        // 2. Pruefe Whitelist
        if !self.allowed_paths.iter().any(|p| path.starts_with(p)) {
            return false; // Ausserhalb erlaubter Pfade
        }
        
        // 3. Pruefe Operation
        match operation {
            FileOperation::Read => self.allow_read,
            FileOperation::Write => {
                self.allow_write && 
                !self.read_only_paths.iter().any(|p| path.starts_with(p))
            }
            FileOperation::Delete => self.allow_delete,
            FileOperation::Execute => self.allow_execute,
        }
    }
}
```

### 5.6 Best Practices fuer KI-Agent-Sandboxing [^442^][^443^]

1. **CPU/Memory/Disk Limits** - Resource-Beschraenkungen
2. **Zero-Trust Network** - Default-Deny fuer alle Outbound-Verbindungen
3. **Short-lived Credentials** - Temporaere Tokens mit limitiertem Scope
4. **Human-in-the-Loop** - Explizite Bestaetigung fuer High-Risk Actions
5. **Comprehensive Logging** - Alle Aktionen loggen
6. **Path Traversal Protection** - `../` und Symlink Escapes blockieren
7. **Command Whitelist** - Bekannte sichere Befehle ohne Prompt

### 5.7 Codex CLI Permission Modes [^443^]

| Modus | Lesen | Schreiben | Shell | Netzwerk |
|-------|-------|-----------|-------|----------|
| Read-Only | Ja | Prompt | Nein | Nein |
| Auto (Default) | Ja | Ja | Ja (Sandbox) | Prompt |
| Full Access | Ja | Ja | Ja | Ja |

---

## 6. Transport-Sicherheit

### 6.1 WebRTC DTLS-SRTP

WebRTC verwendet mandatory DTLS-SRTP fuer End-to-End-Verschluesselung [^284^][^286^]:

**Schichten:**
1. **DTLS (Datagram Transport Layer Security)** - Key Exchange und Peer Authentication
2. **SRTP (Secure Real-time Transport Protocol)** - Media Stream Verschluesselung
3. **SCTP over DTLS** - Data Channel Verschluesselung

**Sicherheitseigenschaften [^284^]:**
- Jeder Audio/Video Frame wird verschluesselt bevor er das Geraet verlaesst
- Relay-Server koennen den Inhalt NICHT entschluesseln
- Keine Opt-Out Moeglichkeit - Verschluesselung ist mandatory
- Neue DTLS Key-Pairs werden fuer jeden Call generiert (PFS)

**DTLS Handshake [^285^]:**
```
Peer A                    Peer B
  |                         |
  |-------- DTLS Hello ---->|
  |<------- DTLS Hello -----|
  |-------- Certificate ---->|
  |<------- Certificate -----|
  |-------- Finished ------->|
  |<------- Finished --------|
  |                         |
  |==== SRTP Keys derived ===|
  |                         |
```

**Fingerprint Verification:**
- Jeder Peer enthaelt SHA-256 Hash seines DTLS-Zertifikats im SDP
- Waehrend DTLS Handshake wird das Zertifikat gegen den SDP-Fingerprint geprueft
- Ein Angreifer muesste den Fingerprint im SDP aendern (MITM auf Signaling)

### 6.2 Signaling-Sicherheit

**Kritisch:** Der Signaling-Channel MUSS gesichert sein (WSS/HTTPS). Unsicheres Signaling bricht den gesamten DTLS-SRTP Schutz [^284^]:

```
UNSAFE:  ws://  (plain WebSocket)
SAFE:    wss:// (Secure WebSocket over TLS)
SAFE:    https:// (REST API mit TLS)
```

### 6.3 TLS 1.3 mit rustls

`rustls` ist eine moderne TLS-Implementierung in Rust [^382^][^386^]:

```rust
use rustls::{ClientConfig, ServerConfig};
use std::sync::Arc;

// Client Konfiguration
let config = ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth();

// Server Konfiguration
let config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(cert_chain, private_key)?;
```

**Unterstuetzte Features [^386^]:**
- TLS 1.2 und TLS 1.3
- ECDSA, Ed25519, RSA Server-Authentifizierung
- Forward Secrecy mit ECDHE (curve25519, nistp256, nistp384)
- AES128-GCM, AES256-GCM, ChaCha20-Poly1305
- TLS 1.3 0-RTT
- Session Resumption

**Nicht unterstuetzt (bewusst):**
- SSLv1-3, TLS 1.0/1.1
- RC4, DES, 3DES
- MAC-then-encrypt
- Non-PFS Ciphersuites
- Renegotiation

**Post-Quantum:** `rustls` mit `aws-lc-rs` unterstuetzt X25519MLKEM768 Key Exchange [^382^].

### 6.4 Noise Protocol Framework (Snow)

Das Noise Protocol Framework ist eine Alternative zu TLS fuer P2P-Verbindungen [^408^][^418^]:

```rust
use snow::Builder;

static PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

let mut initiator = Builder::new(PATTERN.parse()?)
    .build_initiator()?;

let mut responder = Builder::new(PATTERN.parse()?)
    .build_responder()?;

// Handshake
let (mut read_buf, mut first_msg, mut second_msg) = 
    ([0u8; 1024], [0u8; 1024], [0u8; 1024]);

// -> e
let len = initiator.write_message(&[], &mut first_msg)?;
responder.read_message(&first_msg[..len], &mut read_buf)?;

// <- e, ee, s, es
let len = responder.write_message(&[], &mut second_msg)?;
initiator.read_message(&second_msg[..len], &mut read_buf)?;

// -> s, se
let mut third_msg = [0u8; 1024];
let len = initiator.write_message(&[], &mut third_msg)?;
responder.read_message(&third_msg[..len], &mut read_buf)?;

// Transport Mode
let initiator = initiator.into_transport_mode()?;
let responder = responder.into_transport_mode()?;
```
[Quelle: docs.rs/snow ^408^]

**Verwendet von:** WireGuard, WhatsApp, Slack Nebula, I2P, Lightning Network [^418^]

### 6.5 RustDesk Protokoll-Stack

RustDesk verwendet folgenden kryptographischen Stack [^348^]:

| Komponente | Algorithmus | Verwendung |
|------------|-------------|------------|
| Signaturen | Ed25519 (crypto_sign_ed25519) | Authentifizierung |
| Key Exchange | X25519 + XSalsa20Poly1305 (crypto_box) | Session-Key-Austausch |
| Symmetrische Verschluesselung | XSalsa20Poly1305 (crypto_secretbox) | Session-Daten |

**Eigenschaften:**
- NaCl (Salt) basiert
- Ende-zu-Ende verschluesselt
- Relay-Server kann Inhalt NICHT entschluesseln
- Direct P2P bevorzugt, Relay als Fallback

### 6.6 End-to-End-Verschluesselung fuer P2P-Remote-Desktop

```
+-------------+                        +-------------+
|   Client A  |<==== P2P Connection ===>|   Client B  |
|             |    (ICE/STUN/TURN)     |             |
| +---------+ |                        | +---------+ |
| | Ed25519 | |<-- Auth Challenge ---> | | Ed25519 | |
| | KeyPair | |<-- Signature Verify -->| | KeyPair | |
| +---------+ |                        | +---------+ |
| | X25519  | |<-- DH Key Exchange --> | | X25519  | |
| +---------+ |                        | +---------+ |
| |ChaCha20 | |<-- Encrypted Session ->| |ChaCha20 | |
| |Poly1305 | |                        | |Poly1305 | |
| +---------+ |                        | +---------+ |
+-------------+                        +-------------+

Schritte:
1. TOFU: Public Key Fingerprint Vergleich
2. Ed25519: Challenge-Response Authentifizierung  
3. X25519: Ephemeral Diffie-Hellman Key Exchange
4. ChaCha20-Poly1305: Symmetrische Session-Verschluesselung
```

---

## 7. Passwort-Generierung

### 7.1 Sichere Zufallszahlen in Rust

```rust
use rand::{Rng, thread_rng, SeedableRng};
use rand::rngs::OsRng;

// ===== Method 1: OsRng (empfohlen) =====
fn secure_random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    OsRng.fill(&mut buf[..]);
    buf
}

// ===== Method 2: Alphanumerisches Passwort =====
fn gen_alphanumeric(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ\
                            abcdefghijkmnopqrstuvwxyz\
                            23456789";  // 0, O, I, l weggelassen
    let mut rng = thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

// ===== Method 3: Passphrase (Diceware) =====
fn gen_passphrase(word_count: usize) -> String {
    let wordlist = vec!["alpha", "bravo", "charlie", "delta", "echo", 
                        "foxtrot", "golf", "hotel", "india", "juliet"];
    let mut rng = thread_rng();
    (0..word_count)
        .map(|_| wordlist[rng.gen_range(0..wordlist.len())])
        .collect::<Vec<_>>()
        .join("-")
}

// ===== Method 4: Numeric OTP =====
fn gen_numeric_otp(digits: usize) -> String {
    let mut rng = thread_rng();
    (0..digits)
        .map(|_| rng.gen_range(0..10).to_string())
        .collect()
}
```

### 7.2 getrandom Crate

Die `getrandom` Crate ist die Grundlage fuer alle kryptographischen Zufallszahlengeneratoren in Rust:

| OS | Implementierung |
|----|----------------|
| Linux/Android | `getrandom()` syscall oder `/dev/urandom` |
| Windows | `ProcessPrng` oder `RtlGenRandom` |
| macOS/iOS | `getentropy()` oder `/dev/urandom` |
| FreeBSD/OpenBSD | `getrandom()` syscall |
| WebAssembly | Web Crypto API |

### 7.3 Sicheres Speichern von generierten Passwoertern

```rust
use zeroize::{Zeroize, Zeroizing};

// Zeroizing Wrapper: Automatisch Nullen bei Drop
fn use_session_password() {
    let password = Zeroizing::new(
        generate_session_password(16)
    );
    
    // Passwort verwenden...
    send_to_peer(&password);
    
    // Automatisch ueberschrieben mit Nullen beim Verlassen des Scopes
}

// Explizites Loeschen
fn explicit_clear() {
    let mut secret = b"temporary password".to_vec();
    // ... verwenden ...
    secret.zeroize(); // Sicheres Ueberschreiben
}
```
[Quelle: docs.rs/zeroize ^384^]

### 7.4 Zeroize Crate Details

Das `zeroize` Crate mit ueber 11 Millionen Downloads/Monat bietet [^384^][^388^]:

- **Garantie 1:** Zeroing kann nicht vom Compiler "weg-optimiert" werden
- **Garantie 2:** Alle nachfolgenden Reads sehen zeroized Werte
- **Implementierung:** Nutzt `core::ptr::write_volatile` und `compiler_fence(SeqCst)`
- **no_std kompatibel:** Funktioniert auch in embedded Umgebungen
- **WASM friendly:** Kein FFI oder Inline Assembly

```rust
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Zeroize, ZeroizeOnDrop)]
struct SessionKeys {
    #[zeroize(skip)]  // Nicht loeschen (z.B. Public Key)
    pub_key: [u8; 32],
    
    // Wird automatisch geloescht
    secret_key: [u8; 32],
    session_key: [u8; 32],
}
```

**Wichtiger Pitfall:** Rust's Move-Semantik kann Kopien des Secrets auf dem Stack erzeugen. Heap-Allokation (Box) oder `Zeroizing` Wrapper verwenden [^422^].

---

## 8. Komplett-Beispiel: P2P Auth-Flow

### 8.1 Session-Etablierung

```rust
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier};
use x25519_dalek::{EphemeralSecret, PublicKey};
use crypto_box::SalsaBox;
use rand_core::OsRng;
use sha2::{Sha256, Digest};
use zeroize::Zeroizing;

/// Peer Authentication mit Ed25519 + X25519
struct P2PAuth {
    // Long-term Identity Key (Ed25519)
    identity_key: SigningKey,
    
    // TOFU Trust Store
    trusted_peers: HashMap<String, [u8; 32]>, // peer_id -> public_key_hash
}

impl P2PAuth {
    fn new() -> Self {
        let identity_key = SigningKey::generate(&mut OsRng);
        Self {
            identity_key,
            trusted_peers: HashMap::new(),
        }
    }
    
    /// Schritt 1: Verbindungsanfrage mit Challenge
    fn create_auth_challenge(&self) -> AuthChallenge {
        let mut challenge = [0u8; 32];
        OsRng.fill(&mut challenge);
        
        AuthChallenge {
            challenger_pubkey: self.identity_key.verifying_key().to_bytes(),
            nonce: challenge,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    /// Schritt 2: Challenge signieren und zuruecksenden
    fn respond_to_challenge(&self, challenge: &AuthChallenge) -> AuthResponse {
        let mut message = Vec::new();
        message.extend_from_slice(&challenge.nonce);
        message.extend_from_slice(&challenge.timestamp.to_le_bytes());
        
        let signature = self.identity_key.sign(&message);
        
        AuthResponse {
            responder_pubkey: self.identity_key.verifying_key().to_bytes(),
            signature: signature.to_bytes(),
        }
    }
    
    /// Schritt 3: Verifikation und TOFU-Entscheidung
    fn verify_response(&mut self, 
                       challenge: &AuthChallenge,
                       response: &AuthResponse,
                       peer_hostname: &str
    ) -> Result<TrustDecision, AuthError> {
        let peer_key = VerifyingKey::from_bytes(&response.responder_pubkey)
            .map_err(|_| AuthError::InvalidKey)?;
        
        // Signatur verifizieren
        let mut message = Vec::new();
        message.extend_from_slice(&challenge.nonce);
        message.extend_from_slice(&challenge.timestamp.to_le_bytes());
        
        let signature = ed25519_dalek::Signature::from_bytes(&response.signature);
        peer_key.verify(&message, &signature)
            .map_err(|_| AuthError::InvalidSignature)?;
        
        // TOFU Pruefung
        let fingerprint = compute_fingerprint(&response.responder_pubkey);
        
        match self.trusted_peers.get(peer_hostname) {
            Some(trusted) if trusted == &fingerprint => {
                Ok(TrustDecision::Trusted)
            }
            Some(trusted) => {
                Ok(TrustDecision::KeyChanged {
                    expected: trusted.clone(),
                    actual: fingerprint,
                })
            }
            None => {
                // Erste Verbindung
                self.trusted_peers.insert(peer_hostname.to_string(), fingerprint.clone());
                Ok(TrustDecision::FirstConnect { fingerprint })
            }
        }
    }
    
    /// Schritt 4: Ephemeral Key Exchange (X25519)
    fn perform_key_exchange(&self, peer_pubkey: [u8; 32]) -> Result<SessionKeys, AuthError> {
        let ephemeral_secret = EphemeralSecret::random();
        let ephemeral_public = PublicKey::from(&ephemeral_secret);
        
        let peer_public = PublicKey::from(peer_pubkey);
        let shared_secret = ephemeral_secret.diffie_hellman(&peer_public);
        
        // Session Keys aus Shared Secret ableiten
        let mut hasher = Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"ClawViewer-v1");
        let session_key = hasher.finalize();
        
        Ok(SessionKeys {
            ephemeral_public: ephemeral_public.to_bytes(),
            session_key: session_key.into(),
        })
    }
}

struct AuthChallenge {
    challenger_pubkey: [u8; 32],
    nonce: [u8; 32],
    timestamp: u64,
}

struct AuthResponse {
    responder_pubkey: [u8; 32],
    signature: [u8; 64],
}

enum TrustDecision {
    Trusted,
    FirstConnect { fingerprint: String },
    KeyChanged { expected: String, actual: String },
}

enum AuthError {
    InvalidKey,
    InvalidSignature,
    Timeout,
}

struct SessionKeys {
    ephemeral_public: [u8; 32],
    session_key: [u8; 32],
}

fn compute_fingerprint(public_key: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    let result = hasher.finalize();
    hex::encode(&result[..16])
}
```

---

## 9. Zusammenfassung: Empfohlene Crate-Kombination

| Funktion | Crate | Version |
|----------|-------|---------|
| Ed25519 Signaturen | `ed25519-dalek` | ^3.0 |
| X25519 Key Exchange | `x25519-dalek` | ^2.0 |
| NaCl crypto_box | `crypto_box` | ^0.9 |
| OS Keyring | `keyring` + `keyring-core` | ^4.0 |
| Secure Zeroing | `zeroize` | ^1.8 |
| TLS 1.3 | `rustls` | ^0.23 |
| Noise Protocol | `snow` | ^0.9 |
| Passwort-Generierung | `rand` (built-in) | ^0.8 |
| Diceware Passphrases | `chbs` | ^0.3 |
| Hashing | `sha2` | ^0.10 |
| Hex-Kodierung | `hex` | ^0.4 |

### Cargo.toml

```toml
[dependencies]
# Kryptographie
ed25519-dalek = { version = "3", features = ["fast", "zeroize", "rand_core"] }
x25519-dalek = "2"
crypto_box = "0.9"

# Speicher-Sicherheit
zeroize = { version = "1.8", features = ["derive"] }

# OS Keyring
keyring = "4"
keyring-core = "0.7"

# TLS (optional, fuer Signaling)
rustls = { version = "0.23", default-features = false, features = ["tls12", "ring"] }

# Noise Protocol (optional, fuer P2P-Handshake)
snow = "0.9"

# Hashing
sha2 = "0.10"
hex = "0.4"

# Zufall
rand = "0.8"
getrandom = "0.2"

# Serialisierung
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Async
 tokio = { version = "1", features = ["full"] }

# Passphrase-Generierung (optional)
chbs = "0.3"
```

---

## 10. Referenzen

### Ed25519 & Kryptographie
- [^97^] docs.rs/ed25519-dalek - Offizielle Dokumentation
- [^361^] crates.io/ed25519-dalek - Crate Registry
- [^373^] docs.rs/x25519-dalek - X25519 Key Exchange
- [^379^] lib.rs/crates/ed25519-dalek - Download-Statistiken
- [^390^] asecuritysite.com - Ed25519 mit ring
- [^394^] docs.rs/crypto_box - NaCl crypto_box in Rust
- [^407^] docs.rs/ed25519_to_curve25519 - Key-Konversion
- [^411^] libsodium.gitbook.io - Ed25519 zu Curve25519 Konversion
- [^421^] medium.com - X25519 und Ed25519 Erklaerung

### TOFU & Trust Model
- [^352^] MDN Web Docs - TOFU Definition
- [^355^] Security Wiki - Trust on First Use
- [^359^] Wikipedia - Trust on first use
- [^350^] nhimg.org - TOFU for Workloads
- [^354^] Medium - TOFU Balancing Security and Convenience

### RustDesk Security
- [^348^] realvnc.com - RustDesk Security Evaluation
- [^396^] Medium - RustDesk DFIR Investigation
- [^89^] rustdesk.com - Advanced Settings

### OS Keyring & API-Key-Management
- [^347^] docs.rs/windows-dpapi - Windows DPAPI
- [^393^] docs.rs/keyring-core - Keyring Core API
- [^397^] Qiita - Rust Keyring Tutorial (v4)
- [^363^] docs.rs/oo7 - Linux Secret Service
- [^436^] strac.io - API Key Best Practices
- [^438^] oneuptime.com - API Key Management

### KI-Sandbox & Permissions
- [^377^] knostic.ai - AI Agent Security Framework
- [^400^] vellum.ai - The Permissions Model
- [^410^] dev.to - Human Approval for AI Agent Actions
- [^442^] northflank.com - AI Agent Sandboxing
- [^443^] ubos.tech - AI Agent Sandbox Best Practices

### Transport-Sicherheit
- [^284^] antmedia.io - WebRTC Security
- [^285^] voipmonitor.org - WebRTC Protocol
- [^286^] webrtcforthecurious.com - WebRTC Security
- [^382^] docs.rs/rustls - TLS in Rust
- [^386^] docs.rs/rustls - Feature-Uebersicht
- [^408^] docs.rs/snow - Noise Protocol Framework
- [^418^] Wikipedia - Noise Protocol Framework

### Secure Memory & Password Generation
- [^384^] docs.rs/zeroize - Secure Memory Clearing
- [^371^] github.com/defuse/passgenr - Password Generator
- [^395^] docs.rs/chbs - Diceware Passphrases
- [^398^] cure53.de - Security Audit crypto_box
- [^422^] benma.github.io - Zeroize Pitfalls in Rust
