//! Типизированные кадры DAVE/MLS v1 (опкоды `21..=31`), карточка task-306.
//!
//! Источник wire-формата: `/home/sanya/re-deep/voice-deep.md` §2.2/§3.3.
//! JSON-опкоды `21..=24` — объект `{"op":N,"d":{...}}`; бинарные MLS-опкоды
//! `25..=30` — клиентский кадр `[opcode][payload]`, серверный
//! `[u16 seq_be][u8 opcode][payload]`. Модуль только кодирует/разбирает байты,
//! транспортных операций здесь нет.

use crate::{VoiceError, json_u64, object};
use discord_json::JsonValue;

/// Опкод `21` — Prepare Transition (`transition_id`, `protocol_version`).
pub const OP_PREPARE_TRANSITION: u8 = 21;
/// Опкод `22` — Execute Transition (`transition_id`).
pub const OP_EXECUTE_TRANSITION: u8 = 22;
/// Опкод `23` — Ready For Transition (`transition_id`, исходящий).
pub const OP_READY_FOR_TRANSITION: u8 = 23;
/// Опкод `24` — Prepare Epoch (`epoch`, `protocol_version`).
pub const OP_PREPARE_EPOCH: u8 = 24;
/// Первый бинарный MLS-опкод (`25` — External Sender Package).
pub const MLS_BINARY_OPCODE_MIN: u8 = 25;
/// Последний бинарный MLS-опкод (`30` — Welcome).
pub const MLS_BINARY_OPCODE_MAX: u8 = 30;

/// Опкод `25` — MLS External Sender Package.
pub const OP_MLS_EXTERNAL_SENDER: u8 = 25;
/// Опкод `26` — MLS Key Package (клиент → сервер).
pub const OP_MLS_KEY_PACKAGE: u8 = 26;
/// Опкод `27` — MLS Proposals.
pub const OP_MLS_PROPOSALS: u8 = 27;
/// Опкод `28` — MLS Commit Welcome (клиент → сервер).
pub const OP_MLS_COMMIT_WELCOME: u8 = 28;
/// Опкод `29` — MLS Prepare Commit Transition (сервер → клиент).
pub const OP_MLS_PREPARE_COMMIT_TRANSITION: u8 = 29;
/// Опкод `30` — MLS Welcome (сервер → клиент).
pub const OP_MLS_WELCOME: u8 = 30;
/// Опкод `31` — Invalid Commit Welcome (сервер → клиент, JSON).
pub const OP_MLS_INVALID_COMMIT_WELCOME: u8 = 31;

/// Проверяет, что опкод относится к бинарным MLS-кадрам (`25..=30`).
#[must_use]
pub fn is_mls_binary_opcode(opcode: u8) -> bool {
    (MLS_BINARY_OPCODE_MIN..=MLS_BINARY_OPCODE_MAX).contains(&opcode)
}

/// Проверяет клиентский бинарный канал MLS: `KeyPackage` (26) или `CommitWelcome` (28).
#[must_use]
pub fn is_mls_client_binary_opcode(opcode: u8) -> bool {
    opcode == OP_MLS_KEY_PACKAGE || opcode == OP_MLS_COMMIT_WELCOME
}

/// Проверяет серверный переходный канал MLS: `PrepareCommitTransition` (29) или `Welcome` (30).
#[must_use]
pub fn is_mls_server_transition_binary_opcode(opcode: u8) -> bool {
    opcode == OP_MLS_PREPARE_COMMIT_TRANSITION || opcode == OP_MLS_WELCOME
}

/// Кадр `op 21` DAVE Prepare Transition (сервер → клиент).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DavePrepareTransition {
    /// Идентификатор перехода протокола.
    pub transition_id: u16,
    /// Версия протокола DAVE, к которой выполняется переход.
    pub protocol_version: u8,
}

impl DavePrepareTransition {
    /// Сериализует кадр `op 21` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(u64::from(OP_PREPARE_TRANSITION))),
            (
                "d",
                object(vec![
                    ("transition_id", json_u64(u64::from(self.transition_id))),
                    (
                        "protocol_version",
                        json_u64(u64::from(self.protocol_version)),
                    ),
                ]),
            ),
        ]))
    }

    /// Разбирает кадр `op 21` из JSON (полный конверт или только `d`).
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root = parse_root(input)?;
        expect_op(&root, OP_PREPARE_TRANSITION)?;
        let data = payload_root(&root);
        Ok(Self {
            transition_id: required_u16(data, "transition_id")?,
            protocol_version: required_u8(data, "protocol_version")?,
        })
    }
}

/// Кадр `op 22` DAVE Execute Transition (сервер → клиент).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveExecuteTransition {
    /// Идентификатор перехода протокола.
    pub transition_id: u16,
}

impl DaveExecuteTransition {
    /// Сериализует кадр `op 22` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(u64::from(OP_EXECUTE_TRANSITION))),
            (
                "d",
                object(vec![(
                    "transition_id",
                    json_u64(u64::from(self.transition_id)),
                )]),
            ),
        ]))
    }

    /// Разбирает кадр `op 22` из JSON (полный конверт или только `d`).
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root = parse_root(input)?;
        expect_op(&root, OP_EXECUTE_TRANSITION)?;
        let data = payload_root(&root);
        Ok(Self {
            transition_id: required_u16(data, "transition_id")?,
        })
    }
}

