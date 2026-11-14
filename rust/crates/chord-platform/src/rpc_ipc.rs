//! P2.6: Бинарный протокол Discord Local RPC IPC (`discord-ipc-0..9`).
//!
//! Оракулы:
//! - `docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md`
//! - `docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs`
//! - `docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/activity.rs`
//! - `docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs` (`serve_ipc`, `read_frame`, `write_frame`)
//! - `docs/reference-projects/sources/Serein/crates/discord-protocol/src/rpc.rs`

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Длина бинарного заголовка фрейма Discord IPC (`[opcode: u32 LE][length: u32 LE]`).
pub const IPC_HEADER_LEN: usize = 8;

/// Максимальный размер JSON-тела одного фрейма Discord IPC (`16 KiB`, по эталону `Serein`).
pub const MAX_IPC_FRAME_BYTES: usize = 16 * 1024;

/// Максимальный индекс слота `discord-ipc-0..9`.
pub const MAX_IPC_SLOT: u8 = 9;

/// Количество стандартных слотов локального IPC-сокета Discord (`discord-ipc-0..9`).
pub const IPC_SLOT_COUNT: usize = 10;

/// Поддерживаемая версия протокола Discord RPC (`v = 1`).
pub const RPC_PROTOCOL_VERSION: u64 = 1;

/// Максимальное число кнопок в Rich Presence (`rust-discord-activity`: `Up to 2 buttons`).
pub const MAX_RPC_BUTTONS: usize = 2;

/// Максимальная длина текстового поля Rich Presence (`details`, `state`, `name`, `nonce`).
pub const MAX_RPC_TEXT_LEN: usize = 128;

/// Максимальная длина ключа ассета Rich Presence (`Serein/crates/discord-protocol/src/rpc.rs:L10`).
pub const MAX_RPC_ASSET_KEY_LEN: usize = 1024;

/// Максимальное число одновременных клиентских RPC-сессий (`Serein/apps/desktop/src/game_activity.rs`).
pub const MAX_RPC_CLIENTS: usize = 8;

/// Максимальное число экземпляров именованного канала Windows (`8` активных клиентов + `1` слушатель).
pub const WINDOWS_PIPE_MAX_INSTANCES: u32 = 9;

/// Префикс имени файла Unix domain socket (`discord-ipc-0..9`).
pub const UNIX_SOCKET_PREFIX: &str = "discord-ipc-";

/// Префикс пути именованного канала Windows (`\\.\pipe\discord-ipc-0..9`).
pub const WINDOWS_PIPE_PREFIX: &str = r"\\.\pipe\discord-ipc-";

/// Права доступа к файлу Unix-сокета (`0o600`, только владелец процесса).
pub const RPC_SOCKET_MODE: u32 = 0o600;

/// Очередь `listen(fd, 511)` по RE-трассе `/home/sanya/re-deep/voice/strace-renderer-s3.log:14744`.
pub const RPC_LISTEN_BACKLOG: i32 = 511;

/// Максимальная длина инвайт-кода в команде `INVITE_BROWSER` (`Serein/crates/discord-protocol/src/rpc.rs:L12`).
pub const MAX_INVITE_CODE_LEN: usize = 64;

/// Код ошибки Discord RPC для неподдерживаемой команды или невалидного payload (`4000`).
pub const RPC_ERROR_CODE_INVALID_PAYLOAD: u32 = 4000;

/// Порог перевода legacy-таймстемпов RPC из секунд в миллисекунды (`< 10_000_000_000`).
pub const LEGACY_TIMESTAMP_SECONDS_CUTOFF: u64 = 10_000_000_000;

/// Коды операций бинарного протокола Discord Local RPC IPC (`u32 LE`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RpcOpCode {
    /// Начальное рукопожатие (`{"v": 1, "client_id": "..."}`).
    Handshake = 0,
    /// Рабочий фрейм команды/события (`SET_ACTIVITY`, `DISPATCH READY` и др.).
    Frame = 1,
    /// Закрытие IPC-сессии.
    Close = 2,
    /// Проверка активности соединения (Ping).
    Ping = 3,
    /// Ответ на проверку активности соединения (Pong).
    Pong = 4,
}

impl RpcOpCode {
    /// Преобразует код операции в `u32`.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Разбирает `u32` в [`RpcOpCode`].
    #[must_use]
    pub const fn from_u32(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Handshake),
            1 => Some(Self::Frame),
            2 => Some(Self::Close),
            3 => Some(Self::Ping),
            4 => Some(Self::Pong),
            _ => None,
        }
    }
}

impl TryFrom<u32> for RpcOpCode {
    type Error = RpcIpcError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_u32(value).ok_or(RpcIpcError::InvalidOpCode(value))
    }
}

impl From<RpcOpCode> for u32 {
    fn from(value: RpcOpCode) -> Self {
        value.as_u32()
    }
}

/// Ошибки протокола Discord Local RPC IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcIpcError {
    /// Номер слота вне диапазона `0..=9`.
    InvalidSlot(u8),
    /// Заголовок фрейма короче 8 байт.
    HeaderTooShort(usize),
    /// Длина фрейма превышает [`MAX_IPC_FRAME_BYTES`].
    FrameTooLarge { length: usize, max: usize },
    /// Фактическая длина буфера меньше заявленной в заголовке.
    TruncatedPayload { expected: usize, actual: usize },
    /// Неизвестный код операции.
    InvalidOpCode(u32),
    /// Payload не является валидным UTF-8.
    InvalidUtf8,
    /// Ошибка синтаксиса или структуры JSON.
    InvalidJson(String),
    /// Неподдерживаемая версия рукопожатия (`v != 1`).
    UnsupportedVersion(u64),
    /// Отсутствует обязательное поле.
    MissingField(&'static str),
    /// Некорректное значение поля.
    InvalidField(&'static str),
    /// Клиент ещё не подключён к сокету.
    NotConnected,
    /// Рабочая команда получена до завершения начального рукопожатия (`Handshake`).
    HandshakeRequired,
    /// Все 10 стандартных слотов `discord-ipc-0..9` заняты живыми слушателями.
    AllSlotsBusy,
    /// Каталог размещения IPC-сокета имеет небезопасные права доступа или владельца.
    UnsafeDirectory(String),
    /// Подключившийся процесс принадлежит другому пользователю ОС (`SO_PEERCRED`/`getpeereid`).
    UnauthorizedPeer { peer_uid: u32, expected_uid: u32 },
    /// Ошибка ввода-вывода Unix-сокета.
    IoError(String),
}

impl fmt::Display for RpcIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlot(slot) => {
                write!(f, "недопустимый слот discord-ipc: {slot} (ожидался 0..=9)")
            }
            Self::HeaderTooShort(len) => write!(f, "заголовок IPC короче 8 байт: {len}"),
            Self::FrameTooLarge { length, max } => {
                write!(f, "размер IPC фрейма {length} превышает лимит {max}")
            }
            Self::TruncatedPayload { expected, actual } => {
                write!(
                    f,
                    "неполное тело IPC фрейма: ожидалось {expected}, получено {actual}"
                )
            }
            Self::InvalidOpCode(op) => write!(f, "неизвестный код операции IPC: {op}"),
            Self::InvalidUtf8 => f.write_str("некорректная UTF-8 последовательность в IPC фрейме"),
            Self::InvalidJson(msg) => write!(f, "ошибка JSON в IPC фрейме: {msg}"),
            Self::UnsupportedVersion(v) => write!(f, "неподдерживаемая версия RPC: {v}"),
            Self::MissingField(field) => write!(f, "отсутствует обязательное поле RPC: {field}"),
            Self::InvalidField(field) => write!(f, "некорректное поле RPC: {field}"),
            Self::NotConnected => f.write_str("клиент DiscordRpcClient не подключён к сокету"),
            Self::HandshakeRequired => {
                f.write_str("требуется начальное рукопожатие Handshake перед отправкой команд RPC")
            }
            Self::AllSlotsBusy => {
                f.write_str("все 10 слотов discord-ipc-0..9 заняты активными слушателями")
            }
            Self::UnsafeDirectory(dir) => {
                write!(f, "небезопасные права доступа к каталогу IPC: {dir}")
            }
            Self::UnauthorizedPeer {
                peer_uid,
                expected_uid,
            } => write!(
                f,
                "отклонено подключение IPC от чужого UID {peer_uid} (ожидался {expected_uid})"
            ),
            Self::IoError(msg) => write!(f, "ошибка ввода-вывода IPC: {msg}"),
        }
    }
}

impl std::error::Error for RpcIpcError {}

/// Временные метки активности Rich Presence (`timestamps`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcTimestamps {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

impl RpcTimestamps {
    #[must_use]
    pub const fn new(start: Option<u64>, end: Option<u64>) -> Self {
        Self { start, end }
    }

    /// Переводит legacy-таймстемпы RPC из секунд (`< 10_000_000_000`) в миллисекунды Gateway
    /// (по эталону `Serein/crates/discord-protocol/src/rpc.rs:L215-L227`).
    #[must_use]
    pub const fn normalized_millis(&self) -> Self {
        const fn norm(val: u64) -> u64 {
            if val > 0 && val < LEGACY_TIMESTAMP_SECONDS_CUTOFF {
                val.saturating_mul(1000)
            } else {
                val
            }
        }
        Self {
            start: match self.start {
                Some(s) => Some(norm(s)),
                None => None,
            },
            end: match self.end {
                Some(e) => Some(norm(e)),
                None => None,
            },
        }
    }
}

/// Изображения и подписи Rich Presence (`assets`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcAssets {
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

impl RpcAssets {
    #[must_use]
    pub fn new(
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
    ) -> Self {
        Self {
            large_image,
            large_text,
            small_image,
            small_text,
        }
    }
}

/// Информация о группе/лобби игроков (`party`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcParty {
    pub id: Option<String>,
    pub size: Option<[u32; 2]>,
}

impl RpcParty {
    #[must_use]
    pub fn new(id: Option<String>, size: Option<[u32; 2]>) -> Self {
        Self { id, size }
    }
}

