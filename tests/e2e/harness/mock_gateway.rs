//! SUB-901-2.1 & 2.2: Локальный Mock Discord Gateway Daemon на базе Tokio
//! (HTTP/2 h2c + HTTP/1.1 Keep-Alive REST v9 + WebSocket Gateway v9).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use discord_core::rest::base64;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::HarnessError;
use crate::trace_loader::{CaptureReadyTrace, HydratedMessage};

const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const F_DATA: u8 = 0x0;
const F_HEADERS: u8 = 0x1;
const F_SETTINGS: u8 = 0x4;
const F_PING: u8 = 0x6;
const FL_ACK: u8 = 0x1;
const FL_END_S: u8 = 0x1;
const FL_END_H: u8 = 0x4;

type HeaderList = Vec<(String, String)>;
type RestResponse = (u16, HeaderList, Vec<u8>);
type PendingH2Req = (String, String, HeaderList);

fn io_err(ctx: &str, err: impl std::fmt::Display) -> HarnessError {
    HarnessError::Io(format!("{ctx}: {err}"))
}

async fn read_exact_io(s: &mut TcpStream, b: &mut [u8], c: &str) -> Result<(), HarnessError> {
    s.read_exact(b).await.map_err(|e| io_err(c, e))?;
    Ok(())
}

async fn write_all_io(s: &mut TcpStream, buf: &[u8], ctx: &str) -> Result<(), HarnessError> {
    s.write_all(buf).await.map_err(|e| io_err(ctx, e))
}

async fn connect_tcp(addr: SocketAddr) -> Result<TcpStream, HarnessError> {
    TcpStream::connect(addr)
        .await
        .map_err(|e| io_err("connect", e))
}

#[derive(Debug, Clone, Default)]
pub struct MockGatewayConfig {
    pub require_auth: bool,
    pub expected_token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Http2ClientResponse {
    pub status: u16,
    pub stream_id: u32,
    pub headers: HeaderList,
    pub body: Vec<u8>,
    pub settings_acked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonMetrics {
    pub http2_requests: usize,
    pub http1_requests: usize,
    pub ws_connections: usize,
    pub identifies: usize,
    pub heartbeats_acked: usize,
    pub voice_state_updates: usize,
    pub resumes: usize,
}

struct DaemonState {
    trace: CaptureReadyTrace,
    config: MockGatewayConfig,
    messages: Mutex<HashMap<String, Vec<HydratedMessage>>>,
    injected_status: Mutex<HashMap<String, (u16, Option<u64>)>>,
    metrics: Mutex<DaemonMetrics>,
    next_msg_id: AtomicU64,
}

impl DaemonState {
    fn new(trace: CaptureReadyTrace, config: MockGatewayConfig) -> Arc<Self> {
        let initial_msgs = trace.messages_by_channel.clone();
        Arc::new(Self {
            trace,
            config,
            messages: Mutex::new(initial_msgs),
            injected_status: Mutex::new(HashMap::new()),
            metrics: Mutex::new(DaemonMetrics::default()),
            next_msg_id: AtomicU64::new(1_300_000_000_000_000_000),
        })
    }

    fn bump(&self, f: impl FnOnce(&mut DaemonMetrics)) {
        if let Ok(mut m) = self.metrics.lock() {
            f(&mut m);
        }
    }
}

fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDCu32),
                _ => (b ^ c ^ d, 0xCA62_C1D6u32),
            };
            let t = [a.rotate_left(5), f, e, k, wi]
                .into_iter()
                .fold(0u32, u32::wrapping_add);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (slot, add) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(add);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn websocket_accept_key(client_key: &str) -> String {
    let mut input = client_key.trim().as_bytes().to_vec();
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::encode(&sha1_digest(&input))
}

fn json_resp(status: u16, body: Vec<u8>) -> RestResponse {
    let ct = ("content-type".into(), "application/json".into());
    (status, vec![ct], body)
}

