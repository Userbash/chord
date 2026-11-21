//! SUB-901-2.3: Загрузчик эталонных трасс `/home/sanya/re-deep/capture-ready.jsonl`
//! для 100% реалистичной гидратации состояния Mock Gateway Daemon.
//!
//! OPT-3: Потоковое построчное чтение вместо fs::read_to_string(2.4 MB).
//! Останавливаемся сразу после получения 8 wsrecv-кадров — остаток файла не читается.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::OnceLock;
use chord_codec::etf::{self, EtfValue};
use chord_codec::zstd::ZstdStream;
use chord_core::ready::Ready;
use chord_core::rest::base64;

use crate::HarnessError;

/// Путь по умолчанию к эталонному дампу RE.
pub const DEFAULT_CAPTURE_READY_PATH: &str = "/home/sanya/re-deep/capture-ready.jsonl";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydratedUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub discriminator: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydratedChannel {
    pub id: String,
    pub guild_id: String,
    pub name: String,
    pub kind: u8,
    pub position: i64,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydratedGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: Option<String>,
    pub channels: Vec<HydratedChannel>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HydratedMessage {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    pub timestamp: String,
    pub author: HydratedUser,
}

#[derive(Debug, Clone)]
pub struct CaptureReadyTrace {
    pub gateway_request_id: String,
    pub gateway_url: String,
    pub attached_guild_id: String,
    pub attached_channel_id: String,
    pub heartbeat_interval_ms: u64,
    pub session_id: String,
    pub current_user: HydratedUser,
    pub guilds: Vec<HydratedGuild>,
    pub messages_by_channel: HashMap<String, Vec<HydratedMessage>>,
    pub raw_wsrecv_frames: Vec<Vec<u8>>,
    pub hello_etf_frame: Vec<u8>,
    pub ready_etf_frame: Vec<u8>,
}

struct RawCaptureScan {
    gateway_request_id: String,
    gateway_url: String,
    attached_guild_id: String,
    attached_channel_id: String,
    raw_wsrecv_frames: Vec<Vec<u8>>,
}

fn parse_attached_url(url: &str) -> Option<(String, String)> {
    let marker = "/channels/";
    let idx = url.find(marker)?;
    let rest = &url[idx + marker.len()..];
    let mut parts = rest.split('/');
    let guild_id = parts.next()?.trim();
    let channel_id = parts.next()?.trim();
    if guild_id.is_empty() || channel_id.is_empty() {
        return None;
    }
    Some((guild_id.to_string(), channel_id.to_string()))
}

fn process_capture_event(
    val: &serde_json::Value,
    scan: &mut RawCaptureScan,
) -> Result<(), HarnessError> {
    let str_field = |k: &str| val.get(k).and_then(serde_json::Value::as_str).unwrap_or("");
    match str_field("ev") {
        "attached" => {
            if let Some(url) = val
                .pointer("/target/url")
                .and_then(serde_json::Value::as_str)
                && let Some((gid, cid)) = parse_attached_url(url)
            {
                (scan.attached_guild_id, scan.attached_channel_id) = (gid, cid);
            }
        }
        "wsc" if str_field("url").contains("gateway.discord.gg") => {
            scan.gateway_url = str_field("url").to_string();
            scan.gateway_request_id = str_field("requestId").to_string();
        }
        "wsrecv"
            if !scan.gateway_request_id.is_empty()
                && str_field("requestId") == scan.gateway_request_id
                && scan.raw_wsrecv_frames.len() < 8 =>
        {
            let b64 = str_field("payload");
            if !b64.is_empty() {
                let bytes = base64::decode(b64)
                    .map_err(|e| HarnessError::Protocol(format!("base64 wsrecv: {e:?}")))?;
                scan.raw_wsrecv_frames.push(bytes);
            }
        }
        _ => {}
    }
    Ok(())
}

fn scan_capture_lines(content: &str) -> Result<RawCaptureScan, HarnessError> {
    let mut scan = RawCaptureScan {
        gateway_request_id: String::new(),
        gateway_url: String::new(),
        attached_guild_id: String::new(),
        attached_channel_id: String::new(),
        raw_wsrecv_frames: Vec::new(),
    };

    for line in content.lines() {
        if !(line.contains("\"ev\":\"attached\"")
            || line.contains("\"ev\":\"wsc\"")
            || line.contains("\"ev\":\"wsrecv\""))
        {
            continue;
        }
        let val: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| HarnessError::Protocol(format!("JSONL parse error: {e}")))?;
        process_capture_event(&val, &mut scan)?;
    }

    if scan.gateway_request_id.is_empty() || scan.raw_wsrecv_frames.len() < 2 {
        return Err(HarnessError::Protocol(
            "В трассе не найдена сессия Gateway (wsc + минимум 2 кадра wsrecv)".into(),
        ));
    }
    Ok(scan)
}

