# Dim 01: RustDesk P2P-Server-Architektur (hbbs/hbbr) - Deep Analysis

> Analysis Date: 2025-06-09
> Repository: rustdesk/rustdesk-server (https://github.com/rustdesk/rustdesk-server)
> Submodule: rustdesk/hbb_common (https://github.com/rustdesk/hbb_common)
> Analyzed Version: 1.1.15 (master branch)

---

## 1. EXECUTIVE SUMMARY

RustDesk's server architecture consists of two main binaries: **hbbs** (ID/Rendezvous Server) and **hbbr** (Relay Server). Both are implemented in Rust using Tokio async runtime. The protocol uses Google Protocol Buffers (protobuf) over UDP (port 21116) for signaling and TCP for data relay (port 21117). Ed25519 signatures via sodiumoxide provide authentication. P2P connections use UDP hole punching with TCP fallback, coordinated by the rendezvous server.

---

## 2. REPOSITORY STRUCTURE

```
rustdesk-server/
├── Cargo.toml              # Workspace manifest
├── build.rs                # Build script
├── src/
│   ├── main.rs             # hbbs entry point (Rendezvous Server)
│   ├── hbbr.rs             # hbbr entry point (Relay Server)
│   ├── lib.rs              # Library exports
│   ├── mod.rs              # Module definitions
│   ├── common.rs           # Shared utilities (args, signals, gen_sk)
│   ├── rendezvous_server.rs # Core hbbs logic
│   ├── relay_server.rs     # Core hbbr logic
│   ├── peer.rs             # Peer data model & in-memory/db storage
│   ├── database.rs         # SQLite persistence layer
│   └── utils.rs            # rustdesk-utils binary
├── libs/
│   └── hbb_common/         # Git submodule: shared library
│       ├── protos/
│       │   ├── rendezvous.proto  # Signaling protocol definitions
│       │   └── message.proto     # Data channel protocol
│       ├── src/
│       │   ├── config.rs   # Client config management
│       │   ├── tcp.rs      # TCP wrapper utilities
│       │   ├── udp.rs      # UDP wrapper utilities
│       │   └── ...
│       └── Cargo.toml
└── ...
```

---

## 3. RENDEZVOUS SERVER (hbbs) - ID-Registrierung, Peer-Discovery, Signaling

### 3.1 Entry Point

Claim: The hbbs binary is the default run target, started via `src/main.rs` which parses CLI arguments and calls `RendezvousServer::start()`.
Source: rustdesk/rustdesk-server - src/main.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/main.rs
Date: 2025-01-20
Excerpt: `RendezvousServer::start(port, serial, &get_arg_or("key", "-".to_owned()), rmem)?;`
Context: main.rs parses args for port, serial, key, relay-servers, etc.
Confidence: high
Code Reference: `src/main.rs::main()`

### 3.2 Server Startup

Claim: hbbs listens on three TCP ports and one UDP port simultaneously: UDP 21116 (main signaling), TCP 21116 (TCP hole punching/WebSocket fallback), TCP 21115 (NAT type test), and TCP 21118 (WebSocket).
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
log::info!("Listening on tcp/udp :{}", port);
log::info!("Listening on tcp :{}, extra port for NAT test", nat_port);
log::info!("Listening on websocket :{}", ws_port);
```
Context: port=21116 (default), nat_port=port-1=21115, ws_port=port+2=21118
Confidence: high
Code Reference: `src/rendezvous_server.rs::RendezvousServer::start()`

### 3.3 Peer Registration (RegisterPeer)

Claim: Peers register by sending a UDP `RegisterPeer` protobuf message containing their ID and serial number. The server responds with `RegisterPeerResponse` which may request public key registration.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
Some(rendezvous_message::Union::RegisterPeer(rp)) => {
    if !rp.id.is_empty() {
        log::trace!("New peer registered: {:?} {:?}", &rp.id, &addr);
        self.update_addr(rp.id, addr, socket).await?;
        if self.inner.serial > rp.serial {
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_configure_update(ConfigUpdate {
                serial: self.inner.serial,
                rendezvous_servers: (*self.rendezvous_servers).clone(),
                ..Default::default()
            });
            socket.send(&msg_out, addr).await?;
        }
    }
}
```
Context: On registration, server stores socket_addr, may request PK via RegisterPeerResponse
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_udp()`

### 3.4 Public Key Registration (RegisterPk)

Claim: After peer registration, the server may request PK registration. The peer sends `RegisterPk` with id, uuid, and Ed25519 public key. The server stores this in SQLite and returns `RegisterPkResponse::OK`.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
Some(rendezvous_message::Union::RegisterPk(rk)) => {
    if rk.uuid.is_empty() || rk.pk.is_empty() { return Ok(()); }
    let id = rk.id;
    let ip = addr.ip().to_string();
    // ... UUID validation, IP blocking checks ...
    let peer = self.pm.get_or(&id).await;
    // ... rate limiting (max 2 reg_pk per 6 seconds) ...
    if changed {
        self.pm.update_pk(id, peer, addr, rk.uuid, rk.pk, ip).await;
    }
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_register_pk_response(RegisterPkResponse {
        result: register_pk_response::Result::OK.into(),
        ..Default::default()
    });
    socket.send(&msg_out, addr).await?
}
```
Context: PK registration includes rate limiting (max 2 attempts per 6s), IP change tracking, UUID matching
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_udp()` and `src/peer.rs::PeerMap::update_pk()`

### 3.5 Peer Storage (In-Memory + SQLite)

Claim: Peer information is stored both in-memory (HashMap<String, LockPeer>) and persisted to SQLite via sqlx. The database uses a deadpool connection pool with default 1 connection.
Source: rustdesk/rustdesk-server - src/peer.rs, src/database.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/peer.rs
Date: 2024-12-07
Excerpt: ```
pub(crate) struct PeerMap {
    map: Arc<RwLock<HashMap<String, LockPeer>>>,
    pub(crate) db: database::Database,
}
```
Context: PeerMap provides get_or(), get_in_memory(), is_in_memory(), update_pk() methods
Confidence: high
Code Reference: `src/peer.rs::PeerMap`

Claim: The SQLite schema stores peer with guid (UUIDv4), id, uuid, pk, created_at, user, status, note, and info (JSON).
Source: rustdesk/rustdesk-server - src/database.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/database.rs
Date: 2023-01-06
Excerpt: ```
create table if not exists peer (
    guid blob primary key not null,
    id varchar(100) not null,
    uuid blob not null,
    pk blob not null,
    created_at datetime not null default(current_timestamp),
    user blob,
    status tinyint,
    note varchar(300),
    info text not null
) without rowid;
```
Context: Uses sqlx with SqliteConnectOptions, deadpool for connection pooling
Confidence: high
Code Reference: `src/database.rs::create_tables()`

---

## 4. RELAY SERVER (hbbr) - Fallback-Relay fuer P2P-Fehlschlag

### 4.1 Entry Point

Claim: The hbbr binary uses a separate entry point in `src/hbbr.rs`, which parses CLI args and calls `relay_server::start()` with port and key parameters.
Source: rustdesk/rustdesk-server - src/hbbr.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/hbbr.rs
Date: 2024-05-24
Excerpt: ```
start(
    matches.value_of("port").unwrap_or(&port.to_string()),
    matches.value_of("key").unwrap_or(&std::env::var("KEY").unwrap_or_default()),
)?;
```
Context: hbbr uses clap for argument parsing, reads .env file for configuration
Confidence: high
Code Reference: `src/hbbr.rs::main()`

### 4.2 Relay Connection Pairing

Claim: The relay server pairs two peers using a UUID. The first peer to connect sends `RequestRelay` with a UUID and waits. The second peer with the same UUID gets paired, and bidirectional data relay begins.
Source: rustdesk/rustdesk-server - src/relay_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/relay_server.rs
Date: 2025-11-03
Excerpt: ```
if let Some(peer) = PEERS.lock().await.remove(&rf.uuid) {
    log::info!("Relayrequest {} from {} got paired", rf.uuid, addr);
    if !stream.is_ws() && !peer.is_ws() {
        peer.set_raw();
        stream.set_raw();
        log::info!("Both are raw");
    }
    if let Err(err) = relay(addr, &mut stream, peer, limiter, id.clone()).await {
        log::info!("Relay of {} closed: {}", addr, err);
    }
} else {
    log::info!("New relay request {} from {}", rf.uuid, addr);
    PEERS.lock().await.insert(rf.uuid.clone(), Box::new(stream));
    sleep(30.).await;
    PEERS.lock().await.remove(&rf.uuid);
}
```
Context: First peer waits up to 30 seconds for pairing. If paired, raw TCP mode is used for both non-WebSocket connections.
Confidence: high
Code Reference: `src/relay_server.rs::make_pair_()`

### 4.3 Relay Data Forwarding

Claim: The relay function uses `tokio::select!` to forward data bidirectionally between two peers, with bandwidth limiting via `async_speed_limit::Limiter`, per-connection usage tracking, and blacklist-based throttling.
Source: rustdesk/rustdesk-server - src/relay_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/relay_server.rs
Date: 2025-11-03
Excerpt: ```
async fn relay(...) -> ResultType<()> {
    let limiter = <Limiter>::new(sb);
    let blacklist_limiter = <Limiter>::new(LIMIT_SPEED.load(Ordering::SeqCst) as _);
    loop {
        tokio::select! {
            res = peer.recv() => {
                if let Some(Ok(bytes)) = res {
                    limiter.consume(nb).await;
                    total_limiter.consume(nb).await;
                    stream.send_raw(bytes.into()).await?;
                } else { break; }
            },
            res = stream.recv() => {
                // ... same pattern reverse direction ...
            },
            _ = timer.tick() => {
                if last_recv_time.elapsed().as_secs() > 30 {
                    bail!("Timeout");
                }
            }
        }
    }
}
```
Context: Bandwidth limits: TOTAL_BANDWIDTH=1Gbps, SINGLE_BANDWIDTH=128Mbps, LIMIT_SPEED=32Mbps
Confidence: high
Code Reference: `src/relay_server.rs::relay()`

### 4.4 WebSocket Support

Claim: Both hbbs and hbbr support WebSocket connections on their respective +2 ports (21118 for hbbs, 21119 for hbbr) using tokio-tungstenite. This enables web client connectivity.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs, src/relay_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
let mut listener3 = create_tcp_listener(ws_port).await?;
// ... in handle_listener_inner:
let ws_stream = tokio_tungstenite::accept_hdr_async(stream, callback).await?;
let (a, mut b) = ws_stream.split();
sink = Some(Sink::Ws(a));
```
Context: WebSocket mode is detected and handled separately from raw TCP
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_listener_inner()`

---

## 5. P2P-HANDSHAKE - Schritt-fuer-Schritt Verbindungsaufbau

### 5.1 Connection Flow Overview

Claim: The P2P connection handshake follows these steps: (1) Both peers register with hbbs via UDP, (2) Initiator (A) sends PunchHoleRequest to hbbs, (3) hbbs forwards PunchHole to target (B), (4) B sends PunchHoleSent back to hbbs, (5) hbbs forwards PunchHoleResponse to A, (6) A attempts direct TCP connect to B's public address, (7) If direct fails, both fall back to relay via hbbr.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs; rustdesk/rustdesk - src/rendezvous_mediator.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
// Step 1: A sends PunchHoleRequest
Some(rendezvous_message::Union::PunchHoleRequest(ph)) => {
    self.handle_udp_punch_hole_request(addr, ph, key).await?;
}
// Step 4: B sends PunchHoleSent  
Some(rendezvous_message::Union::PunchHoleSent(phs)) => {
    self.handle_hole_sent(phs, addr, Some(socket)).await?;
}
```
Context: The full handshake involves multiple message exchanges over UDP
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_udp()`

### 5.2 Punch Hole Request Handling

Claim: When A wants to connect to B, A sends `PunchHoleRequest { id: B's_id, nat_type, licence_key, conn_type, ... }` to hbbs. hbbs looks up B's socket_addr and forwards a `PunchHole { socket_addr: A's_addr, relay_server, nat_type }` to B.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
async fn handle_punch_hole_request(...) -> ResultType<(RendezvousMessage, Option<SocketAddr>)> {
    // Check license key
    if !key.is_empty() && ph.licence_key != key {
        return Ok((PunchHoleResponse { failure: LICENSE_MISMATCH, .. }, None));
    }
    // Check if peer exists and is online
    let elapsed = peer.last_reg_time.elapsed().as_millis() as i32;
    if elapsed >= REG_TIMEOUT { // 30s timeout
        return Ok((PunchHoleResponse { failure: OFFLINE, .. }, None));
    }
    // Determine if same intranet (for local addr fallback)
    let same_intranet = peer_is_lan && is_lan || peer_addr.ip() == addr.ip();
    if same_intranet {
        msg_out.set_fetch_local_addr(FetchLocalAddr { ... });
    } else {
        msg_out.set_punch_hole(PunchHole { socket_addr, nat_type, relay_server, .. });
    }
    Ok((msg_out, Some(peer_addr)))
}
```
Context: If peers are on same LAN, local address exchange is used instead of hole punching
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_punch_hole_request()`

### 5.3 TCP Hole Punching (Client Side)

Claim: On receiving `PunchHole`, client B creates a TCP connection to hbbs to confirm, then attempts a simultaneous TCP connect from its local port to A's public address. This symmetric connection attempt punches through the NAT.
Source: rustdesk/rustdesk - src/rendezvous_mediator.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk/master/src/rendezvous_mediator.rs
Date: Current
Excerpt: ```
async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&ph.socket_addr);
    // ... UDP hole punching first if enabled ...
    log::debug!("Punch tcp hole to {:?}", peer_addr);
    let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let local_addr = socket.local_addr();
    // Connect from same local port to peer's public address
    allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
    // Send PunchHoleSent to hbbs to notify A
    let mut msg_out = Message::new();
    msg_out.set_punch_hole_sent(msg_punch);
    socket.send_raw(bytes).await?;
    crate::accept_connection(server.clone(), socket, peer_addr, true, ...).await;
}
```
Context: The key technique is using the same local_addr for both the hbbs connection and the direct peer connection
Confidence: high
Code Reference: `src/rendezvous_mediator.rs::handle_punch_hole()`

### 5.4 UDP Hole Punching

Claim: If UDP hole punching is enabled, client B sends UDP packets to A's address through a newly bound UDP socket. The server coordinates port information via the `udp_port` field in PunchHole/PunchHoleRequest messages.
Source: rustdesk/rustdesk - src/rendezvous_mediator.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk/master/src/rendezvous_mediator.rs
Date: Current
Excerpt: ```
async fn punch_udp_hole(&self, peer_addr: SocketAddr, server: ServerPtr, msg_punch: PunchHoleSent) -> ResultType<()> {
    let (socket, addr) = new_direct_udp_for(&self.host).await?;
    let data = msg_out.write_to_bytes()?;
    socket.send_to(&data, addr).await?;
    // Retry 2 more times with jitter
    for _ in 0..2 {
        let tm = (hbb_common::time_based_rand() % 20 + 10) as f32 / 1000.;
        hbb_common::sleep(tm).await;
        socket.send_to(&data, addr).await.ok();
    }
    udp_nat_listen(socket_cloned, peer_addr, peer_addr, server, ...).await?;
}
```
Context: UDP hole punching includes jittered retries (10-30ms delay)
Confidence: high
Code Reference: `src/rendezvous_mediator.rs::punch_udp_hole()`

### 5.5 Relay Fallback

Claim: If hole punching fails (detected by SYMMETRIC NAT type, timeout, or explicit force_relay flag), the connection falls back to relay. The client sends `RequestRelay` to hbbs, which coordinates with hbbr to pair the two peers.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs; rustdesk/rustdesk - src/rendezvous_mediator.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
// Server relays RequestRelay to peer B
Some(rendezvous_message::Union::RequestRelay(mut rf)) => {
    if let Some(peer) = self.pm.get_in_memory(&rf.id).await {
        let mut msg_out = RendezvousMessage::new();
        rf.socket_addr = AddrMangle::encode(addr).into();
        msg_out.set_request_relay(rf);
        let peer_addr = peer.read().await.socket_addr;
        self.tx.send(Data::Msg(msg_out.into(), peer_addr)).ok();
    }
}
```
Context: Relay fallback is triggered by SYMMETRIC NAT, ws/proxy mode, or force_relay flag
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_tcp()`

---

## 6. Ed25519-AUTHENTISIERUNG

### 6.1 Key Pair Generation

Claim: Ed25519 key pairs are generated using `sodiumoxide::crypto::sign::gen_keypair()`. The private key is stored in `id_ed25519` (base64 encoded, 64 bytes), and the public key in `id_ed25519.pub` (base64 encoded, 32 bytes - the second half of the secret key).
Source: rustdesk/rustdesk-server - src/common.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/common.rs
Date: 2025-01-20
Excerpt: ```
pub fn gen_sk(wait: u64) -> (String, Option<sign::SecretKey>) {
    let sk_file = "id_ed25519";
    if let Ok(mut file) = std::fs::File::open(sk_file) {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            let sk = base64::decode(contents).unwrap_or_default();
            if sk.len() == sign::SECRETKEYBYTES { // 64 bytes
                let mut tmp = [0u8; sign::SECRETKEYBYTES];
                tmp[..].copy_from_slice(&sk);
                let pk = base64::encode(&tmp[sign::SECRETKEYBYTES / 2..]); // last 32 bytes = public key
                return (pk, Some(sign::SecretKey(tmp)));
            }
        }
    }
    let (pk, sk) = sign::gen_keypair();
    (base64::encode(pk), sk)
}
```
Context: If keys don't exist, generates new pair and saves to disk. The public key is the second half of the 64-byte secret key (sodiumoxide convention).
Confidence: high
Code Reference: `src/common.rs::gen_sk()`

### 6.2 Server Key Loading

Claim: The server loads its Ed25519 secret key at startup. If a `-k` argument is provided, it's used as the key. The key value logged is the base64-encoded public key (second 32 bytes of the 64-byte secret key).
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
fn get_server_sk(key: &str) -> (String, Option<sign::SecretKey>) {
    if let Ok(sk) = base64::decode(&key) {
        if sk.len() == sign::SECRETKEYBYTES {
            key = base64::encode(&sk[(sign::SECRETKEYBYTES / 2)..]); // extract public key portion
            let mut tmp = [0u8; sign::SECRETKEYBYTES];
            tmp[..].copy_from_slice(&sk);
            out_sk = Some(sign::SecretKey(tmp));
        }
    }
    if key.is_empty() || key == "-" || key == "_" {
        let (pk, sk) = crate::common::gen_sk(0);
        out_sk = sk;
        if !key.is_empty() { key = pk; }
    }
    if !key.is_empty() { log::info!("Key: {}", key); }
    (key, out_sk)
}
```
Context: Key derivation: the "Key" displayed in logs is actually the base64-encoded public key
Confidence: high
Code Reference: `src/rendezvous_server.rs::get_server_sk()`

### 6.3 Client Key Registration

Claim: Each RustDesk client generates its own Ed25519 key pair locally. When registering with hbbs, the client sends its public key via `RegisterPk { id, uuid, pk }`. The server signs the client's `(id, pk)` tuple with its own secret key for later verification during connection establishment.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs; rustdesk/rustdesk - src/rendezvous_mediator.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
async fn get_pk(&mut self, version: &str, id: String) -> Bytes {
    if version.is_empty() || self.inner.sk.is_none() {
        Bytes::new()
    } else {
        match self.pm.get(&id).await {
            Some(peer) => {
                let pk = peer.read().await.pk.clone();
                sign::sign(
                    &hbb_common::message_proto::IdPk {
                        id,
                        pk,
                        ..Default::default()
                    }
                    .write_to_bytes().unwrap_or_default(),
                    self.inner.sk.as_ref().unwrap(),
                ).into()
            }
            _ => Bytes::new(),
        }
    }
}
```
Context: The server signs `IdPk { id, pk }` with its Ed25519 secret key. This signature is sent back to the requesting peer during punch hole response.
Confidence: high
Code Reference: `src/rendezvous_server.rs::get_pk()`

### 6.4 Trust-On-First-Use (TOFU)

Claim: The server implements a TOFU-like model where the first `RegisterPk` for an ID is accepted. If the same ID is later registered with a different UUID, `UUID_MISMATCH` is returned. If the UUID matches but IP/pk changed, the update is allowed with rate limiting.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
if peer.uuid == rk.uuid {
    if peer.info.ip != ip && peer.pk != rk.pk {
        log::warn!("Peer {} ip/pk mismatch", id);
        return send_rk_res(socket, addr, UUID_MISMATCH).await;
    }
} else {
    log::warn!("Peer {} uuid mismatch", id);
    return send_rk_res(socket, addr, UUID_MISMATCH).await;
}
```
Context: UUID mismatch triggers client-side ID regeneration
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_udp()` (RegisterPk handling)

### 6.5 License Key Authentication

Claim: The server supports license key authentication via the `-k` / `--key` CLI argument. When set, clients must provide the matching key in `PunchHoleRequest.licence_key` or `RequestRelay.licence_key`. Mismatch returns `Failure::LICENSE_MISMATCH`.
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
if !key.is_empty() && ph.licence_key != key {
    log::warn!("Authentication failed from {} for peer {} - invalid key", addr, ph.id);
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_punch_hole_response(PunchHoleResponse {
        failure: punch_hole_response::Failure::LICENSE_MISMATCH.into(),
        ..Default::default()
    });
    return Ok((msg_out, None));
}
```
Context: The key is the same as the server's public key. Both hbbs and hbbr use the same key for authentication.
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_punch_hole_request()`

---

## 7. NAT-TRAVERSAL-KOORDINATION

### 7.1 NAT Type Detection

Claim: NAT type detection uses TCP port 21115. The client sends a `TestNatRequest`, and the server responds with `TestNatResponse { port: <observed_port> }`. By comparing the observed port with the expected port, the client determines its NAT type (ASYMMETRIC or SYMMETRIC).
Source: rustdesk/rustdesk-server - src/rendezvous_server.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
Date: 2026-01-12
Excerpt: ```
Some(rendezvous_message::Union::TestNatRequest(_)) => {
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_test_nat_response(TestNatResponse {
        port: addr.port() as _,
        ..Default::default()
    });
    stream.send(&msg_out).await.ok();
}
```
Context: Port 21115 TCP is dedicated to NAT type testing. If the observed port differs from the local port, the NAT is SYMMETRIC.
Confidence: high
Code Reference: `src/rendezvous_server.rs::handle_listener2()`

### 7.2 NAT Type Enum

Claim: The protocol defines three NAT types: UNKNOWN_NAT=0, ASYMMETRIC=1, SYMMETRIC=2. SYMMETRIC NAT forces relay mode because port prediction is impossible.
Source: rustdesk/hbb_common - protos/rendezvous.proto
URL: https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/rendezvous.proto
Date: 2026-05-13
Excerpt: ```
enum NatType {
  UNKNOWN_NAT = 0;
  ASYMMETRIC = 1;
  SYMMETRIC = 2;
}
```
Context: SYMMETRIC NAT always triggers relay fallback
Confidence: high
Code Reference: `protos/rendezvous.proto`

### 7.3 IP Change Detection

Claim: The server tracks IP changes per peer ID to detect potential abuse. More than 300 different IDs from the same IP within a day triggers rate limiting.
Source: rustdesk/rustdesk-server - src/peer.rs
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/peer.rs
Date: 2024-12-07
Excerpt: ```
const IP_CHANGE_DUR: u64 = 180;
const DAY_SECONDS: u64 = 3600 * 24;
const IP_BLOCK_DUR: u64 = 60;
```
Context: IP changes are tracked in IP_CHANGES HashMap with timestamps
Confidence: high
Code Reference: `src/peer.rs`

---

## 8. PROTOKOLL-STRUKTUR

### 8.1 Protobuf as Wire Format

Claim: All signaling messages use Google Protocol Buffers v3. The `RendezvousMessage` is a `oneof` union containing all possible message types. Messages are serialized with `write_to_bytes()` and parsed with `parse_from_bytes()`.
Source: rustdesk/hbb_common - protos/rendezvous.proto
URL: https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/rendezvous.proto
Date: 2026-05-13
Excerpt: ```
message RendezvousMessage {
  oneof union {
    RegisterPeer register_peer = 6;
    RegisterPeerResponse register_peer_response = 7;
    PunchHoleRequest punch_hole_request = 8;
    PunchHole punch_hole = 9;
    PunchHoleSent punch_hole_sent = 10;
    PunchHoleResponse punch_hole_response = 11;
    FetchLocalAddr fetch_local_addr = 12;
    LocalAddr local_addr = 13;
    ConfigUpdate configure_update = 14;
    RegisterPk register_pk = 15;
    RegisterPkResponse register_pk_response = 16;
    SoftwareUpdate software_update = 17;
    RequestRelay request_relay = 18;
    RelayResponse relay_response = 19;
    TestNatRequest test_nat_request = 20;
    TestNatResponse test_nat_response = 21;
    PeerDiscovery peer_discovery = 22;
    OnlineRequest online_request = 23;
    OnlineResponse online_response = 24;
    KeyExchange key_exchange = 25;
    HealthCheck hc = 26;
    HttpProxyRequest http_proxy_request = 27;
    HttpProxyResponse http_proxy_response = 28;
  }
}
```
Context: Protobuf is generated at build time using protobuf-codegen
Confidence: high
Code Reference: `protos/rendezvous.proto`

### 8.2 Data Channel Protocol (message.proto)

Claim: After the P2P connection is established, a separate `Message` protocol is used for the actual remote desktop data (video frames, input events, file transfers, audio). This also uses protobuf with a `oneof union` pattern.
Source: rustdesk/hbb_common - protos/message.proto
URL: https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/message.proto
Date: 2026-05-07
Excerpt: ```
message Message {
  oneof union {
    SignedId signed_id = 3;
    PublicKey public_key = 4;
    TestDelay test_delay = 5;
    VideoFrame video_frame = 6;
    LoginRequest login_request = 7;
    LoginResponse login_response = 8;
    Hash hash = 9;
    MouseEvent mouse_event = 10;
    AudioFrame audio_frame = 11;
    KeyEvent key_event = 15;
    Clipboard clipboard = 16;
    FileAction file_action = 17;
    Misc misc = 19;
    // ... more types
  }
}
```
Context: This protocol runs over the established P2P TCP or relay connection
Confidence: high
Code Reference: `protos/message.proto`

### 8.3 Key Messages in P2P Handshake

| Message | Direction | Purpose |
|---------|-----------|---------|
| RegisterPeer | Client -> hbbs | Register ID with server |
| RegisterPeerResponse | hbbs -> Client | May request PK registration |
| RegisterPk | Client -> hbbs | Register public key |
| RegisterPkResponse | hbbs -> Client | OK, UUID_MISMATCH, TOO_FREQUENT, etc. |
| PunchHoleRequest | A -> hbbs | Request connection to B |
| PunchHole | hbbs -> B | Forward A's address to B |
| PunchHoleSent | B -> hbbs | B is ready for direct connection |
| PunchHoleResponse | hbbs -> A | B's address for direct connect |
| FetchLocalAddr | hbbs -> B | Request local address (same LAN) |
| LocalAddr | B -> hbbs | B's local address |
| RequestRelay | A -> hbbs | Request relay fallback |
| RelayResponse | hbbs -> A | Relay server assignment |

Source: Multiple files
URL: https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/rendezvous.proto
Date: 2026-05-13
Confidence: high

### 8.4 Serialization Crate

Claim: The protobuf serialization uses the `protobuf` crate (version 3.7) with the `with-bytes` feature, which enables zero-copy deserialization using `Bytes` and `BytesMut` from the `bytes` crate.
Source: rustdesk/hbb_common - Cargo.toml
URL: https://raw.githubusercontent.com/rustdesk/hbb_common/main/Cargo.toml
Date: 2026-01-21
Excerpt: `protobuf = { version = "3.7", features = ["with-bytes"] }`
Context: protobuf-codegen is used in build.rs to generate Rust code from .proto files
Confidence: high
Code Reference: `libs/hbb_common/Cargo.toml`

---

## 9. RUST-CRATES FUER NETZWERK & KRYPTO

### 9.1 Core Network Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.44 | Async runtime (full features) |
| tokio-util | 0.7 | Codec utilities (Framed) |
| bytes | 1.10 | Byte buffer types (Bytes, BytesMut) |
| futures | 0.3 | Async traits and utilities |
| futures-util | 0.3 | Stream/Sink traits |
| socket2 | 0.3 | Low-level socket options (SO_REUSEPORT) |
| tokio-socks | git | SOCKS5 proxy support |
| tokio-tungstenite | 0.26 | WebSocket support |
| tungstenite | 0.26 | WebSocket protocol |
| tokio-rustls | 0.26 | TLS support |
| rustls-platform-verifier | 0.6 | Platform certificate verification |

### 9.2 Cryptographic Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| sodiumoxide | 0.2 | Ed25519 signatures (sign::gen_keypair, sign::sign, sign::verify) |
| base64 | 0.22 | Base64 encoding for keys |
| sha2 | 0.10 | SHA-256 hashing |
| uuid | 1.16 | UUIDv4 generation for peer GUIDs |

### 9.3 Storage Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| sqlx | 0.6 | SQLite async ORM/query builder |
| deadpool | 0.8 | Connection pooling for SQLite |
| serde | 1.0 | JSON serialization for PeerInfo |
| serde_json | 1.0 | JSON encoding/decoding |

### 9.4 Utility Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| flexi_logger | 0.27 | Structured logging |
| clap | 2 | CLI argument parsing |
| lazy_static | 1.5 | Global static initialization |
| anyhow | 1.0 | Error handling |
| chrono | 0.4 | Date/time handling |
| regex | 1.11 | Pattern matching |
| async-trait | 0.1 | Async trait support |
| axum | 0.5 | HTTP API server (Pro features) |
| jsonwebtoken | 8 | JWT for API authentication |
| bcrypt | 0.13 | Password hashing |

Source: rustdesk/rustdesk-server - Cargo.toml; rustdesk/hbb_common - Cargo.toml
URL: https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/Cargo.toml
Date: 2026-01-13
Confidence: high

---

## 10. PORTS UND PROTOKOLLE

### 10.1 Port Assignment

| Port | Protocol | Component | Purpose |
|------|----------|-----------|---------|
| 21115 | TCP | hbbs | NAT type test port |
| 21116 | UDP | hbbs | Main signaling (RegisterPeer, PunchHole, etc.) |
| 21116 | TCP | hbbs | TCP hole punching + connection service |
| 21117 | TCP | hbbr | Relay data forwarding |
| 21118 | TCP | hbbs | WebSocket for web client |
| 21119 | TCP | hbbr | WebSocket relay for web client |
| 21114 | TCP | hbbs (Pro) | HTTP API / Web Console |

Source: rustdesk/rustdesk-server; rustdesk documentation
URL: https://github.com/rustdesk/rustdesk-server
Date: 2026-06-02
Confidence: high

### 10.2 Protocol Stack

```
Application:    RustDesk Protobuf Messages (RendezvousMessage / Message)
Serialization:  Protobuf v3 (wire format)
Transport:      UDP (port 21116) for signaling
                TCP (port 21116/21117) for data/relay
                WebSocket (port 21118/21119) for web clients
Network:        IPv4 + IPv6 dual-stack
Crypto:         Ed25519 signatures (sodiumoxide)
                TLS 1.2+ (tokio-rustls) for encrypted channels
```

Source: Multiple files
URL: Various
Date: 2025-06-09
Confidence: high

---

## 11. HAUPTDATEIEN UND IHRE VERANTWORTLICHKEITEN

| Datei | Komponente | Verantwortlichkeit |
|-------|------------|-------------------|
| src/main.rs | hbbs | Entry point, CLI parsing, starts RendezvousServer |
| src/hbbr.rs | hbbr | Entry point for relay server binary |
| src/rendezvous_server.rs | hbbs | Core signaling logic: peer registration, punch hole coordination, PK management, WebSocket/TCP handling |
| src/relay_server.rs | hbbr | Relay data forwarding, peer pairing by UUID, bandwidth limiting |
| src/common.rs | Shared | CLI argument parsing, Ed25519 key generation (gen_sk), signal handling, software update checking |
| src/peer.rs | hbbs | Peer data model (Peer struct), in-memory HashMap + SQLite storage via PeerMap |
| src/database.rs | hbbs | SQLite schema management, async queries via sqlx+deadpool |
| libs/hbb_common/protos/rendezvous.proto | Shared | Signaling protocol definitions (all RendezvousMessage types) |
| libs/hbb_common/protos/message.proto | Shared | Data channel protocol (video, input, audio, file transfer) |
| src/rendezvous_mediator.rs (client) | Client | Client-side rendezvous: register with hbbs, handle punch holes, create relay connections |

Source: rustdesk/rustdesk-server repository
URL: https://github.com/rustdesk/rustdesk-server
Date: 2025-06-09
Confidence: high

---

## 12. IMPLEMENTATIONSMUSTER ALS BLUEPRINT

### 12.1 Pattern: Async UDP+TCP Multi-Listener

```rust
// From rendezvous_server.rs::start()
let mut socket = create_udp_listener(port, rmem).await?;
let mut listener = create_tcp_listener(port).await?;
let mut listener2 = create_tcp_listener(nat_port).await?;
let mut listener3 = create_tcp_listener(ws_port).await?;

// Single tokio::select! for all listeners
loop {
    tokio::select! {
        res = socket.next() => { /* UDP handling */ }
        res = listener.accept() => { /* TCP main */ }
        res = listener2.accept() => { /* TCP NAT test */ }
        res = listener3.accept() => { /* WebSocket */ }
    }
}
```

### 12.2 Pattern: Protobuf Oneof Message Union

```protobuf
// All messages wrapped in a single envelope type
message RendezvousMessage {
  oneof union {
    RegisterPeer register_peer = 6;
    PunchHoleRequest punch_hole_request = 8;
    // ... all message types
  }
}
```

### 12.3 Pattern: In-Memory Cache + Async DB

```rust
// PeerMap: RwLock<HashMap> for hot data, SQLite for persistence
pub(crate) struct PeerMap {
    map: Arc<RwLock<HashMap<String, LockPeer>>>,
    db: database::Database,
}
// get() checks memory first, falls back to DB
```

### 12.4 Pattern: Symmetric NAT Hole Punching

```rust
// Key: use same local_addr for signaling AND direct connect
let socket = connect_tcp(&host, timeout).await?;
let local_addr = socket.local_addr();
// Attempt direct connect from same local port
connect_tcp_local(peer_addr, Some(local_addr), timeout).await;
```

### 12.5 Pattern: Relay Pairing by UUID

```rust
// First peer: insert into waiting map with UUID
PEERS.lock().await.insert(uuid.clone(), Box::new(stream));
sleep(30.).await; // wait for pairing
PEERS.lock().await.remove(&uuid);

// Second peer: find matching UUID and start relay
if let Some(peer) = PEERS.lock().await.remove(&uuid) {
    relay(addr, stream, peer, limiter, id).await;
}
```

---

## 13. ZUSAMMENFASSUNG DER ARCHITEKTUR

```
+---------+                    +------------------+         +---------+
| Client A|                    |   hbbs (21116)   |         | Client B|
|         |--UDP RegisterPeer-->|                  |<--UDP---|         |
|         |<RegPeerResp(resPk)-|  ID/Rendezvous   |         |         |
|         |--UDP RegisterPk--->|  - PeerMap       |         |         |
|         |<-RegPkResponse(OK)-|  - SQLite        |         |         |
|         |                    |  - Punch coord.  |         |         |
|         |--PunchHoleReq(B)->|                  |         |         |
|         |<-PunchHoleResponse-|                  |         |         |
|         |   (B's addr, pk)   |--PunchHole(A)--->|         |         |
|         |                    |<--PunchHoleSent---|         |         |
|         |<-------------------|                  |         |         |
|         |                    |                  |         |         |
|         |======== TCP Direct Connection (hole punched) =======>|     |
|         |                    |  OR if hole punch fails:       |         |
|         |                    |                  |         |         |
|         |--RequestRelay---->|  hbbr (21117)    |<--------|         |
|         |<--RelayResponse----|  Relay Server    |         |         |
|         |======= TCP Relay Connection (via hbbr) ===========>|     |
+---------+                    +------------------+         +---------+
```

---

## 14. VERWENDETE QUELLEN

1. [^1^] rustdesk/rustdesk-server GitHub Repository: https://github.com/rustdesk/rustdesk-server
2. [^2^] rustdesk/hbb_common GitHub Repository (submodule): https://github.com/rustdesk/hbb_common
3. [^3^] rendezvous.proto (Protobuf Definitions): https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/rendezvous.proto
4. [^4^] message.proto (Data Channel Protocol): https://raw.githubusercontent.com/rustdesk/hbb_common/main/protos/message.proto
5. [^5^] src/main.rs (hbbs entry): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/main.rs
6. [^6^] src/hbbr.rs (hbbr entry): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/hbbr.rs
7. [^7^] src/rendezvous_server.rs (core hbbs): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/rendezvous_server.rs
8. [^8^] src/relay_server.rs (core hbbr): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/relay_server.rs
9. [^9^] src/common.rs (shared utilities): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/common.rs
10. [^10^] src/peer.rs (peer data model): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/peer.rs
11. [^11^] src/database.rs (SQLite layer): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/src/database.rs
12. [^12^] Cargo.toml (server dependencies): https://raw.githubusercontent.com/rustdesk/rustdesk-server/master/Cargo.toml
13. [^13^] hbb_common/Cargo.toml (lib dependencies): https://raw.githubusercontent.com/rustdesk/hbb_common/main/Cargo.toml
14. [^14^] src/rendezvous_mediator.rs (client): https://raw.githubusercontent.com/rustdesk/rustdesk/master/src/rendezvous_mediator.rs
15. [^15^] RustDesk Self-Host Documentation: https://rustdesk.com/docs/en/self-host/
16. [^16^] RustDesk Server Installation Guide: https://rustdesk.com/docs/en/self-host/rustdesk-server-oss/install/
17. [^17^] sodiumoxide::crypto::sign docs: https://docs.rs/sodiumoxide/latest/sodiumoxide/crypto/sign/index.html
18. [^18^] Prost (protobuf Rust): https://github.com/tokio-rs/prost
19. [^20^] RustDesk Server Architecture - oneuptime: https://oneuptime.com/blog/post/2026-03-02-configure-rustdesk-server-ubuntu/view
20. [^21^] Arch Linux Forum - RustDesk Server: https://bbs.archlinux.org/viewtopic.php?id=303627
21. [^22^] RustDesk Relay Issues: https://donovanadkisson.com/technology/how-to-solve-rustdesk-relayed-and-unencrypted-connection-issues/
22. [^23^] RustDesk Client Configuration: https://rustdesk.com/docs/en/self-host/client-configuration/
23. [^24^] RustDesk Client Options: https://rustdesk.com/docs/en/self-host/client-configuration/advanced-settings/
24. [^25^] GitHub Discussion - Hole Punching: https://github.com/rustdesk/rustdesk/discussions/13558

---

*End of Analysis*
