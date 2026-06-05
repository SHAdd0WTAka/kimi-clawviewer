# Dim 05 – xrdp Linux RDP-Server: Tiefenanalyse

## Executive Summary

**xrdp** (neutrinolabs/xrdp) ist ein Open-Source RDP-Server für Linux, der Microsofts Remote Desktop Protocol (RDP) implementiert. Das Projekt besteht aus ~96.3% C-Code und ermöglicht den Zugriff auf Linux-Desktops über standard RDP-Clients. Diese Analyse untersucht die Architektur, Session-Management, Module-System, PAM-Authentifizierung und Protokoll-Implementierung als Blueprint für ClawViewer.

**Repository:** https://github.com/neutrinolabs/xrdp  
**License:** Apache-2.0  
**Stars:** 6.6k | **Forks:** 1.8k | **Contributors:** 110+  
**Latest Release:** v0.10.6 (Apr 2026)  
**Primary Language:** C (96.3%)

---

## 1. Architektur-Überblick

### 1.1 Gesamtarchitektur

xrdp folgt einer **mehrprozessigen, modularen Architektur** mit klaren Trennungen zwischen Protokoll-Handling, Session-Management und Backend-Integration [^147^]:

```
┌─────────────────────────────────────────────────────────────┐
│                        RDP CLIENT                           │
│              (MSTSC, FreeRDP, Remmina, etc.)                │
└───────────────────────┬─────────────────────────────────────┘
                        │ RDP Protocol (Port 3389)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      XRDP DAEMON                            │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │  TCP/ISO    │ │    MCS      │ │       GCC           │   │
│  │   Layer     │ │  (T.125)    │ │ Conference Create   │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │   Security  │ │  libxrdp    │ │  Channel Manager    │   │
│  │   Layer     │ │  (core)     │ │  (static/dynamic)   │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Window Manager (login screen)           │   │
│  └─────────────────────────────────────────────────────┘   │
└───────────────────────┬─────────────────────────────────────┘
                        │ SCP Protocol (Port 3350)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                   XRDP-SESMAN                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │   PAM Auth  │ │  Session    │ │  Session List       │   │
│  │  (auth_*)   │ │  Management │ │  (session_list.*)   │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │   SESEXEC   │ │   EICP/ERCP │ │  Display Utils      │   │
│  │ (fork exec) │ │  Protocol   │ │  (display_utils.*)  │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
└──────┬────────────────────┬────────────────────────────────┘
       │                    │
       ▼                    ▼
┌─────────────┐    ┌─────────────────────────────────────┐
│ Xorg/Xvnc   │    │           XRDP-CHANSRV              │
│  Backend    │    │  ┌─────────┐ ┌─────────┐ ┌────────┐ │
│             │    │  │ cliprdr │ │ rdpsnd  │ │ rdpdr  │ │
│ xorgxrdp    │    │  │clipboard│ │  audio  │ │ drives │ │
│  driver     │    │  └─────────┘ └─────────┘ └────────┘ │
└─────────────┘    │  ┌─────────┐ ┌─────────────────────┐ │
                   │  │  rail   │ │     drdynvc         │ │
                   │  │ remote  │ │  dynamic virtual    │ │
                   │  │  apps   │ │     channels        │ │
                   │  └─────────┘ └─────────────────────┘ │
                   └─────────────────────────────────────┘
```

### 1.2 Kernkomponenten

| Komponente | Zweck | Quellcode-Pfad |
|---|---|---|
| **xrdp** | Haupt-RDP-Server, Protokoll-Stack | `xrdp/` |
| **xrdp-sesman** | Session Manager, Auth, Prozessverwaltung | `sesman/` |
| **xrdp-chansrv** | Channel Server (Clipboard, Audio, Drives) | `sesman/chansrv/` |
| **xrdp-sesexec** | Session Executor (fork/exec Sessions) | `sesman/sesexec/` |
| **libxrdp** | Core RDP-Protokoll-Implementierung | `libxrdp/` |
| **libvnc** | VNC-Client-Modul | `vnc/` |
| **libxup** | Xorgxrdp-Client-Modul | `xup/` |
| **neutrinordp** | RDP-Client-Modul (RDP Proxy) | `neutrinordp/` |
| **libpainter** | Bitmap-Zeichenbibliothek | `libpainter/` |
| **librfxcodec** | RemoteFX Codec | `librfxcodec/` |

**Source:** [^147^] – Offizielle Repository-Dokumentation

---

## 2. RDP-Protokoll-Implementierung (libxrdp)

### 2.1 Protokoll-Stack

xrdp implementiert den vollständigen RDP-Protokoll-Stack in der `libxrdp`-Bibliothek. Die Schichtenarchitektur folgt der Microsoft-Spezifikation [MS-RDPBCGR] [^164^]:

