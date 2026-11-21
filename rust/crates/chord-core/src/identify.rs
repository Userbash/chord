#![forbid(unsafe_code)]

//! # IDENTIFY (op2): сборка терма идентификации и генератор handshake (RW-05, P1-04 / STAGE-05.3.1)
//!
//! Структура — из кода реального веб-клиента Discord (бандл
//! `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`):
//! `_doIdentify` (~5017678) собирает `{token, capabilities, properties, presence,
//! compress, client_state, qos_token}`, `handleIdentify` (~4907630) — свойства.
//!
//! Кодирование — как у клиентского ETF-энкодера (wetf, ~4961704):
//! `encoding:{string:"binary", key:"binary", array:"list", null:"atom"}` — поэтому строки и
//! ключи map здесь [`EtfValue::Str`], массивы — [`EtfValue::List`], `null` — [`EtfValue::Nil`],
//! булевы значения — атомы `"true"`/`"false"`.

use chord_codec::etf::{EtfError, EtfValue};

use crate::rest::headers::{BrowserImpersonationProfile, DEFAULT_LOCALE, format_accept_language};
use crate::rest::identity::ClientIdentity;
use crate::rest::super_properties::SuperProperties;

/// Opcode 2: `IDENTIFY` (`docs/re/gateway-protocol.md` §4).
pub const IDENTIFY_OP: u8 = 2;

/// capabilities активного клиента (без обфускации приватных каналов) — из бандла.
pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;

/// capabilities с фичей `private_channel_obfuscation` — из бандла.
pub const CAPABILITIES_CHANNEL_OBFUSCATION: u64 = 1_767_421;

/// Причина подключения при холодном старте (модуль 469177 бандла: `r=true`).
const GATEWAY_CONNECT_COLD_START: &str = "COLD_START";

/// Дефолтная таймзона для системных заголовков handshake.
const DEFAULT_TIMEZONE: &str = "UTC";

/// Дефолтное значение `X-Debug-Options` в официальных сборках Discord Desktop.
const DEFAULT_DEBUG_OPTIONS: &str = "bugReporterEnabled";

/// Супер-свойства клиента плюс поля идентификации (см. `handleIdentify` в бандле и ТЗ).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifyProperties {
    /// ОС: у веб-клиента `getOs()` отображает `linux` → `"Linux"`.
    pub os: String,
    /// Тип клиента: `"Discord Client"`.
    pub browser: String,
    /// Канал выпуска: `"stable"`.
    pub release_channel: String,
    /// Версия клиента (у веб-клиента — `"1.0.162"`).
    pub client_version: String,
    /// Версия ОС.
    pub os_version: String,
    /// Архитектура ОС (`"x64"`, `"arm64"`).
    pub os_arch: String,
    /// Архитектура приложения (`"x64"`, `"arm64"`).
    pub app_arch: String,
    /// Системная локаль.
    pub system_locale: String,
    /// Модификации клиента (у нас — `false`).
    pub has_client_mods: bool,
    /// Идентификатор запуска клиента (у веб-клиента — UUID).
    pub client_launch_id: String,
    /// UUID подписи запуска: в клиенте — то же значение, что уходит в `X-Super-Properties`.
    pub launch_signature: String,
    /// User-Agent браузерного движка (у веб-клиента — Electron).
    pub browser_user_agent: String,
    /// Версия браузерного движка.
    pub browser_version: String,
    /// 64-битный номер сборки клиента (в бандле — `parseInt("641004")`).
    pub client_build_number: u64,
    /// Состояние окна: `"focused"` либо `"unfocused"`.
    pub client_app_state: String,
    /// Быстрое подключение (у обычного логина — `false`).
    pub is_fast_connect: bool,
    /// Причины коннекта (в бандле — строка из модуля 469177: `"COLD_START"` и т.п.).
    pub gateway_connect_reasons: String,
    /// ID установки — ключ появляется, только если значение есть.
    pub installation_id: Option<String>,
}

impl IdentifyProperties {
    /// Дефолты нашего клиента (Linux, `"Discord Client"`, stable, …) — по ТЗ RW-05.
    #[must_use]
    pub fn for_rw_client() -> Self {
        Self::from_client_identity(&ClientIdentity::rw_client())
    }

    /// IDENTIFY-вид единой идентичности [`ClientIdentity`].
    #[must_use]
    pub fn from_client_identity(identity: &ClientIdentity) -> Self {
        Self::from_super_properties(identity.super_properties())
    }