/// OPT-3: Потоковое построчное сканирование (BufReader, 64 KB буфер).
/// Читаем только строки с нужными событиями, останавливаемся после 8 wsrecv-кадров.
/// Экономия: вместо загрузки 2.4 MB в RAM — читаем ≤ первые ~50-100 KB файла.
fn scan_capture_stream(path: &Path) -> Result<RawCaptureScan, HarnessError> {
    let file = File::open(path)
        .map_err(|e| HarnessError::Io(format!("open {}: {e}", path.display())))?;
    let reader = BufReader::with_capacity(64 * 1024, file);

    let mut scan = RawCaptureScan {
        gateway_request_id: String::new(),
        gateway_url: String::new(),
        attached_guild_id: String::new(),
        attached_channel_id: String::new(),
        raw_wsrecv_frames: Vec::new(),
    };

    for line_result in reader.lines() {
        let line = line_result
            .map_err(|e| HarnessError::Io(format!("readline {}: {e}", path.display())))?;
        // Быстрый строковый фильтр без аллокации JSON
        if !(line.contains("\"ev\":\"attached\"")
            || line.contains("\"ev\":\"wsc\"")
            || line.contains("\"ev\":\"wsrecv\""))
        {
            continue;
        }
        let val: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| HarnessError::Protocol(format!("JSONL parse: {e}")))?;
        process_capture_event(&val, &mut scan)?;

        // Ранний выход: достаточно 8 кадров для HELLO + READY
        if scan.raw_wsrecv_frames.len() >= 8
            && !scan.gateway_request_id.is_empty()
            && !scan.attached_guild_id.is_empty()
        {
            break;
        }
    }

    if scan.gateway_request_id.is_empty() || scan.raw_wsrecv_frames.len() < 2 {
        return Err(HarnessError::Protocol(
            "В трассе не найдена сессия Gateway (wsc + минимум 2 кадра wsrecv)".into(),
        ));
    }
    Ok(scan)
}

fn decode_hello_and_ready_terms(
    frames: &[Vec<u8>],
) -> Result<(EtfValue, Vec<u8>, EtfValue, Vec<u8>), HarnessError> {
    let mut zstd = ZstdStream::new();
    let mut decompressed = Vec::new();
    let mut terms: Vec<(EtfValue, Vec<u8>)> = Vec::new();

    for frame in frames.iter().take(3) {
        let chunk = zstd
            .push(frame)
            .map_err(|e| HarnessError::Protocol(format!("zstd push: {e}")))?;
        decompressed.extend_from_slice(&chunk);
        loop {
            if decompressed.is_empty() {
                break;
            }
            match etf::decode(&decompressed) {
                Ok((val, consumed)) => {
                    let raw = decompressed[..consumed].to_vec();
                    decompressed.drain(..consumed);
                    terms.push((val, raw));
                }
                Err(etf::EtfError::UnexpectedEof { .. }) => break,
                Err(e) => return Err(HarnessError::Protocol(format!("etf decode: {e}"))),
            }
        }
        if terms.len() >= 2 {
            break;
        }
    }

    if terms.len() < 2 {
        return Err(HarnessError::Protocol(
            "Не удалось декодировать HELLO и READY из zstd-потока".into(),
        ));
    }
    let mut iter = terms.into_iter();
    let (hello_val, hello_raw) = iter
        .next()
        .ok_or_else(|| HarnessError::Protocol("missing HELLO".into()))?;
    let (ready_val, ready_raw) = iter
        .next()
        .ok_or_else(|| HarnessError::Protocol("missing READY".into()))?;
    Ok((hello_val, hello_raw, ready_val, ready_raw))
}

fn convert_ready_guilds(ready: &Ready) -> Vec<HydratedGuild> {
    ready
        .guilds
        .iter()
        .map(|g| {
            let gid = g.id.to_string();
            let channels = g
                .channels
                .iter()
                .map(|c| HydratedChannel {
                    id: c.id.to_string(),
                    guild_id: gid.clone(),
                    name: c.name.clone().unwrap_or_else(|| "channel".to_string()),
                    kind: u8::try_from(c.channel_type).unwrap_or(0),
                    position: c.position,
                    topic: c.topic.clone(),
                })
                .collect();
            HydratedGuild {
                id: gid,
                name: g.name.clone().unwrap_or_else(|| "Guild".to_string()),
                icon: g.icon.clone(),
                owner_id: g.owner_id.map(|id| id.to_string()),
                channels,
            }
        })
        .collect()
}