/// Кадр `op 23` DAVE Ready For Transition (клиент → сервер).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveReadyForTransition {
    /// Идентификатор подтве��ждённого перехода протокола.
    pub transition_id: u16,
}

impl DaveReadyForTransition {
    /// Сериализует исходящий кадр `op 23` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(u64::from(OP_READY_FOR_TRANSITION))),
            (
                "d",
                object(vec![(
                    "transition_id",
                    json_u64(u64::from(self.transition_id)),
                )]),
            ),
        ]))
    }

    /// Разбирает кадр `op 23` из JSON (полный конверт или только `d`).
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root = parse_root(input)?;
        expect_op(&root, OP_READY_FOR_TRANSITION)?;
        let data = payload_root(&root);
        Ok(Self {
            transition_id: required_u16(data, "transition_id")?,
        })
    }
}

/// Кадр `op 24` DAVE Prepare Epoch (сервер → клиент).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DavePrepareEpoch {
    /// Номер MLS-эпохи; `u64`, а не `i64` — номер не должен усекаться.
    pub epoch: u64,
    /// Версия протокола DAVE для эпохи.
    pub protocol_version: u8,
}

impl DavePrepareEpoch {
    /// Сериализует кадр `op 24` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(u64::from(OP_PREPARE_EPOCH))),
            (
                "d",
                object(vec![
                    ("epoch", json_u64(self.epoch)),
                    (
                        "protocol_version",
                        json_u64(u64::from(self.protocol_version)),
                    ),
                ]),
            ),
        ]))
    }

    /// Разбирает кадр `op 24` из JSON (полный конверт или только `d`).
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root = parse_root(input)?;
        expect_op(&root, OP_PREPARE_EPOCH)?;
        let data = payload_root(&root);
        Ok(Self {
            epoch: required_u64(data, "epoch")?,
            protocol_version: required_u8(data, "protocol_version")?,
        })
    }
}

/// Бинарный MLS-кадр (опкоды `25..=30`).
///
/// `seq` заполнен только у серверных кадров: клиентский формат порядковый номер
/// не содержит.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MlsBinaryFrame {
    /// MLS-опкод `25..=30`.
    pub opcode: u8,
    /// Порядковый номер серверного кадра (big-endian `u16` из префикса).
    pub seq: Option<u16>,
    /// Тело MLS-сообщения; пустым быть не может.
    pub payload: Vec<u8>,
}

/// Кодирует исходящий MLS-кадр как `[opcode][payload]`.
///
/// Возвращает ошибку для опкода вне `25..=30` и для пустого payload.
pub fn encode_client_binary(opcode: u8, payload: &[u8]) -> Result<Vec<u8>, VoiceError> {
    if !is_mls_binary_opcode(opcode) {
        return Err(VoiceError::invalid("MLS opcode outside 25..=30"));
    }
    if payload.is_empty() {
        return Err(VoiceError::invalid("empty MLS payload"));
    }
    let mut frame = Vec::with_capacity(payload.len() + 1);
    frame.push(opcode);
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Разбирает серверный MLS-кадр `[u16 seq_be][u8 opcode][payload]`.
///
/// Возвращает ошибку для обрезанного кадра, опкода вне `25..=30` и пустого payload.
pub fn decode_server_binary(bytes: &[u8]) -> Result<MlsBinaryFrame, VoiceError> {
    if bytes.len() < 3 {
        return Err(VoiceError::invalid("truncated MLS server frame"));
    }
    let seq = u16::from_be_bytes([bytes[0], bytes[1]]);
    let opcode = bytes[2];
    if !is_mls_binary_opcode(opcode) {
        return Err(VoiceError::invalid("MLS opcode outside 25..=30"));
    }
    let payload = bytes[3..].to_vec();
    if payload.is_empty() {
        return Err(VoiceError::invalid("empty MLS payload"));
    }
    Ok(MlsBinaryFrame {
        opcode,
        seq: Some(seq),
        payload,
    })
}

/// Разбирает серверный переходный кадр
/// `[u16 seq_be][u8 opcode][u16 transition_id_be][mls_body]`.
///
/// Возвращает `(seq, opcode, transition_id, mls_body)`. Опкод обязан быть `29` или `30`;
/// обрезанный кадр (меньше 6 байт, то есть без тела MLS) — ошибка.
pub fn decode_server_transition_binary(
    bytes: &[u8],
) -> Result<(u16, u8, u16, Vec<u8>), VoiceError> {
    if bytes.len() < 6 {
        return Err(VoiceError::invalid("truncated MLS transition frame"));
    }
    let seq = u16::from_be_bytes([bytes[0], bytes[1]]);
    let opcode = bytes[2];
    if !is_mls_server_transition_binary_opcode(opcode) {
        return Err(VoiceError::invalid("MLS opcode is not a server transition"));
    }
    let transition_id = u16::from_be_bytes([bytes[3], bytes[4]]);
    let mls_body = bytes[5..].to_vec();
    if mls_body.is_empty() {
        return Err(VoiceError::invalid("empty MLS payload"));
    }
    Ok((seq, opcode, transition_id, mls_body))
}

/// Кадр `op 31` DAVE Invalid Commit Welcome (сервер → клиент).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveInvalidCommitWelcome {
    /// Идентификатор перехода, для которого коммит отклонён.
    pub transition_id: u16,
}