| Schicht | Datei | Funktion |
|---|---|---|
| **ISO/OSI Transport** | `xrdp_iso.c` | X.224 Connection Confirm/Request |
| **MCS (T.125)** | `xrdp_mcs.c` | Multipoint Communication Service |
| **Security** | `xrdp_sec.c` | Encryption, Licensing, Credentials |
| **RDP Core** | `xrdp_rdp.c` | Hauptprotokoll-Logik |
| **Capabilities** | `xrdp_caps.c` | Capability Exchange |
| **Channels** | `xrdp_channel.c` | Virtual Channel Management |
| **Fastpath** | `xrdp_fastpath.c` | Optimized Output |
| **Surface** | `xrdp_surface.c` | Surface Commands |
| **Orders** | `xrdp_orders.c` | Drawing Orders |

### 2.2 MCS (Multipoint Communication Service) – xrdp_mcs.c

**Claim:** xrdp implementiert MCS gemäß ITU-T T.125 für RDP-Multiplexing.  
**Source:** [^200^] – libxrdp/xrdp_mcs.c  
**Evidence:**

```c
// Datei: libxrdp/xrdp_mcs.c (843 Zeilen)
// Kernfunktionen:
- xrdp_mcs_init()          // Initialisiert MCS-Struktur
- xrdp_mcs_recv()          // Empfängt MCS-PDUs
- xrdp_mcs_send()          // Sendet MCS-PDUs  
- xrdp_mcs_connect()       // MCS-Connect-Initial mit GCC Conference Create
- xrdp_mcs_send_connect_response()  // MCS Connect Response
- xrdp_mcs_send_erdq()     // Erect Domain Request
- xrdp_mcs_send_cjcf()     // Channel Join Confirm
```

Der MCS-Layer multiplext mehrere virtuelle Kanäle über eine einzelne Transportverbindung. Er kapselt GCC Conference Create Request/Response für die Session-Establishment.

### 2.3 Security Layer – xrdp_sec.c

**Claim:** xrdp unterstützt alle drei RDP-Sicherheitsstufen (Low, Medium, High) plus TLS.  
**Source:** [^135^], [^200^]  
**Evidence:**

```c
// Datei: libxrdp/xrdp_sec.c (~2000+ Zeilen)
// Sicherheitsstufen via xrdp.ini:
// crypt_level=low    → 40-bit, client→server encrypted
// crypt_level=medium → 40-bit, bidirectional
// crypt_level=high   → 128-bit, bidirectional
// security_layer=tls → TLS encryption (default)
```

Key functions:
- `xrdp_sec_init()` – Initialisiert Security-Struktur
- `xrdp_sec_recv()` – Verarbeitet Security-PDUs
- `xrdp_sec_send()` – Sendet verschlüsselte PDUs
- `xrdp_sec_process_mcs_data()` – Verarbeitet MCS-Daten mit Client-Info
- `xrdp_sec_generate_keys()` – Generiert RC4-Schlüssel aus Client/Server-Randoms

### 2.4 Channel Management – xrdp_channel.c

**Claim:** xrdp unterstützt statische und dynamische virtuelle Kanäle gemäß RDP-Spezifikation.  
**Source:** [^200^], [^156^]  
**Evidence:**

```c
// Datei: libxrdp/xrdp_channel.c
// Statische Kanäle (Channel Join bei Connection Establishment):
- cliprdr    // Clipboard Redirection
- rdpsnd     // Audio Output
- rdpdr      // Device Redirection (Laufwerke, Drucker)
- rail       // Remote Applications Integrated Locally
- drdynvc    // Dynamic Virtual Channel (für erweiterte Kanäle)

// Dynamische Kanäle (DRDYNVC):
- Audio Input
- Video Redirect (TSMF)
- USB Redirection
- Custom Channels
```

Der `xrdp_channel.c`-Code verwaltet:
- Channel-Join während der MCS-Connection-Phase
- I/O-Request-Pakete (IRPs) für Device Redirection
- DVC (Dynamic Virtual Channel) Nachrichten-Multiplexing

**CVE Note:** CVE-2026-35512 (Heap overflow in dynvc processing) wurde in `xrdp_channel.c` gefixt [^200^].

### 2.5 Bitmap-Komprimierung

**Claim:** xrdp unterstützt mehrere Bitmap-Komprimierungsalgorithmen für verschiedene Inhaltstypen.  
**Source:** [^169^], [^200^]  
**Evidence:**

| Algorithmus | Datei | Verwendung |
|---|---|---|
| RLE (8-bit) | `xrdp_bitmap_compress.c` | Einfache Grafiken, UI-Elemente |
| RLE (32-bit) | `xrdp_bitmap32_compress.c` | True-Color-Bitmaps |
| JPEG | `xrdp_jpeg_compress.c` | Fotorealistische Inhalte |
| RemoteFX | `librfxcodec/` | H.264-ähnliche Videokomprimierung |
| H.264 | `xrdp_encoder_x264.c` | Hardware-beschleunigte Video-Encoding |

---

## 3. Session-Management (sesman)

### 3.1 Architektur des Session Managers