/// Кнопка действия в карточке Rich Presence (`buttons`, до 2 штук).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcButton {
    pub label: String,
    pub url: String,
}

impl RpcButton {
    #[must_use]
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
        }
    }
}

/// Модель активности Rich Presence (`activity` в `SET_ACTIVITY` и Gateway Op 3 `PresenceUpdate`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcActivity {
    pub name: Option<String>,
    pub activity_type: u8,
    pub state: Option<String>,
    pub details: Option<String>,
    pub application_id: Option<String>,
    pub timestamps: Option<RpcTimestamps>,
    pub assets: Option<RpcAssets>,
    pub party: Option<RpcParty>,
    pub buttons: Vec<RpcButton>,
    pub instance: Option<bool>,
    pub url: Option<String>,
}

impl RpcActivity {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_activity_type(mut self, activity_type: u8) -> Self {
        self.activity_type = activity_type;
        self
    }

    #[must_use]
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    #[must_use]
    pub fn with_application_id(mut self, application_id: impl Into<String>) -> Self {
        self.application_id = Some(application_id.into());
        self
    }

    #[must_use]
    pub fn with_timestamps(mut self, timestamps: RpcTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn with_assets(mut self, assets: RpcAssets) -> Self {
        self.assets = Some(assets);
        self
    }

    #[must_use]
    pub fn with_party(mut self, party: RpcParty) -> Self {
        self.party = Some(party);
        self
    }

    #[must_use]
    pub fn with_buttons(mut self, buttons: Vec<RpcButton>) -> Self {
        self.buttons = buttons;
        self
    }

    #[must_use]
    pub fn with_instance(mut self, instance: bool) -> Self {
        self.instance = Some(instance);
        self
    }

    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Проверяет структурные ограничения полей `RpcActivity` (по эталону `Serein/crates/discord-protocol/src/rpc.rs`).
    pub fn validate(&self) -> Result<(), RpcIpcError> {
        if !matches!(self.activity_type, 0..=5) {
            return Err(RpcIpcError::InvalidField("type"));
        }
        for (field_name, opt_val) in [
            ("name", &self.name),
            ("state", &self.state),
            ("details", &self.details),
        ] {
            if let Some(text) = opt_val
                && (text.len() > MAX_RPC_TEXT_LEN || text.chars().any(char::is_control))
            {
                return Err(RpcIpcError::InvalidField(field_name));
            }
        }
        if let Some(ref ts) = self.timestamps
            && let (Some(start), Some(end)) = (ts.start, ts.end)
            && end < start
        {
            return Err(RpcIpcError::InvalidField("timestamps"));
        }
        if let Some(ref assets) = self.assets {
            for (field_name, opt_key) in [
                ("large_image", &assets.large_image),
                ("small_image", &assets.small_image),
            ] {
                if let Some(k) = opt_key
                    && (k.len() > MAX_RPC_ASSET_KEY_LEN || k.chars().any(char::is_control))
                {
                    return Err(RpcIpcError::InvalidField(field_name));
                }
            }
            for (field_name, opt_txt) in [
                ("large_text", &assets.large_text),
                ("small_text", &assets.small_text),
            ] {
                if let Some(t) = opt_txt
                    && (t.len() > MAX_RPC_TEXT_LEN || t.chars().any(char::is_control))
                {
                    return Err(RpcIpcError::InvalidField(field_name));
                }
            }
        }
        if let Some(ref party) = self.party
            && let Some([current, max]) = party.size
            && (current == 0 || current > max || max > 9999)
        {
            return Err(RpcIpcError::InvalidField("party.size"));
        }
        if self.buttons.len() > MAX_RPC_BUTTONS {
            return Err(RpcIpcError::InvalidField("buttons"));
        }
        for btn in &self.buttons {
            if btn.label.trim().is_empty()
                || btn.label.len() > 32
                || btn.label.chars().any(char::is_control)
                || !is_valid_https_url(&btn.url)
            {
                return Err(RpcIpcError::InvalidField("buttons"));
            }
        }
        if let Some(ref url) = self.url
            && !is_valid_https_url(url)
        {
            return Err(RpcIpcError::InvalidField("url"));
        }
        Ok(())
    }

    fn to_json_value(&self) -> JsonVal {
        let mut map = BTreeMap::new();
        map.insert(
            "type".to_owned(),
            JsonVal::Uint(u64::from(self.activity_type)),
        );
        if let Some(ref name) = self.name {
            map.insert("name".to_owned(), JsonVal::Str(name.clone()));
        }
        if let Some(ref state) = self.state {
            map.insert("state".to_owned(), JsonVal::Str(state.clone()));
        }
        if let Some(ref details) = self.details {
            map.insert("details".to_owned(), JsonVal::Str(details.clone()));
        }
        if let Some(ref app_id) = self.application_id {
            map.insert("application_id".to_owned(), JsonVal::Str(app_id.clone()));
        }
        if let Some(ref ts) = self.timestamps {
            let mut ts_map = BTreeMap::new();
            if let Some(start) = ts.start {
                ts_map.insert("start".to_owned(), JsonVal::Uint(start));
            }
            if let Some(end) = ts.end {
                ts_map.insert("end".to_owned(), JsonVal::Uint(end));
            }
            map.insert("timestamps".to_owned(), JsonVal::Obj(ts_map));
        }
        if let Some(ref assets) = self.assets {
            let mut a_map = BTreeMap::new();
            if let Some(ref li) = assets.large_image {
                a_map.insert("large_image".to_owned(), JsonVal::Str(li.clone()));
            }
            if let Some(ref lt) = assets.large_text {
                a_map.insert("large_text".to_owned(), JsonVal::Str(lt.clone()));
            }
            if let Some(ref si) = assets.small_image {
                a_map.insert("small_image".to_owned(), JsonVal::Str(si.clone()));
            }
            if let Some(ref st) = assets.small_text {
                a_map.insert("small_text".to_owned(), JsonVal::Str(st.clone()));
            }
            map.insert("assets".to_owned(), JsonVal::Obj(a_map));
        }
        if let Some(ref party) = self.party {
            let mut p_map = BTreeMap::new();
            if let Some(ref id) = party.id {
                p_map.insert("id".to_owned(), JsonVal::Str(id.clone()));
            }
            if let Some([cur, max]) = party.size {
                p_map.insert(
                    "size".to_owned(),
                    JsonVal::Arr(vec![
                        JsonVal::Uint(u64::from(cur)),
                        JsonVal::Uint(u64::from(max)),
                    ]),
                );
            }
            map.insert("party".to_owned(), JsonVal::Obj(p_map));
        }
        if !self.buttons.is_empty() {
            let btns = self
                .buttons
                .iter()
                .map(|b| {
                    let mut b_map = BTreeMap::new();
                    b_map.insert("label".to_owned(), JsonVal::Str(b.label.clone()));
                    b_map.insert("url".to_owned(), JsonVal::Str(b.url.clone()));
                    JsonVal::Obj(b_map)
                })
                .collect();
            map.insert("buttons".to_owned(), JsonVal::Arr(btns));
        }
        if let Some(inst) = self.instance {
            map.insert("instance".to_owned(), JsonVal::Bool(inst));
        }
        if let Some(ref url) = self.url {
            map.insert("url".to_owned(), JsonVal::Str(url.clone()));
        }
        JsonVal::Obj(map)
    }

    fn from_json_value(val: &JsonVal) -> Result<Self, RpcIpcError> {
        let obj = val.as_obj().ok_or(RpcIpcError::InvalidField("activity"))?;

        let activity_type = match obj.get("type") {
            Some(JsonVal::Uint(n)) => {
                u8::try_from(*n).map_err(|_| RpcIpcError::InvalidField("type"))?
            }
            Some(JsonVal::Null) | None => 0,
            _ => return Err(RpcIpcError::InvalidField("type")),
        };

        let name = opt_string_field(obj, "name")?;
        let state = opt_string_field(obj, "state")?;
        let details = opt_string_field(obj, "details")?;
        let application_id = opt_string_field(obj, "application_id")?;
        let url = opt_string_field(obj, "url")?;

        let instance = match obj.get("instance") {
            Some(JsonVal::Bool(b)) => Some(*b),
            Some(JsonVal::Null) | None => None,
            _ => return Err(RpcIpcError::InvalidField("instance")),
        };

        let timestamps = match obj.get("timestamps") {
            Some(JsonVal::Obj(ts_obj)) => {
                let start = opt_u64_field(ts_obj, "start")?;
                let end = opt_u64_field(ts_obj, "end")?;
                Some(RpcTimestamps { start, end })
            }
            Some(JsonVal::Null) | None => None,
            _ => return Err(RpcIpcError::InvalidField("timestamps")),
        };

        let assets = match obj.get("assets") {
            Some(JsonVal::Obj(a_obj)) => Some(RpcAssets {
                large_image: opt_string_field(a_obj, "large_image")?,
                large_text: opt_string_field(a_obj, "large_text")?,
                small_image: opt_string_field(a_obj, "small_image")?,
                small_text: opt_string_field(a_obj, "small_text")?,
            }),
            Some(JsonVal::Null) | None => None,
            _ => return Err(RpcIpcError::InvalidField("assets")),
        };

        let party = match obj.get("party") {
            Some(JsonVal::Obj(p_obj)) => {
                let id = opt_string_field(p_obj, "id")?;
                let size = match p_obj.get("size") {
                    Some(JsonVal::Arr(items)) => {
                        if items.len() != 2 {
                            return Err(RpcIpcError::InvalidField("party.size"));
                        }
                        let cur = items[0]
                            .as_u64()
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or(RpcIpcError::InvalidField("party.size"))?;
                        let max = items[1]
                            .as_u64()
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or(RpcIpcError::InvalidField("party.size"))?;
                        Some([cur, max])
                    }
                    Some(JsonVal::Null) | None => None,
                    _ => return Err(RpcIpcError::InvalidField("party.size")),
                };
                Some(RpcParty { id, size })
            }
            Some(JsonVal::Null) | None => None,
            _ => return Err(RpcIpcError::InvalidField("party")),
        };

        let buttons = match obj.get("buttons") {
            Some(JsonVal::Arr(items)) => {
                let mut list = Vec::with_capacity(items.len());
                for item in items {
                    let b_obj = item.as_obj().ok_or(RpcIpcError::InvalidField("buttons"))?;
                    let label = b_obj
                        .get("label")
                        .and_then(JsonVal::as_str)
                        .ok_or(RpcIpcError::InvalidField("buttons"))?
                        .to_owned();
                    let btn_url = b_obj
                        .get("url")
                        .and_then(JsonVal::as_str)
                        .ok_or(RpcIpcError::InvalidField("buttons"))?
                        .to_owned();
                    list.push(RpcButton {
                        label,
                        url: btn_url,
                    });
                }
                list
            }
            Some(JsonVal::Null) | None => Vec::new(),
            _ => return Err(RpcIpcError::InvalidField("buttons")),
        };

        let activity = Self {
            name,
            activity_type,
            state,
            details,
            application_id,
            timestamps,
            assets,
            party,
            buttons,
            instance,
            url,
        };
        activity.validate()?;
        Ok(activity)
    }

    /// Строит JSON-фрейм Gateway Op 3 (`PresenceUpdate`) для отправки активности в шлюз Discord.
    #[must_use]
    pub fn to_gateway_presence_update_json(&self) -> String {
        build_gateway_presence_update_json(Some(self))
    }
}

fn build_gateway_presence_update_json(activity: Option<&RpcActivity>) -> String {
    let activities = match activity {
        Some(act) => vec![act.to_json_value()],
        None => Vec::new(),
    };
    let mut d = BTreeMap::new();
    d.insert("since".to_owned(), JsonVal::Null);
    d.insert("activities".to_owned(), JsonVal::Arr(activities));
    d.insert("status".to_owned(), JsonVal::Str("online".to_owned()));
    d.insert("afk".to_owned(), JsonVal::Bool(false));

    let mut root = BTreeMap::new();
    root.insert("op".to_owned(), JsonVal::Uint(3));
    root.insert("d".to_owned(), JsonVal::Obj(d));
    JsonVal::Obj(root).to_json_string()
}

fn is_valid_https_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if url.len() > 2048 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..host_end];
    !authority.is_empty() && !authority.contains('@') && authority.contains('.')
}