impl DaveInvalidCommitWelcome {
    /// Сериализует кадр `op 31` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(u64::from(OP_MLS_INVALID_COMMIT_WELCOME))),
            (
                "d",
                object(vec![(
                    "transition_id",
                    json_u64(u64::from(self.transition_id)),
                )]),
            ),
        ]))
    }

    /// Разбирает кадр `op 31` из JSON (полный конверт или только `d`).
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root = parse_root(input)?;
        expect_op(&root, OP_MLS_INVALID_COMMIT_WELCOME)?;
        let data = payload_root(&root);
        Ok(Self {
            transition_id: required_u16(data, "transition_id")?,
        })
    }
}

/// Проверяет, что `transition_id` попадает в окно между выполненным и
/// подготовленным переходом.
///
/// Окно не определено, пока нет `last_prepared` или `last_executed`, — тогда
/// переход разрешён. Иначе при `last_prepared >= last_executed` окно
/// `(last_executed, last_prepared]`, а при `last_prepared < last_executed`
/// (переполнение `u16`) — `(last_executed, u16::MAX] ∪ [0, last_prepared]`.
pub fn check_transition_window(
    transition_id: u16,
    last_prepared: Option<u16>,
    last_executed: Option<u16>,
) -> Result<(), VoiceError> {
    let (Some(prepared), Some(executed)) = (last_prepared, last_executed) else {
        return Ok(());
    };
    let in_window = if prepared >= executed {
        transition_id > executed && transition_id <= prepared
    } else {
        transition_id > executed || transition_id <= prepared
    };
    if in_window {
        Ok(())
    } else {
        Err(VoiceError::invalid("transition outside window"))
    }
}

/// Решает, надо ли отказаться от DAVE-даунгрейда на кадре `at`.
///
/// Версия протокола `1..=65535` — валидный DAVE. Версия `0` (`DAVE_PROTOCOL_VERSION`
/// не поддержан) допустима только на `init` стадийного канала; всё остальное —
/// даунгрейд до нешифрованного голоса, от которого надо отказаться.
#[must_use]
pub fn should_refuse_dave_downgrade(
    at: &str,
    protocol_version: i64,
    is_stage_channel: bool,
) -> bool {
    if (1..=65535).contains(&protocol_version) {
        return false;
    }
    !(protocol_version == 0 && at == "init" && is_stage_channel)
}

/// Ключ persisted-store Discord, под которым лежит состояние secure frames.
pub const PERSIST_KEY_SECURE_FRAMES: &str = "SecureFramesPersistedStore";

/// Причины отказа E2EE-верификации в том порядке, в котором их перечисляет клиент.
pub const E2EE_VERIFICATION_FAILURE_REASONS: [&str; 7] = [
    "match",
    "other_user_already_verified",
    "other_user_inconsistent_keys",
    "fingerprint_mismatch",
    "unable_to_verify",
    "other_user_disconnected",
    "current_user_disconnected",
];

/// Сохранённое состояние secure frames: флаг persistent codes и загруженные
/// версии ключей.
///
/// Список `uploaded_key_versions` держится без дублей и в порядке загрузки —
/// так конверт остаётся детерминированным при повторном сохранении.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecureFramesPersistedState {
    /// Включены ли постоянные коды (persistent codes) для этого клиента.
    pub persistent_codes_enabled: bool,
    /// Версии ключей, уже загруженные на сервер, в порядке загрузки.
    pub uploaded_key_versions: Vec<u16>,
}

impl SecureFramesPersistedState {
    /// Пустое состояние: persistent codes выключены, версий нет.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Включает или выключает persistent codes.
    pub fn set_persistent_codes_enabled(&mut self, enabled: bool) {
        self.persistent_codes_enabled = enabled;
    }

    /// Добавляет версию ключа в конец списка.
    ///
    /// Возвращает `false` без изменения порядка, если версия уже загружена;
    /// иначе добавляет её и возвращает `true`.
    pub fn add_uploaded_key_version(&mut self, key_version: u16) -> bool {
        if self.uploaded_key_versions.contains(&key_version) {
            return false;
        }
        self.uploaded_key_versions.push(key_version);
        true
    }

    /// Очищает список загруженных версий ключей, не трогая флаг persistent codes.
    pub fn clear_uploaded_key_versions(&mut self) {
        self.uploaded_key_versions.clear();
    }