**Claim:** xrdp-sesman verwaltet Benutzer-Sessions als separater Prozess mit PAM-Authentifizierung und Prozess-Forking.  
**Source:** [^128^], [^122^], [^136^]  
**Evidence:**

```
sesman/
├── sesman.c/h              # Hauptprogramm, Event-Loop
├── sesman.ini.in           # Konfigurationsdatei
├── scp_process.c/h         # SCP (Sesman Control Protocol)
├── scp_list.c/h            # Session-Listen-Verwaltung
├── session_list.c/h        # Aktive Sessions (user, display, pid)
├── display_utils.c/h       # X11 Display-Nummern-Verwaltung
├── eicp_process.c/h        # EICP (Exec Instance Control Protocol)
├── ercp_process.c/h        # ERCP (Exec Runtime Control Protocol)
├── sesexec_control.c/h     # Steuerung von sesexec-Prozessen
├── sesman_restart.c/h      # Session-Restart-Logik
├── sig.c/h                 # Signal-Handling
├── startwm.sh              # Window Manager Start-Skript
├── reconnectwm.sh          # Reconnect-Skript
│
├── chansrv/                # Channel Server
│   ├── chansrv.c           # Hauptprogramm
│   ├── clipboard.c         # Clipboard (cliprdr)
│   ├── sound.c             # Audio (rdpsnd)
│   ├── devredir.c          # Device Redirection (rdpdr)
│   ├── fuse_devredir.c     # FUSE für Laufwerksweiterleitung
│   ├── rail.c              # Remote Applications
│   └── ...
│
├── sesexec/                # Session Executor (ab v0.10.x)
│   ├── sesexec.c           # PAM-Session-Management, fork/exec
│   ├── env.c               # Umgebungsvariablen-Setup
│   ├── xserver.c           # X-Server-Start
│   └── ...
│
├── libsesman/              # Gemeinsame Bibliothek
│   ├── sesman_auth.c       # PAM-Authentifizierung
│   ├── sesman_config.c     # Konfigurations-Parser
│   └── ...
│
└── tools/                  # Admin-Tools
    ├── sesrun.c            # Session-Start von Kommandozeile
    ├── sesadmin.c          # Session-Administration
    └── seslist.c           # Session-Listing
```

### 3.2 Session-Lebenszyklus

**Claim:** Der Session-Lebenszyklus umfasst Authentifizierung, Session-Erstellung, Monitoring und Cleanup über mehrere Prozesse.  
**Source:** [^136^] – "Redesign of authentication architecture"  
**Evidence:**

```
┌──────────┐     ┌──────────┐     ┌──────────────┐
│  Client  │────▶│   xrdp   │────▶│ xrdp-sesman  │
│ (RDP)    │◀────│  (3389)  │◀────│   (3350)     │
└──────────┘     └──────────┘     └──────┬───────┘
                                         │
                    ┌────────────────────┼────────────────────┐
                    ▼                    ▼                    ▼
              ┌──────────┐       ┌────────────┐      ┌──────────┐
              │ Existing │       │  sesexec   │      │  Session │
              │ Session  │◀─────│  (PAM)     │───▶  │  Start   │
              │ Reconnect│       │ Auth+Fork  │      │ (X+WM)   │
              └──────────┘       └────────────┘      └──────────┘
```

**Ablauf (Current Process in v0.10.x):**

1. **User Input** → Login-Daten über RDP an xrdp
2. **SCP Request** → xrdp sendet Credentials an xrdp-sesman via SCP (port 3350)
3. **Authentication** → sesman delegiert an sesexec (forked Prozess)
4. **sesexec PAM** → Führt PAM-Authentifizierung aus
5. **Session Check** → Bei Erfolg: Prüfung auf existierende Session
6. **Session Creation** → Falls neu: Fork von X-Server + Window Manager + chansrv
7. **Display Info** → Rückgabe von Display-Nummer an xrdp
8. **Module Connect** → xrdp verbindet mit Display via Module (libxup.so/libvnc.so)

### 3.3 Session-Liste und -Tracking

**Claim:** Sessions werden in einer zentralen Liste mit User, Display, PID und Status verwaltet.  
**Source:** [^195^] – sesman/session_list.c  
**Evidence:**

```c
// Datei: sesman/session_list.c
// Session-Eintrag enthält:
- username          // Benutzername
- display           // X11 Display (z.B. ":10")
- ip_addr           // Client-IP-Adresse
- pid               // Prozess-ID der Session
- status            // Active, Disconnected, etc.
- type              // Xorg, Xvnc, etc.
- bpp               // Color depth
- width, height     // Display-Größe
- start_time        // Session-Startzeit
```

**Admin-Tools:**
- `xrdp-seslist` – Listet alle aktiven Sessions
- `xrdp-sesadmin -u <user> -k` – Beendet Session eines Benutzers
- `xrdp-sesrun` – Startet Session von Kommandozeile

### 3.4 Session-Policies