fn json_ok<T: serde::Serialize>(val: &T) -> RestResponse {
    json_resp(200, serde_json::to_vec(val).unwrap_or_default())
}

fn check_auth_header(config: &MockGatewayConfig, headers: &[(String, String)]) -> bool {
    if !config.require_auth {
        return true;
    }
    let Some(expected) = config.expected_token.as_deref() else {
        return true;
    };
    headers.iter().any(|(k, v)| {
        let tok = v
            .trim()
            .trim_start_matches("Bearer ")
            .trim_start_matches("Bot ");
        k.eq_ignore_ascii_case("authorization") && !tok.is_empty() && tok == expected
    })
}

fn handle_post_channel_message(state: &DaemonState, channel_id: &str, body: &[u8]) -> RestResponse {
    let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) else {
        return json_resp(400, br#"{"code":50109,"message":"Invalid JSON"}"#.to_vec());
    };
    let content = val["content"].as_str().unwrap_or("");
    if !(1..=2000).contains(&content.chars().count()) {
        return json_resp(
            400,
            br#"{"code":50035,"message":"Invalid Form Body"}"#.to_vec(),
        );
    }
    let id = state
        .next_msg_id
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    let msg = HydratedMessage {
        id,
        channel_id: channel_id.to_string(),
        content: content.to_string(),
        timestamp: "2026-11-11T15:00:00.000000+00:00".to_string(),
        author: state.trace.current_user.clone(),
    };
    let resp = json_ok(&msg);
    if let Ok(mut guard) = state.messages.lock() {
        guard.entry(channel_id.to_string()).or_default().push(msg);
    }
    resp
}