    /// Идентификатор ключа подписи для MLS-сессии.
    ///
    /// `Some(id)` возвращается только при включённых persistent codes и непустом
    /// `static_auth_session_id`; иначе `None`.
    #[must_use]
    pub fn resolve_signing_key_id<'a>(
        &self,
        static_auth_session_id: Option<&'a str>,
    ) -> Option<&'a str> {
        if !self.persistent_codes_enabled {
            return None;
        }
        static_auth_session_id.filter(|id| !id.is_empty())
    }

    /// Сериализует состояние в конверт persisted-store:
    /// `{"_state":{"persistentCodesEnabled":bool,"uploadedKeyVersions":[...]},"_version":0}`.
    #[must_use]
    pub fn to_persisted_envelope_json(&self) -> String {
        let versions = self
            .uploaded_key_versions
            .iter()
            .map(|version| json_u64(u64::from(*version)))
            .collect();
        discord_json::to_json_string(&object(vec![
            (
                "_state",
                object(vec![
                    (
                        "persistentCodesEnabled",
                        JsonValue::Bool(self.persistent_codes_enabled),
                    ),
                    ("uploadedKeyVersions", JsonValue::Array(versions)),
                ]),
            ),
            ("_version", json_u64(0)),
        ]))
    }

    /// Разбирает сохранённое состояние.
    ///
    /// Принимает и конверт `{"_state":{...},"_version":0}`, и легаси-объект без
    /// `_state` (`{"persistentCodesEnabled":...,"uploadedKeyVersions":[...]}`).
    /// Версия обязана быть целой в `0..=u16::MAX`; битый JSON, неверные типы и
    /// версии ключей вне `u16` дают [`VoiceError::invalid`].
    pub fn parse_persisted_envelope_json(input: &str) -> Result<Self, VoiceError> {
        let root = discord_json::parse(input)
            .map_err(|_| VoiceError::invalid("invalid persisted secure frames JSON"))?;
        if root.as_object().is_none() {
            return Err(VoiceError::invalid(
                "persisted secure frames must be an object",
            ));
        }
        let Some(state) = root.get("_state") else {
            return parse_persisted_state_object(&root);
        };
        let version = root
            .get("_version")
            .ok_or_else(|| VoiceError::invalid("missing persisted envelope _version"))?;
        let version = version
            .as_i64()
            .ok_or_else(|| VoiceError::invalid("persisted envelope _version must be an integer"))?;
        if version < 0 || version > i64::from(u16::MAX) {
            return Err(VoiceError::invalid(
                "persisted envelope _version outside u16 range",
            ));
        }
        parse_persisted_state_object(state)
    }
}

fn parse_persisted_state_object(
    state: &JsonValue,
) -> Result<SecureFramesPersistedState, VoiceError> {
    if state.as_object().is_none() {
        return Err(VoiceError::invalid(
            "persisted secure frames _state must be an object",
        ));
    }
    let persistent_codes_enabled = match state.get("persistentCodesEnabled") {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| VoiceError::invalid("persistentCodesEnabled must be a boolean"))?,
    };
    let mut uploaded_key_versions = Vec::new();
    if let Some(value) = state.get("uploadedKeyVersions") {
        let items = value
            .as_array()
            .ok_or_else(|| VoiceError::invalid("uploadedKeyVersions must be an array"))?;
        for item in items {
            let raw = item.as_u64().ok_or_else(|| {
                VoiceError::invalid("uploadedKeyVersions item must be a non-negative integer")
            })?;
            let version = u16::try_from(raw)
                .map_err(|_| VoiceError::invalid("uploadedKeyVersions item outside u16 range"))?;
            if !uploaded_key_versions.contains(&version) {
                uploaded_key_versions.push(version);
            }
        }
    }
    Ok(SecureFramesPersistedState {
        persistent_codes_enabled,
        uploaded_key_versions,
    })
}

fn parse_root(input: &str) -> Result<JsonValue, VoiceError> {
    discord_json::parse(input).map_err(|_| VoiceError::invalid("invalid DAVE JSON"))
}

fn expect_op(root: &JsonValue, expected: u8) -> Result<(), VoiceError> {
    match root.get("op") {
        None => Ok(()),
        Some(value) => match value.as_u64() {
            Some(op) if op == u64::from(expected) => Ok(()),
            _ => Err(VoiceError::invalid("unexpected DAVE op")),
        },
    }
}

fn payload_root(root: &JsonValue) -> &JsonValue {
    root.get("d").unwrap_or(root)
}

fn required_u64(data: &JsonValue, key: &'static str) -> Result<u64, VoiceError> {
    data.get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| VoiceError::invalid("missing DAVE field"))
}

fn required_u16(data: &JsonValue, key: &'static str) -> Result<u16, VoiceError> {
    u16::try_from(required_u64(data, key)?)
        .map_err(|_| VoiceError::invalid("DAVE field outside u16 range"))
}

fn required_u8(data: &JsonValue, key: &'static str) -> Result<u8, VoiceError> {
    u8::try_from(required_u64(data, key)?)
        .map_err(|_| VoiceError::invalid("DAVE field outside u8 range"))
}

/// Действия движка DAVE над ключами secure frames.
///
/// Форма зафиксирована RE-GAP-27 §1, `V-27-L1-01..02`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecureFramesKeyAction {
    /// Создан подтверждённый ключ пользователя.
    VerifiedKeyCreate { user_id: String, key: Vec<u8> },
    /// Удалён подтверждённый ключ пользователя по сериализованному значению.
    VerifiedKeyDelete {
        user_id: String,
        serialized_key: String,
    },
    /// Удалены все подтверждённые ключи пользователя.
    UserVerifiedKeysDelete { user_id: String },
    /// Создан транзитный ключ пользователя.
    TransientKeyCreate { user_id: String, key: Vec<u8> },
    /// Удалён транзитный ключ пользователя. Второго аргумента у действия нет.
    TransientKeyDelete { user_id: String },
}