**Claim:** xrdp unterstützt verschiedene Session-Allocations-Policies für Multi-Session-Szenarien.  
**Source:** [^130^], [^159^]  
**Evidence:**

```ini
; sesman.ini - [Sessions] Section
Policy=Default  ; Session pro <User,BitPerPixel>
; Alternativen:
; UBD  = User, BPP, DisplaySize
; UBI  = User, BPP, IPAddr
; UBC  = User, BPP, Connection (immer neue Session)
; UBDI = User, BPP, DisplaySize, IPAddr
; UBDC = User, BPP, DisplaySize, Connection
```

---

## 4. Authentifizierung und PAM-Integration

### 4.1 PAM-Architektur

**Claim:** xrdp nutzt PAM (Pluggable Authentication Modules) als primären Authentifizierungsmechanismus mit Unterstützung für verschiedene Auth-Backends.  
**Source:** [^123^], [^136^], [^146^]  
**Evidence:**

```
PAM-Stack für xrdp (konfiguriert in /etc/pam.d/xrdp-sesman):

auth      include   system-auth      # oder system-remote-login
account   include   system-auth
password  include   system-auth
session   include   system-auth
```

PAM unterstützt:
- Lokale Passwort-Authentifizierung (`/etc/shadow`)
- LDAP/Active Directory (via pam_ldap, sssd)
- Kerberos (via pam_krb5)
- Zwei-Faktor-Authentifizierung (vorbereitet in v0.10.x Architektur)

### 4.2 Auth-Code-Struktur (sesexec)

**Claim:** In v0.10.x wurde die Authentifizierung in einen separaten sesexec-Prozess ausgelagert, um Blocking und systemd-Probleme zu vermeiden.  
**Source:** [^136^] – Authentication Architecture Redesign  
**Evidence:**

```c
// Datei: sesman/sesexec/sesexec.c
// Architektur:
// - sesman forked sesexec für jeden Auth-Vorgang
// - sesexec führt PAM-Auth + PAM-Session durch
// - Bei Erfolg: fork von X-Server + WM + chansrv
// - EICP/ERCP Protokolle für Kommunikation mit sesman

Key-Dateien:
- sesman/libsesman/sesman_auth.c   # PAM-Implementierung
- sesman/sesexec/sesexec.c          # Session-Executor
- sesman/sesexec/env.c              # Umgebungsvariablen
- sesman/sesexec/xserver.c          # X-Server-Management
```

### 4.3 SCP (Sesman Control Protocol)

**Claim:** xrdp und sesman kommunizieren über das SCP-Protokoll, das in v0.10.x über Unix Domain Sockets (UDS) läuft.  
**Source:** [^136^], [^195^]  
**Evidence:**

```c
// SCP V0 Nachrichten-Typen:
- SCP_LOGIN_REQUEST       // Username + Password
- SCP_LOGIN_RESPONSE      // Success/Failure + Display-Info
- SCP_LOGOUT_REQUEST      // Session beenden
- SCP_KILL_REQUEST        // Session forcieren
- SCP_LIST_REQUEST        // Session-Liste abrufen
- SCP_LIST_RESPONSE       // Session-Einträge
```

**Vorteile der neuen Architektur (v0.10.x):**
- PAM-Auth blockiert nicht mehr den Haupt-sesman-Prozess
- Jeder Auth-Vorgang läuft in eigenem Prozess (isoliert)
- Bessere systemd-Integration (D-Bus, pam_systemd)
- Grundlage für interaktive PAM-Conversations (2FA)

---

## 5. Module-System

### 5.1 Modul-Architektur

**Claim:** xrdp lädt Backend-Module dynamisch zur Laufzeit als Shared Libraries. Module werden über `xrdp.ini` konfiguriert.  
**Source:** [^135^], [^154^], [^157^]  
**Evidence:**

```ini
; xrdp.ini - Modul-Konfiguration
[Xorg]
name=sesman-Xorg
lib=libxup.so          ; Xorg-Modul (xorgxrdp)
username=ask
password=ask
ip=127.0.0.1
port=-1                ; Auto-Port via sesman
code=20                ; 20=Xorg, 0=Xvnc, 10=X11rdp

[Xvnc]
name=Xvnc
lib=libvnc.so          ; VNC-Modul
username=ask
password=ask
ip=127.0.0.1
port=-1

[vnc-any]
name=vnc-any
lib=libvnc.so          ; VNC-Client für externe Server
ip=ask                 ; Manuelle IP-Eingabe
port=ask5900

[neutrinordp-any]
name=neutrinordp-any
lib=libxrdpneutrinordp.so  ; RDP Proxy
ip=ask
port=ask3389
```

### 5.2 Xorg-Modul (libxup.so)

**Claim:** Das Xorg-Modul verbindet xrdp mit einem xorgxrdp-X-Server für native RDP-Sessions.  
**Source:** [^138^], [^153^], [^139^]  
**Evidence:**