/// Полезная нагрузка команды `SET_ACTIVITY`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetActivityPayload {
    pub pid: u32,
    pub activity: Option<RpcActivity>,
    pub nonce: String,
}

impl SetActivityPayload {
    #[must_use]
    pub fn new(pid: u32, activity: Option<RpcActivity>, nonce: impl Into<String>) -> Self {
        Self {
            pid,
            activity,
            nonce: nonce.into(),
        }
    }

    /// Сериализует команду `SET_ACTIVITY` в JSON-строку протокола Discord RPC.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut args = BTreeMap::new();
        args.insert("pid".to_owned(), JsonVal::Uint(u64::from(self.pid)));
        match &self.activity {
            Some(act) => {
                args.insert("activity".to_owned(), act.to_json_value());
            }
            None => {
                args.insert("activity".to_owned(), JsonVal::Null);
            }
        }
        let mut root = BTreeMap::new();
        root.insert("cmd".to_owned(), JsonVal::Str("SET_ACTIVITY".to_owned()));
        root.insert("args".to_owned(), JsonVal::Obj(args));
        root.insert("nonce".to_owned(), JsonVal::Str(self.nonce.clone()));
        JsonVal::Obj(root).to_json_string()
    }

    /// Декодирует JSON-команду `SET_ACTIVITY`.
    pub fn from_json(raw: &str) -> Result<Self, RpcIpcError> {
        let val = parse_json(raw)?;
        Self::from_json_value(&val)
    }

    fn from_json_value(val: &JsonVal) -> Result<Self, RpcIpcError> {
        let obj = val
            .as_obj()
            .ok_or_else(|| RpcIpcError::InvalidJson("root must be object".to_owned()))?;
        let cmd = obj
            .get("cmd")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("cmd"))?;
        if cmd != "SET_ACTIVITY" {
            return Err(RpcIpcError::InvalidField("cmd"));
        }
        let nonce = obj
            .get("nonce")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("nonce"))?;
        if nonce.is_empty() || nonce.len() > MAX_RPC_TEXT_LEN || nonce.chars().any(char::is_control)
        {
            return Err(RpcIpcError::InvalidField("nonce"));
        }
        let args = obj
            .get("args")
            .and_then(JsonVal::as_obj)
            .ok_or(RpcIpcError::MissingField("args"))?;
        let pid_u64 = args
            .get("pid")
            .and_then(JsonVal::as_u64)
            .ok_or(RpcIpcError::MissingField("pid"))?;
        let pid = u32::try_from(pid_u64)
            .ok()
            .filter(|&p| p != 0)
            .ok_or(RpcIpcError::InvalidField("pid"))?;

        let activity = match args.get("activity") {
            Some(JsonVal::Null) | None => None,
            Some(act_val) => Some(RpcActivity::from_json_value(act_val)?),
        };

        Ok(Self {
            pid,
            activity,
            nonce: nonce.to_owned(),
        })
    }
}

/// Проверяет, что инвайт-код в `INVITE_BROWSER` состоит только из безопасных символов Discord
/// (`1..=64` байт, ASCII alphanumeric, `-` или `_`; по эталону `Serein/crates/discord-protocol/src/rpc.rs:L346`).
#[must_use]
pub fn is_valid_invite_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_INVITE_CODE_LEN
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Полезная нагрузка команды `INVITE_BROWSER`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteBrowserPayload {
    pub code: String,
    pub nonce: String,
}

impl InviteBrowserPayload {
    #[must_use]
    pub fn new(code: impl Into<String>, nonce: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            nonce: nonce.into(),
        }
    }

    /// Сериализует команду `INVITE_BROWSER` в JSON-строку протокола Discord RPC.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut args = BTreeMap::new();
        args.insert("code".to_owned(), JsonVal::Str(self.code.clone()));
        let mut root = BTreeMap::new();
        root.insert("cmd".to_owned(), JsonVal::Str("INVITE_BROWSER".to_owned()));
        root.insert("args".to_owned(), JsonVal::Obj(args));
        root.insert("nonce".to_owned(), JsonVal::Str(self.nonce.clone()));
        JsonVal::Obj(root).to_json_string()
    }

    /// Декодирует JSON-команду `INVITE_BROWSER`.
    pub fn from_json(raw: &str) -> Result<Self, RpcIpcError> {
        let val = parse_json(raw)?;
        Self::from_json_value(&val)
    }

    fn from_json_value(val: &JsonVal) -> Result<Self, RpcIpcError> {
        let obj = val
            .as_obj()
            .ok_or_else(|| RpcIpcError::InvalidJson("root must be object".to_owned()))?;
        let cmd = obj
            .get("cmd")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("cmd"))?;
        if cmd != "INVITE_BROWSER" {
            return Err(RpcIpcError::InvalidField("cmd"));
        }
        let nonce = obj
            .get("nonce")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("nonce"))?;
        if nonce.is_empty() || nonce.len() > MAX_RPC_TEXT_LEN || nonce.chars().any(char::is_control)
        {
            return Err(RpcIpcError::InvalidField("nonce"));
        }
        let args = obj
            .get("args")
            .and_then(JsonVal::as_obj)
            .ok_or(RpcIpcError::MissingField("args"))?;
        let code = args
            .get("code")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("code"))?;
        if !is_valid_invite_code(code) {
            return Err(RpcIpcError::InvalidField("code"));
        }
        Ok(Self {
            code: code.to_owned(),
            nonce: nonce.to_owned(),
        })
    }
}

/// Локально поддерживаемые RPC-команды Discord IPC (`SET_ACTIVITY` и `INVITE_BROWSER`).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcCommand {
    SetActivity(SetActivityPayload),
    InviteBrowser(InviteBrowserPayload),
}

impl RpcCommand {
    /// Декодирует входящую JSON-команду фрейма `RpcOpCode::Frame`.
    pub fn from_json(raw: &str) -> Result<Self, RpcIpcError> {
        if raw.len() > MAX_IPC_FRAME_BYTES {
            return Err(RpcIpcError::FrameTooLarge {
                length: raw.len(),
                max: MAX_IPC_FRAME_BYTES,
            });
        }
        let val = parse_json(raw)?;
        let obj = val
            .as_obj()
            .ok_or_else(|| RpcIpcError::InvalidJson("root must be object".to_owned()))?;
        let cmd = obj
            .get("cmd")
            .and_then(JsonVal::as_str)
            .ok_or(RpcIpcError::MissingField("cmd"))?;
        match cmd {
            "SET_ACTIVITY" => SetActivityPayload::from_json_value(&val).map(Self::SetActivity),
            "INVITE_BROWSER" => {
                InviteBrowserPayload::from_json_value(&val).map(Self::InviteBrowser)
            }
            _ => Err(RpcIpcError::InvalidField("cmd")),
        }
    }
}

/// Раскодированный фрейм Discord Local RPC IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcFrame {
    pub opcode: RpcOpCode,
    pub payload: String,
}

/// Упаковывает фрейм Discord IPC в 8-байтный заголовок `[opcode: u32 LE][length: u32 LE]` + UTF-8 JSON payload.
pub fn encode_ipc_frame(opcode: RpcOpCode, payload: &str) -> Result<Vec<u8>, RpcIpcError> {
    let length = payload.len();
    if length > MAX_IPC_FRAME_BYTES {
        return Err(RpcIpcError::FrameTooLarge {
            length,
            max: MAX_IPC_FRAME_BYTES,
        });
    }
    let mut buf = Vec::with_capacity(IPC_HEADER_LEN + length);
    buf.extend_from_slice(&opcode.as_u32().to_le_bytes());
    buf.extend_from_slice(&(length as u32).to_le_bytes());
    buf.extend_from_slice(payload.as_bytes());
    Ok(buf)
}