    /// Единый источник свойств: IDENTIFY берёт те же значения, что и `X-Super-Properties`.
    #[must_use]
    pub fn from_super_properties(properties: &SuperProperties) -> Self {
        Self {
            os: properties.os.clone(),
            browser: properties.browser.clone(),
            release_channel: properties.release_channel.clone(),
            client_version: properties.client_version.clone(),
            os_version: properties.os_version.clone(),
            os_arch: properties.os_arch.clone(),
            app_arch: properties.app_arch.clone(),
            system_locale: properties.system_locale.clone(),
            has_client_mods: properties.has_client_mods,
            client_launch_id: properties.client_launch_id.clone(),
            launch_signature: properties.launch_signature.clone(),
            browser_user_agent: properties.browser_user_agent.clone(),
            browser_version: properties.browser_version.clone(),
            client_build_number: u64::from(properties.client_build_number),
            client_app_state: properties.client_app_state.clone(),
            is_fast_connect: false,
            gateway_connect_reasons: GATEWAY_CONNECT_COLD_START.to_string(),
            installation_id: None,
        }
    }

    /// Проверяет, что архитектура ОС и приложения соответствует 64-битной платформе (`x64`/`x86_64`/`arm64`/`aarch64`).
    #[must_use]
    pub fn is_64bit_client(&self) -> bool {
        let is_64 = |arch: &str| matches!(arch, "x64" | "x86_64" | "arm64" | "aarch64");
        is_64(&self.os_arch) && is_64(&self.app_arch) && self.client_build_number > 0
    }
}

impl ClientIdentity {
    /// Свойства OP2 IDENTIFY из той же идентичности, что уходит в `X-Super-Properties`.
    #[must_use]
    pub fn to_identify_properties(&self) -> IdentifyProperties {
        IdentifyProperties::from_client_identity(self)
    }
}

/// 64-битное состояние клиента (`client_state`) в OP2 IDENTIFY с сохранением Snowflake ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentifyClientState {
    /// Список пар `(guild_snowflake_id, guild_version)` с полными 64-битными Snowflake ID.
    guild_versions: Vec<(u64, u64)>,
    /// Опциональный 64-битный Snowflake ID последнего прочитанного сообщения (`highest_last_message_id`).
    highest_last_message_id: Option<u64>,
}

impl IdentifyClientState {
    /// Создаёт пустое состояние `client_state` (`{guild_versions: {}}`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавляет 64-битный Snowflake ID гильдии и её версию в `guild_versions`.
    #[must_use]
    pub fn with_guild_version(mut self, guild_id: u64, version: u64) -> Self {
        self.guild_versions.push((guild_id, version));
        self
    }

    /// Задаёт 64-битный Snowflake ID `highest_last_message_id`.
    #[must_use]
    pub fn with_highest_last_message_id(mut self, message_id: u64) -> Self {
        self.highest_last_message_id = Some(message_id);
        self
    }

    /// Возвращает срез сохранённых 64-битных пар `(guild_id, version)`.
    #[must_use]
    pub fn guild_versions(&self) -> &[(u64, u64)] {
        &self.guild_versions
    }

    /// Возвращает сохранённый 64-битный Snowflake `highest_last_message_id`.
    #[must_use]
    pub const fn highest_last_message_id(&self) -> Option<u64> {
        self.highest_last_message_id
    }

    /// Проверяет валидность Snowflake ID (нулевые Snowflake ID запрещены контрактом).
    pub fn validate(&self) -> Result<(), IdentifyError> {
        if self.guild_versions.iter().any(|(gid, _)| *gid == 0) {
            return Err(IdentifyError::ZeroSnowflakeId);
        }
        if self.highest_last_message_id == Some(0) {
            return Err(IdentifyError::ZeroSnowflakeId);
        }
        Ok(())
    }

    /// Конвертирует 64-битные Snowflake ID гильдий в строковые пары без потери точности.
    #[must_use]
    pub fn to_string_pairs(&self) -> Vec<(String, String)> {
        self.guild_versions
            .iter()
            .map(|(gid, ver)| (gid.to_string(), ver.to_string()))
            .collect()
    }
}