```c
// Datei: xup/xup.c (Xorgxrdp Client Module)
// libxup.so implementiert:
- mod_init()          // Modul-Initialisierung
- mod_connect()       // Verbindung zum Xorg-Server
- mod_start()         // Session-Start
- mod_event()         // Input-Events (Maus, Tastatur)
- mod_get_event()     // Screen-Updates empfangen
- mod_end()           // Cleanup

// Xorg-Konfiguration (/etc/X11/xrdp/xorg.conf):
Section "Device"
    Identifier "Video Card (xrdpdev)"
    Driver "xrdpdev"              // xrdpdev Treiber
    Option "DRMDevice" "/dev/dri/renderD128"
    Option "DRI3" "1"
EndSection

Section "InputDevice"
    Identifier "xrdpKeyboard"
    Driver "xrdpkeyb"             // xrdp Keyboard-Treiber
EndSection

Section "InputDevice"
    Identifier "xrdpMouse"
    Driver "xrdpmouse"            // xrdp Mouse-Treiber
EndSection
```

**xorgxrdp-Treiber-Paket (separates Repository):**
- `xrdpdev_drv.so` – Display-Treiber (Framebuffer → RDP)
- `xrdpkeyb_drv.so` – Keyboard-Treiber
- `xrdpmouse_drv.so` – Mouse-Treiber

### 5.3 VNC-Modul (libvnc.so)

**Claim:** Das VNC-Modul ist ein VNC-Client, der xrdp mit Xvnc-Servern verbindet.  
**Source:** [^135^], [^137^], [^143^]  
**Evidence:**

```c
// Datei: vnc/vnc.c (VNC Client Module)
// libvnc.so implementiert:
- lib_mod_connect()      // VNC-Verbindung aufbauen
- lib_mod_event()        // Input-Events an VNC
- lib_mod_check_wait_objs()  // Screen-Updates von VNC

// Unterstützte VNC-Encodings:
- Raw           // Unkomprimiert
- RRE           // Rise-and-Run-length Encoding
- CopyRect      // Bildschirmkopien
- Cursor        // Cursor-Updates
- Hextile       // Hextile-Encoding

// Verbindungsdaten:
// - Standardmäßig localhost (ip=127.0.0.1)
// - Port: 5900 + Display-Nummer (z.B. 5910 für :10)
// - VNC-Passwort wird von sesman generiert
```

**Hinweis:** `libvnc.so` ist ein VNC-*Client*, nicht Server. xrdp verbindet als Client zum lokalen Xvnc-Server.

### 5.4 NeutrinoRDP-Modul (RDP Proxy)

**Claim:** Das NeutrinoRDP-Modul ermöglicht RDP-zu-RDP-Proxying (xrdp als Gateway zu anderen RDP-Servern).  
**Source:** [^166^], [^167^], [^168^]  
**Evidence:**

```c
// Datei: neutrinordp/neutrinordp.c
// libxrdpneutrinordp.so implementiert:
- Verbindung zu entfernten RDP-Servern via FreeRDP/NeutrinoRDP
- Proxying von RDP-Verbindungen
- Weiterleitung von Channels

// Use Case: RDP Client → xrdp → Windows RDP Server
// Workflow:
// 1. Client verbindet zu xrdp
// 2. xrdp lädt neutrinordp-Modul
// 3. NeutrinoRDP verbindet zum Ziel-RDP-Server
// 4. Daten werden transparent durchgeleitet
```

---

## 6. X11-Integration

### 6.1 Xorg-Backend (xorgxrdp)

**Claim:** Das bevorzugte Backend ist xorgxrdp, ein spezieller Xorg-Treiber, der X11-Rendering in RDP-Bitmaps umwandelt.  
**Source:** [^121^], [^139^], [^153^]  
**Evidence:**

```
Xorg-Backend Architektur:

┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  X-Clients  │────▶│  Xorg-Server │────▶│  xrdpdev    │
│  (WM, Apps) │◀────│  mit xorgxrdp│◀────│  Driver     │
└─────────────┘     └──────────────┘     └──────┬──────┘
                                                │
                                          ┌─────┴─────┐
                                          ▼           ▼
                                     ┌────────┐ ┌──────────┐
                                     │  SHM   │ │  RDP     │
                                     │Memory  │ │  Updates │
                                     └────────┘ └─────┬────┘
                                                      │
                                                ┌─────┴─────┐
                                                ▼           ▼
                                           ┌─────────┐ ┌─────────┐
                                           │ libxup  │ │  Client │
                                           │ (xrdp)  │ │  (RDP)  │
                                           └─────────┘ └─────────┘
```

**Ablauf:**
1. X-Clients zeichnen auf den Xorg-Server
2. xrdpdev-Treiber empfängt Drawing Operations
3. Drawing Operations werden in SHM (Shared Memory) geschrieben
4. libxup (in xrdp) liest SHM und sendet RDP-Bitmap-Updates
5. Input (Maustasten, Tastatur) geht vom RDP-Client → xrdp → xrdpmouse/xrdpkeyb → Xorg