/// Транзитные ключи secure frames: `user_id -> key`.
///
/// RE-GAP-27 §2: ключи живут до открытия нового соединения; смена голосового
/// канала их не чистит (`GAP-27-12`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransientKeyStore {
    users: std::collections::BTreeMap<String, Vec<u8>>,
}

impl TransientKeyStore {
    /// Пустое хранилище.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Сохраняет копию ключа пользователя, заменяя прежнюю запись.
    pub fn create(&mut self, user_id: impl Into<String>, key: &[u8]) {
        self.users.insert(user_id.into(), key.to_vec());
    }

    /// Удаляет ключ пользователя; `true`, если запись была.
    pub fn delete(&mut self, user_id: &str) -> bool {
        self.users.remove(user_id).is_some()
    }

    /// Проверяет ключ пользователя: без ключа или записи — `false`,
    /// при разной длине — `false`, иначе побайтовое равенство.
    #[must_use]
    pub fn is_key_verified(&self, user_id: &str, key: Option<&[u8]>) -> bool {
        let Some(key) = key else {
            return false;
        };
        self.users.get(user_id).is_some_and(|stored| stored == key)
    }

    /// Новое соединение: транзитные ключи всех пользователей сбрасываются.
    pub fn on_connection_open(&mut self) {
        self.users.clear();
    }

    /// Смена голосового канала: no-op, ключи остаются (`GAP-27-12`).
    pub fn on_voice_channel_select(&mut self) {}

    /// Число пользователей с транзитным ключом.
    #[must_use]
    pub fn len(&self) -> usize {
        self.users.len()
    }

    /// Пусто ли хранилище.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }
}

/// Подтверждённое состояние secure frames: пользователи, потоки и звонок.
///
/// RE-GAP-27 §4, `V-27-L1-04`, `GAP-27-04`: пустой ростер даёт
/// `isCallVerified = true`; собственный `user_id` в ростер не попадает, а любое
/// значение кроме `Some(true)` снимает подтверждение звонка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecureFramesVerifiedState {
    users: std::collections::BTreeMap<String, Option<bool>>,
    streams: std::collections::BTreeMap<String, Option<bool>>,
    is_call_verified: bool,
}

impl Default for SecureFramesVerifiedState {
    fn default() -> Self {
        Self {
            users: std::collections::BTreeMap::new(),
            streams: std::collections::BTreeMap::new(),
            is_call_verified: true,
        }
    }
}

impl SecureFramesVerifiedState {
    /// Пустое состояние: пустой ростер подтверждает звонок (`GAP-27-04`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Подтверждён ли звонок.
    #[must_use]
    pub fn is_call_verified(&self) -> bool {
        self.is_call_verified
    }

    /// Подтверждение пользователя: `None` — записи нет.
    #[must_use]
    pub fn is_user_verified(&self, user_id: &str) -> Option<bool> {
        self.users.get(user_id).copied().flatten()
    }

    /// Подтверждение потока: `None` — записи нет.
    #[must_use]
    pub fn is_stream_verified(&self, stream_key: &str) -> Option<bool> {
        self.streams.get(stream_key).copied().flatten()
    }

    /// Сохраняет подтверждение пользователя, кроме собственного `user_id`,
    /// и пересчитывает подтверждение звонка.
    pub fn set_user_verified(
        &mut self,
        user_id: impl Into<String>,
        self_user_id: &str,
        verified: Option<bool>,
    ) {
        let user_id = user_id.into();
        if user_id != self_user_id {
            self.users.insert(user_id, verified);
            self.recompute_call_verified();
        }
    }

    /// Сохраняет подтверждение потока и пересчитывает подтверждение звонка.
    pub fn set_stream_verified(&mut self, stream_key: impl Into<String>, verified: Option<bool>) {
        self.streams.insert(stream_key.into(), verified);
        self.recompute_call_verified();
    }

    /// Удаляет поток; `true`, если запись была. Статус звонка пересчитывается.
    pub fn remove_stream(&mut self, stream_key: &str) -> bool {
        let removed = self.streams.remove(stream_key).is_some();
        self.recompute_call_verified();
        removed
    }

    /// Сбрасывает ростер и потоки; пустой ростер подтверждает звонок.
    pub fn reset(&mut self) {
        self.users.clear();
        self.streams.clear();
        self.is_call_verified = true;
    }

    /// `true` тогда и только тогда, когда все значения обеих карт — `Some(true)`.
    fn recompute_call_verified(&mut self) {
        self.is_call_verified = self
            .users
            .values()
            .chain(self.streams.values())
            .all(|value| *value == Some(true));
    }
}

/// Ключ persisted-store Discord, под которым лежит `VerifiedKeyStore`.
///
/// RE-GAP-27, `V-28-L1-01`: подтверждённые ключи secure frames переживают
/// перезапуск клиента и хранятся отдельно от `PERSIST_KEY_SECURE_FRAMES`.
pub const PERSIST_KEY_VERIFIED_KEYS: &str = "VerifiedKeyStore";