/// Системные заголовки клиента, синхронизированные с [`IdentifyProperties`] и [`SuperProperties`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifySystemHeaders {
    /// Заголовок `User-Agent` (совпадает с `properties.browser_user_agent`).
    pub user_agent: String,
    /// Заголовок `X-Super-Properties` (Base64 от 22 полей [`SuperProperties`]).
    pub super_properties_base64: String,
    /// Заголовок `X-Discord-Locale` (совпадает с `properties.system_locale`).
    pub discord_locale: String,
    /// Заголовок `X-Discord-Timezone`.
    pub discord_timezone: String,
    /// Заголовок `X-Debug-Options`.
    pub debug_options: String,
    /// Заголовок `Accept-Language`.
    pub accept_language: String,
    /// Заголовок `Origin`.
    pub origin: String,
}

impl IdentifySystemHeaders {
    /// Собирает системные заголовки из [`ClientIdentity`], гарантируя 1:1 совпадение с OP2 IDENTIFY.
    #[must_use]
    pub fn from_client_identity(identity: &ClientIdentity) -> Self {
        Self::from_super_properties(identity.super_properties())
    }

    /// Собирает системные заголовки из [`SuperProperties`].
    #[must_use]
    pub fn from_super_properties(sp: &SuperProperties) -> Self {
        let profile = BrowserImpersonationProfile::desktop_default();
        let locale = if sp.system_locale.trim().is_empty() {
            DEFAULT_LOCALE
        } else {
            sp.system_locale.as_str()
        };
        let user_agent = if sp.browser_user_agent.trim().is_empty() {
            profile.user_agent
        } else {
            sp.browser_user_agent.clone()
        };
        Self {
            user_agent,
            super_properties_base64: sp.to_base64(),
            discord_locale: locale.to_string(),
            discord_timezone: DEFAULT_TIMEZONE.to_string(),
            debug_options: DEFAULT_DEBUG_OPTIONS.to_string(),
            accept_language: format_accept_language(&[locale]),
            origin: profile.origin,
        }
    }

    /// Возвращает массив пар `(имя_заголовка, значение)` для HTTP/WS handshake.
    #[must_use]
    pub fn to_header_pairs(&self) -> [(&'static str, &str); 7] {
        [
            ("User-Agent", &self.user_agent),
            ("X-Super-Properties", &self.super_properties_base64),
            ("X-Discord-Locale", &self.discord_locale),
            ("X-Discord-Timezone", &self.discord_timezone),
            ("X-Debug-Options", &self.debug_options),
            ("Accept-Language", &self.accept_language),
            ("Origin", &self.origin),
        ]
    }
}

/// Результат генерации OP2 IDENTIFY handshake с сохранением 64-битных Snowflake ID и системных заголовков.
#[derive(Clone, PartialEq, Eq)]
pub struct IdentifyHandshakeFrame {
    /// Код операции шлюза (`IDENTIFY_OP == 2`).
    pub op: u8,
    /// 64-битная битовая маска возможностей клиента.
    pub capabilities: u64,
    /// 64-битные свойства клиента, отправленные в кадре `d.properties`.
    pub properties: IdentifyProperties,
    /// Синхронизированные системные заголовки (`X-Super-Properties`, `User-Agent`, `X-Discord-Locale` и др.).
    pub system_headers: IdentifySystemHeaders,
    /// Сохранённые 64-битные Snowflake ID гильдий и их версии.
    guild_snowflakes: Vec<(u64, u64)>,
    /// Сохранённый 64-битный Snowflake ID `highest_last_message_id`.
    pub highest_last_message_id: Option<u64>,
    /// Готовый бинарный ETF-кадр `{op: 2, d: ...}` с ведущим байтом `0x83`.
    pub etf_frame: Vec<u8>,
    /// Только внутренний ETF-терм `d` с ведущим байтом `0x83`.
    pub etf_payload: Vec<u8>,
}

impl IdentifyHandshakeFrame {
    /// Возвращает сохранённые 64-битные Snowflake ID гильдий `(guild_id, version)`.
    #[must_use]
    pub fn preserved_guild_snowflakes(&self) -> &[(u64, u64)] {
        &self.guild_snowflakes
    }
}

impl std::fmt::Debug for IdentifyHandshakeFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentifyHandshakeFrame")
            .field("op", &self.op)
            .field("token", &"REDACTED")
            .field("capabilities", &self.capabilities)
            .field("properties", &self.properties)
            .field("system_headers", &self.system_headers)
            .field("guild_snowflakes", &self.guild_snowflakes)
            .field("highest_last_message_id", &self.highest_last_message_id)
            .field("etf_frame_len", &self.etf_frame.len())
            .finish()
    }
}