fn build_seed_messages(
    attached_channel_id: &str,
    user: &HydratedUser,
    guilds: &[HydratedGuild],
) -> HashMap<String, Vec<HydratedMessage>> {
    let mut map = HashMap::new();
    let target_cid = if attached_channel_id.is_empty() {
        "1163408591315812392"
    } else {
        attached_channel_id
    };
    map.insert(
        target_cid.to_string(),
        vec![
            HydratedMessage {
                id: "1290000000000000001".to_string(),
                channel_id: target_cid.to_string(),
                content: "Добро пожаловать в #📃┃новости | ITSKIDON PC BOOST".to_string(),
                timestamp: "2026-10-19T11:28:00.000000+00:00".to_string(),
                author: user.clone(),
            },
            HydratedMessage {
                id: "1290000000000000002".to_string(),
                channel_id: target_cid.to_string(),
                content: "Тестовая гидратация из capture-ready.jsonl активна".to_string(),
                timestamp: "2026-10-19T11:28:05.000000+00:00".to_string(),
                author: user.clone(),
            },
        ],
    );
    for guild in guilds {
        if let Some(first_ch) = guild.channels.first() {
            map.entry(first_ch.id.clone()).or_insert_with(|| {
                vec![HydratedMessage {
                    id: format!(
                        "129000000000{}",
                        guild.id.chars().take(6).collect::<String>()
                    ),
                    channel_id: first_ch.id.clone(),
                    content: format!("Канал {} гильдии {}", first_ch.name, guild.name),
                    timestamp: "2026-10-19T11:28:10.000000+00:00".to_string(),
                    author: user.clone(),
                }]
            });
        }
    }
    map
}

fn convert_ready_user(ready: &Ready) -> HydratedUser {
    HydratedUser {
        id: ready.user.id.to_string(),
        username: ready
            .user
            .username
            .clone()
            .unwrap_or_else(|| "User".to_string()),
        global_name: ready.user.global_name.clone(),
        discriminator: ready
            .user
            .discriminator
            .clone()
            .unwrap_or_else(|| "0".to_string()),
        avatar: ready.user.avatar.clone(),
    }
}

impl CaptureReadyTrace {
    /// Загружает и кэширует эталонный дамп `/home/sanya/re-deep/capture-ready.jsonl`.
    pub fn load_default() -> Result<Self, HarnessError> {
        static CACHED: OnceLock<CaptureReadyTrace> = OnceLock::new();
        if let Some(trace) = CACHED.get() {
            return Ok(trace.clone());
        }
        let loaded = Self::load_from_path(Path::new(DEFAULT_CAPTURE_READY_PATH))?;
        let _ = CACHED.set(loaded.clone());
        Ok(loaded)
    }

    /// Загружает трассу из указанного JSONL-файла захвата.
    /// OPT-3: Использует потоковое чтение scan_capture_stream (BufReader),
    /// не загружает весь файл в память. scan_capture_lines оставлен для unit-тестов.
    pub fn load_from_path(path: &Path) -> Result<Self, HarnessError> {
        let scan = scan_capture_stream(path)?;
        let (hello_val, hello_etf_frame, ready_val, ready_etf_frame) =
            decode_hello_and_ready_terms(&scan.raw_wsrecv_frames)?;
        let heartbeat_interval_ms = hello_val
            .get("d")
            .and_then(|d| d.get("heartbeat_interval"))
            .and_then(EtfValue::as_int)
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(41_250);
        let ready = Ready::from_dispatch(&ready_val)
            .map_err(|e| HarnessError::Protocol(format!("Ready::from_dispatch: {e:?}")))?;
        let current_user = convert_ready_user(&ready);
        let guilds = convert_ready_guilds(&ready);
        let messages_by_channel =
            build_seed_messages(&scan.attached_channel_id, &current_user, &guilds);

        Ok(Self {
            gateway_request_id: scan.gateway_request_id,
            gateway_url: scan.gateway_url,
            attached_guild_id: scan.attached_guild_id,
            attached_channel_id: scan.attached_channel_id,
            heartbeat_interval_ms,
            session_id: ready.session_id,
            current_user,
            guilds,
            messages_by_channel,
            raw_wsrecv_frames: scan.raw_wsrecv_frames,
            hello_etf_frame,
            ready_etf_frame,
        })
    }
}