/// Декодирует бинарный фрейм Discord IPC из среза байт.
pub fn decode_ipc_frame(bytes: &[u8]) -> Result<IpcFrame, RpcIpcError> {
    if bytes.len() < IPC_HEADER_LEN {
        return Err(RpcIpcError::HeaderTooShort(bytes.len()));
    }
    let raw_opcode = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let length = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if length > MAX_IPC_FRAME_BYTES {
        return Err(RpcIpcError::FrameTooLarge {
            length,
            max: MAX_IPC_FRAME_BYTES,
        });
    }
    let body = &bytes[IPC_HEADER_LEN..];
    if body.len() < length {
        return Err(RpcIpcError::TruncatedPayload {
            expected: length,
            actual: body.len(),
        });
    }
    let opcode = RpcOpCode::try_from(raw_opcode)?;
    let payload_str = std::str::from_utf8(&body[..length]).map_err(|_| RpcIpcError::InvalidUtf8)?;
    Ok(IpcFrame {
        opcode,
        payload: payload_str.to_owned(),
    })
}

/// Определяет путь к Unix-сокету `discord-ipc-<slot>` (`slot` в `0..=9`), проверяя
/// `XDG_RUNTIME_DIR`, `TMPDIR`, `TMP`, `TEMP` и откатываясь на `/tmp`.
pub fn resolve_ipc_socket_path(slot: u8) -> Result<PathBuf, RpcIpcError> {
    if slot > MAX_IPC_SLOT {
        return Err(RpcIpcError::InvalidSlot(slot));
    }
    let base_dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|val| !val.trim().is_empty()))
        .unwrap_or_else(|| "/tmp".to_owned());
    Ok(PathBuf::from(base_dir).join(format!("discord-ipc-{slot}")))
}

/// Определяет путь к сокету `discord-ipc-<slot>` по явно переданному окружению (для детерминированных тестов).
pub fn resolve_ipc_socket_path_from_env(
    slot: u8,
    env_vars: &[(&str, &str)],
) -> Result<PathBuf, RpcIpcError> {
    if slot > MAX_IPC_SLOT {
        return Err(RpcIpcError::InvalidSlot(slot));
    }
    let base_dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .find_map(|key| {
            env_vars
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).trim())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or("/tmp");
    Ok(PathBuf::from(base_dir).join(format!("discord-ipc-{slot}")))
}

/// Формирует путь к именованному каналу Windows `\\.\pipe\discord-ipc-<slot>` (`slot` в `0..=9`).
pub fn resolve_windows_named_pipe_path(slot: u8) -> Result<String, RpcIpcError> {
    if slot > MAX_IPC_SLOT {
        return Err(RpcIpcError::InvalidSlot(slot));
    }
    Ok(format!("{WINDOWS_PIPE_PREFIX}{slot}"))
}

/// Извлекает индекс слота `0..=9` из пути Unix-сокета, имени файла `discord-ipc-<slot>` или
/// Windows named pipe `\\.\pipe\discord-ipc-<slot>`.
pub fn parse_ipc_slot(path_or_name: &str) -> Result<u8, RpcIpcError> {
    let digits = if let Some(rest) = path_or_name.strip_prefix(WINDOWS_PIPE_PREFIX) {
        rest
    } else {
        let base_name = path_or_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path_or_name);
        base_name
            .strip_prefix(UNIX_SOCKET_PREFIX)
            .ok_or(RpcIpcError::InvalidField("slot"))?
    };

    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(RpcIpcError::InvalidField("slot"));
    }

    let slot = digits
        .parse::<u8>()
        .map_err(|_| RpcIpcError::InvalidField("slot"))?;
    if slot > MAX_IPC_SLOT {
        return Err(RpcIpcError::InvalidSlot(slot));
    }
    Ok(slot)
}

/// Строит SDDL-строку дескриптора безопасности Windows DACL (`D:P(A;;GA;;;{sid})`),
/// разрешающую доступ к `\\.\pipe\discord-ipc-0..9` только текущему пользователю
/// (`Serein/crates/platform/src/game_activity.rs:L137`).
pub fn format_windows_sddl_for_sid(sid: &str) -> Result<String, RpcIpcError> {
    let Some(rest) = sid.strip_prefix("S-1-") else {
        return Err(RpcIpcError::InvalidField("sid"));
    };
    if sid.len() < 6
        || sid.len() > 184
        || rest.is_empty()
        || rest.starts_with('-')
        || rest.ends_with('-')
        || rest.contains("--")
        || !rest.contains('-')
        || !rest.bytes().all(|b| b.is_ascii_digit() || b == b'-')
    {
        return Err(RpcIpcError::InvalidField("sid"));
    }
    Ok(format!("D:P(A;;GA;;;{sid})"))
}

/// Конфигурация серверного именованного канала Windows `\\.\pipe\discord-ipc-<slot>`
/// (`Serein/crates/platform/src/game_activity.rs:L173-L183`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNamedPipeConfig {
    pub pipe_name: String,
    pub slot: u8,
    pub sddl_dacl: String,
    pub first_pipe_instance: bool,
    pub reject_remote_clients: bool,
    pub max_instances: u32,
    pub in_buffer_size: u32,
    pub out_buffer_size: u32,
}

impl WindowsNamedPipeConfig {
    /// Создаёт проверенную конфигурацию именованного канала Windows для слота `0..=9` и SID владельца.
    pub fn for_slot(slot: u8, sid: &str, first_pipe_instance: bool) -> Result<Self, RpcIpcError> {
        let pipe_name = resolve_windows_named_pipe_path(slot)?;
        let sddl_dacl = format_windows_sddl_for_sid(sid)?;
        Ok(Self {
            pipe_name,
            slot,
            sddl_dacl,
            first_pipe_instance,
            reject_remote_clients: true,
            max_instances: WINDOWS_PIPE_MAX_INSTANCES,
            in_buffer_size: MAX_IPC_FRAME_BYTES as u32,
            out_buffer_size: MAX_IPC_FRAME_BYTES as u32,
        })
    }
}

/// Кроссплатформенный дескриптор конечной точки `discord-ipc-0..9` (Unix domain socket или Windows named pipe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEndpoint {
    UnixSocket { slot: u8, path: PathBuf },
    WindowsNamedPipe { slot: u8, pipe_name: String },
}

impl IpcEndpoint {
    /// Формирует Unix-сокет конечную точку для слота `0..=9` по заданному окружению.
    pub fn unix_from_env(slot: u8, env_vars: &[(&str, &str)]) -> Result<Self, RpcIpcError> {
        let path = resolve_ipc_socket_path_from_env(slot, env_vars)?;
        Ok(Self::UnixSocket { slot, path })
    }

    /// Формирует Windows named pipe конечную точку `\\.\pipe\discord-ipc-<slot>` для слота `0..=9`.
    pub fn windows(slot: u8) -> Result<Self, RpcIpcError> {
        let pipe_name = resolve_windows_named_pipe_path(slot)?;
        Ok(Self::WindowsNamedPipe { slot, pipe_name })
    }

    /// Возвращает номер слота `0..=9`.
    #[must_use]
    pub const fn slot(&self) -> u8 {
        match self {
            Self::UnixSocket { slot, .. } | Self::WindowsNamedPipe { slot, .. } => *slot,
        }
    }
}

/// Результат серверной обработки одного входящего фрейма `handle_ipc_frame`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcServerResponse {
    /// Ответный IPC-фрейм для отправки клиенту (`READY`, `SET_ACTIVITY` ACK, `INVITE_BROWSER` ACK, `ERROR`, `Pong`).
    pub reply_frame: Option<IpcFrame>,
    /// Распарсенная активность из `SET_ACTIVITY` для публикации в Gateway Op 3 `PresenceUpdate`.
    pub activity: Option<RpcActivity>,
    /// Сериализованный JSON-пакет Gateway Op 3 `PresenceUpdate`, если состояние присутствия изменилось.
    pub gateway_presence_update_json: Option<String>,
    /// Код приглашения из команды `INVITE_BROWSER`, если была получена команда открытия инвайта.
    pub invite_code: Option<String>,
    /// Флаг завершения соединения (`RpcOpCode::Close`).
    pub closed: bool,
}

/// Серверный обработчик входящего IPC-фрейма:
/// - На `Handshake` (`v=1`) возвращает `DISPATCH` `READY`;
/// - На `Frame` `SET_ACTIVITY` возвращает ACK-фрейм и распарсенную [`RpcActivity`] вместе с JSON для Gateway Op 3 `PresenceUpdate`;
/// - На `Frame` `INVITE_BROWSER` возвращает ACK-фрейм и проверенный код приглашения `invite_code`;
/// - На `Ping` отвечает `Pong`;
/// - На `Close` помечает сессию закрытой.
pub fn handle_ipc_frame(frame: &IpcFrame) -> Result<RpcServerResponse, RpcIpcError> {
    if frame.payload.len() > MAX_IPC_FRAME_BYTES {
        return Err(RpcIpcError::FrameTooLarge {
            length: frame.payload.len(),
            max: MAX_IPC_FRAME_BYTES,
        });
    }
    match frame.opcode {
        RpcOpCode::Handshake => {
            let client_id = parse_handshake_client_id(&frame.payload)?;
            let _ = client_id;
            let ready_payload = build_ready_dispatch_json();
            Ok(RpcServerResponse {
                reply_frame: Some(IpcFrame {
                    opcode: RpcOpCode::Frame,
                    payload: ready_payload,
                }),
                activity: None,
                gateway_presence_update_json: None,
                invite_code: None,
                closed: false,
            })
        }
        RpcOpCode::Frame => match RpcCommand::from_json(&frame.payload)? {
            RpcCommand::SetActivity(parsed) => {
                let ack_payload = build_set_activity_ack_json(&parsed);
                let op3_json = build_gateway_presence_update_json(parsed.activity.as_ref());
                Ok(RpcServerResponse {
                    reply_frame: Some(IpcFrame {
                        opcode: RpcOpCode::Frame,
                        payload: ack_payload,
                    }),
                    activity: parsed.activity,
                    gateway_presence_update_json: Some(op3_json),
                    invite_code: None,
                    closed: false,
                })
            }
            RpcCommand::InviteBrowser(inv) => {
                let ack_payload = build_invite_browser_ack_json(&inv);
                Ok(RpcServerResponse {
                    reply_frame: Some(IpcFrame {
                        opcode: RpcOpCode::Frame,
                        payload: ack_payload,
                    }),
                    activity: None,
                    gateway_presence_update_json: None,
                    invite_code: Some(inv.code),
                    closed: false,
                })
            }
        },
        RpcOpCode::Ping => Ok(RpcServerResponse {
            reply_frame: Some(IpcFrame {
                opcode: RpcOpCode::Pong,
                payload: frame.payload.clone(),
            }),
            activity: None,
            gateway_presence_update_json: None,
            invite_code: None,
            closed: false,
        }),
        RpcOpCode::Pong => Ok(RpcServerResponse {
            reply_frame: None,
            activity: None,
            gateway_presence_update_json: None,
            invite_code: None,
            closed: false,
        }),
        RpcOpCode::Close => Ok(RpcServerResponse {
            reply_frame: None,
            activity: None,
            gateway_presence_update_json: Some(build_gateway_presence_update_json(None)),
            invite_code: None,
            closed: true,
        }),
    }
}

