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

use crate::rest::identity::ClientIdentity;
use crate::rest::super_properties::SuperProperties;

/// capabilities активного клиента (без обфускации приватных каналов) — из бандла.
pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;

/// capabilities с фичей `private_channel_obfuscation` — из бандла.
pub const CAPABILITIES_CHANNEL_OBFUSCATION: u64 = 1_767_421;

/// Причина подключения при холодном старте (модуль 469177 бандла: `r=true`).
const GATEWAY_CONNECT_COLD_START: &str = "COLD_START";

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
    /// UUID подписи запуска: в клиенте — то же значение, что уходит в `X-Super-Properties`.
    pub launch_signature: String,
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
    ///
    /// Строятся из [`ClientIdentity::rw_client`]: те же значения уходят и в
    /// `X-Super-Properties`, поэтому дефолтный IDENTIFY не расходится с REST.
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
    ///
    /// Супер-свойства содержат 22 поля REST-заголовка, из них 15 пересекаются с IDENTIFY
    /// (включая `launch_signature`); `client_build_number` приводится `u32 → u64`.
    /// Поля IDENTIFY, которых в супер-свойствах нет, — константы нашего клиента:
    /// `is_fast_connect = false`, `gateway_connect_reasons = "COLD_START"`,
    /// `installation_id = None`.
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
}

impl ClientIdentity {
    /// Свойства OP2 IDENTIFY из той же идентичности, что уходит в `X-Super-Properties`.
    #[must_use]
    pub fn to_identify_properties(&self) -> IdentifyProperties {
        IdentifyProperties::from_client_identity(self)
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
    /// Пустой (или состоящий из пробелов) токен — IDENTIFY не уходит.
    EmptyToken,
    /// `qos_token` не строгий UTF-8 (бинарный protobuf не представим строкой ETF).
    QosTokenNotUtf8,
    /// Собранный терм не разбирается кодеком (самопроверка сборки).
    Encode(EtfError),
}

impl std::fmt::Display for IdentifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyToken => write!(f, "пустой токен"),
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
            Self::EmptyToken | Self::QosTokenNotUtf8 => None,
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
            key("launch_signature"),
            EtfValue::Str(p.launch_signature.clone()),
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
    if p.token.trim().is_empty() {
        return Err(IdentifyError::EmptyToken);
    }
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

#[cfg(test)]
mod tests {
    //! Единая идентичность: IDENTIFY и `X-Super-Properties` одной сессии не расходятся.
    //!
    //! Оракул — код веб-клиента (`handleIdentify`): `properties` собираются как
    //! `{...getSuperProperties(), client_app_state, is_fast_connect, gateway_connect_reasons,
    //! installation_id?}`. Поэтому все общие поля, включая `launch_signature`, обязаны
    //! приходить из одного [`ClientIdentity`], а не собираться вторым независимым путём.

    use super::*;
    use discord_codec::etf::decode;

    /// Супер-свойства с различимыми значениями во всех 22 полях: любая подмена поля видна.
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

    /// Общие поля IDENTIFY и супер-свойств сравниваются по значению — без исключений.
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
        assert_eq!(
            properties.client_launch_id, sp.client_launch_id,
            "client_launch_id: один запуск — одна идентичность"
        );
        assert_eq!(properties.browser_user_agent, sp.browser_user_agent);
        assert_eq!(properties.browser_version, sp.browser_version);
        assert_eq!(
            properties.client_build_number,
            u64::from(sp.client_build_number)
        );
        assert_eq!(properties.client_app_state, sp.client_app_state);
        assert_eq!(
            properties.launch_signature, sp.launch_signature,
            "launch_signature уходит и в OP2, и в X-Super-Properties"
        );
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
            "оба входа дают один и тот же набор свойств"
        );
    }

    #[test]
    fn identify_wire_matches_super_properties_byte_for_byte() {
        let sp = distinct_super_properties();
        let identity = ClientIdentity::new(sp.clone());
        let bytes = build_identify(&params(&identity)).expect("IDENTIFY собирается");
        let (value, consumed) = decode(&bytes).expect("ETERM разбирается");
        assert_eq!(consumed, bytes.len());
        let properties = value
            .get("properties")
            .expect("properties в payload IDENTIFY");

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
                "поле {key} в OP2 IDENTIFY обязано совпадать с X-Super-Properties"
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
        assert_eq!(
            properties,
            identity.to_identify_properties(),
            "дефолтный клиент тоже обязан иметь один отпечаток"
        );
    }
}