/// Генератор payload Identify (Opcode 2) с 64-битными свойствами клиента, сохранением Snowflake ID
/// и синхронизацией системных заголовков (`STAGE-05.3.1`).
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifyHandshakeGenerator {
    super_properties: SuperProperties,
    properties: IdentifyProperties,
    system_headers: IdentifySystemHeaders,
    capabilities: u64,
    presence: Option<PresenceInfo>,
    compress: bool,
    client_state: IdentifyClientState,
    qos_active: bool,
}

impl IdentifyHandshakeGenerator {
    /// Создаёт генератор из единой идентичности [`ClientIdentity`].
    #[must_use]
    pub fn from_client_identity(identity: &ClientIdentity) -> Self {
        Self::from_super_properties(identity.super_properties().clone())
    }

    /// Создаёт генератор из [`SuperProperties`].
    #[must_use]
    pub fn from_super_properties(sp: SuperProperties) -> Self {
        let properties = IdentifyProperties::from_super_properties(&sp);
        let system_headers = IdentifySystemHeaders::from_super_properties(&sp);
        Self {
            super_properties: sp,
            properties,
            system_headers,
            capabilities: CAPABILITIES_ACTIVE,
            presence: None,
            compress: false,
            client_state: IdentifyClientState::new(),
            qos_active: false,
        }
    }

    /// Задаёт 64-битную маску `capabilities`.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: u64) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Задаёт состояние `client_state` с 64-битными Snowflake ID.
    #[must_use]
    pub fn with_client_state(mut self, client_state: IdentifyClientState) -> Self {
        self.client_state = client_state;
        self
    }

    /// Задаёт начальное присутствие (`presence`).
    #[must_use]
    pub fn with_presence(mut self, presence: PresenceInfo) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Задаёт таймзону в системных заголовках (`X-Discord-Timezone`).
    #[must_use]
    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Self {
        self.system_headers.discord_timezone = timezone.into();
        self
    }

    /// Задаёт значение `X-Debug-Options` в системных заголовках.
    #[must_use]
    pub fn with_debug_options(mut self, debug_options: impl Into<String>) -> Self {
        self.system_headers.debug_options = debug_options.into();
        self
    }

    /// Генерирует полный пакет OP2 IDENTIFY handshake с самопроверкой ETF-декодирования.
    pub fn generate_handshake(&self, token: &str) -> Result<IdentifyHandshakeFrame, IdentifyError> {
        self.client_state.validate()?;
        let params = IdentifyParams {
            token: token.to_string(),
            capabilities: self.capabilities,
            properties: self.properties.clone(),
            presence: self.presence.clone(),
            compress: self.compress,
            client_state_guild_versions: self.client_state.to_string_pairs(),
            qos_token: qos_token_client_provided(self.qos_active),
        };
        let payload_val =
            identify_value_with_msg_id(&params, self.client_state.highest_last_message_id())?;
        let etf_payload = self_check(chord_codec::etf::encode(&payload_val))?;
        let frame_val = EtfValue::Map(vec![
            (key("op"), EtfValue::Int(i64::from(IDENTIFY_OP))),
            (key("d"), payload_val),
        ]);
        let etf_frame = self_check(chord_codec::etf::encode(&frame_val))?;
        Ok(IdentifyHandshakeFrame {
            op: IDENTIFY_OP,
            capabilities: self.capabilities,
            properties: self.properties.clone(),
            system_headers: self.system_headers.clone(),
            guild_snowflakes: self.client_state.guild_versions().to_vec(),
            highest_last_message_id: self.client_state.highest_last_message_id(),
            etf_frame,
            etf_payload,
        })
    }
}

/// Присутствие из IDENTIFY: `{status, since, activities, afk}` (как `getLocalPresence()`).
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceInfo {
    /// Статус (`"online"`, `"idle"`, …).
    pub status: String,
    /// Время смены статуса (0 — не менять).
    pub since: u64,
    /// Активности (для P1 обычно пусто).
    pub activities: Vec<EtfValue>,
    /// AFK-режим.
    pub afk: bool,
}

