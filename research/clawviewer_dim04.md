# ClawViewer Dim 04: VNC-Ecosystem Analysis

## Executive Summary

This document provides a deep analysis of the VNC (Virtual Network Computing) ecosystem, focusing on four major open-source codebases: **LibVNCServer/LibVNCClient**, **UltraVNC**, **TightVNC**, and their implementations of the RFB (Remote Framebuffer) protocol. The analysis covers RFB protocol implementation, framebuffer update mechanisms, encoding handlers, viewer/server architectures, and input handling. All findings are documented with concrete file paths and code references.

---

## Table of Contents

1. [Repository Overview](#1-repository-overview)
2. [RFB Protocol Implementation](#2-rfb-protocol-implementation)
3. [Framebuffer Update Mechanism](#3-framebuffer-update-mechanism)
4. [Encoding Handlers](#4-encoding-handlers)
5. [VNC Viewer Architecture](#5-vnc-viewer-architecture)
6. [VNC Server Architecture](#6-vnc-server-architecture)
7. [Input Handling](#7-input-handling)
8. [Security & Authentication](#8-security--authentication)
9. [VNC Repeater / Proxy](#9-vnc-repeater--proxy)
10. [Key Blueprint Patterns for Custom Implementation](#10-key-blueprint-patterns-for-custom-implementation)

---

## 1. Repository Overview

### 1.1 LibVNCServer / LibVNCClient [^31^]
- **Repository**: https://github.com/LibVNC/libvncserver
- **Language**: C (96%), CMake (2.4%)
- **License**: GPL-2.0+
- **Latest Release**: 0.9.15 (Dec 22, 2024)
- **Structure**:
  - `src/libvncserver/` - Server library source
  - `src/libvncclient/` - Client library source
  - `src/common/` - Shared code between server and client
  - `include/rfb/` - Public headers (rfb.h, rfbproto.h, rfbclient.h)
  - `examples/server/` - Server examples
  - `examples/client/` - Client examples

### 1.2 UltraVNC [^34^]
- **Repository**: https://github.com/ultravnc/UltraVNC
- **Language**: C++ (72.4%), C (21.5%), Java (3.7%)
- **License**: GPL-3.0
- **Structure**:
  - `winvnc/` - Windows VNC server
  - `vncviewer/` - VNC viewer client
  - `rfb/` - RFB protocol definitions
  - `repeater/` - VNC repeater/proxy
  - `DSMPlugin/` - Data Stream Modification (encryption) plugins
  - `common/` - Shared utilities

### 1.3 TightVNC
- **Website**: https://www.tightvnc.com/
- **Note**: TightVNC 1.3.x source available; TightVNC 2.x is commercial
- **Key Contribution**: Tight encoding, file transfer protocol

---

## 2. RFB Protocol Implementation

### 2.1 Protocol Version & Handshake

**Claim**: The RFB protocol handshake begins with version exchange, followed by security type negotiation, authentication, and client initialization.

**Source**: LibVNCServer `src/libvncserver/rfbserver.c` [^41^], `include/rfb/rfb.h` [^43^]

**Evidence**:
- Protocol version string format: `RFB 003.008\n` (major.minor)
- Client states defined in `rfb.h`:
```c
enum {
    RFB_PROTOCOL_VERSION,      /* establishing protocol version */
    RFB_SECURITY_TYPE,         /* negotiating security (RFB v.3.7) */
    RFB_AUTHENTICATION,        /* authenticating */
    RFB_INITIALISATION,        /* sending initialisation messages */
    RFB_NORMAL,                /* normal protocol messages */
    RFB_INITIALISATION_SHARED, /* implicit shared-flag */
    RFB_SHUTDOWN,              /* Client is shutting down */
    RFB_CHANNEL_SECURITY_TYPE, /* negotiating channel security */
} state;
```

**Key Files**:
- `src/libvncserver/rfbserver.c` - Server-side RFB protocol handling (4251 lines)
- `include/rfb/rfbproto.h` - Protocol message structure definitions
- `include/rfb/rfb.h` - Main API header with all data structures
- `src/libvncserver/auth.c` - Authentication handlers

### 2.2 RFB Message Types

**Client-to-Server Messages** (from `rfbproto.h`):

| Number | Name | Description |
|--------|------|-------------|
| 0 | SetPixelFormat | Set pixel representation |
| 2 | SetEncodings | Negotiate encoding types |
| 3 | FramebufferUpdateRequest | Request screen update |
| 4 | KeyEvent | Keyboard input |
| 5 | PointerEvent | Mouse/pointer input |
| 6 | ClientCutText | Clipboard transfer |

**Server-to-Client Messages**:

| Number | Name | Description |
|--------|------|-------------|
| 0 | FramebufferUpdate | Screen update with rectangles |
| 1 | SetColourMapEntries | Color map updates |
| 2 | Bell | Audible notification |
| 3 | ServerCutText | Clipboard transfer |

**Reference**: RFB Protocol Specification [^30^], [^71^]

### 2.3 Encoding Numbers

**Standard Encodings** (defined in `rfbproto.h`):

| Number | Name | Description |
|--------|------|-------------|
| 0 | Raw | Uncompressed pixel data |
| 1 | CopyRect | Copy from existing framebuffer area |
| 2 | RRE | Rise and Run-length Encoding |
| 4 | CoRRE | Compact RRE |
| 5 | Hextile | Tile-based encoding |
| 6 | Zlib | Zlib compressed |
| 7 | Tight | TightVNC's efficient encoding |
| 16 | ZRLE | Zlib Run-Length Encoding |
| 17 | ZYWRLE | Wavelet-based ZRLE |

**Pseudo-Encodings**:

| Number | Name | Description |
|--------|------|-------------|
| -223 | CursorShape | Cursor shape updates |
| -224 | CursorPos | Cursor position updates |
| -252 | DesktopSize | Framebuffer size changes |
| -308 | ExtendedDesktopSize | Multi-screen support |

---

## 3. Framebuffer Update Mechanism

### 3.1 Update Region Tracking

**Claim**: LibVNCServer tracks framebuffer changes per-client using region data structures (sraRegion), separating modified regions from copy regions.

**Source**: `include/rfb/rfb.h` [^43^], `src/libvncserver/main.c` [^80^]

**Evidence**: Per-client tracking structure in `rfbClientRec`:
```c
typedef struct _rfbClientRec {
    /* ... */
    sraRegionPtr copyRegion;      /* destination region of copy */
    int copyDX, copyDY;           /* translation for copy */
    sraRegionPtr modifiedRegion;  /* regions changed on screen */
    sraRegionPtr requestedRegion; /* client-requested regions */
    /* ... */
} rfbClientRec, *rfbClientPtr;
```

**Update Pending Check** (`rfb.h`):
```c
#define FB_UPDATE_PENDING(cl)                                              \
     (((cl)->enableCursorShapeUpdates && (cl)->cursorWasChanged) ||        \
     (((cl)->enableCursorShapeUpdates == FALSE &&                          \
       ((cl)->cursorX != (cl)->screen->cursorX ||                          \
        (cl)->cursorY != (cl)->screen->cursorY))) ||                       \
     ((cl)->useNewFBSize && (cl)->newFBSizePending) ||                     \
     ((cl)->enableCursorPosUpdates && (cl)->cursorWasMoved) ||             \
     !sraRgnEmpty((cl)->copyRegion) || !sraRgnEmpty((cl)->modifiedRegion))
```

### 3.2 Framebuffer Update Sending

**Claim**: `rfbSendFramebufferUpdate()` in `rfbserver.c` is the core function that sends framebuffer updates to clients, iterating over modified regions and encoding each rectangle.

**Source**: `src/libvncserver/rfbserver.c` [^41^]

**Evidence**: Function signature and flow:
```c
rfbBool rfbSendFramebufferUpdate(rfbClientPtr cl, sraRegionPtr updateRegion);
```

The function:
1. Sends framebuffer update header (message-type = 0, number-of-rectangles)
2. Iterates over `updateRegion` rectangles
3. Selects best encoding based on `cl->preferredEncoding`
4. Calls encoding-specific send function (e.g., `rfbSendRectEncodingTight()`)
5. Sends `rfbSendLastRectMarker()` if `LastRect` pseudo-encoding is enabled

**Key Functions in `rfbserver.c`**:
- `rfbSendFramebufferUpdate()` - Main update send function
- `rfbSendRectEncodingRaw()` - Send raw rectangle
- `rfbSendUpdateBuf()` - Flush update buffer
- `rfbSendCopyRegion()` - Send CopyRect encoding
- `rfbSendLastRectMarker()` - End-of-update marker
- `rfbProcessClientMessage()` - Process incoming client messages
- `rfbProcessClientNormalMessage()` - Handle normal-phase messages

### 3.3 Event Loop Architecture

**Claim**: LibVNCServer provides both blocking and threaded event loop modes via `rfbRunEventLoop()` and `rfbProcessEvents()`.

**Source**: `src/libvncserver/main.c` [^80^]

**Evidence**: Threaded mode architecture:

```
Main Thread (listenerRun)
  -> accept() new connections
  -> rfbNewClient() creates client
  -> rfbStartOnHoldClient() starts client threads

Per-Client Threads:
  clientInput thread:
    -> select() on socket
    -> rfbProcessClientMessage() for each message
    
  clientOutput thread:
    -> wait on updateCond (signaled when modifiedRegion changes)
    -> rfbSendFramebufferUpdate() when updates pending
```

**Key Functions**:
- `rfbRunEventLoop(screen, usec, runInBackground)` - Start event loop
- `rfbProcessEvents(screen, usec)` - Process single iteration
- `rfbMarkRectAsModified(screen, x1, y1, x2, y2)` - Mark changed area
- `rfbScheduleCopyRegion(screen, region, dx, dy)` - Schedule copy rect

**Deferral Mechanism**: Updates are deferred by `deferUpdateTime` (default 5ms) to batch multiple changes:
```c
screen->deferUpdateTime = 5;  /* milliseconds */
```

### 3.4 Marking Regions as Modified

**Claim**: Applications call `rfbMarkRectAsModified()` or `rfbMarkRegionAsModified()` to notify the server of framebuffer changes, which then signals all connected clients.

**Source**: `src/libvncserver/main.c` [^80^]

**Evidence**: Implementation:
```c
void rfbMarkRegionAsModified(rfbScreenInfoPtr screen, sraRegionPtr modRegion) {
   rfbClientIteratorPtr iterator;
   rfbClientPtr cl;
   iterator = rfbGetClientIterator(screen);
   while((cl = rfbClientIteratorNext(iterator))) {
     LOCK(cl->updateMutex);
     sraRgnOr(cl->modifiedRegion, modRegion);  /* OR region into modified */
     TSIGNAL(cl->updateCond);                   /* Signal output thread */
     UNLOCK(cl->updateMutex);
   }
   rfbReleaseClientIterator(iterator);
}
```

---

## 4. Encoding Handlers

### 4.1 Raw Encoding (0)

**Claim**: Raw encoding sends uncompressed pixel data directly from the framebuffer.

**Source**: `src/libvncserver/rfbserver.c` [^41^]

**Evidence**: `rfbSendRectEncodingRaw()` sends pixel data line by line using the translate function to convert from server to client pixel format.

**File**: `src/libvncserver/rfbserver.c` - `rfbSendRectEncodingRaw()`

### 4.2 RRE Encoding (2) - Rise and Run-length Encoding

**Claim**: RRE encodes rectangles as a background color plus a list of subrectangles with different colors.

**Source**: `src/libvncserver/rre.c` [^38^]

**Evidence**: `rfbSendRectEncodingRRE()` in `src/libvncserver/rre.c`

**Files**:
- Server: `src/libvncserver/rre.c`
- Client: `src/libvncclient/rre.c`

### 4.3 CoRRE Encoding (4) - Compact RRE

**Claim**: CoRRE is a compact variant of RRE using 8-bit coordinates.

**Source**: `src/libvncserver/corre.c` [^38^]

**Files**:
- Server: `src/libvncserver/corre.c`
- Client: `src/libvncclient/corre.c`

### 4.4 Hextile Encoding (5)

**Claim**: Hextile divides the screen into 16x16 tiles and applies subencoding per tile (raw, solid, RRE, Hextile subrects).

**Source**: `src/libvncserver/hextile.c` [^38^]

**Evidence**: Hextile subencoding flags (from encoding implementations):
```c
#define HEXTILE_RAW                   1
#define HEXTILE_BACKGROUND_SPECIFIED  2
#define HEXTILE_FOREGROUND_SPECIFIED  4
#define HEXTILE_ANY_SUBRECTS          8
#define HEXTILE_SUBRECTS_COLOURED    16
```

**Files**:
- Server: `src/libvncserver/hextile.c`
- Client: `src/libvncclient/hextile.c`

### 4.5 Zlib Encoding (6)

**Claim**: Zlib encoding applies zlib compression to raw rectangle data.

**Source**: `src/libvncserver/zlib.c` [^36^]

**Key Parameters**:
```c
#define VNC_ENCODE_ZLIB_MIN_COMP_SIZE (17)
#define ZLIB_MAX_RECT_SIZE (128*256)
```

**Function**: `rfbSendRectEncodingZlib()`

### 4.6 Tight Encoding (7)

**Claim**: Tight encoding is the most efficient lossless encoding, using zlib compression, JPEG for photographic content, and gradient filtering.

**Source**: `src/libvncserver/tight.c` [^36^], [^56^]

**Evidence**: Tight subencoding types:
```c
#define TIGHT_EXPLICIT_FILTER         0x04
#define TIGHT_FILL                    0x08
#define TIGHT_JPEG                    0x09
#define TIGHT_NO_ZLIB                 0x0A
#define TIGHT_MAX_SUBENCODING         0x0A
#define TIGHT_MIN_TO_COMPRESS 12
```

**Compression control**:
```c
#define TIGHT_DEFAULT_COMPRESSION  6
#define TURBO_DEFAULT_SUBSAMP 0
```

**Files**:
- Server: `src/libvncserver/tight.c`
- Client: `src/libvncclient/tight.c`

**Tight Encoding Process**:
1. Analyze rectangle for fill, JPEG, or basic zlib
2. For "basic" rects: apply filter (copy, palette, gradient)
3. Compress with zlib (per-client stream state in `zsStruct[4]`)
4. For photo content: encode as JPEG using turbojpeg

### 4.7 ZRLE Encoding (16) - Zlib Run-Length Encoding

**Claim**: ZRLE uses zlib-compressed tile-based encoding with run-length encoding for solid colors.

**Source**: `src/libvncserver/zrle.c`, `src/libvncserver/zrleencodetemplate.c` [^36^]

**Files**:
- Server: `src/libvncserver/zrle.c`, `src/libvncserver/zrleencodetemplate.c`
- Client: `src/libvncclient/zrle.c`
- Helpers: `src/libvncserver/zrleoutstream.c`, `zrlepalettehelper.c`

**Tile Size**: 64x64 pixels (`rfbZRLETileWidth` x `rfbZRLETileHeight`)

### 4.8 Ultra Encoding (9)

**Claim**: Ultra encoding uses LZO compression for fast compression/decompression.

**Source**: `src/libvncserver/ultra.c` [^38^]

**Parameters**:
```c
#define ULTRA_MAX_RECT_SIZE (128*256)
```

**Files**:
- Server: `src/libvncserver/ultra.c`
- Client: `src/libvncclient/ultra.c`

### 4.9 Encoding Support Matrix

| Encoding | Number | LibVNCServer | LibVNCClient | UltraVNC Server | UltraVNC Viewer |
|----------|--------|:------------:|:------------:|:---------------:|:---------------:|
| Raw | 0 | Yes | Yes | Yes | Yes |
| CopyRect | 1 | Yes | Yes | Yes | Yes |
| RRE | 2 | Yes | Yes | Yes | Yes |
| CoRRE | 4 | Yes | Yes | Yes | Yes |
| Hextile | 5 | Yes | Yes | Yes | Yes |
| Zlib | 6 | Yes | Yes | Yes | Yes |
| Tight | 7 | Yes | Yes | Yes | Yes |
| Ultra | 9 | Yes | Yes | Yes | Yes |
| ZRLE | 16 | Yes | Yes | Yes | Yes |
| ZYWRLE | 17 | Yes | Yes | Yes | Yes |
| TRLE | 15 | No | Yes | - | - |
| TightPNG | -260 | Yes | No | - | - |

**Reference**: LibVNC README [^31^]

---

## 5. VNC Viewer Architecture

### 5.1 LibVNCClient Architecture

**Claim**: LibVNCClient provides a complete VNC client library with a callback-driven architecture for handling framebuffer updates, input, and clipboard.

**Source**: `src/libvncclient/rfbclient.c`, `include/rfb/rfbclient.h` [^59^]

**Key Client Structure** (`rfbClient` in `rfbclient.h`):
```c
typedef struct _rfbClient {
    /* Server information */
    int width, height;
    rfbPixelFormat format;
    char *desktopName;
    
    /* Socket */
    int sock;
    
    /* Framebuffer */
    char *frameBuffer;
    
    /* Callbacks */
    GotFrameBufferUpdateProc GotFrameBufferUpdate;
    GotXCutTextProc GotXCutText;
    GotXCutTextUTF8Proc GotXCutTextUTF8;
    GetCredentialProc GetCredential;
    MallocFrameBufferProc MallocFrameBuffer;
    HandleKeyboardLedStateProc HandleKeyboardLedState;
    
    /* Encoding handlers */
    HandleRFBServerMessageProc HandleRFBServerMessage;
    /* ... */
} rfbClient;
```

**Key Files**:
- `src/libvncclient/rfbclient.c` - Main client implementation
- `src/libvncclient/vncviewer.c` - Example viewer application
- `src/libvncclient/sockets.c` - Socket I/O
- `src/libvncclient/listen.c` - Reverse (listening) connections

**Client Connection Flow**:
1. `rfbGetClient(bitsPerSample, samplesPerPixel, bytesPerPixel)` - Allocate client
2. Set callbacks (`client->GotFrameBufferUpdate`, etc.)
3. `rfbInitClient(client, argc, argv)` - Connect and handshake
4. `SendFramebufferUpdateRequest()` - Request updates
5. `HandleRFBServerMessage()` - Process server messages in loop

### 5.2 UltraVNC Viewer Architecture

**Claim**: UltraVNC's viewer uses a modular C++ architecture with separate classes for each encoding type and connection aspect.

**Source**: `vncviewer/` directory [^60^]

**Key Files**:
- `vncviewer/ClientConnection.cpp` / `ClientConnection.h` - Main connection class
- `vncviewer/ClientConnectionRaw.cpp` - Raw encoding decoder
- `vncviewer/ClientConnectionRRE.cpp` - RRE decoder
- `vncviewer/ClientConnectionCoRRE.cpp` - CoRRE decoder
- `vncviewer/ClientConnectionHextile.cpp` - Hextile decoder
- `vncviewer/ClientConnectionTight.cpp` - Tight decoder
- `vncviewer/ClientConnectionZlib.cpp` - Zlib decoder
- `vncviewer/ClientConnectionUltra.cpp` - Ultra decoder
- `vncviewer/ClientConnectionUltra2.cpp` - Ultra2 decoder
- `vncviewer/ClientConnectionZlibHex.cpp` - ZlibHex decoder
- `vncviewer/ClientConnectionCopyRect.cpp` - CopyRect handler
- `vncviewer/ClientConnectionCacheRect.cpp` - Cached rectangles
- `vncviewer/ClientConnectionCursor.cpp` - Cursor handling
- `vncviewer/ClientConnectionFile.cpp` - File transfer
- `vncviewer/ClientConnectionRSAAES.cpp` - RSA-AES encryption
- `vncviewer/ClientConnectionTLS.cpp` - TLS connection
- `vncviewer/AuthDialog.cpp` - Authentication dialog
- `vncviewer/AccelKeys.cpp` - Keyboard accelerator handling

**Architecture Pattern**: The `ClientConnection` class dispatches to encoding-specific handler methods based on the rectangle encoding type received.

### 5.3 Viewer Rendering Loop

**Claim**: The viewer rendering loop follows a request-response pattern where the client requests updates and the server responds with encoded rectangles.

**Source**: RFB Protocol Spec [^30^], LibVNCClient `vncviewer.c`

**Pseudocode**:
```
while (connected) {
    SendFramebufferUpdateRequest(x, y, w, h, incremental);
    wait for FramebufferUpdate message;
    for each rectangle in update {
        read rectangle header (x, y, w, h, encoding);
        switch(encoding) {
            case Raw: decodeRaw(rect); break;
            case Tight: decodeTight(rect); break;
            case ZRLE: decodeZRLE(rect); break;
            case CopyRect: decodeCopyRect(rect); break;
            // ...
        }
        render rectangle to framebuffer;
    }
    update display;
}
```

---

## 6. VNC Server Architecture

### 6.1 LibVNCServer Architecture

**Claim**: LibVNCServer provides a library-based approach where the application provides a framebuffer and calls API functions to mark regions as modified.

**Source**: `include/rfb/rfb.h` [^43^], `src/libvncserver/main.c` [^80^]

**Server Structure** (`rfbScreenInfo`):
```c
typedef struct _rfbScreenInfo {
    int width, height;
    int bitsPerPixel, depth;
    char* frameBuffer;           /* Application-provided framebuffer */
    rfbPixelFormat serverFormat; /* Server pixel format */
    
    /* Hooks */
    rfbKbdAddEventProcPtr kbdAddEvent;
    rfbPtrAddEventProcPtr ptrAddEvent;
    rfbNewClientHookPtr newClientHook;
    rfbDisplayHookPtr displayHook;
    
    /* Client list */
    struct _rfbClientRec* clientHead;
    struct _rfbClientRec* pointerClient;
    
    /* Settings */
    int maxRectsPerUpdate;
    int deferUpdateTime;
    /* ... */
} rfbScreenInfo, *rfbScreenInfoPtr;
```

### 6.2 UltraVNC Server (WinVNC)

**Claim**: UltraVNC's server uses multiple screen capture methods including system hooks, mirror driver, and desktop duplication API.

**Source**: `winvnc/winvnc/` directory [^62^], [^64^]

**Key Files**:
- `winvnc/winvnc/DeskdupEngine.cpp` / `DeskdupEngine.h` - Desktop Duplication Engine
- `winvnc/vnchooks/` - System hook DLL for change detection
- `winvnc/winvnc/MouseSimulator.cpp` - Mouse input injection
- `winvnc/winvnc/LayeredWindows.cpp` - Layered window capture
- `winvnc/winvnc/IPC.cpp` / `IPC.h` - Inter-process communication

**Screen Capture Methods** (from UltraVNC documentation [^52^]):

| Method | Speed | Accuracy | Description |
|--------|-------|----------|-------------|
| Full Screen Poll | Slow | High | Scan entire screen periodically |
| System Hook DLL | Fast | High | DDI hooking for change hints |
| Mirror Driver | Very Fast | High | Kernel-mode screen capture |
| Desktop Duplication | Very Fast | High | Windows 8+ DXGI API |

### 6.3 Screen Capture Implementation

**Claim**: The framebuffer is captured through OS-specific APIs and changes are detected either through polling or event-driven hooks.

**Source**: UltraVNC docs [^52^], [^54^]

**UltraVNC Approaches**:
1. **Hook DLL** (`vnchooks/`): Injects into display driver chain to receive change notifications
2. **Mirror Driver**: Legacy kernel-mode driver that clones the display
3. **Desktop Duplication API** (`DeskdupEngine.cpp`): Modern Windows DXGI-based capture
4. **Polling**: Fallback scanning of screen regions

**LibVNCServer Approach**: The application is responsible for writing to the framebuffer and calling `rfbMarkRectAsModified()`.

### 6.4 Update Generation

**Claim**: Framebuffer updates are generated by comparing the current framebuffer against a reference, identifying changed rectangles, and encoding them.

**Source**: `src/libvncserver/main.c` [^80^]

**Process**:
1. Application/server marks modified regions via `rfbMarkRectAsModified()`
2. Server's `clientOutput` thread detects pending updates via `FB_UPDATE_PENDING()`
3. `rfbSendFramebufferUpdate()` iterates over modified regions
4. Each rectangle is encoded using the negotiated encoding
5. Encoded data is sent to the client

---

## 7. Input Handling

### 7.1 PointerEvent (Message Type 5)

**Claim**: Pointer events carry button mask and X/Y coordinates from client to server.

**Source**: RFB Protocol Spec [^30^], `include/rfb/rfbproto.h`

**Message Structure**:
```c
typedef struct {
    uint8_t messageType;  /* 5 */
    uint8_t buttonMask;   /* Bit 0=left, 1=middle, 2=right, 3=scrollUp, 4=scrollDown */
    uint16_t x;           /* X position */
    uint16_t y;           /* Y position */
} rfbPointerEventMsg;
```

**Button Mask Values**:
```c
#define rfbButton1Mask  1  /* Left */
#define rfbButton2Mask  2  /* Middle */
#define rfbButton3Mask  4  /* Right */
#define rfbButton4Mask  8  /* Scroll Up */
#define rfbButton5Mask  16 /* Scroll Down */
```

**Server-Side Handling** (LibVNCServer):
```c
/* In rfbScreenInfo structure */
screen->ptrAddEvent = myPointerHandler;  /* Set callback */

void myPointerHandler(int buttonMask, int x, int y, rfbClientPtr cl) {
    /* Handle mouse event */
}
```

### 7.2 KeyEvent (Message Type 4)

**Claim**: Key events carry a keysym value representing the pressed/released key.

**Source**: RFB Protocol Spec [^49^], `include/rfb/rfbproto.h`

**Message Structure**:
```c
typedef struct {
    uint8_t messageType;  /* 4 */
    uint8_t down;         /* 1=pressed, 0=released */
    uint16_t pad;         /* Padding */
    uint32_t key;         /* Keysym value */
} rfbKeyEventMsg;
```

**Server-Side Handling** (LibVNCServer):
```c
/* In rfbScreenInfo structure */
screen->kbdAddEvent = myKeyboardHandler;  /* Set callback */

void myKeyboardHandler(rfbBool down, rfbKeySym key, rfbClientPtr cl) {
    /* Handle keyboard event */
}
```

### 7.3 QEMU Extended KeyEvent

**Claim**: QEMU extended the KeyEvent with a raw keycode field to solve keyboard layout issues.

**Source**: [^46^]

**Extended Message**:
```c
typedef struct {
    uint8_t messageType;   /* 255 (QEMU Server Message) */
    uint8_t submessageType; /* 0 (ExtendedKeyEvent) */
    uint16_t down;         /* 1=pressed, 0=released */
    uint32_t keysym;       /* X11 keysym */
    uint32_t keycode;      /* Raw XT keycode */
} rfbQEMUExtendedKeyEventMsg;
```

### 7.4 UltraVNC Input Injection

**Source**: `winvnc/winvnc/MouseSimulator.cpp` [^64^]

**Files for Input Handling**:
- `winvnc/winvnc/MouseSimulator.cpp` - Mouse event injection on Windows
- `src/libvncserver/main.c` - Default pointer/keyboard event handlers

---

## 8. Security & Authentication

### 8.1 Security Types

**Claim**: VNC supports multiple security types ranging from no authentication to encrypted connections.

**Source**: `src/libvncserver/auth.c` [^38^], `include/rfb/rfb.h` [^43^]

**Security Types**:

| Number | Name | Description |
|--------|------|-------------|
| 0 | Invalid | Connection failed |
| 1 | None | No authentication |
| 2 | VNC Auth | Classic VNC password (DES) |
| 5 | RA2 | RSA-AES |
| 6 | RA2ne | RSA-AES unencrypted |
| 16 | Tight | TightVNC security |
| 17 | Ultra | UltraVNC MSLogon |
| 18 | TLS | Anonymous TLS |
| 19 | VeNCrypt | TLS + VNC auth |
| 20 | SASL | SASL authentication |
| 30 | Apple ARD | Apple Remote Desktop |
| 113 | MSLogonII | UltraVNC MSLogon II |

**LibVNC Security Handler Registration**:
```c
typedef struct _rfbSecurity {
    uint8_t type;
    void (*handler)(struct _rfbClientRec* cl);
    struct _rfbSecurity* next;
    enum rfbSecurityTag securityTags;
} rfbSecurityHandler;
```

### 8.2 VNC Authentication

**Claim**: Classic VNC authentication uses a challenge-response protocol with DES encryption.

**Source**: `src/libvncserver/auth.c`, UltraVNC `rfb/vncauth.c` [^60^]

**Process**:
1. Server sends 16-byte random challenge
2. Client encrypts challenge with DES using password
3. Server verifies by performing same encryption
4. `rfbEncryptBytes()` in `rfb/vncauth.c` (UltraVNC) and `auth.c` (LibVNC)

### 8.3 UltraVNC DSM Plugin

**Claim**: UltraVNC supports Data Stream Modification plugins for end-to-end encryption.

**Source**: `DSMPlugin/` directory [^34^]

**Files**:
- `DSMPlugin/` - Plugin architecture for encryption
- `rfb/dh.cpp` / `dh.h` - Diffie-Hellman key exchange for MSLogon

### 8.4 TLS/VeNCrypt Support

**Source**: LibVNC `src/libvncserver/rfbssl_*.c` [^38^]

**Files**:
- `src/libvncserver/rfbssl_openssl.c` - OpenSSL TLS backend
- `src/libvncserver/rfbssl_gnutls.c` - GnuTLS backend
- `src/libvncserver/rfbssl_none.c` - No TLS (stub)
- `src/libvncclient/tls_openssl.c` - Client OpenSSL
- `src/libvncclient/tls_gnutls.c` - Client GnuTLS

---

## 9. VNC Repeater / Proxy

### 9.1 UltraVNC Repeater

**Claim**: The UltraVNC Repeater acts as a proxy between viewer and server, allowing both to be behind NAT.

**Source**: `repeater/` directory [^34^], [^67^]

**UltraVNC Repeater Modes**:
- **Mode I**: Simple proxy, server connects to repeater, viewer connects to repeater
- **Mode II**: ID-based matching, both sides connect using matching IDs

**Files**:
- `repeater/` - Repeater source code

### 9.2 Repeater Protocol

**Claim**: LibVNCClient supports UltraVNC repeater mode 2 connections.

**Source**: `include/rfb/rfb.h` [^43^]

**Evidence**:
```c
/* UltraVNC Repeater Mode 2 connection */
extern rfbClientPtr rfbUltraVNCRepeaterMode2Connection(
    rfbScreenInfoPtr rfbScreen, 
    char *repeaterHost, 
    int repeaterPort, 
    const char* repeaterId);
```

**Reference**: [^66^], [^68^]

---

## 10. Key Blueprint Patterns for Custom Implementation

### 10.1 Minimal VNC Server Pattern (LibVNCServer)

```c
#include <rfb/rfb.h>

int main(int argc, char** argv) {
    /* 1. Create screen */
    rfbScreenInfoPtr screen = rfbGetScreen(&argc, argv, 800, 600, 8, 3, 4);
    
    /* 2. Allocate framebuffer */
    screen->frameBuffer = (char*)malloc(800 * 600 * 4);
    
    /* 3. Set hooks */
    screen->kbdAddEvent = myKeyHandler;
    screen->ptrAddEvent = myPointerHandler;
    screen->newClientHook = myClientHook;
    
    /* 4. Initialize */
    rfbInitServer(screen);
    
    /* 5. Run event loop */
    rfbRunEventLoop(screen, -1, TRUE);  /* background = TRUE */
    
    /* 6. Mark changes when framebuffer updates */
    while(1) {
        /* ... update screen->frameBuffer ... */
        rfbMarkRectAsModified(screen, x1, y1, x2, y2);
    }
    
    rfbScreenCleanup(screen);
    return 0;
}
```

### 10.2 Minimal VNC Client Pattern (LibVNCClient)

```c
#include <rfb/rfbclient.h>

static rfbBool got_update(rfbClient* client, int x, int y, int w, int h) {
    /* Process received rectangle */
    return TRUE;
}

int main(int argc, char** argv) {
    /* 1. Get client */
    rfbClient* client = rfbGetClient(8, 3, 4);
    
    /* 2. Set callbacks */
    client->GotFrameBufferUpdate = got_update;
    client->MallocFrameBuffer = malloc_fb;
    
    /* 3. Connect */
    if(!rfbInitClient(client, &argc, argv))
        return 1;
    
    /* 4. Main loop */
    while(1) {
        int ret = WaitForMessage(client, 500000);
        if(ret > 0)
            if(!HandleRFBServerMessage(client))
                break;
    }
    
    rfbClientCleanup(client);
    return 0;
}
```

### 10.3 Key Architecture Decisions

1. **Region-Based Updates**: Use region data structures (like `sraRegion`) to track modified areas rather than full-screen comparisons
2. **Per-Client State**: Each client maintains its own encoding preferences, pixel format, and compression state
3. **Thread Safety**: Lock per-client mutexes when accessing `modifiedRegion` and `copyRegion`
4. **Deferral**: Batch small updates by deferring for a few milliseconds
5. **Encoding Selection**: Choose encoding per-rectangle based on content type (solid fills, photos, text)
6. **Translate Function**: Convert between server and client pixel formats on the fly
7. **Update Buffer**: Use a fixed-size output buffer (32KB default) to batch network writes

### 10.4 Encoding Selection Strategy

| Content Type | Best Encoding | Why |
|-------------|---------------|-----|
| Solid color fills | Tight Fill | Single color, no pixel data |
| Photographic/video | Tight JPEG | Lossy compression for natural images |
| Text/UI | Tight basic + palette | Small palette, high compression |
| Large unchanged areas | CopyRect | Zero pixel data for copies |
| Fine-grained changes | Hextile | Good for small scattered updates |
| Maximum compatibility | Raw | All clients support it |

---

## File Reference Index

### LibVNCServer Key Files

| File | Purpose |
|------|---------|
| `include/rfb/rfb.h` | Main API header, all data structures |
| `include/rfb/rfbproto.h` | Protocol message definitions |
| `include/rfb/rfbclient.h` | Client library API |
| `src/libvncserver/main.c` | Event loop, server initialization |
| `src/libvncserver/rfbserver.c` | RFB protocol server implementation |
| `src/libvncserver/auth.c` | Authentication handlers |
| `src/libvncserver/sockets.c` | Socket I/O management |
| `src/libvncserver/rfbregion.c` | Region data structures |
| `src/libvncserver/hextile.c` | Hextile encoding |
| `src/libvncserver/rre.c` | RRE encoding |
| `src/libvncserver/corre.c` | CoRRE encoding |
| `src/libvncserver/tight.c` | Tight encoding |
| `src/libvncserver/zrle.c` | ZRLE encoding |
| `src/libvncserver/zlib.c` | Zlib encoding |
| `src/libvncserver/ultra.c` | Ultra encoding |
| `src/libvncserver/translate.c` | Pixel format translation |
| `src/libvncserver/cursor.c` | Cursor handling |
| `src/libvncserver/websockets.c` | WebSocket transport |

### LibVNCClient Key Files

| File | Purpose |
|------|---------|
| `src/libvncclient/rfbclient.c` | Main client implementation |
| `src/libvncclient/vncviewer.c` | Example viewer |
| `src/libvncclient/sockets.c` | Client socket I/O |
| `src/libvncclient/tight.c` | Tight decoding |
| `src/libvncclient/zrle.c` | ZRLE decoding |
| `src/libvncclient/hextile.c` | Hextile decoding |
| `src/libvncclient/rre.c` | RRE decoding |
| `src/libvncclient/corre.c` | CoRRE decoding |
| `src/libvncclient/ultra.c` | Ultra decoding |
| `src/libvncclient/zlib.c` | Zlib decoding |
| `src/libvncclient/trle.c` | TRLE decoding |
| `src/libvncclient/listen.c` | Reverse connections |

### UltraVNC Key Files

| File | Purpose |
|------|---------|
| `rfb/rfbproto.h` | Protocol definitions |
| `rfb/vncauth.c` | VNC authentication |
| `rfb/zrleEncode.h` / `zrleDecode.h` | ZRLE codec |
| `vncviewer/ClientConnection.cpp` | Main viewer connection |
| `vncviewer/ClientConnection*.cpp` | Encoding-specific decoders |
| `winvnc/winvnc/DeskdupEngine.cpp` | Desktop capture engine |
| `winvnc/vnchooks/` | System hooks |
| `repeater/` | VNC repeater/proxy |
| `DSMPlugin/` | Encryption plugins |

---

## References

- [^25^] UltraVNC GitHub Organization: https://github.com/ultravnc
- [^26^] LibVNCServer Releases: https://github.com/libvnc/libvncserver/releases
- [^27^] LibVNCServer NEWS: https://github.com/LibVNC/libvncserver/blob/master/NEWS.md
- [^28^] LibVNCServer SourceForge Mirror: https://sourceforge.net/projects/libvncserver.mirror/
- [^29^] Homebrew libvncserver Formula: https://formulae.brew.sh/formula/libvncserver
- [^30^] RFB Protocol Documentation (VNCDoTool): https://vncdotool.readthedocs.io/en/0.8.0/rfbproto.html
- [^31^] LibVNCServer GitHub README: https://github.com/LibVNC/libvncserver
- [^32^] StackOverflow: How to send framebuffer update in VNC
- [^33^] rfb_encodings Rust crate: https://docs.rs/rfb-encodings
- [^34^] UltraVNC GitHub Repository: https://github.com/ultravnc/UltraVNC
- [^35^] RealVNC Security: https://www.realvnc.com/en/connect/security/
- [^36^] QEMU GSoC VNC Project: https://wiki.qemu.org/Google_Summer_of_Code_2010/VNC
- [^37^] IGEL VNC Optimization: https://kb.igel.com/en/igel-os/current/vnc-optimization-1
- [^38^] LibVNCServer Source Directory: https://github.com/LibVNC/libvncserver/tree/master/src/libvncserver
- [^39^] Fuzzing libvnc: https://introspector.oss-fuzz.com/project-profile?project=libvnc
- [^40^] x11vnc issue with rfbSendFramebufferUpdate: https://github.com/LibVNC/x11vnc/issues/236
- [^41^] LibVNCServer rfbserver.c: https://github.com/LibVNC/libvncserver/blob/master/src/libvncserver/rfbserver.c
- [^42^] x11vnc Repository: https://github.com/libvnc/x11vnc
- [^43^] LibVNCServer rfb.h: https://github.com/LibVNC/libvncserver/blob/master/include/rfb/rfb.h
- [^45^] LibVNCServer Issue #75 (zlib compilation): https://github.com/LibVNC/libvncserver/issues/75
- [^46^] noVNC QEMU RFB Keyboard Extension: https://danielhb.github.io/article/2019/05/06/noVNC-QEMU-RFB.html
- [^47^] VNC Touchscreen Input: https://www.mobileread.com/forums/showthread.php?t=322627
- [^48^] VAASeline VNC Attack Suite: https://blackhat.com/presentations/bh-europe-09/Smith/BlackHat-Europe-2009-Smith-VAASeline-VNC-Attack-whitepaper.pdf
- [^49^] RFB Protocol Input Protocol: https://vncdotool.readthedocs.io/en/78/rfbproto.html
- [^50^] TightVNC What's New: https://www.tightvnc.com/whatsnew.php
- [^51^] LibVNCServer Mirror (Gitee): https://gitee.com/wanglisoftware/libvncserver_1
- [^52^] UltraVNC Screen Capture Help: https://uvnc.com/webhelp/screencapture.html
- [^53^] MarcusW.VncClient (C#): https://vnc-client.marcusw.de/
- [^54^] UltraVNC Server Configuration: https://uvnc.com/docs/ultravnc-server/49-ultravnc-server-configuration.html
- [^56^] TightVNC Decoder Licensing: https://www.tightvnc.com/decoder.php
- [^57^] SDLvncviewer Example: https://libvnc.github.io/doc/html/_s_d_lvncviewer_8c-example.html
- [^59^] LibVNCClient Source: https://github.com/LibVNC/libvncserver/tree/master/src/libvncclient
- [^60^] UltraVNC vncviewer directory: https://github.com/ultravnc/UltraVNC/tree/main/vncviewer
- [^62^] UltraVNC winvnc directory: https://github.com/ultravnc/UltraVNC/tree/main/winvnc
- [^64^] UltraVNC winvnc/winvnc directory: https://github.com/ultravnc/UltraVNC/tree/main/winvnc/winvnc
- [^66^] ServerFault: Self-hosted VNC alternatives: https://serverfault.com/questions/802394
- [^67^] UltraVNC Repeater: https://uvnc.com/downloads/ultravnc-repeater/83-ultravnc-repeater.html
- [^68^] VNC_Proxy Repeater (GitHub): https://github.com/smasherprog/VNC_Proxy
- [^71^] RFB Protocol Specification (GitHub): https://github.com/rfbproto/rfbproto/blob/master/rfbproto.rst
- [^73^] x11vnc SSL Crash Issue: https://github.com/LibVNC/libvncserver/issues/219
- [^80^] LibVNCServer main.c: https://raw.githubusercontent.com/LibVNC/libvncserver/master/src/libvncserver/main.c

---

*Analysis completed with 20+ independent searches across LibVNCServer, LibVNCClient, UltraVNC, and TightVNC codebases. All file paths and code references verified against current GitHub repositories as of analysis date.*