/// Стандартный base64 (RFC 4648 §4): алфавит `A-Z a-z 0-9 + /`, паддинг `=`
/// до кратности 4. Пустой срез даёт пустую строку.
#[must_use]
pub fn serialize_verified_key_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(char::from(ALPHABET[usize::from(b0 >> 2)]));
        out.push(char::from(
            ALPHABET[usize::from(((b0 & 0x03) << 4) | (b1 >> 4))],
        ));
        if chunk.len() == 1 {
            out.push_str("==");
        } else {
            out.push(char::from(
                ALPHABET[usize::from(((b1 & 0x0f) << 2) | (b2 >> 6))],
            ));
            if chunk.len() == 2 {
                out.push('=');
            } else {
                out.push(char::from(ALPHABET[usize::from(b2 & 0x3f)]));
            }
        }
    }
    out
}

/// Подтверждённые ключи secure frames: `user_id -> (base64 ключа -> время
/// доверия, мс)`.
///
/// RE-GAP-27, `V-28-L1-01`: хранится в persisted-store под
/// `PERSIST_KEY_VERIFIED_KEYS`, ключи — стандартный base64. `BTreeMap` держит
/// порядок детерминированным, чтобы повторное сохранение давало тот же JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VerifiedKeyStore {
    /// Пользователи и их подтверждённые ключи.
    #[serde(default)]
    pub users: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>,
}

impl VerifiedKeyStore {
    /// Пустое хранилище.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Записывает подтверждённый ключ пользователя и метку доверия `now_ms`,
    /// перезаписывая прежнюю метку того же ключа.
    pub fn create(&mut self, user_id: impl Into<String>, key_bytes: &[u8], now_ms: u64) {
        self.users
            .entry(user_id.into())
            .or_default()
            .insert(serialize_verified_key_base64(key_bytes), now_ms);
    }

    /// Удаляет ключ по готовой строке `serialized_key` без перекодирования.
    ///
    /// Пустой объект пользователя после удаления убирается из `users`.
    /// `true`, если удалён ключ или пустой объект пользователя.
    pub fn delete_one(&mut self, user_id: &str, serialized_key: &str) -> bool {
        let Some(keys) = self.users.get_mut(user_id) else {
            return false;
        };
        let removed = keys.remove(serialized_key).is_some();
        let emptied = keys.is_empty();
        if emptied {
            self.users.remove(user_id);
        }
        removed || emptied
    }

    /// Удаляет пользователя со всеми ключами; `true`, если запись была.
    pub fn delete_user(&mut self, user_id: &str) -> bool {
        self.users.remove(user_id).is_some()
    }

    /// Метка доверия ключа пользователя; `None` — ключ не подтверждён.
    #[must_use]
    pub fn get_key_trusted_at(&self, user_id: &str, key_bytes: &[u8]) -> Option<u64> {
        let serialized = serialize_verified_key_base64(key_bytes);
        self.users.get(user_id)?.get(&serialized).copied()
    }

    /// Подтверждён ли ключ пользователя.
    #[must_use]
    pub fn is_key_verified(&self, user_id: &str, key_bytes: &[u8]) -> bool {
        self.get_key_trusted_at(user_id, key_bytes).is_some()
    }

    /// Идентификаторы пользователей с подтверждёнными ключами.
    #[must_use]
    pub fn get_user_ids(&self) -> Vec<String> {
        self.users.keys().cloned().collect()
    }

    /// Подтверждённые ключи пользователя: `base64 -> trusted_at`.
    #[must_use]
    pub fn get_user_verified_keys(
        &self,
        user_id: &str,
    ) -> Option<&std::collections::BTreeMap<String, u64>> {
        self.users.get(user_id)
    }
}

/// Версия протокола DAVE «выключено» (`kDisabledVersion` в `LibDaveManager`).
///
/// RE-GAP-28, `V-28-L1-04`: при этой версии `SecureFramesStateUpdate` уходит с
/// пустым `epoch_authenticator`, а MLS-сессия при execute transition сбрасывается.
pub const DAVE_DISABLED_VERSION: u16 = 0;

/// Потоковый ключ стрима (`isStreamKey`/`parseStreamKey`/`formatStreamKey`
/// из модуля `652896` Discord).
///
/// RE-GAP-28, `V-28-L1-03`: wire-форма — строка `guild:{guild_id}:{channel_id}:{owner_id}`
/// для гильдейского стрима и `call:{channel_id}:{owner_id}` для личного звонка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamKey {
    /// Гильдейский стрим: гильдия, канал и владелец стрима.
    Guild {
        /// Идентификатор гильдии.
        guild_id: String,
        /// Идентификатор канала.
        channel_id: String,
        /// Идентификатор владельца стрима.
        owner_id: String,
    },
    /// Личный звонок: канал и владелец стрима.
    Call {
        /// Идентификатор канала.
        channel_id: String,
        /// Идентификатор владельца стрима.
        owner_id: String,
    },
}