### 6.2 Xvnc-Backend

**Claim:** Das VNC-Backend verwendet Xvnc als X-Server und xrdp verbindet als VNC-Client.  
**Source:** [^121^], [^143^]  
**Evidence:**

```
Xvnc-Backend Architektur:

┌─────────────┐     ┌──────────┐     ┌──────────┐
│  X-Clients  │────▶│  Xvnc    │────▶│  VNC     │
│  (WM, Apps) │◀────│  Server  │◀────│  Protocol│
└─────────────┘     └──────────┘     └────┬─────┘
                                          │
                                    ┌─────┴─────┐
                                    ▼           ▼
                              ┌──────────┐ ┌──────────┐
                              │libvnc.so │ │  Client  │
                              │ (xrdp)   │ │  (RDP)   │
                              └──────────┘ └──────────┘
```

**Unterschiede Xorg vs. Xvnc:**

| Feature | Xorg (xorgxrdp) | Xvnc |
|---|---|---|
| Performance | Besser (direkter Pfad) | Gut |
| Resizing | On-the-fly | Eingeschränkt |
| GPU-Acceleration | Ja (glamor) | Nein |
| H.264/RemoteFX | Ja | Nein |
| Code-Qualität | Aktiver Entwicklung | Legacy |

### 6.3 Umgebungsvariablen und Session-Setup

**Claim:** xrdp-sesman konfiguriert eine vollständige X11-Session-Umgebung.  
**Source:** [^159^], [^155^]  
**Evidence:**

```ini
; sesman.ini - [SessionVariables] Section
[SessionVariables]
PULSE_SCRIPT=/etc/xrdp/pulse/default.pa
; Alle Einträge hier werden als Environment-Variablen gesetzt:
; DISPLAY=:10
; XAUTHORITY=~/.Xauthority
; HOME=/home/<user>
; USERNAME=<user>
; LOGNAME=<user>
; PATH=/usr/local/bin:/usr/bin:/bin
; SHELL=/bin/bash
; PWD=/home/<user>
; XRDP_SESSION=1
; XRDP_SOCKET_PATH=/run/xrdp
```

---

## 7. Channel-Server (xrdp-chansrv)

### 7.1 Kanal-Implementierungen

**Claim:** xrdp-chansrv implementiert alle wichtigen RDP-Virtual-Channels als separater Prozess pro Session.  
**Source:** [^156^], [^158^]  
**Evidence:**

```
sesman/chansrv/
├── chansrv.c           # Hauptprogramm, Channel-Multiplexer
├── chansrv.h
├── clipboard.c         # CLIPRDR - Clipboard Redirection
├── clipboard.h
├── clipboard_common.c  # Gemeinsame Clipboard-Funktionen
├── clipboard_file.c    # Datei-Transfer über Clipboard
├── devredir.c          # RDPDR - Device Redirection
├── devredir.h
├── fuse_devredir.c     # FUSE-Integration für Laufwerke
├── fuse_devredir.h
├── irp.c               # I/O Request Packets
├── irp.h
├── rail.c              # RAIL - Remote Applications
├── rail.h
├── sound.c             # RDPSND - Audio Output
├── sound.h
├── mic.c               # Audio Input (Mikrofon)
├── smartcard.c         # Smartcard-Weiterleitung
├── xcommon.c           # X11-Hilfsfunktionen
├── chansrv_fuse.c      # FUSE-Dateisystem
├── chansrv_xfs.c       # xrdp-Dateisystem-Abstraktion
└── ...
```

### 7.2 Clipboard-Redirection (cliprdr)

**Claim:** Zwei-Wege-Clipboard-Transfer wird für Text, Bitmap und Dateien unterstützt.  
**Source:** [^147^], [^163^]  
**Evidence:**

```c
// Datei: sesman/chansrv/clipboard.c
// Unterstützte Formate:
- CF_TEXT, CF_UNICODETEXT     // Text
- CF_BITMAP, CF_DIB           // Bilder
- CF_HDROP, FileGroupDescriptor // Dateien (via FUSE)

// Security:
; sesman.ini:
RestrictOutboundClipboard=none   ; Server → Client
RestrictInboundClipboard=none    ; Client → Server
; Werte: none, text, file, image, all (oder Kombinationen)
```

### 7.3 Drive Redirection (rdpdr)

**Claim:** Lokale Client-Laufwerke werden via FUSE im Remote-Session gemountet.  
**Source:** [^123^]  
**Evidence:**

```c
// Datei: sesman/chansrv/fuse_devredir.c
// Mount-Punkt: ~/thinclient_drives/ (konfigurierbar)
// FUSE-Integration: Dateisystem-Operationen → RDP IRPs

// Konfiguration:
; sesman.ini:
[Chansrv]
FuseMountName=thinclient_drives
FileUmask=077
; EnableFuseMount=true
```

---

## 8. Konfigurationssystem