fn check_injected_status(state: &DaemonState, path_only: &str) -> Option<RestResponse> {
    let (status, retry_ms) = state.injected_status.lock().ok()?.remove(path_only)?;
    let secs = (retry_ms.unwrap_or(250) as f64) / 1000.0;
    let body = format!(r#"{{"message":"Rate limited","retry_after":{secs:.3}}}"#).into_bytes();
    let ct = ("content-type".into(), "application/json".into());
    let ra = ("retry-after".into(), format!("{secs:.3}"));
    Some((status, vec![ct, ra], body))
}

fn strip_wrap<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    s.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn route_rest_method(state: &DaemonState, method: &str, route: &str, body: &[u8]) -> RestResponse {
    if let Some(gid) = strip_wrap(route, "/guilds/", "/channels") {
        return match state.trace.guilds.iter().find(|g| g.id == gid) {
            Some(g) if method == "GET" => json_ok(&g.channels),
            _ => json_resp(404, br#"{"code":10004,"message":"Unknown Guild"}"#.to_vec()),
        };
    }
    if let Some(cid) = strip_wrap(route, "/channels/", "/messages") {
        if method == "POST" {
            return handle_post_channel_message(state, cid, body);
        } else if method == "GET" {
            let msgs = state.messages.lock().ok().and_then(|g| g.get(cid).cloned());
            return json_ok(&msgs.unwrap_or_default());
        }
    }
    match (method, route) {
        ("GET", "/users/@me") => json_ok(&state.trace.current_user),
        ("GET", "/guilds" | "/users/@me/guilds") => json_ok(&state.trace.guilds),
        _ => json_resp(404, br#"{"code":10003,"message":"Unknown Route"}"#.to_vec()),
    }
}

fn dispatch_rest_request(
    state: &DaemonState,
    method: &str,
    raw_target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> RestResponse {
    let path_only = raw_target.split('?').next().unwrap_or(raw_target);
    if let Some(injected) = check_injected_status(state, path_only) {
        return injected;
    }
    if !check_auth_header(&state.config, headers) {
        return json_resp(401, br#"{"message":"401: Unauthorized","code":0}"#.to_vec());
    }
    let route = path_only.strip_prefix("/api/v9").unwrap_or(path_only);
    route_rest_method(state, method, route, body)
}

fn encode_h2_frame(ftype: u8, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let [_, b0, b1, b2] = (payload.len() as u32).to_be_bytes();
    let mut out = Vec::with_capacity(9 + payload.len());
    out.extend_from_slice(&[b0, b1, b2, ftype, flags]);
    out.extend_from_slice(&(stream_id & 0x7FFF_FFFF).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

async fn read_h2_frame(s: &mut TcpStream) -> Result<Option<(u8, u8, u32, Vec<u8>)>, HarnessError> {
    let mut hdr = [0u8; 9];
    match s.read_exact(&mut hdr).await {
        Ok(_) => {}
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(io_err("read h2 hdr", e)),
    }
    let len = ((hdr[0] as usize) << 16) | ((hdr[1] as usize) << 8) | (hdr[2] as usize);
    let sid = u32::from_be_bytes([hdr[5], hdr[6], hdr[7], hdr[8]]) & 0x7FFF_FFFF;
    let mut payload = vec![0u8; len];
    if len > 0 {
        read_exact_io(s, &mut payload, "read h2 payload").await?;
    }
    Ok(Some((hdr[3], hdr[4], sid, payload)))
}

fn split_h2_header_line(line: &str) -> Option<(&str, &str)> {
    if let Some(rest) = line.strip_prefix(':') {
        let idx = rest.find(':')?;
        Some((&line[..idx + 1], &rest[idx + 1..]))
    } else {
        line.split_once(':')
    }
}

fn parse_h2_headers_block(payload: &[u8]) -> PendingH2Req {
    let text = String::from_utf8_lossy(payload);
    let (mut method, mut path, mut hdrs) = ("GET".to_string(), "/".to_string(), Vec::new());
    for line in text.lines() {
        if let Some((k, v)) = split_h2_header_line(line) {
            match k.trim() {
                ":method" => method = v.trim().to_string(),
                ":path" => path = v.trim().to_string(),
                key => hdrs.push((key.to_string(), v.trim().to_string())),
            }
        }
    }
    (method, path, hdrs)
}

async fn serve_h2_connection(
    mut s: TcpStream,
    state: Arc<DaemonState>,
) -> Result<(), HarnessError> {
    let mut init = encode_h2_frame(F_SETTINGS, 0, 0, &[]);
    init.extend_from_slice(&encode_h2_frame(F_SETTINGS, FL_ACK, 0, &[]));
    write_all_io(&mut s, &init, "write h2 settings").await?;

    let mut pending: HashMap<u32, PendingH2Req> = HashMap::new();
    while let Some((ftype, flags, sid, payload)) = read_h2_frame(&mut s).await? {
        match ftype {
            F_SETTINGS if (flags & FL_ACK) == 0 => {
                let _ = s
                    .write_all(&encode_h2_frame(F_SETTINGS, FL_ACK, 0, &[]))
                    .await;
            }
            F_PING if (flags & FL_ACK) == 0 => {
                let _ = s
                    .write_all(&encode_h2_frame(F_PING, FL_ACK, 0, &payload))
                    .await;
            }
            F_HEADERS => {
                let (m, p, h) = parse_h2_headers_block(&payload);
                if (flags & FL_END_S) != 0 {
                    state.bump(|x| x.http2_requests += 1);
                    let res = dispatch_rest_request(&state, &m, &p, &h, &[]);
                    write_h2_response(&mut s, sid, res).await?;
                } else {
                    pending.insert(sid, (m, p, h));
                }
            }
            F_DATA => {
                if let Some((m, p, h)) = pending.remove(&sid) {
                    state.bump(|x| x.http2_requests += 1);
                    let res = dispatch_rest_request(&state, &m, &p, &h, &payload);
                    write_h2_response(&mut s, sid, res).await?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn write_h2_response(
    s: &mut TcpStream,
    sid: u32,
    (status, headers, body): RestResponse,
) -> Result<(), HarnessError> {
    let mut hdr_block = format!(":status:{status}\n");
    for (k, v) in headers {
        hdr_block.push_str(&format!("{}:{}\n", k.to_ascii_lowercase(), v));
    }
    let mut out = encode_h2_frame(F_HEADERS, FL_END_H, sid, hdr_block.as_bytes());
    out.extend_from_slice(&encode_h2_frame(F_DATA, FL_END_S, sid, &body));
    write_all_io(s, &out, "write h2 resp").await
}

async fn read_ws_frame(s: &mut TcpStream) -> Result<Option<(u8, Vec<u8>)>, HarnessError> {
    let mut hdr = [0u8; 2];
    match s.read_exact(&mut hdr).await {
        Ok(_) => {}
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(io_err("ws hdr", e)),
    }
    let opcode = hdr[0] & 0x0F;
    let masked = (hdr[1] & 0x80) != 0;
    let mut len = (hdr[1] & 0x7F) as usize;
    if len == 126 {
        let mut ext = [0u8; 2];
        read_exact_io(s, &mut ext, "ws len16").await?;
        len = u16::from_be_bytes(ext) as usize;
    }
    let mut mask = [0u8; 4];
    if masked {
        read_exact_io(s, &mut mask, "ws mask").await?;
    }
    let mut payload = vec![0u8; len];
    if len > 0 {
        read_exact_io(s, &mut payload, "ws payload").await?;
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i & 3];
            }
        }
    }
    Ok(Some((opcode, payload)))
}

fn encode_ws_frame(opcode: u8, masked: bool, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + payload.len());
    out.push(0x80 | (opcode & 0x0F));
    let mask_bit = if masked { 0x80 } else { 0x00 };
    if payload.len() < 126 {
        out.push(mask_bit | (payload.len() as u8));
    } else if payload.len() <= u16::MAX as usize {
        out.push(mask_bit | 126);
        out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        out.push(mask_bit | 127);
        out.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    if masked {
        let key = [0x37u8, 0xFA, 0x21, 0x3D];
        out.extend_from_slice(&key);
        out.extend(payload.iter().enumerate().map(|(i, &b)| b ^ key[i & 3]));
    } else {
        out.extend_from_slice(payload);
    }
    out
}

async fn send_ws_json(
    s: &mut TcpStream,
    masked: bool,
    val: &serde_json::Value,
) -> Result<(), HarnessError> {
    let bytes = serde_json::to_vec(val).map_err(|e| HarnessError::Protocol(e.to_string()))?;
    write_all_io(s, &encode_ws_frame(0x1, masked, &bytes), "send_ws_json").await
}

fn build_ready_events(state: &DaemonState, seq: &mut u64) -> [serde_json::Value; 2] {
    *seq = 1;
    let t = &state.trace;
    let r = serde_json::json!({"op": 0, "t": "READY", "s": 1, "d": {"v": 9, "session_id": t.session_id, "user": t.current_user, "guilds": t.guilds, "private_channels": [], "relationships": []}});
    *seq = 2;
    let s = serde_json::json!({"op": 0, "t": "READY_SUPPLEMENTAL", "s": 2, "d": {"disclose": ["pomelo"], "guilds": []}});
    [r, s]
}

fn build_voice_events(
    state: &DaemonState,
    seq: &mut u64,
    d: &serde_json::Value,
    addr: SocketAddr,
) -> [serde_json::Value; 2] {
    let b = |k: &str| d.get(k).and_then(serde_json::Value::as_bool) == Some(true);
    let (uid, sid) = (&state.trace.current_user.id, &state.trace.session_id);
    let (gid, cid) = (d.get("guild_id"), d.get("channel_id"));
    *seq += 1;
    let vsu = serde_json::json!({"op": 0, "t": "VOICE_STATE_UPDATE", "s": *seq, "d": {"guild_id": gid, "channel_id": cid, "user_id": uid, "session_id": sid, "self_mute": b("self_mute"), "self_deaf": b("self_deaf"), "self_video": b("self_video")}});
    *seq += 1;
    let vsrv = serde_json::json!({"op": 0, "t": "VOICE_SERVER_UPDATE", "s": *seq, "d": {"token": "mock-voice-token-e2e", "guild_id": gid, "endpoint": addr.to_string()}});
    [vsu, vsrv]
}

async fn handle_ws_opcode(
    s: &mut TcpStream,
    state: &DaemonState,
    seq: &mut u64,
    msg: &serde_json::Value,
    addr: SocketAddr,
) -> Result<(), HarnessError> {
    let op = msg
        .get("op")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(999);
    match op {
        1 | 40 => {
            state.bump(|x| x.heartbeats_acked += 1);
            let ack = serde_json::json!({"op": 11, "d": null, "s": null, "t": null});
            send_ws_json(s, false, &ack).await?;
        }
        2 => {
            state.bump(|x| x.identifies += 1);
            for ev in build_ready_events(state, seq) {
                send_ws_json(s, false, &ev).await?;
            }
        }
        4 => {
            state.bump(|x| x.voice_state_updates += 1);
            let d = msg.get("d").unwrap_or(&serde_json::Value::Null);
            for ev in build_voice_events(state, seq, d, addr) {
                send_ws_json(s, false, &ev).await?;
            }
        }
        6 => {
            state.bump(|x| x.resumes += 1);
            *seq += 1;
            let res = serde_json::json!({"op": 0, "t": "RESUMED", "s": *seq, "d": {"_trace": ["[\"gateway-mock-e2e\",{\"micros\":0.0}]"]}});
            send_ws_json(s, false, &res).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn serve_ws_connection(
    mut s: TcpStream,
    state: Arc<DaemonState>,
    ws_key: &str,
    local_addr: SocketAddr,
) -> Result<(), HarnessError> {
    state.bump(|x| x.ws_connections += 1);
    let hs = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        websocket_accept_key(ws_key)
    );
    write_all_io(&mut s, hs.as_bytes(), "ws handshake").await?;
    let hb = state.trace.heartbeat_interval_ms;
    let hello = serde_json::json!({"op": 10, "t": null, "s": null, "d": {"heartbeat_interval": hb, "_trace": ["[\"gateway-prd-mock-e2e\",{\"micros\":0.0}]"]}});
    send_ws_json(&mut s, false, &hello).await?;

    let mut seq = 0u64;
    while let Some((opcode, payload)) = read_ws_frame(&mut s).await? {
        match opcode {
            0x1 | 0x2 => {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&payload) {
                    handle_ws_opcode(&mut s, &state, &mut seq, &val, local_addr).await?;
                }
            }
            0x8 => {
                let _ = s.write_all(&encode_ws_frame(0x8, false, &[])).await;
                break;
            }
            0x9 => {
                let _ = s.write_all(&encode_ws_frame(0xA, false, &payload)).await;
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_http1_head(req_str: &str) -> (String, String, HeaderList, Option<String>) {
    let mut lines = req_str.lines();
    let mut parts = lines.next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (mut headers, mut ws_key, mut is_ws) = (Vec::new(), None, false);
    for line in lines.take_while(|l| !l.is_empty()) {
        if let Some((k, v)) = line.split_once(':') {
            let (key, val) = (k.trim().to_string(), v.trim().to_string());
            if key.eq_ignore_ascii_case("upgrade") && val.eq_ignore_ascii_case("websocket") {
                is_ws = true;
            }
            if key.eq_ignore_ascii_case("sec-websocket-key") {
                ws_key = Some(val.clone());
            }
            headers.push((key, val));
        }
    }
    (method, target, headers, if is_ws { ws_key } else { None })
}

async fn write_http1_response(
    s: &mut TcpStream,
    (st, hdrs, b): RestResponse,
) -> Result<(), HarnessError> {
    let mut resp = format!(
        "HTTP/1.1 {st} OK\r\nConnection: keep-alive\r\nKeep-Alive: timeout=90, max=100\r\nContent-Length: {}\r\n",
        b.len()
    );
    for (k, v) in hdrs {
        resp.push_str(&format!("{k}: {v}\r\n"));
    }
    resp.push_str("\r\n");
    write_all_io(s, resp.as_bytes(), "http1 write").await?;
    write_all_io(s, &b, "http1 body").await
}

async fn handle_tcp_connection(
    mut s: TcpStream,
    state: Arc<DaemonState>,
    local_addr: SocketAddr,
) -> Result<(), HarnessError> {
    let mut preface = [0u8; 24];
    let peek_n = s.peek(&mut preface).await.map_err(|e| io_err("peek", e))?;
    if peek_n == 0 {
        return Ok(());
    }
    if peek_n >= H2_PREFACE.len() && &preface == H2_PREFACE {
        read_exact_io(&mut s, &mut preface, "read h2 preface").await?;
        return serve_h2_connection(s, state).await;
    }

    let mut buf = vec![0u8; 4096];
    let n = s.read(&mut buf).await.map_err(|e| io_err("tcp read", e))?;
    if n == 0 {
        return Ok(());
    }
    let (method, target, headers, ws_key) = parse_http1_head(&String::from_utf8_lossy(&buf[..n]));
    if let Some(key) = ws_key {
        return serve_ws_connection(s, state, &key, local_addr).await;
    }
    state.bump(|x| x.http1_requests += 1);
    let sep = buf[..n].windows(4).position(|w| w == b"\r\n\r\n");
    let resp = dispatch_rest_request(
        &state,
        &method,
        &target,
        &headers,
        &buf[sep.map_or(n, |p| p + 4)..n],
    );
    write_http1_response(&mut s, resp).await
}

async fn read_h2_client_response(
    s: &mut TcpStream,
    stream_id: u32,
) -> Result<Http2ClientResponse, HarnessError> {
    let mut res = Http2ClientResponse {
        status: 200,
        stream_id,
        ..Default::default()
    };
    while let Some((ftype, flags, sid, payload)) = read_h2_frame(s).await? {
        if ftype == F_SETTINGS && (flags & FL_ACK) != 0 {
            res.settings_acked = true;
        } else if ftype == F_HEADERS && sid == stream_id {
            for line in String::from_utf8_lossy(&payload).lines() {
                if let Some((k, v)) = split_h2_header_line(line) {
                    if k == ":status" {
                        res.status = v.parse::<u16>().unwrap_or(500);
                    } else {
                        res.headers.push((k.to_string(), v.to_string()));
                    }
                }
            }
        } else if ftype == F_DATA && sid == stream_id {
            res.body.extend_from_slice(&payload);
            if (flags & FL_END_S) != 0 {
                break;
            }
        }
    }
    Ok(res)
}

pub struct MockWsClient {
    stream: TcpStream,
}

impl MockWsClient {
    pub async fn recv_json(&mut self) -> Result<serde_json::Value, HarnessError> {
        while let Some((opcode, payload)) = read_ws_frame(&mut self.stream).await? {
            if opcode == 0x1 || opcode == 0x2 {
                return serde_json::from_slice(&payload)
                    .map_err(|e| HarnessError::Protocol(format!("WS JSON: {e}")));
            }
        }
        Err(HarnessError::Protocol("WS EOF".into()))
    }

    pub async fn send_json(&mut self, payload: &serde_json::Value) -> Result<(), HarnessError> {
        send_ws_json(&mut self.stream, true, payload).await
    }

    pub async fn close(&mut self) -> Result<(), HarnessError> {
        let _ = write_all_io(&mut self.stream, &encode_ws_frame(0x8, true, &[]), "close").await;
        let _ = self.stream.shutdown().await;
        Ok(())
    }
}

pub struct MockGatewayDaemon {
    addr: SocketAddr,
    inode: Option<String>,
    state: Arc<DaemonState>,
    tx: Option<watch::Sender<bool>>,
    task: Option<JoinHandle<()>>,
}

impl MockGatewayDaemon {
    pub async fn start(trace: CaptureReadyTrace) -> Result<Self, HarnessError> {
        Self::start_with_config(trace, MockGatewayConfig::default()).await
    }

    pub async fn start_with_config(
        trace: CaptureReadyTrace,
        config: MockGatewayConfig,
    ) -> Result<Self, HarnessError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| io_err("bind", e))?;
        let inode = std::fs::read_link(format!("/proc/self/fd/{}", listener.as_raw_fd()))
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        let addr = listener.local_addr().map_err(|e| io_err("local_addr", e))?;
        let state = DaemonState::new(trace, config);
        let (tx, mut rx) = watch::channel(false);
        let loop_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rx.changed() => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, _)) = accepted {
                            let st = Arc::clone(&loop_state);
                            tokio::spawn(async move { let _ = handle_tcp_connection(stream, st, addr).await; });
                        }
                    }
                }
            }
        });
        let (tx, task) = (Some(tx), Some(task));
        Ok(Self {
            addr,
            inode,
            state,
            tx,
            task,
        })
    }

    #[must_use]
    pub fn listener_inode(&self) -> Option<&str> {
        self.inode.as_deref()
    }

    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    #[must_use]
    pub fn http_base_url(&self) -> String {
        format!("http://{}/api/v9", self.addr)
    }

    #[must_use]
    pub fn ws_url(&self) -> String {
        format!("ws://{}/?v=9&encoding=json", self.addr)
    }

    pub fn inject_http_status(&self, route_prefix: &str, status: u16, retry_after_ms: Option<u64>) {
        if let Ok(mut guard) = self.state.injected_status.lock() {
            guard.insert(route_prefix.to_string(), (status, retry_after_ms));
        }
    }

    pub async fn send_http2_request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<Http2ClientResponse, HarnessError> {
        let mut s = connect_tcp(self.addr).await?;
        let mut hdr_text = format!(":method:{method}\n:path:{path}\n");
        for (k, v) in headers {
            hdr_text.push_str(&format!("{k}:{v}\n"));
        }
        let h_flags = FL_END_H | if body.is_some() { 0 } else { FL_END_S };
        let mut wire = H2_PREFACE.to_vec();
        wire.extend(encode_h2_frame(F_SETTINGS, 0, 0, &[]));
        wire.extend(encode_h2_frame(F_HEADERS, h_flags, 1, hdr_text.as_bytes()));
        if let Some(bytes) = body {
            wire.extend(encode_h2_frame(F_DATA, FL_END_S, 1, bytes));
        }
        write_all_io(&mut s, &wire, "h2 write").await?;
        let resp = read_h2_client_response(&mut s, 1).await?;
        let _ = s.shutdown().await;
        Ok(resp)
    }

    pub async fn connect_ws_client(&self) -> Result<MockWsClient, HarnessError> {
        let mut s = connect_tcp(self.addr).await?;
        let req = format!(
            "GET /?v=9&encoding=json HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            self.addr
        );
        write_all_io(&mut s, req.as_bytes(), "ws upgrade write").await?;
        let (mut hdr_buf, mut byte) = (Vec::new(), [0u8; 1]);
        while !hdr_buf.ends_with(b"\r\n\r\n") {
            if s.read(&mut byte).await.map_err(|e| io_err("ws read", e))? == 0 {
                return Err(HarnessError::Protocol("WS handshake EOF".into()));
            }
            hdr_buf.push(byte[0]);
        }
        Ok(MockWsClient { stream: s })
    }

    #[must_use]
    pub fn metrics(&self) -> DaemonMetrics {
        let Ok(g) = self.state.metrics.lock() else {
            return DaemonMetrics::default();
        };
        g.clone()
    }

    pub async fn shutdown(&mut self) -> Result<(), HarnessError> {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(true);
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        Ok(())
    }
}