/// Параметры IDENTIFY (op2).
#[derive(Clone)]
pub struct IdentifyParams {
    /// Токен пользователя: **никогда** не попадает в `Debug`/ошибки (см. `impl Debug`).
    pub token: String,
    /// Битовая маска возможностей ([`CAPABILITIES_ACTIVE`]/[`CAPABILITIES_CHANNEL_OBFUSCATION`]).
    pub capabilities: u64,
    /// Свойства идентификации.
    pub properties: IdentifyProperties,
    /// Присутствие; `None` → `Nil` в терме.
    pub presence: Option<PresenceInfo>,
    /// Сжатие последующих кадров (zstd-поток не legacy → `false`).
    pub compress: bool,
    /// Версии гильдий (минимум — пусто, `{guild_versions: {}}`).
    pub client_state_guild_versions: Vec<(String, String)>,
    /// Protobuf-байты `QosToken` (минимум — [`qos_token_client_provided`]).
    pub qos_token: Vec<u8>,
}

impl std::fmt::Debug for IdentifyParams {
    /// Токен заменяется на `"REDACTED"`; QoS-байты показываются только длиной.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentifyParams")
            .field("token", &"REDACTED")
            .field("capabilities", &self.capabilities)
            .field("properties", &self.properties)
            .field("presence", &self.presence)
            .field("compress", &self.compress)
            .field(
                "client_state_guild_versions",
                &self.client_state_guild_versions,
            )
            .field("qos_token_len", &self.qos_token.len())
            .finish()
    }
}

/// Ошибки сборки IDENTIFY: ни одна не является паникой.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifyError {
    /// Пустой (или состоящий из пробелов) токен — IDENTIFY не уходит.
    EmptyToken,
    /// `qos_token` не строгий UTF-8 (бинарный protobuf не представим строкой ETF).
    QosTokenNotUtf8,
    /// Нулевой 64-битный Snowflake ID в `client_state`.
    ZeroSnowflakeId,
    /// Собранный терм не разбирается кодеком (самопроверка сборки).
    Encode(EtfError),
}

impl std::fmt::Display for IdentifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyToken => write!(f, "пустой токен"),
            Self::QosTokenNotUtf8 => write!(f, "qos_token не является строгим UTF-8"),
            Self::ZeroSnowflakeId => write!(f, "нулевой Snowflake ID недопустим в client_state"),
            Self::Encode(error) => {
                write!(f, "собранный IDENTIFY не разбирается ETF-кодеком: {error}")
            }
        }
    }
}

impl std::error::Error for IdentifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::EmptyToken | Self::QosTokenNotUtf8 | Self::ZeroSnowflakeId => None,
        }
    }
}

/// Минимальное protobuf-кодирование `QosToken{client_provided{is_active}}`.
#[must_use]
pub fn qos_token_client_provided(is_active: bool) -> Vec<u8> {
    vec![0x0A, 0x02, 0x08, u8::from(is_active)]
}

/// Ключ map в форме клиентского wetf-энкодера (`key:"binary"`).
fn key(name: &str) -> EtfValue {
    EtfValue::Str(name.to_string())
}

/// Булево значение в форме ETF-атома (`true`/`false`).
fn bool_atom(flag: bool) -> EtfValue {
    EtfValue::Atom(if flag { "true" } else { "false" }.to_string())
}

/// `u64` → `i64` с насыщением (значения протокола не превышают `i64::MAX`).
fn int(value: u64) -> EtfValue {
    EtfValue::Int(i64::try_from(value).unwrap_or(i64::MAX))
}

fn str_pair(name: &str, val: &str) -> (EtfValue, EtfValue) {
    (key(name), EtfValue::Str(val.to_string()))
}

/// Свойства идентификации как map (порядок — супер-свойства, затем поля `handleIdentify`).
fn properties_value(p: &IdentifyProperties) -> EtfValue {
    let mut pairs = vec![
        str_pair("os", &p.os),
        str_pair("browser", &p.browser),
        str_pair("release_channel", &p.release_channel),
        str_pair("client_version", &p.client_version),
        str_pair("os_version", &p.os_version),
        str_pair("os_arch", &p.os_arch),
        str_pair("app_arch", &p.app_arch),
        str_pair("system_locale", &p.system_locale),
        (key("has_client_mods"), bool_atom(p.has_client_mods)),
        str_pair("client_launch_id", &p.client_launch_id),
        str_pair("launch_signature", &p.launch_signature),
        str_pair("browser_user_agent", &p.browser_user_agent),
        str_pair("browser_version", &p.browser_version),
        (key("client_build_number"), int(p.client_build_number)),
        str_pair("client_app_state", &p.client_app_state),
        (key("is_fast_connect"), bool_atom(p.is_fast_connect)),
        str_pair("gateway_connect_reasons", &p.gateway_connect_reasons),
    ];
    if let Some(id) = &p.installation_id {
        pairs.push(str_pair("installation_id", id));
    }
    EtfValue::Map(pairs)
}