### 8.1 xrdp.ini – Hauptserver-Konfiguration

**Claim:** Die xrdp.ini definiert globale Einstellungen, Logging, Channels und Session-Typen.  
**Source:** [^154^], [^157^], [^160^]  
**Evidence:**

```ini
[Globals]
port=3389                    # RDP-Listen-Port
security_layer=negotiate    # tls | rdp | negotiate
crypt_level=high             # low | medium | high
bitmap_cache=yes
bitmap_compression=yes
max_bpp=32
fork=true                    # Multi-Process

tls_ciphers=HIGH
#certificate=               # TLS-Zertifikat
#key_file=                  # TLS-Private-Key

runtime_user=xrdp           # Unprivileged User
runtime_group=xrdp

[Logging]
LogFile=xrdp.log
LogLevel=INFO

[Channels]
rdpdr=true                  # Device Redirection
rdpsnd=true                 # Audio
cliprdr=true                # Clipboard
rail=true                   # Remote Apps
drdynvc=true                # Dynamic Virtual Channels
```

### 8.2 sesman.ini – Session Manager Konfiguration

**Claim:** sesman.ini konfiguriert Authentifizierung, Session-Limits und X-Server-Parameter.  
**Source:** [^149^], [^155^], [^159^]  
**Evidence:**

```ini
[Globals]
ListenAddress=127.0.0.1      # Nur lokale Verbindungen von xrdp
ListenPort=3350              # SCP-Port
EnableUserWindowManager=true
UserWindowManager=startwm.sh
DefaultWindowManager=/etc/xrdp/startwm.sh
ReconnectScript=reconnectwm.sh

[Security]
AllowRootLogin=false
MaxLoginRetry=4
TerminalServerUsers=tsusers
TerminalServerAdmins=tsadmins
PAMServiceName=xrdp-sesman

[Sessions]
X11DisplayOffset=10          # Erstes Display ist :10
MaxSessions=50
KillDisconnected=false       # Sessions bleiben bei Disconnect erhalten
DisconnectedTimeLimit=0      # Nie killen
IdleTimeLimit=0              # Kein Idle-Timeout
Policy=Default               # Session-Allocation

[Xorg]
param=/usr/lib/xorg/Xorg
param=-config
param=xrdp/xorg.conf
param=-noreset
param=-nolisten
param=tcp

[Xvnc]
param=Xvnc
param=-bs
param=-nolisten
param=tcp
param=-localhost
```

---

## 9. RDP-Verbindungsaufbau (Connection Sequence)

### 9.1 Verbindungsphasen

**Claim:** xrdp implementiert die vollständige RDP Connection Sequence gemäß [MS-RDPBCGR].  
**Source:** [^164^], [^193^]  
**Evidence:**

```
Phase 1: Connection Initiation
  Client ──TCP──▶ xrdp (Port 3389)
  Client ◀──X.224 Connection Confirm── xrdp
  
Phase 2: Basic Settings Exchange (MCS + GCC)
  Client ──MCS Connect Initial + GCC Conference Create──▶ xrdp
  Client ◀──MCS Connect Response + GCC Conference Create── xrdp
  
Phase 3: Channel Connection
  Client ──MCS Erect Domain Request──▶ xrdp
  Client ──MCS Attach User Request──▶ xrdp
  Client ◀──MCS Attach User Confirm── xrdp
  Client ──MCS Channel Join (global)──▶ xrdp
  Client ──MCS Channel Join (user)──▶ xrdp
  Client ──MCS Channel Join (channels...)──▶ xrdp
  
Phase 4: RDP Security/Settings
  Client ──Client Info PDU (credentials, resolution)──▶ xrdp
  Client ──Confirm Active PDU──▶ xrdp
  Client ◀──Server Synchronize PDU── xrdp
  Client ◀──Server Control (Cooperate)── xrdp
  Client ◀──Server Control (Request Control)── xrdp
  
Phase 5: Login/Session
  xrdp ──SCP──▶ sesman (Port 3350)
  xrdp ◀──Session Info── sesman
  xrdp ──Modul Connect──▶ Xorg/Xvnc
  
Phase 6: Desktop
  Client ◀──Bitmap Updates── xrdp ◀──X11── Xorg
  Client ──Input Events──▶ xrdp ──▶ Xorg
```

---

## 10. Adaptierbare Architektur-Muster für ClawViewer

### 10.1 Empfohlene Design-Patterns

| xrdp-Pattern | ClawViewer-Adaptation | Nutzen |
|---|---|---|
| **Multi-Prozess-Architektur** | Separate Prozesse für Server, Session-Mgmt, Channels | Stabilität, Isolation |
| **Dynamisches Modul-System** | Pluggable Backends (VNC, RDP, P2P) via .so | Erweiterbarkeit |
| **SCP/IPC-Protokoll** | Definiertes Protokoll zwischen Komponenten | Lose Kopplung |
| **PAM-Integration** | Authentifizierungs-Abstraktion | Flexible Auth |
| **Channel-Multiplexer** | Separate Kanäle für verschiedene Datenarten | Modularität |
| **Session-Liste mit Policies** | Session-Management mit Allocation-Rules | Multi-User |
| **SHM für Framebuffer** | Shared Memory für Screen-Capture | Performance |
| **Prozess-Forking pro Session** | Isolierte Sessions | Security |