fn parse_handshake_client_id(raw_payload: &str) -> Result<String, RpcIpcError> {
    let val = parse_json(raw_payload)?;
    let obj = val
        .as_obj()
        .ok_or_else(|| RpcIpcError::InvalidJson("handshake must be object".to_owned()))?;
    let version = obj
        .get("v")
        .and_then(JsonVal::as_u64)
        .ok_or(RpcIpcError::MissingField("v"))?;
    if version != RPC_PROTOCOL_VERSION {
        return Err(RpcIpcError::UnsupportedVersion(version));
    }
    let client_id = obj
        .get("client_id")
        .and_then(JsonVal::as_str)
        .ok_or(RpcIpcError::MissingField("client_id"))?;
    if client_id.trim().is_empty()
        || client_id == "0"
        || client_id.len() > 64
        || client_id.chars().any(char::is_control)
    {
        return Err(RpcIpcError::InvalidField("client_id"));
    }
    Ok(client_id.to_owned())
}

fn build_ready_dispatch_json() -> String {
    r#"{"cmd":"DISPATCH","data":{"config":{"api_endpoint":"//discord.com/api","cdn_host":"cdn.discordapp.com","environment":"production"},"user":{"avatar":null,"discriminator":"0","id":"1","username":"discord-lab"},"v":1},"evt":"READY"}"#.to_owned()
}

fn build_set_activity_ack_json(cmd: &SetActivityPayload) -> String {
    let mut root = BTreeMap::new();
    root.insert("cmd".to_owned(), JsonVal::Str("SET_ACTIVITY".to_owned()));
    root.insert("evt".to_owned(), JsonVal::Null);
    root.insert("nonce".to_owned(), JsonVal::Str(cmd.nonce.clone()));
    match &cmd.activity {
        Some(act) => {
            root.insert("data".to_owned(), act.to_json_value());
        }
        None => {
            root.insert("data".to_owned(), JsonVal::Null);
        }
    }
    JsonVal::Obj(root).to_json_string()
}

fn build_invite_browser_ack_json(inv: &InviteBrowserPayload) -> String {
    let mut data = BTreeMap::new();
    data.insert("code".to_owned(), JsonVal::Str(inv.code.clone()));
    let mut root = BTreeMap::new();
    root.insert("cmd".to_owned(), JsonVal::Str("INVITE_BROWSER".to_owned()));
    root.insert("data".to_owned(), JsonVal::Obj(data));
    root.insert("evt".to_owned(), JsonVal::Null);
    root.insert("nonce".to_owned(), JsonVal::Str(inv.nonce.clone()));
    JsonVal::Obj(root).to_json_string()
}

/// Строит JSON-ответ ошибки Discord RPC (`evt: "ERROR"`, `data: {"code": 4000, "message": ...}`).
#[must_use]
pub fn build_rpc_error_json(cmd: &str, nonce: Option<&str>, code: u32, message: &str) -> String {
    let safe_cmd = if !cmd.is_empty()
        && cmd.len() <= 64
        && cmd.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
    {
        cmd
    } else {
        "SET_ACTIVITY"
    };
    let safe_nonce = nonce.filter(|n| {
        !n.is_empty() && n.len() <= MAX_RPC_TEXT_LEN && !n.chars().any(char::is_control)
    });

    let mut data = BTreeMap::new();
    data.insert("code".to_owned(), JsonVal::Uint(u64::from(code)));
    data.insert("message".to_owned(), JsonVal::Str(message.to_owned()));

    let mut root = BTreeMap::new();
    root.insert("cmd".to_owned(), JsonVal::Str(safe_cmd.to_owned()));
    root.insert("data".to_owned(), JsonVal::Obj(data));
    root.insert("evt".to_owned(), JsonVal::Str("ERROR".to_owned()));
    match safe_nonce {
        Some(n) => root.insert("nonce".to_owned(), JsonVal::Str(n.to_owned())),
        None => root.insert("nonce".to_owned(), JsonVal::Null),
    };
    JsonVal::Obj(root).to_json_string()
}

/// Строит ответный фрейм `ERROR` (`code: 4000`) с сохранением корреляции `cmd` и `nonce`
/// из произвольного входящего JSON-запроса (`Serein/crates/discord-protocol/src/rpc.rs:L394-L419`).
#[must_use]
pub fn build_rpc_error_frame_for_payload(raw_payload: &str) -> IpcFrame {
    let parsed = if raw_payload.len() <= MAX_IPC_FRAME_BYTES {
        parse_json(raw_payload).ok()
    } else {
        None
    };
    let obj = parsed.as_ref().and_then(JsonVal::as_obj);
    let cmd = obj
        .and_then(|o| o.get("cmd"))
        .and_then(JsonVal::as_str)
        .unwrap_or("SET_ACTIVITY");
    let nonce = obj.and_then(|o| o.get("nonce")).and_then(JsonVal::as_str);
    IpcFrame {
        opcode: RpcOpCode::Frame,
        payload: build_rpc_error_json(
            cmd,
            nonce,
            RPC_ERROR_CODE_INVALID_PAYLOAD,
            "Unsupported or invalid activity payload",
        ),
    }
}

/// Псевдоним типа локального слушателя IPC-сокета Discord (`discord-ipc-0..9`).
pub type RpcIpcListener = DiscordRpcListener;

/// Локальный серверный слушатель Unix domain socket `discord-ipc-0..9`
/// (с защитой прав `0o600`, проверкой `SO_PEERCRED`/`getpeereid`, очисткой мёртвых сокетов `remove_stale`
/// и проверкой `(device, inode)` при удалении в `Drop`).
#[derive(Debug)]
pub struct DiscordRpcListener {
    listener: UnixListener,
    path: PathBuf,
    slot: Option<u8>,
    device: u64,
    inode: u64,
    uid: u32,
}

impl DiscordRpcListener {
    /// Привязывается к первому свободному слоту `discord-ipc-0..9` в системном каталоге
    /// (`XDG_RUNTIME_DIR`, `TMPDIR`, `TMP`, `TEMP`, либо `/tmp`).
    pub fn bind() -> Result<Self, RpcIpcError> {
        let sock0 = resolve_ipc_socket_path(0)?;
        let base_dir = sock0.parent().unwrap_or_else(|| Path::new("/tmp"));
        Self::bind_in_dir(base_dir)
    }

    /// Перебирает слоты `discord-ipc-0..9` в каталоге `base_dir` и привязывается к первому свободному.
    /// Живые сокеты никогда не перезаписываются; осиротевшие сокеты текущего UID очищаются автоматически.
    pub fn bind_in_dir(base_dir: &Path) -> Result<Self, RpcIpcError> {
        let uid = effective_uid();
        validate_ipc_dir_permissions(base_dir, uid)?;
        for slot in 0..=MAX_IPC_SLOT {
            match Self::bind_slot_in_dir(base_dir, slot) {
                Ok(listener) => return Ok(listener),
                Err(RpcIpcError::UnsafeDirectory(dir)) => {
                    return Err(RpcIpcError::UnsafeDirectory(dir));
                }
                Err(_) => continue,
            }
        }
        Err(RpcIpcError::AllSlotsBusy)
    }

    /// Привязывает конкретный слот `discord-ipc-<slot>` (`0..=9`) внутри каталога `base_dir`.
    pub fn bind_slot_in_dir(base_dir: &Path, slot: u8) -> Result<Self, RpcIpcError> {
        if slot > MAX_IPC_SLOT {
            return Err(RpcIpcError::InvalidSlot(slot));
        }
        let path = base_dir.join(format!("{UNIX_SOCKET_PREFIX}{slot}"));
        Self::bind_path_with_slot(&path, Some(slot))
    }