/// Присутствие как map `{status, since, activities, afk}`.
fn presence_value(p: &PresenceInfo) -> EtfValue {
    EtfValue::Map(vec![
        (key("status"), EtfValue::Str(p.status.clone())),
        (key("since"), int(p.since)),
        (key("activities"), EtfValue::List(p.activities.clone())),
        (key("afk"), bool_atom(p.afk)),
    ])
}

/// Собирает **payload** IDENTIFY (ровно поля `_doIdentify`) с ведущим `0x83`; проводная форма —
/// [`build_identify_frame`] (`{op: 2, d: payload}`).
pub fn build_identify(p: &IdentifyParams) -> Result<Vec<u8>, IdentifyError> {
    self_check(chord_codec::etf::encode(&identify_value(p)?))
}

/// Кадр IDENTIFY (op2) в форме провода: `{op: 2, d: <терм build_identify>}` с ведущим `0x83`.
pub fn build_identify_frame(p: &IdentifyParams) -> Result<Vec<u8>, IdentifyError> {
    let frame = EtfValue::Map(vec![
        (key("op"), EtfValue::Int(i64::from(IDENTIFY_OP))),
        (key("d"), identify_value(p)?),
    ]);
    self_check(chord_codec::etf::encode(&frame))
}

/// Значение payload IDENTIFY (без внешнего кадра `{op, d}`).
fn identify_value(p: &IdentifyParams) -> Result<EtfValue, IdentifyError> {
    identify_value_with_msg_id(p, None)
}

/// Значение payload IDENTIFY с опциональным 64-битным `highest_last_message_id` в `client_state`.
fn identify_value_with_msg_id(
    p: &IdentifyParams,
    highest_last_message_id: Option<u64>,
) -> Result<EtfValue, IdentifyError> {
    if p.token.trim().is_empty() {
        return Err(IdentifyError::EmptyToken);
    }
    let qos_token =
        String::from_utf8(p.qos_token.clone()).map_err(|_| IdentifyError::QosTokenNotUtf8)?;
    let guild_versions = EtfValue::Map(
        p.client_state_guild_versions
            .iter()
            .map(|(gid, ver)| (EtfValue::Str(gid.clone()), EtfValue::Str(ver.clone())))
            .collect(),
    );
    let mut cs_pairs = vec![(key("guild_versions"), guild_versions)];
    if let Some(msg_id) = highest_last_message_id {
        cs_pairs.push((
            key("highest_last_message_id"),
            EtfValue::Str(msg_id.to_string()),
        ));
    }
    let client_state = EtfValue::Map(cs_pairs);
    let presence = match &p.presence {
        Some(presence) => presence_value(presence),
        None => EtfValue::Nil,
    };
    Ok(EtfValue::Map(vec![
        (key("token"), EtfValue::Str(p.token.clone())),
        (key("capabilities"), int(p.capabilities)),
        (key("properties"), properties_value(&p.properties)),
        (key("presence"), presence),
        (key("compress"), bool_atom(p.compress)),
        (key("client_state"), client_state),
        (key("qos_token"), EtfValue::Str(qos_token)),
    ]))
}