### 10.2 Spezifische Empfehlungen

1. **Protokoll-Stack-Design:**
   - Klare Schichten-Trennung wie in libxrdp (ISO → MCS → Security → RDP)
   - Jede Schicht als separate C-Modul mit definierter API
   - State-Machine-basierte Verbindungsverwaltung

2. **Session-Management:**
   - Sesman-ähnlicher dedizierter Session Manager
   - Fork-basierte Session-Isolation
   - Session-Liste mit Policies (UBC, UBD, etc.)

3. **Modul-System:**
   - Dynamisches Laden von Shared Libraries (.so)
   - Einheitliche Modul-API (init/connect/event/end)
   - Konfiguration über INI-Dateien

4. **Authentifizierung:**
   - PAM-Integration für Linux
   - Abstrahierte Auth-API für verschiedene Backends
   - Separate Auth-Prozesse für Non-Blocking

5. **Channel-System:**
   - Statische Kanäle (Clipboard, Audio, Input)
   - Dynamische Kanäle (Erweiterungen)
   - Separate Prozesse pro Channel-Typ

### 10.3 Code-Struktur-Vorschlag für ClawViewer

```
clawviewer/
├── server/              # Hauptserver (analog zu xrdp/)
│   ├── main.c
│   ├── listen.c         # Netzwerk-Listener
│   ├── process.c        # Verbindungsverarbeitung
│   └── wm.c             # Window Manager / Login
├── protocol/            # Protokoll-Stack (analog zu libxrdp/)
│   ├── transport.c      # TCP/WebSocket
│   ├── session.c        # Session-Management
│   ├── security.c       # Auth/Encryption
│   └── channels.c       # Virtual Channels
├── sesman/              # Session Manager
│   ├── sesman.c
│   ├── auth.c           # PAM-Integration
│   ├── session_list.c
│   └── exec.c           # Prozess-Forking
├── modules/             # Pluggable Backends
│   ├── vnc/             # VNC-Modul
│   ├── rdp/             # RDP-Client-Modul
│   └── native/          # Nativer Capture
├── channels/            # Channel-Implementierungen
│   ├── clipboard.c
│   ├── audio.c
│   ├── input.c
│   └── files.c
├── capture/             # Screen Capture
│   ├── x11_capture.c
│   ├── wayland_capture.c
│   └── pipewire.c
└── config/              # Konfiguration
    └── clawviewer.ini
```

---

## 11. Zusammenfassung der Kern-Erkenntnisse

### 11.1 Was macht xrdp architektonisch stark?

1. **Klare Trennung von Protokoll, Session und Backend** – Jede Ebene ist austauschbar
2. **Multi-Prozess-Design** – Sessions laufen isoliert, Crash einer Session beeinflusst andere nicht
3. **Dynamisches Modul-System** – Backends können zur Laufzeit geladen werden
4. **PAM-Integration** – Flexible Authentifizierung ohne Code-Änderungen
5. **Channel-Architektur** – Erweiterbar für neue Features
6. **Session-Policies** – Flexible Multi-Session-Verwaltung

### 11.2 Kritische Sicherheitsaspekte

| CVE | Beschreibung | Fix-Status |
|---|---|---|
| CVE-2025-68670 | Stack-based Buffer Overflow in Domain-String | Gefixt in v0.9.27 |
| CVE-2026-32623 | Vulns in NeutrinoRDP Fragment Reassembly | Gefixt in v0.10.6 |
| CVE-2026-33145 | AllowAlternateShell Default auf 'no' | Gefixt |
| CVE-2026-33516 | OOB Read in Caps Processing | Gefixt |
| CVE-2026-35512 | Heap Overflow in DYNVC Processing | Gefixt |

### 11.3 Referenzen

- **GitHub Repository:** https://github.com/neutrinolabs/xrdp [^147^]
- **Offizielle Website:** https://www.xrdp.org/ [^165^]
- **RDP Spezifikation:** [MS-RDPBCGR] https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpbcgr/ [^164^]
- **Man Pages:** xrdp(8), xrdp-sesman(8), xrdp.ini(5), sesman.ini(5) [^122^] [^128^]
- **Arch Wiki:** https://wiki.archlinux.org/title/Xrdp [^121^]
- **Auth Redesign Discussion:** https://github.com/neutrinolabs/xrdp/discussions/1961 [^136^]

---

*Analyse erstellt: Dim 05 – xrdp Linux RDP-Server*  
*Searches durchgeführt: 20+*  
*Quellen: GitHub-Repository, Man-Pages, Microsoft-Spezifikationen, Community-Dokumentationen*