    /// Привязывает слушатель к произвольному пути Unix-сокета.
    pub fn bind_path(path: &Path) -> Result<Self, RpcIpcError> {
        let slot = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| parse_ipc_slot(s).ok());
        Self::bind_path_with_slot(path, slot)
    }

    fn bind_path_with_slot(path: &Path, slot: Option<u8>) -> Result<Self, RpcIpcError> {
        let uid = effective_uid();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| RpcIpcError::UnsafeDirectory(path.display().to_string()))?;
        validate_ipc_dir_permissions(parent, uid)?;

        let listener = match UnixListener::bind(path) {
            Err(err)
                if err.kind() == std::io::ErrorKind::AddrInUse
                    && remove_stale_socket(path, uid) =>
            {
                UnixListener::bind(path).map_err(|e| RpcIpcError::IoError(e.to_string()))?
            }
            Ok(l) => l,
            Err(err) => return Err(RpcIpcError::IoError(err.to_string())),
        };

        let meta = fs::symlink_metadata(path).map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        let owned = Self {
            listener,
            path: path.to_path_buf(),
            slot,
            device: meta.dev(),
            inode: meta.ino(),
            uid,
        };
        fs::set_permissions(path, fs::Permissions::from_mode(RPC_SOCKET_MODE))
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        Ok(owned)
    }

    /// Возвращает путь к привязанному сокету.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Возвращает индекс слота `0..=9`, если сокет привязан к стандартному слоту `discord-ipc-0..9`.
    #[must_use]
    pub const fn slot(&self) -> Option<u8> {
        self.slot
    }

    /// Возвращает эффективный UID владельца слушателя.
    #[must_use]
    pub const fn uid(&self) -> u32 {
        self.uid
    }

    /// Возвращает идентификатор устройства (`st_dev`) созданного файла сокета.
    #[must_use]
    pub const fn device(&self) -> u64 {
        self.device
    }

    /// Возвращает номер inode (`st_ino`) созданного файла сокета.
    #[must_use]
    pub const fn inode(&self) -> u64 {
        self.inode
    }

    /// Форматирует строку запуска сервера по эталону RE `/home/sanya/re-deep/discord-run3.log:64`.
    #[must_use]
    pub fn format_startup_log(&self) -> String {
        format!("[RPCServer:IPC] Starting on {}", self.path.display())
    }

    /// Переключает неблокирующий режим сокета.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), RpcIpcError> {
        self.listener
            .set_nonblocking(nonblocking)
            .map_err(|e| RpcIpcError::IoError(e.to_string()))
    }

    /// Принимает входящее соединение и проверяет, что `SO_PEERCRED`/`getpeereid` UID совпадает с владельцем слушателя.
    pub fn accept(&self) -> Result<UnixStream, RpcIpcError> {
        self.accept_checked_with_expected_uid(self.uid)
    }

    /// Принимает входящее соединение и сверяет `SO_PEERCRED`/`getpeereid` UID с явно переданным `expected_uid`.
    pub fn accept_checked_with_expected_uid(
        &self,
        expected_uid: u32,
    ) -> Result<UnixStream, RpcIpcError> {
        let (stream, _) = self
            .listener
            .accept()
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        let actual_uid = peer_uid(&stream)?;
        if actual_uid != expected_uid {
            return Err(RpcIpcError::UnauthorizedPeer {
                peer_uid: actual_uid,
                expected_uid,
            });
        }
        Ok(stream)
    }
}

impl Drop for DiscordRpcListener {
    fn drop(&mut self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path)
            && meta.file_type().is_socket()
            && meta.dev() == self.device
            && meta.ino() == self.inode
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn validate_ipc_dir_permissions(dir: &Path, uid: u32) -> Result<(), RpcIpcError> {
    let meta = fs::symlink_metadata(dir).map_err(|e| RpcIpcError::IoError(e.to_string()))?;
    let trusted_owner = meta.uid() == uid || meta.uid() == 0;
    let world_writable_without_sticky = (meta.mode() & 0o002 != 0) && (meta.mode() & 0o1000 == 0);
    if !meta.is_dir() || !trusted_owner || world_writable_without_sticky {
        return Err(RpcIpcError::UnsafeDirectory(dir.display().to_string()));
    }
    Ok(())
}

fn remove_stale_socket(path: &Path, uid: u32) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.file_type().is_socket() || meta.uid() != uid {
        return false;
    }
    // Датаграммный зонд не попадает в очередь accept живого потокового слушателя,
    // но возвращает ConnectionRefused, если процесс-слушатель завершился.
    let is_dead = UnixDatagram::unbound()
        .and_then(|probe| probe.connect(path))
        .is_err_and(|err| err.kind() == std::io::ErrorKind::ConnectionRefused);
    is_dead && fs::remove_file(path).is_ok()
}

#[allow(unsafe_code)]
fn effective_uid() -> u32 {
    // SAFETY: POSIX geteuid не принимает аргументов и всегда возвращает эффективный UID процесса.
    unsafe { libc::geteuid() }
}

fn peer_uid(stream: &UnixStream) -> Result<u32, RpcIpcError> {
    use std::os::fd::AsRawFd;
    peer_uid_for_fd(stream.as_raw_fd())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(unsafe_code)] // This adapter contains only the checked native credential query.