/// Самопроверка сборки: терм обязан разбираться кодеком целиком.
fn self_check(bytes: Vec<u8>) -> Result<Vec<u8>, IdentifyError> {
    let (_decoded, consumed) = chord_codec::etf::decode(&bytes).map_err(IdentifyError::Encode)?;
    if consumed != bytes.len() {
        return Err(IdentifyError::Encode(
            chord_codec::etf::EtfError::UnexpectedEof {
                at: consumed,
                need: bytes.len() - consumed,
            },
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_codec::etf::decode;

    fn distinct_super_properties() -> SuperProperties {
        SuperProperties {
            os: "Linux".to_string(),
            browser: "Discord Client".to_string(),
            release_channel: "stable".to_string(),
            client_version: "1.0.162".to_string(),
            os_version: "6.17.0-lab".to_string(),
            os_arch: "x64".to_string(),
            app_arch: "arm64".to_string(),
            system_locale: "ru".to_string(),
            has_client_mods: false,
            client_launch_id: "11111111-2222-4333-8444-555555555555".to_string(),
            browser_user_agent: "Mozilla/5.0 (X11; Linux x86_64) discord/1.0.162".to_string(),
            browser_version: "42.11.12".to_string(),
            window_manager: "KDE".to_string(),
            distro: "\"Arch Linux\"".to_string(),
            runtime_environment: "native".to_string(),
            display_server: "wayland".to_string(),
            client_build_number: 641_004,
            native_build_number: 0,
            client_event_source: None,
            launch_signature: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_string(),
            client_heartbeat_session_id: "ffffffff-0000-4111-8222-333333333333".to_string(),
            client_app_state: "focused".to_string(),
        }
    }

    fn params(identity: &ClientIdentity) -> IdentifyParams {
        IdentifyParams {
            token: "TEST_TOKEN".to_string(),
            capabilities: CAPABILITIES_ACTIVE,
            properties: identity.to_identify_properties(),
            presence: None,
            compress: false,
            client_state_guild_versions: Vec::new(),
            qos_token: qos_token_client_provided(false),
        }
    }

    fn assert_shared_fields(properties: &IdentifyProperties, sp: &SuperProperties) {
        assert_eq!(properties.os, sp.os);
        assert_eq!(properties.browser, sp.browser);
        assert_eq!(properties.release_channel, sp.release_channel);
        assert_eq!(properties.client_version, sp.client_version);
        assert_eq!(properties.os_version, sp.os_version);
        assert_eq!(properties.os_arch, sp.os_arch);
        assert_eq!(properties.app_arch, sp.app_arch);
        assert_eq!(properties.system_locale, sp.system_locale);
        assert_eq!(properties.has_client_mods, sp.has_client_mods);
        assert_eq!(properties.client_launch_id, sp.client_launch_id);
        assert_eq!(properties.browser_user_agent, sp.browser_user_agent);
        assert_eq!(properties.browser_version, sp.browser_version);
        assert_eq!(
            properties.client_build_number,
            u64::from(sp.client_build_number)
        );
        assert_eq!(properties.client_app_state, sp.client_app_state);
        assert_eq!(properties.launch_signature, sp.launch_signature);
    }

    #[test]
    fn client_identity_drives_identify_properties() {
        let sp = distinct_super_properties();
        let identity = ClientIdentity::new(sp.clone());

        let properties = identity.to_identify_properties();
        assert_shared_fields(&properties, &sp);
        assert_eq!(
            IdentifyProperties::from_client_identity(&identity),
            properties,
        );
    }

    #[test]
    fn identify_wire_matches_super_properties_byte_for_byte() {
        let sp = distinct_super_properties();
        let identity = ClientIdentity::new(sp.clone());
        let bytes = build_identify(&params(&identity)).expect("IDENTIFY собирается");
        let (value, consumed) = decode(&bytes).expect("ETERM разбирается");
        assert_eq!(consumed, bytes.len());
        let properties = value.get("properties").expect("properties в payload");

        let string_fields = [
            ("os", &sp.os),
            ("browser", &sp.browser),
            ("release_channel", &sp.release_channel),
            ("client_version", &sp.client_version),
            ("os_version", &sp.os_version),
            ("os_arch", &sp.os_arch),
            ("app_arch", &sp.app_arch),
            ("system_locale", &sp.system_locale),
            ("client_launch_id", &sp.client_launch_id),
            ("browser_user_agent", &sp.browser_user_agent),
            ("browser_version", &sp.browser_version),
            ("launch_signature", &sp.launch_signature),
            ("client_app_state", &sp.client_app_state),
        ];
        for (key, expected) in string_fields {
            assert_eq!(
                properties.get(key).and_then(EtfValue::as_str),
                Some(expected.as_str()),
            );
        }
        assert_eq!(
            properties
                .get("client_build_number")
                .and_then(EtfValue::as_int),
            Some(i64::from(sp.client_build_number))
        );
        assert_eq!(
            properties.get("has_client_mods"),
            Some(&EtfValue::Atom("false".to_string()))
        );
    }

    #[test]
    fn rw_client_identity_matches_identify_defaults() {
        let identity = ClientIdentity::rw_client();
        let sp = identity.super_properties();
        let properties = IdentifyProperties::for_rw_client();

        assert_shared_fields(&properties, sp);
        assert_eq!(properties, identity.to_identify_properties());
    }
}