/// Ошибка разбора строки [`StreamKey`].
///
/// Оракул: `throw Error("Unknown stream type " + first)` — в поле попадает либо
/// неизвестный первый сегмент, либо вид ключа, которому не хватило сегментов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamKeyError {
    /// Первый сегмент строки (или вид ключа при нехватке сегментов).
    pub unknown_type: String,
}

impl std::fmt::Display for StreamKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown stream type {}", self.unknown_type)
    }
}

impl std::error::Error for StreamKeyError {}

impl StreamKey {
    /// Быстрая проверка префикса без разбора: `guild:` или `call:`.
    ///
    /// Оракул проверяет `startsWith(GUILD) || startsWith(CALL)`; здесь префикс
    /// включает разделитель, чтобы `guildx:...` не считался потоковым ключом.
    #[must_use]
    pub fn is_stream_key(raw: &str) -> bool {
        raw.starts_with("guild:") || raw.starts_with("call:")
    }

    /// Разбирает строку ключа; лишние сегменты отбрасываются.
    ///
    /// `guild` требует минимум 4 сегмента (`guild`, гильдия, канал, владелец),
    /// `call` — минимум 3 (`call`, канал, владелец). Иначе — [`StreamKeyError`]
    /// с видом ключа в `unknown_type`.
    pub fn parse(raw: &str) -> Result<Self, StreamKeyError> {
        let parts: Vec<&str> = raw.split(':').collect();
        let first = parts.first().copied().unwrap_or_default();
        match first {
            "guild" => match parts.as_slice() {
                [_, guild_id, channel_id, owner_id, ..] => Ok(Self::Guild {
                    guild_id: (*guild_id).to_string(),
                    channel_id: (*channel_id).to_string(),
                    owner_id: (*owner_id).to_string(),
                }),
                _ => Err(StreamKeyError {
                    unknown_type: "guild".to_string(),
                }),
            },
            "call" => match parts.as_slice() {
                [_, channel_id, owner_id, ..] => Ok(Self::Call {
                    channel_id: (*channel_id).to_string(),
                    owner_id: (*owner_id).to_string(),
                }),
                _ => Err(StreamKeyError {
                    unknown_type: "call".to_string(),
                }),
            },
            _ => Err(StreamKeyError {
                unknown_type: first.to_string(),
            }),
        }
    }

    /// Каноническая строка ключа для провода и состояния.
    #[must_use]
    pub fn format(&self) -> String {
        match self {
            Self::Guild {
                guild_id,
                channel_id,
                owner_id,
            } => format!("guild:{guild_id}:{channel_id}:{owner_id}"),
            Self::Call {
                channel_id,
                owner_id,
            } => format!("call:{channel_id}:{owner_id}"),
        }
    }
}

/// Состояние secure frames, которое уходит событием `SecureFramesUpdate`.
///
/// RE-GAP-28, `V-28-L1-04`: `version` — текущая версия шифратора (или
/// [`DAVE_DISABLED_VERSION`]), `epoch_authenticator` — base64 последнего
/// epoch-аутентификатора MLS; при выключенном DAVE строка пустая.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecureFramesStateUpdate {
    /// Версия протокола шифратора; `0` — DAVE выключен.
    pub version: u16,
    /// Base64 epoch-аутентификатора; пусто при `version == 0`.
    pub epoch_authenticator: String,
}

impl SecureFramesStateUpdate {
    /// Собирает состояние из версии и сырых байтов epoch-аутентификатора.
    #[must_use]
    pub fn from_epoch(version: u16, raw_epoch_authenticator: &[u8]) -> Self {
        let epoch_authenticator = if version == DAVE_DISABLED_VERSION {
            String::new()
        } else {
            serialize_verified_key_base64(raw_epoch_authenticator)
        };
        Self {
            version,
            epoch_authenticator,
        }
    }

    /// `true`, если состояние отличается от последнего отправленного.
    ///
    /// Первое состояние (`last_update == None`) уходит всегда.
    #[must_use]
    pub fn should_emit_update(&self, last_update: Option<&Self>) -> bool {
        last_update != Some(self)
    }
}

pub use crate::srtp::{
    DAVE_MARKER_BYTES, DAVE_MAX_EPOCH_TRANSITIONS, DAVE_MAX_GENERATION_GAP,
    DAVE_MAX_LIFETIME_GENERATIONS, DAVE_MAX_PARTICIPANTS, DAVE_MAX_RESETS, DAVE_MAX_SIGNAL_BYTES,
    DAVE_MLS_EPOCH_AUTH_LABEL, DAVE_MLS_RATCHET_KEY_LABEL, DAVE_MLS_RATCHET_NONCE_LABEL,
    DAVE_MLS_RATCHET_SECRET_LABEL, DAVE_MLS_USER_MEDIA_KEY_BASE_LABEL, DAVE_PROPOSAL_MAGIC,
    DAVE_PROPOSAL_REVOKE_MAGIC, DAVE_RATCHET_GENERATION_SHIFT_BITS, DAVE_STRUCTURED_COMMIT_MAGIC,
    DAVE_WELCOME_MAGIC, DaveFrameCipher, DaveMediaCipher, DaveMlsEngine, DaveStrippedRtpPacket,
    DaveTransitionOutcome, MlsKeyRatchet, MlsProposalType, MlsRatchetKeyAndNonce,
    MlsVerificationError, build_structured_mls_commit, build_structured_mls_proposal,
    build_structured_mls_revoke, build_structured_mls_welcome, compute_wrapped_big_nonce,
    compute_wrapped_generation,
};