fn peer_uid_for_fd(raw: libc::c_int) -> Result<u32, RpcIpcError> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: getsockopt с SOL_SOCKET/SO_PEERCRED заполняет выровненную структуру libc::ucred.
    let rc = unsafe {
        libc::getsockopt(
            raw,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast::<libc::c_void>(),
            &raw mut len,
        )
    };
    if rc != 0 || len as usize != std::mem::size_of::<libc::ucred>() {
        return Err(RpcIpcError::IoError(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(cred.uid)
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
#[allow(unsafe_code)] // This adapter contains only the checked native credential query.
fn peer_uid_for_fd(raw: libc::c_int) -> Result<u32, RpcIpcError> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: getpeereid only queries the socket identified by raw; an invalid fd
    // returns an OS error. Both output pointers reference live, aligned local values.
    let result = unsafe { libc::getpeereid(raw, &raw mut uid, &raw mut gid) };
    if result != 0 {
        return Err(RpcIpcError::IoError(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(uid)
}

/// Серверная машина состояний одной клиентской RPC IPC-сессии (`serve_ipc` в `Serein`).
#[derive(Debug, Clone, Default)]
pub struct RpcIpcSession {
    handshaked: bool,
    client_id: Option<String>,
    current_activity: Option<RpcActivity>,
    invites: Vec<String>,
    closed: bool,
}

impl RpcIpcSession {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn is_handshaked(&self) -> bool {
        self.handshaked
    }

    #[must_use]
    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    #[must_use]
    pub fn current_activity(&self) -> Option<&RpcActivity> {
        self.current_activity.as_ref()
    }

    #[must_use]
    pub fn invites(&self) -> &[String] {
        &self.invites
    }

    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Обрабатывает один входящий фрейм с учётом состояния рукопожатия сессии.
    pub fn process_frame(&mut self, frame: &IpcFrame) -> Result<RpcServerResponse, RpcIpcError> {
        if self.closed {
            return Err(RpcIpcError::NotConnected);
        }
        if !self.handshaked {
            if frame.opcode != RpcOpCode::Handshake {
                return Err(RpcIpcError::HandshakeRequired);
            }
            let client_id = parse_handshake_client_id(&frame.payload)?;
            let resp = handle_ipc_frame(frame)?;
            self.handshaked = true;
            self.client_id = Some(client_id);
            return Ok(resp);
        }

        match frame.opcode {
            RpcOpCode::Handshake => Err(RpcIpcError::InvalidField("opcode")),
            RpcOpCode::Frame => match handle_ipc_frame(frame) {
                Ok(resp) => {
                    if let Some(ref code) = resp.invite_code {
                        self.invites.push(code.clone());
                    } else {
                        self.current_activity = resp.activity.clone();
                    }
                    Ok(resp)
                }
                Err(_) => Ok(RpcServerResponse {
                    reply_frame: Some(build_rpc_error_frame_for_payload(&frame.payload)),
                    activity: None,
                    gateway_presence_update_json: None,
                    invite_code: None,
                    closed: false,
                }),
            },
            RpcOpCode::Ping | RpcOpCode::Pong => handle_ipc_frame(frame),
            RpcOpCode::Close => {
                let resp = handle_ipc_frame(frame)?;
                self.current_activity = None;
                self.closed = true;
                Ok(resp)
            }
        }
    }

    /// Обслуживает поток `UnixStream` от `Handshake` до `Close` или закрытия сокета клиентом.
    pub fn serve_stream(
        &mut self,
        stream: &mut UnixStream,
    ) -> Result<Vec<RpcServerResponse>, RpcIpcError> {
        let mut responses = Vec::new();
        loop {
            let frame = match DiscordRpcClient::read_frame(stream) {
                Ok(f) => f,
                Err(RpcIpcError::IoError(_)) if self.handshaked => {
                    self.current_activity = None;
                    self.closed = true;
                    break;
                }
                Err(err) => return Err(err),
            };
            let resp = self.process_frame(&frame)?;
            if let Some(ref reply) = resp.reply_frame {
                DiscordRpcClient::write_frame(stream, reply.opcode, &reply.payload)?;
            }
            let closed = resp.closed;
            responses.push(resp);
            if closed {
                break;
            }
        }
        Ok(responses)
    }
}

/// Реестр до `MAX_RPC_CLIENTS` (`8`) одновременных клиентских RPC-сессий с выбором
/// самой свежей опубликованной активности (`Serein/apps/desktop/src/game_activity.rs:L274-L384`).
#[derive(Debug, Clone)]
pub struct RpcMultiClientRegistry {
    slots: [bool; MAX_RPC_CLIENTS],
    values: [Option<(u64, RpcActivity)>; MAX_RPC_CLIENTS],
    seq: u64,
}

impl Default for RpcMultiClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcMultiClientRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [false; MAX_RPC_CLIENTS],
            values: std::array::from_fn(|_| None),
            seq: 0,
        }
    }

    /// Выделяет первый свободный слот клиента (`0..8`), либо возвращает `None`, если все 8 заняты.
    pub fn claim_slot(&mut self) -> Option<usize> {
        let idx = self.slots.iter().position(|used| !*used)?;
        self.slots[idx] = true;
        self.values[idx] = None;
        Some(idx)
    }

    /// Освобождает слот клиента и очищает его опубликованную активность.
    pub fn release_slot(&mut self, slot: usize) {
        if slot < MAX_RPC_CLIENTS {
            self.slots[slot] = false;
            self.values[slot] = None;
        }
    }

    /// Обновляет активность клиента в слоте `slot` и возвращает текущую приоритетную активность.
    pub fn update_slot_activity(
        &mut self,
        slot: usize,
        activity: Option<RpcActivity>,
    ) -> Result<Option<RpcActivity>, RpcIpcError> {
        if slot >= MAX_RPC_CLIENTS || !self.slots[slot] {
            return Err(RpcIpcError::InvalidSlot(slot as u8));
        }
        self.seq = self.seq.saturating_add(1);
        let stamp = self.seq;
        self.values[slot] = activity.map(|act| (stamp, act));
        Ok(self.active_activity().cloned())
    }

    /// Возвращает самую свежую непустую активность среди всех подключённых клиентов.
    #[must_use]
    pub fn active_activity(&self) -> Option<&RpcActivity> {
        self.values
            .iter()
            .flatten()
            .max_by_key(|(stamp, _)| *stamp)
            .map(|(_, act)| act)
    }

    /// Строит JSON-пакет Gateway Op 3 `PresenceUpdate` для текущей приоритетной активности.
    #[must_use]
    pub fn active_gateway_presence_json(&self) -> String {
        build_gateway_presence_update_json(self.active_activity())
    }
}

/// Клиент локального IPC-сокета Discord (`rust-discord-activity` совместимый).
#[derive(Debug)]
pub struct DiscordRpcClient {
    client_id: String,
    is_connected: bool,
    socket: Option<UnixStream>,
}

impl DiscordRpcClient {
    /// Создаёт новый неактивный клиент для заданного Application ID (`client_id`).
    #[must_use]
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            is_connected: false,
            socket: None,
        }
    }

    /// Создаёт клиент поверх уже открытого [`UnixStream`] (например, `UnixStream::pair()` в тестах).
    #[must_use]
    pub fn from_stream(client_id: impl Into<String>, stream: UnixStream) -> Self {
        Self {
            client_id: client_id.into(),
            is_connected: true,
            socket: Some(stream),
        }
    }

    /// Возвращает Application ID (`client_id`).
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Возвращает признак активного соединения.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Перебирает слоты `discord-ipc-0..9` в указанном каталоге `base_dir`, подключается к первому
    /// доступному слушателю и выполняет `Handshake`.
    pub fn connect_in_dir(&mut self, base_dir: &Path) -> Result<(u8, IpcFrame), RpcIpcError> {
        let mut last_err = RpcIpcError::NotConnected;
        for slot in 0..=MAX_IPC_SLOT {
            let path = base_dir.join(format!("{UNIX_SOCKET_PREFIX}{slot}"));
            match self.connect_to_path(&path) {
                Ok(ready) => return Ok((slot, ready)),
                Err(err) => last_err = err,
            }
        }
        Err(last_err)
    }

    /// Подключается к стандартному слоту `discord-ipc-<slot>` и выполняет `Handshake`.
    pub fn connect_slot(&mut self, slot: u8) -> Result<IpcFrame, RpcIpcError> {
        let path = resolve_ipc_socket_path(slot)?;
        self.connect_to_path(&path)
    }

    /// Подключается к указанному пути Unix-сокета и выполняет `Handshake`.
    pub fn connect_to_path(&mut self, path: &Path) -> Result<IpcFrame, RpcIpcError> {
        let stream = UnixStream::connect(path).map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        self.socket = Some(stream);
        match self.handshake() {
            Ok(frame) => {
                self.is_connected = true;
                Ok(frame)
            }
            Err(err) => {
                self.is_connected = false;
                self.socket = None;
                Err(err)
            }
        }
    }

    /// Отправляет фрейм `Handshake` (`OpCode = 0`, `{"v": 1, "client_id": ...}`) и читает ответ `READY`.
    pub fn handshake(&mut self) -> Result<IpcFrame, RpcIpcError> {
        let mut hs = BTreeMap::new();
        hs.insert("v".to_owned(), JsonVal::Uint(RPC_PROTOCOL_VERSION));
        hs.insert("client_id".to_owned(), JsonVal::Str(self.client_id.clone()));
        let payload = JsonVal::Obj(hs).to_json_string();
        self.send_and_recv(RpcOpCode::Handshake, &payload)
    }

    /// Отправляет команду `SET_ACTIVITY` (`OpCode = 1`) и читает ответ сервера.
    pub fn send_activity(&mut self, payload: &SetActivityPayload) -> Result<IpcFrame, RpcIpcError> {
        let json = payload.to_json();
        self.send_and_recv(RpcOpCode::Frame, &json)
    }

    /// Отправляет команду `INVITE_BROWSER` (`OpCode = 1`) и читает ответ сервера.
    pub fn send_invite_browser(
        &mut self,
        payload: &InviteBrowserPayload,
    ) -> Result<IpcFrame, RpcIpcError> {
        let json = payload.to_json();
        self.send_and_recv(RpcOpCode::Frame, &json)
    }

    /// Отправляет `Ping` (`OpCode = 3`) и читает ответный `Pong` (`OpCode = 4`).
    pub fn ping(&mut self, payload: &str) -> Result<IpcFrame, RpcIpcError> {
        self.send_and_recv(RpcOpCode::Ping, payload)
    }

    /// Отправляет фрейм закрытия `Close` (`OpCode = 2`) и закрывает клиентский сокет.
    pub fn close(&mut self, payload: &str) -> Result<(), RpcIpcError> {
        let sock = self.socket.as_mut().ok_or(RpcIpcError::NotConnected)?;
        let res = Self::write_frame(sock, RpcOpCode::Close, payload);
        self.is_connected = false;
        self.socket = None;
        res.map(|_| ())
    }

    /// Отправляет фрейм и синхронно ждёт ответного фрейма.
    pub fn send_and_recv(
        &mut self,
        opcode: RpcOpCode,
        payload: &str,
    ) -> Result<IpcFrame, RpcIpcError> {
        let sock = self.socket.as_mut().ok_or(RpcIpcError::NotConnected)?;
        Self::write_frame(sock, opcode, payload)?;
        Self::read_frame(sock)
    }

    /// Читает один бинарный фрейм из потока `Read`.
    pub fn read_frame(reader: &mut impl Read) -> Result<IpcFrame, RpcIpcError> {
        let mut header = [0u8; IPC_HEADER_LEN];
        reader
            .read_exact(&mut header)
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        let raw_opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if length > MAX_IPC_FRAME_BYTES {
            return Err(RpcIpcError::FrameTooLarge {
                length,
                max: MAX_IPC_FRAME_BYTES,
            });
        }
        let mut body = vec![0u8; length];
        reader
            .read_exact(&mut body)
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        let opcode = RpcOpCode::try_from(raw_opcode)?;
        let payload = String::from_utf8(body).map_err(|_| RpcIpcError::InvalidUtf8)?;
        Ok(IpcFrame { opcode, payload })
    }

    /// Кодирует и записывает один бинарный фрейм в поток `Write`.
    pub fn write_frame(
        writer: &mut impl Write,
        opcode: RpcOpCode,
        payload: &str,
    ) -> Result<usize, RpcIpcError> {
        let frame = encode_ipc_frame(opcode, payload)?;
        writer
            .write_all(&frame)
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| RpcIpcError::IoError(e.to_string()))?;
        Ok(frame.len())
    }
}

// --- Встроенный минимальный парсер/сериализатор JSON без внешних зависимостей ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum JsonVal {
    Null,
    Bool(bool),
    Uint(u64),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(BTreeMap<String, JsonVal>),
}

