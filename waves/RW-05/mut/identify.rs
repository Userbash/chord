//! # IDENTIFY (op2): сборка терма идентификации (RW-05, P1-04 / T-GW-02)
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

use discord_codec::etf::{EtfError, EtfValue};

/// capabilities активного клиента (без обфускации приватных каналов) — из бандла.
pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;

/// capabilities с фичей `private_channel_obfuscation` — из бандла.
pub const CAPABILITIES_CHANNEL_OBFUSCATION: u64 = 1_767_421;

/// Супер-свойства клиента плюс поля идентификации (см. `handleIdentify` в бандле и ТЗ).
#[derive(Debug, Clone)]
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
    /// Архитектура ОС.
    pub os_arch: String,
    /// Архитектура приложения.
    pub app_arch: String,
    /// Системная локаль.
    pub system_locale: String,
    /// Модификации клиента (у нас — `false`).
    pub has_client_mods: bool,
    /// Идентификатор запуска клиента (у веб-клиента — UUID).
    pub client_launch_id: String,
    /// User-Agent браузерного движка (у веб-клиента — Electron).
    pub browser_user_agent: String,
    /// Версия браузерного движка.
    pub browser_version: String,
    /// Номер сборки клиента (в бандле — `parseInt("641004")`).
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
    pub fn for_rw_client() -> Self {
        let version = env!("CARGO_PKG_VERSION");
        Self {
            os: "Linux".to_string(),
            browser: "Discord Client".to_string(),
            release_channel: "stable".to_string(),
            client_version: version.to_string(),
            os_version: "unknown".to_string(),
            os_arch: std::env::consts::ARCH.to_string(),
            app_arch: std::env::consts::ARCH.to_string(),
            system_locale: "en-US".to_string(),
            has_client_mods: false,
            client_launch_id: "00000000-0000-4000-8000-000000000000".to_string(),
            browser_user_agent: format!("discord-lab-rs/{version}"),
            browser_version: version.to_string(),
            // Бандл: `client_build_number: parseInt("641004")`.
            client_build_number: 641_004,
            client_app_state: "focused".to_string(),
            is_fast_connect: false,
            // Модуль 469177: первый коннект (`r=true`) → "COLD_START".
            gateway_connect_reasons: "COLD_START".to_string(),
            installation_id: None,
        }
    }
}

/// Присутствие из IDENTIFY: `{status, since, activities, afk}` (как `getLocalPresence()`).
#[derive(Debug, Clone)]
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
    /// `qos_token` не строгий UTF-8 (бинарный protobuf не представим строкой ETF).
    QosTokenNotUtf8,
    /// Собранный терм не разбирается кодеком (самопроверка сборки).
    Encode(EtfError),
}

impl std::fmt::Display for IdentifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QosTokenNotUtf8 => write!(f, "qos_token не является строгим UTF-8"),
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
            Self::QosTokenNotUtf8 => None,
        }
    }
}

/// Минимальное protobuf-кодирование `QosToken{client_provided{is_active}}`.
///
/// Байты: поле 1 (`client_provided`, длина 2) → поле 1 (`is_active`, varint).
/// `false` — `0A 02 08 00` (писатель protobuf-ts опустил бы нулевое поле, но серверу
/// эквивалентны обе формы; форма зафиксирована контрактом ТЗ).
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

/// Свойства идентификации как map (порядок — супер-свойства, затем поля `handleIdentify`).
fn properties_value(p: &IdentifyProperties) -> EtfValue {
    let mut pairs = vec![
        (key("os"), EtfValue::Str(p.os.clone())),
        (key("browser"), EtfValue::Str(p.browser.clone())),
        (
            key("release_channel"),
            EtfValue::Str(p.release_channel.clone()),
        ),
        (
            key("client_version"),
            EtfValue::Str(p.client_version.clone()),
        ),
        (key("os_version"), EtfValue::Str(p.os_version.clone())),
        (key("os_arch"), EtfValue::Str(p.os_arch.clone())),
        (key("app_arch"), EtfValue::Str(p.app_arch.clone())),
        (key("system_locale"), EtfValue::Str(p.system_locale.clone())),
        (key("has_client_mods"), bool_atom(p.has_client_mods)),
        (
            key("client_launch_id"),
            EtfValue::Str(p.client_launch_id.clone()),
        ),
        (
            key("browser_user_agent"),
            EtfValue::Str(p.browser_user_agent.clone()),
        ),
        (
            key("browser_version"),
            EtfValue::Str(p.browser_version.clone()),
        ),
        (key("client_build_number"), int(p.client_build_number)),
        (
            key("client_app_state"),
            EtfValue::Str(p.client_app_state.clone()),
        ),
        (key("is_fast_connect"), bool_atom(p.is_fast_connect)),
        (
            key("gateway_connect_reasons"),
            EtfValue::Str(p.gateway_connect_reasons.clone()),
        ),
    ];
    if let Some(installation_id) = &p.installation_id {
        pairs.push((
            key("installation_id"),
            EtfValue::Str(installation_id.clone()),
        ));
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
///
/// Самопроверка: собранные байты обязаны полностью разбираться тем же кодеком; иначе —
/// [`IdentifyError::Encode`] (без паники). Не-UTF8 `qos_token` — [`IdentifyError::QosTokenNotUtf8`].
pub fn build_identify(p: &IdentifyParams) -> Result<Vec<u8>, IdentifyError> {
    self_check(discord_codec::etf::encode(&identify_value(p)?))
}

/// Кадр IDENTIFY (op2) в форме провода: `{op: 2, d: <терм build_identify>}` с ведущим `0x83`.
///
/// Оракул формы — код клиента: `send = (e,t,n) => pack({op: e, d: t})` (бандл, ~5024100),
/// то есть payload IDENTIFY всегда лежит под ключом `d`.
pub fn build_identify_frame(p: &IdentifyParams) -> Result<Vec<u8>, IdentifyError> {
    let frame = EtfValue::Map(vec![
        (key("op"), EtfValue::Int(2)),
        (key("d"), identify_value(p)?),
    ]);
    self_check(discord_codec::etf::encode(&frame))
}

/// Значение payload IDENTIFY (без внешнего кадра `{op, d}`).
fn identify_value(p: &IdentifyParams) -> Result<EtfValue, IdentifyError> {
    let qos_token =
        String::from_utf8(p.qos_token.clone()).map_err(|_| IdentifyError::QosTokenNotUtf8)?;
    let guild_versions = EtfValue::Map(
        p.client_state_guild_versions
            .iter()
            .map(|(guild_id, version)| {
                (
                    EtfValue::Str(guild_id.clone()),
                    EtfValue::Str(version.clone()),
                )
            })
            .collect(),
    );
    let client_state = EtfValue::Map(vec![(key("guild_versions"), guild_versions)]);
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
    let (_decoded, consumed) = discord_codec::etf::decode(&bytes).map_err(IdentifyError::Encode)?;
    if consumed != bytes.len() {
        return Err(IdentifyError::Encode(
            discord_codec::etf::EtfError::UnexpectedEof {
                at: consumed,
                need: bytes.len() - consumed,
            },
        ));
    }
    Ok(bytes)
}