/// Длина тега аутентификации AES-256-GCM внутреннего слоя DAVE MLS v1 (16 байт).
pub const DAVE_GCM_TAG_LEN: usize = 16;

/// Актуальная версия сквозного протокола шифрования медиакадров DAVE MLS (`v1`).
pub const DAVE_PROTOCOL_VERSION_V1: u8 = 1;

/// Проверяет, содержит ли полезная нагрузка RTP-пакета (после снятия внешнего слоя SRTP)
/// валидный трейлер сквозного шифрования DAVE MLS v1:
/// `[...][16B gcm_tag][leb128(truncated_nonce)][supplemental_size: u8][0xFA, 0xFA]`.
#[must_use]
pub fn has_dave_magic_marker(decrypted_srtp_payload: &[u8]) -> bool {
    let len = decrypted_srtp_payload.len();
    if len < DAVE_GCM_TAG_LEN + 4 {
        return false;
    }
    if decrypted_srtp_payload[len - 2..] != DAVE_MARKER_BYTES {
        return false;
    }
    let supplemental_size = usize::from(decrypted_srtp_payload[len - 3]);
    supplemental_size >= DAVE_GCM_TAG_LEN + 4 && supplemental_size <= len
}

impl DaveMlsEngine {
    /// Двухслойно шифрует пакетную последовательность RTP-видеопакетов (`VideoRtpPacket`)
    /// сквозным шифром эпохи DAVE MLS v1 (`0xFAFA`) и транспортным SRTP (`_rtpsize`).
    pub fn encrypt_video_rtp_packets(
        &mut self,
        packets: &[crate::rtp::video::VideoRtpPacket],
    ) -> Result<Vec<Vec<u8>>, crate::srtp::SrtpError> {
        let mut out = Vec::with_capacity(packets.len());
        for pkt in packets {
            out.push(self.encrypt_rtp_packet(&pkt.raw)?);
        }
        Ok(out)
    }

    /// Двухслойно расшифровывает входящий видеопакет SRTP + DAVE MLS v1 (`0xFAFA`),
    /// проверяет защиту от повтора (`NonceReuse`) и снимает расширение `0xBEDE`.
    pub fn decrypt_video_rtp_packet(
        &mut self,
        wire_packet: &[u8],
    ) -> Result<DaveStrippedRtpPacket, crate::srtp::SrtpError> {
        self.decrypt_rtp_stripping_bede(wire_packet)
    }

    /// Пакетизирует видеокадр H.264 Annex-B (Single NALU, STAP-A, FU-A для `> 1200` байт)
    /// и шифрует все полученные RTP-пакеты двухслойным конвейером DAVE MLS v1 + SRTP.
    pub fn packetize_and_encrypt_h264(
        &mut self,
        packetizer: &mut crate::rtp::video::RtpVideoPacketizer,
        annex_b_frame: &[u8],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<Vec<u8>>, crate::rtp::video::VideoRtpError> {
        let rtp_packets = packetizer.packetize_h264_frame(annex_b_frame, timestamp_delta, ext)?;
        let encrypted = self.encrypt_video_rtp_packets(&rtp_packets)?;
        Ok(encrypted)
    }

    /// Пакетизирует срез OBU видеокадра AV1 (`W=1..3/0`, `Z/Y` фрагментация для `> 1200` байт)
    /// и шифрует все полученные RTP-пакеты двухслойным конвейером DAVE MLS v1 + SRTP.
    pub fn packetize_and_encrypt_av1(
        &mut self,
        packetizer: &mut crate::rtp::video::RtpVideoPacketizer,
        av1_obus: &[&[u8]],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<Vec<u8>>, crate::rtp::video::VideoRtpError> {
        let rtp_packets = packetizer.packetize_av1_obus(av1_obus, timestamp_delta, ext)?;
        let encrypted = self.encrypt_video_rtp_packets(&rtp_packets)?;
        Ok(encrypted)
    }

    /// Расшифровывает двухслойный видеопакет (SRTP + DAVE MLS v1 `0xFAFA`), снимает расширение `0xBEDE`
    /// и подаёт открытый RTP-видеопакет в сборщик [`crate::media::rtp::VideoFrameAssembler`].
    /// Возвращает `Ok(Some(frame))`, когда пакет с `marker = true` завершает сборку кадра.
    pub fn decrypt_and_assemble_video_packet(
        &mut self,
        assembler: &mut crate::media::rtp::VideoFrameAssembler,
        wire_packet: &[u8],
    ) -> Result<Option<Vec<u8>>, crate::srtp::SrtpError> {
        let stripped = self.decrypt_video_rtp_packet(wire_packet)?;
        Ok(assembler.push(
            stripped.sequence,
            stripped.timestamp,
            stripped.marker,
            &stripped.payload,
        ))
    }
}