impl JsonVal {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Uint(n) => Some(*n),
            _ => None,
        }
    }

    fn as_obj(&self) -> Option<&BTreeMap<String, JsonVal>> {
        match self {
            Self::Obj(m) => Some(m),
            _ => None,
        }
    }

    fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.write_to(&mut out);
        out
    }

    fn write_to(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Uint(n) => out.push_str(&n.to_string()),
            Self::Str(s) => write_escaped_str(s, out),
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_to(out);
                }
                out.push(']');
            }
            Self::Obj(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped_str(k, out);
                    out.push(':');
                    v.write_to(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_escaped_str(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\u{20}' => {
                let b = c as u8;
                out.push_str(&format!("\\u00{b:02x}"));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn opt_string_field(
    obj: &BTreeMap<String, JsonVal>,
    key: &'static str,
) -> Result<Option<String>, RpcIpcError> {
    match obj.get(key) {
        Some(JsonVal::Str(s)) => Ok(Some(s.clone())),
        Some(JsonVal::Null) | None => Ok(None),
        _ => Err(RpcIpcError::InvalidField(key)),
    }
}

fn opt_u64_field(
    obj: &BTreeMap<String, JsonVal>,
    key: &'static str,
) -> Result<Option<u64>, RpcIpcError> {
    match obj.get(key) {
        Some(JsonVal::Uint(n)) => Ok(Some(*n)),
        Some(JsonVal::Null) | None => Ok(None),
        _ => Err(RpcIpcError::InvalidField(key)),
    }
}

fn parse_json(input: &str) -> Result<JsonVal, RpcIpcError> {
    let mut parser = JsonParser {
        input,
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let val = parser.parse_value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(RpcIpcError::InvalidJson("trailing data".to_owned()));
    }
    Ok(val)
}

struct JsonParser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl JsonParser<'_> {
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonVal, RpcIpcError> {
        if depth > 32 {
            return Err(RpcIpcError::InvalidJson("depth limit exceeded".to_owned()));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'n') => {
                if self.bytes[self.pos..].starts_with(b"null") {
                    self.pos += 4;
                    Ok(JsonVal::Null)
                } else {
                    Err(RpcIpcError::InvalidJson("invalid literal".to_owned()))
                }
            }
            Some(b't') => {
                if self.bytes[self.pos..].starts_with(b"true") {
                    self.pos += 4;
                    Ok(JsonVal::Bool(true))
                } else {
                    Err(RpcIpcError::InvalidJson("invalid literal".to_owned()))
                }
            }
            Some(b'f') => {
                if self.bytes[self.pos..].starts_with(b"false") {
                    self.pos += 5;
                    Ok(JsonVal::Bool(false))
                } else {
                    Err(RpcIpcError::InvalidJson("invalid literal".to_owned()))
                }
            }
            Some(b'"') => Ok(JsonVal::Str(self.parse_string()?)),
            Some(b'0'..=b'9') => Ok(JsonVal::Uint(self.parse_u64()?)),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            _ => Err(RpcIpcError::InvalidJson("unexpected token".to_owned())),
        }
    }

    fn parse_u64(&mut self) -> Result<u64, RpcIpcError> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        self.input[start..self.pos]
            .parse::<u64>()
            .map_err(|_| RpcIpcError::InvalidJson("invalid integer".to_owned()))
    }

    fn parse_string(&mut self) -> Result<String, RpcIpcError> {
        if self.peek() != Some(b'"') {
            return Err(RpcIpcError::InvalidJson("expected string".to_owned()));
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let b = self
                .peek()
                .ok_or_else(|| RpcIpcError::InvalidJson("unterminated string".to_owned()))?;
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self
                        .peek()
                        .ok_or_else(|| RpcIpcError::InvalidJson("bad escape".to_owned()))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .input
                                .get(self.pos..self.pos + 4)
                                .ok_or_else(|| RpcIpcError::InvalidJson("bad \\u".to_owned()))?;
                            let code = u16::from_str_radix(hex, 16)
                                .map_err(|_| RpcIpcError::InvalidJson("bad \\u hex".to_owned()))?;
                            self.pos += 4;
                            let ch = char::from_u32(u32::from(code))
                                .ok_or_else(|| RpcIpcError::InvalidJson("surrogate".to_owned()))?;
                            out.push(ch);
                        }
                        _ => return Err(RpcIpcError::InvalidJson("unknown escape".to_owned())),
                    }
                }
                0x00..=0x1f => {
                    return Err(RpcIpcError::InvalidJson(
                        "unescaped control char".to_owned(),
                    ));
                }
                _ => {
                    let ch = self.input[self.pos..]
                        .chars()
                        .next()
                        .ok_or_else(|| RpcIpcError::InvalidJson("eof".to_owned()))?;
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<JsonVal, RpcIpcError> {
        self.pos += 1; // '['
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonVal::Arr(items));
                }
                _ => return Err(RpcIpcError::InvalidJson("expected ',' or ']'".to_owned())),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonVal, RpcIpcError> {
        self.pos += 1; // '{'
        self.skip_ws();
        let mut map = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Obj(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(RpcIpcError::InvalidJson("expected ':'".to_owned()));
            }
            self.pos += 1;
            let val = self.parse_value(depth + 1)?;
            map.insert(key, val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonVal::Obj(map));
                }
                _ => return Err(RpcIpcError::InvalidJson("expected ',' or '}'".to_owned())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_uid_query_matches_effective_uid_oracle() {
        use std::os::fd::AsRawFd;
        use std::process::Command;

        let output = Command::new("id").arg("-u").output().expect("id -u");
        assert!(output.status.success(), "id -u must succeed");
        let expected: u32 = String::from_utf8(output.stdout)
            .expect("id output is UTF-8")
            .trim()
            .parse()
            .expect("numeric effective UID");
        let (stream, _peer) = UnixStream::pair().expect("socketpair");
        assert_eq!(
            peer_uid_for_fd(stream.as_raw_fd()).expect("peer UID"),
            expected
        );
    }

    #[test]
    fn peer_uid_query_rejects_invalid_descriptor() {
        assert!(matches!(
            peer_uid_for_fd(-1),
            Err(RpcIpcError::IoError(message)) if !message.is_empty()
        ));
    }

    #[test]
    fn encode_and_decode_ipc_frame_roundtrip_and_bounds() {
        let json = r#"{"v":1,"client_id":"123456789012345678"}"#;
        let encoded = encode_ipc_frame(RpcOpCode::Handshake, json).unwrap();
        assert_eq!(&encoded[0..4], &0u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(json.len() as u32).to_le_bytes());
        assert_eq!(&encoded[8..], json.as_bytes());

        let decoded = decode_ipc_frame(&encoded).unwrap();
        assert_eq!(decoded.opcode, RpcOpCode::Handshake);
        assert_eq!(decoded.payload, json);

        // Короткий заголовок и усечённое тело отклоняются
        assert_eq!(
            decode_ipc_frame(&[0, 1, 2]),
            Err(RpcIpcError::HeaderTooShort(3))
        );
        assert_eq!(
            decode_ipc_frame(&encoded[..10]),
            Err(RpcIpcError::TruncatedPayload {
                expected: json.len(),
                actual: 2
            })
        );
        // Неизвестный opcode отклоняется
        let mut bad_op = encoded.clone();
        bad_op[0..4].copy_from_slice(&99u32.to_le_bytes());
        assert_eq!(
            decode_ipc_frame(&bad_op),
            Err(RpcIpcError::InvalidOpCode(99))
        );

        // Слишком большой фрейм (> 16 KiB) отклоняется
        let huge = "a".repeat(MAX_IPC_FRAME_BYTES + 1);
        assert!(matches!(
            encode_ipc_frame(RpcOpCode::Frame, &huge),
            Err(RpcIpcError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn resolve_ipc_socket_path_checks_env_priority_and_validates_slot() {
        let p1 = resolve_ipc_socket_path_from_env(
            0,
            &[
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
                ("TMPDIR", "/custom/tmp"),
            ],
        )
        .unwrap();
        assert_eq!(p1, PathBuf::from("/run/user/1000/discord-ipc-0"));

        let p2 = resolve_ipc_socket_path_from_env(5, &[("TMPDIR", "/var/tmp")]).unwrap();
        assert_eq!(p2, PathBuf::from("/var/tmp/discord-ipc-5"));

        let p3 = resolve_ipc_socket_path_from_env(9, &[]).unwrap();
        assert_eq!(p3, PathBuf::from("/tmp/discord-ipc-9"));

        assert_eq!(
            resolve_ipc_socket_path_from_env(10, &[]),
            Err(RpcIpcError::InvalidSlot(10))
        );
        assert!(resolve_ipc_socket_path(0).is_ok());
        assert_eq!(
            resolve_ipc_socket_path(10),
            Err(RpcIpcError::InvalidSlot(10))
        );
    }

    #[test]
    fn handle_ipc_frame_handshake_and_set_activity_and_client_stream_roundtrip() {
        // 1. Handshake v=1 -> READY
        let hs = IpcFrame {
            opcode: RpcOpCode::Handshake,
            payload: r#"{"v":1,"client_id":"424242"}"#.to_owned(),
        };
        let hs_resp = handle_ipc_frame(&hs).unwrap();
        let ready = hs_resp.reply_frame.expect("READY reply expected");
        assert_eq!(ready.opcode, RpcOpCode::Frame);
        assert!(ready.payload.contains(r#""cmd":"DISPATCH""#));
        assert!(ready.payload.contains(r#""evt":"READY""#));

        // Handshake v=2 -> UnsupportedVersion
        let bad_hs = IpcFrame {
            opcode: RpcOpCode::Handshake,
            payload: r#"{"v":2,"client_id":"424242"}"#.to_owned(),
        };
        assert_eq!(
            handle_ipc_frame(&bad_hs),
            Err(RpcIpcError::UnsupportedVersion(2))
        );

        // 2. SET_ACTIVITY -> распарсенная активность + Gateway Op 3 PresenceUpdate
        let activity = RpcActivity::new()
            .with_name("Rust Arena")
            .with_details("Ranked Match")
            .with_state("In Lobby")
            .with_timestamps(RpcTimestamps::new(
                Some(1_700_000_000_000),
                Some(1_700_003_600_000),
            ))
            .with_assets(RpcAssets::new(
                Some("map_arena".to_owned()),
                Some("Arena Map".to_owned()),
                Some("rank_gold".to_owned()),
                Some("Gold".to_owned()),
            ))
            .with_party(RpcParty::new(Some("party-1".to_owned()), Some([2, 4])))
            .with_buttons(vec![RpcButton::new(
                "Join",
                "https://discord.com/invite/rust",
            )])
            .with_instance(true);

        let set_payload = SetActivityPayload::new(4321, Some(activity.clone()), "nonce-abc-123");
        let cmd_json = set_payload.to_json();
        let set_frame = IpcFrame {
            opcode: RpcOpCode::Frame,
            payload: cmd_json,
        };
        let set_resp = handle_ipc_frame(&set_frame).unwrap();
        assert_eq!(set_resp.activity, Some(activity.clone()));
        let op3 = set_resp
            .gateway_presence_update_json
            .expect("Gateway Op 3 JSON must be generated");
        assert!(op3.contains(r#""op":3"#));
        assert!(op3.contains("Ranked Match"));
        assert!(op3.contains("In Lobby"));

        // Ping -> Pong, Close -> closed = true
        let ping_resp = handle_ipc_frame(&IpcFrame {
            opcode: RpcOpCode::Ping,
            payload: r#"{"ping":1}"#.to_owned(),
        })
        .unwrap();
        assert_eq!(
            ping_resp.reply_frame,
            Some(IpcFrame {
                opcode: RpcOpCode::Pong,
                payload: r#"{"ping":1}"#.to_owned()
            })
        );
        let close_resp = handle_ipc_frame(&IpcFrame {
            opcode: RpcOpCode::Close,
            payload: "{}".to_owned(),
        })
        .unwrap();
        assert!(close_resp.closed);

        // 3. Полный roundtrip через UnixStream::pair между DiscordRpcClient и серверным обработчиком
        let (client_sock, mut server_sock) = UnixStream::pair().unwrap();
        let mut client = DiscordRpcClient::from_stream("424242", client_sock);
        let handle = std::thread::spawn(move || {
            let hs_in = DiscordRpcClient::read_frame(&mut server_sock).unwrap();
            let hs_out = handle_ipc_frame(&hs_in).unwrap().reply_frame.unwrap();
            DiscordRpcClient::write_frame(&mut server_sock, hs_out.opcode, &hs_out.payload)
                .unwrap();

            let act_in = DiscordRpcClient::read_frame(&mut server_sock).unwrap();
            let act_res = handle_ipc_frame(&act_in).unwrap();
            let act_out = act_res.reply_frame.unwrap();
            DiscordRpcClient::write_frame(&mut server_sock, act_out.opcode, &act_out.payload)
                .unwrap();
            act_res.activity
        });

        let ready_reply = client.handshake().unwrap();
        assert!(ready_reply.payload.contains("READY"));
        let ack_reply = client.send_activity(&set_payload).unwrap();
        assert!(ack_reply.payload.contains("nonce-abc-123"));
        let server_parsed = handle.join().unwrap();
        assert_eq!(server_parsed, Some(activity));
    }
}
