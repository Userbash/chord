//! # Транспорт шлюза Discord: пайп zstd→термы→сессия и WS-клиент (RW-04, P1-03 / T-GW-01, T-GW-03)
//!
//! Тонкий транспорт поверх детерминированного ядра сессии (RW-03, [`crate::gateway`]):
//! соединение (`ws://`/`wss://`), приём **бинарных** WS-сообщений, конвейер
//! zstd→ETF-термы→[`Session`], отправка heartbeat по эффектам, таймеры (срок удара и зомби)
//! и исходы `Reconnect`/`Closed` — без паник и `unwrap` на входных данных.
//!
//! ## Живой протокол (спека `docs/re/gateway-protocol.md` §3)
//!
//! - server→client: **один непрерывный zstd-кадр**; каждое WS-сообщение — фрагмент этого кадра,
//!   магия (`28 b5 2f fd`) только в первом, `end()` не вызывается. Поэтому пайп строит
//!   [`ZstdStream`] один раз на соединение и подаёт в него сообщения по мере прихода.
//! - После распаковки идёт поток термов вида `0x83` + ETF-терм; терм может быть разрезан между
//!   сообщениями — хвост копится в пайпе до следующего фрагмента.
//! - client→server: **несжатые** ETF-термы; боевой heartbeat — op40 `{seq, qos}`
//!   ([`encode_qos_heartbeat`], AUTH-RE G-12), fallback — legacy op1 ([`encode_heartbeat`]),
//!   исходящие клиентские команды — op3 ([`send_presence_update`]), op4
//!   ([`send_voice_state_update`]), op8 ([`send_request_guild_members`] с чанкованием по 100
//!   `user_ids`) и op37 ([`send_guild_subscriptions_bulk`]).
//!
//! ## Пайп ([`GatewayPipe`])
//!
//! `push_binary` = `ZstdStream::push` → новые байты в хвост → цикл `etf::decode` по хвосту:
//! `UnexpectedEof` («терм неполный») — ждём следующий push, успех — [`Session::on_value`]
//! и снятие `consumed`, прочие ошибки — [`TransportError::Frame`]. Хвост, начинающийся не с
//! `0x83`, — рассинхронизация: ошибка, а не молчаливый сброс (ошибки не глотаются).
//!
//! Счётчики: [`GatewayPipe::frames_received`] — поданные WS-сообщения (учитываются **до**
//! декодирования: ошибка не отменяет факт приёма), [`GatewayPipe::bytes_received`] — сжатые
//! байты провода, [`GatewayPipe::terms_parsed`] — успешно разобранные термы (включая «не кадры
//! шлюза»: HELLO-филлеры и прочие значения без целого `op` считает сама сессия,
//! [`Session::ignored_values`]), [`GatewayPipe::pending_bytes`] — байты в хвосте,
//! [`GatewayPipe::frames_sent`] / [`GatewayPipe::bytes_sent`] — отправленные исходящие кадры.
//!
//! ## Цикл ([`connect_and_run`])
//!
//! `select!` по (a) следующему WS-сообщению и (b) сну до [`Session::next_deadline_ms`] от
//! [`tokio::time::Instant`] старта. На каждом шаге `now_ms = start.elapsed().as_millis()`
//! (насыщение, без паник), затем [`Session::on_timer`]; эффекты: `SendHeartbeat{d}` → бинарный
//! кадр [`encode_heartbeat`], `Reconnect{reason}` → [`RunOutcome::Reconnect`]. Входящее:
//! `Binary` → пайп (+ его эффекты), `Ping` → `Pong`, `Close` → [`RunOutcome::Closed`] (код из
//! кадра) и выход; `Text`/`Pong`/сырой `Frame` → игнор со счётчиком
//! [`GatewayPipe::ignored_messages`] (шлюз их не присылает; молчание было бы потерей сигнала —
//! счётчик доступен вызывающему). Обрыв потока без close-кадра — `Closed { code: None }`.
//! Ошибки WS-слоя и кадра возвращаются вызывающему (`TransportError`): соединение при этом
//! рвётся по `drop`, и решение о реконнекте принимает вызывающий (цикл сам не переподключается).
//!
//! ## TLS (`wss://`)
//!
//! Провайдер — **ring** (не aws-lc-rs). TLS-коннектор собирается с явным ring-провайдером и
//! корнями `webpki-roots` (см. [`tls_connector`]); для `ws://` коннектор не используется.
//! В тестах `wss://` не покрыт (нет удостоверяющего центра и запрещены подключения к живому
//! Discord) — проверена только сборка/ветка конфигурации; цикл одинаков для обоих схем.
//!
//! ## Потоковая распаковка `libzstd` (F-GW-01 / task-137)
//!
//! [`ZstdStream`] работает поверх потокового декодера `libzstd` (`ZSTD_decompressStream`) и
//! отдаёт все распакованные байты каждого WS-фрагмента сразу, без удержания 64 КиБ скользящего
//! окна (как эталонный `zlib.createZstdDecompress()` в клиенте). Для снимка
//! `gw-proto/zstd-frames-95974_1389.hexl` (137 кадров, 104 066 сжатых Б) пайп получает все
//! 824 816 Б и разбирает все 137 полных термов (`s = 1..113` для 113 диспатчей op0, 23 op11 ACK
//! и 1 op10 HELLO) без неразобранного остатка в хвосте.
#![allow(
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_is_multiple_of
)]

use crate::dispatch::{DispatchEvent, route_dispatch};
use crate::gateway::{
    Effect, GatewayError, HeartbeatMode, QOS_HEARTBEAT_OP, QosPayload, ReconnectReason, Session,
};
pub use crate::gateway::{
    GUILD_MEMBERS_CHUNK_SIZE, GUILD_SUBSCRIPTIONS_BULK_OP, OutboundError,
    OutboundGuildMembersRequest, OutboundGuildSubscription, OutboundPresenceUpdate,
    OutboundVoiceStateUpdate, PRESENCE_UPDATE_OP, REQUEST_GUILD_MEMBERS_OP, VOICE_STATE_UPDATE_OP,
    chunk_user_ids, encode_guild_subscriptions, encode_presence_update,
    encode_request_guild_members, encode_voice_state_update,
};
use crate::identify::{IdentifyError, IdentifyParams, build_identify_frame};
use crate::ready::ReadySummary;
use crate::resume::{ResumeError, ResumeParams, ResumeState, build_resume_frame};
use chord_codec::etf::{self, EtfError, EtfValue};
use chord_codec::zstd::{ZstdError, ZstdStream};
use futures_util::{Sink, SinkExt, StreamExt};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::{Connector, WebSocketStream, connect_async_tls_with_config};

/// Алиас тела исходящего op3 (`PRESENCE_UPDATE`).
pub type PresenceUpdate = OutboundPresenceUpdate;
/// Алиас тела исходящего op4 (`VOICE_STATE_UPDATE`).
pub type VoiceStateUpdateOut = OutboundVoiceStateUpdate;
/// Алиас тела исходящего op8 (`REQUEST_GUILD_MEMBERS`).
pub type RequestGuildMembers = OutboundGuildMembersRequest;
/// Алиас карты подписок гильдий для исходящего op37 (`GUILD_SUBSCRIPTIONS_BULK`).
pub type GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>;

/// База шлюза Discord (`GLOBAL_ENV.GATEWAY_ENDPOINT`, AUTH-RE G-01).
pub const GATEWAY_ENDPOINT: &str = "wss://gateway.discord.gg";

/// Параметры шлюза клиента 1.0.162: `encoding=etf&v=9&compress=zstd-stream` (G-01…G-03).
pub const GATEWAY_QUERY: &str = "encoding=etf&v=9&compress=zstd-stream";

/// Ошибки сборки/нормализации адреса шлюза: значения, не паники.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayUrlError {
    /// Пустая строка (или только пробелы).
    Empty,
    /// Неподдерживаемая схема (`ftp://…`); допустимы wss/ws/https/http.
    UnsupportedScheme(String),
    /// После схемы нет хоста (`wss://`, `wss:///path`).
    MissingHost,
    /// Пробельные/управляющие символы в адресе.
    Whitespace,
    /// Фрагмент (`#…`) не является частью адреса шлюза.
    Fragment,
}

impl std::fmt::Display for GatewayUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "пустой URL шлюза"),
            Self::UnsupportedScheme(scheme) => {
                write!(f, "неподдерживаемая схема URL шлюза: {scheme}")
            }
            Self::MissingHost => write!(f, "в URL шлюза нет хоста"),
            Self::Whitespace => write!(f, "пробельные символы в URL шлюза"),
            Self::Fragment => write!(f, "фрагмент (#…) недопустим в URL шлюза"),
        }
    }
}

impl std::error::Error for GatewayUrlError {}

/// Нормализованный адрес шлюза с каноническим набором query-параметров.
///
/// `wss://gateway.discord.gg` и `resume_gateway_url` из READY приводятся к одной форме:
/// схема WS-транспорта (`https` → `wss`, `http` → `ws`, без схемы → `wss`), хвостовой `/`
/// обрезается, старый query отбрасывается, затем добавляется [`GATEWAY_QUERY`] (G-01, G-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayUrl {
    /// Полный URL провода (с каноническим query).
    url: String,
}

impl GatewayUrl {
    /// Боевой адрес Discord по умолчанию.
    #[must_use]
    pub fn remote() -> Self {
        Self {
            url: format!("{GATEWAY_ENDPOINT}/?{GATEWAY_QUERY}"),
        }
    }

    /// Строит адрес из базы (`wss://…`, `ws://…`, `https://…` или хост без схемы).
    pub fn from_base(base: &str) -> Result<Self, GatewayUrlError> {
        normalize_gateway_url(base)
    }

    /// Нормализует `resume_gateway_url` из READY (G-16): тот же контракт, что [`Self::from_base`].
    pub fn resume(resume_gateway_url: &str) -> Result<Self, GatewayUrlError> {
        normalize_gateway_url(resume_gateway_url)
    }

    /// Полный URL для WS-соединения.
    pub fn as_str(&self) -> &str {
        &self.url
    }
}

impl Default for GatewayUrl {
    fn default() -> Self {
        Self::remote()
    }
}

impl std::fmt::Display for GatewayUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.url)
    }
}

/// Общая нормализация базы и resume-URL.
fn normalize_gateway_url(raw: &str) -> Result<GatewayUrl, GatewayUrlError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(GatewayUrlError::Empty);
    }
    if trimmed.contains('#') {
        return Err(GatewayUrlError::Fragment);
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(GatewayUrlError::Whitespace);
    }
    let (scheme, rest) = match trimmed.find("://") {
        Some(index) => {
            let scheme = trimmed[..index].to_ascii_lowercase();
            let rest = &trimmed[index + 3..];
            let mapped = match scheme.as_str() {
                "wss" => "wss",
                "https" => "wss",
                "ws" => "ws",
                "http" => "ws",
                other => return Err(GatewayUrlError::UnsupportedScheme(other.to_string())),
            };
            (mapped, rest)
        }
        // Хост без схемы — боевой wss (GATEWAY_ENDPOINT в HTML без схемы не встречается,
        // но нормализация не должна молча давать нерабочий адрес).
        None => ("wss", trimmed),
    };
    // query из READY/базы отбрасываем: канонический набор один (без дублей).
    let rest = rest.split('?').next().unwrap_or_default();
    let rest = rest.trim_end_matches('/');
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() {
        return Err(GatewayUrlError::MissingHost);
    }
    Ok(GatewayUrl {
        url: format!("{scheme}://{rest}/?{GATEWAY_QUERY}"),
    })
}

/// Пайп шлюза: непрерывный zstd-поток → ETF-термы → эффекты сессии.
pub struct GatewayPipe {
    /// Инкрементальный декодер непрерывного zstd-кадра (RW-02) — один на соединение.
    stream: ZstdStream,
    /// Хвост распакованных, но ещё не разобранных байт (терм может быть разрезан между push).
    tail: Vec<u8>,
    /// Ядро сессии (RW-03): сюда уходят разобранные термы.
    session: Session,
    /// Сколько WS-сообщений подано в пайп (считается до декодирования).
    frames_received: u64,
    /// Сколько сжатых байт провода подано в пайп (считается до декодирования).
    bytes_received: u64,
    /// Сколько термов успешно разобрано (включая значения без `op`).
    terms_parsed: u64,
    /// Сколько WS-сообщений проигнорировано циклом (Text/Pong/сырой Frame).
    ignored_messages: u64,
    /// Сводка последнего READY (op0 `t: "READY"`), прошедшего через пайп (RW-05).
    last_ready: Option<ReadySummary>,
    /// Проходил ли через пайп диспатч `RESUMED` (op0 `t: "RESUMED"`), сохраняется после `take_events`.
    resumed: bool,
    /// `d` последнего op9 INVALID_SESSION: `Some(true)` — resume разрешён (RW-06).
    last_invalid_session_resumable: Option<bool>,
    /// Диспатчи (op0), накопленные с последнего дренажа (RW-07); включая raw-события.
    ///
    /// Буфер растёт линейно с потоком — вызывающий обязан дренировать его ([`Self::take_events`]).
    events: Vec<DispatchEvent>,
    /// Диспатчей с неизвестным типом (raw) — накопительно; дренаж не сбрасывает (RW-07).
    unknown_dispatches: u64,
    /// Сколько исходящих клиентских кадров (op3/op4/op8/op37) отправлено через пайп.
    frames_sent: u64,
    /// Сколько несжатых ETF-байт исходящих клиентских кадров отправлено через пайп.
    bytes_sent: u64,
    /// Очередь исходящих бинарных ETF-кадров для отправки в цикле `connect_and_run*` после HELLO/handshake.
    outbound_queue: VecDeque<Vec<u8>>,
}

impl GatewayPipe {
    /// Новый пайп поверх готовой сессии.
    ///
    /// zstd-поток создаётся пустым и живёт до конца соединения: у шлюза это один непрерывный
    /// кадр, поэтому переиспользовать пайп после реконнекта нельзя — нужен новый
    /// [`ZstdStream`], то есть новый [`GatewayPipe`] и новая [`Session`].
    pub fn new(session: Session) -> Self {
        Self {
            stream: ZstdStream::new(),
            tail: Vec::new(),
            session,
            frames_received: 0,
            bytes_received: 0,
            terms_parsed: 0,
            ignored_messages: 0,
            last_ready: None,
            resumed: false,
            last_invalid_session_resumable: None,
            events: Vec::new(),
            unknown_dispatches: 0,
            frames_sent: 0,
            bytes_sent: 0,
            outbound_queue: VecDeque::new(),
        }
    }

    /// Подаёт одно WS-сообщение (фрагмент zstd-кадра) и возвращает эффекты разобранных термов.
    ///
    /// Ошибки: [`TransportError::Zstd`] — терминальный отказ декодера zstd;
    /// [`TransportError::Frame`] — терм не разобран (кроме неполного) либо нарушена структура
    /// (`Session::on_value`) либо хвост рассинхронизирован. Ошибки не глотаются; счётчики
    /// приёма ([`Self::frames_received`], [`Self::bytes_received`]) растут и при ошибке.
    pub fn push_binary(&mut self, msg: &[u8], now_ms: u64) -> Result<Vec<Effect>, TransportError> {
        self.frames_received = self.frames_received.saturating_add(1);
        self.bytes_received = self.bytes_received.saturating_add(msg.len() as u64);
        let fresh = self.stream.push(msg).map_err(TransportError::Zstd)?;
        if self.tail.len().saturating_add(fresh.len()) > 16 * 1024 * 1024 {
            self.tail.clear();
        }
        self.tail.extend_from_slice(&fresh);

        let mut effects = Vec::new();
        while self.tail.first() == Some(&etf::VERSION_BYTE) {
            match etf::decode(&self.tail) {
                Ok((value, consumed)) if consumed > 0 => {
                    effects.extend(
                        self.session
                            .on_value(&value, now_ms)
                            .map_err(TransportError::Frame)?,
                    );
                    // READY (op0 `t: "READY"`) — сводка для вызывающего; не-READY → `None`.
                    if let Some(ready) = ReadySummary::from_dispatch(&value) {
                        self.last_ready = Some(ready);
                    }
                    // op9 INVALID_SESSION: запоминаем `d` (разрешение на повторный RESUME).
                    if value.get("op").and_then(EtfValue::as_int) == Some(9) {
                        self.last_invalid_session_resumable =
                            value.get("d").and_then(invalid_session_bool);
                    }
                    // Роутер диспатчей (RW-07): op0 → типизированное событие; неизвестный тип —
                    // raw-событие (`kind: None`) и накопительный счётчик, не молчание.
                    if let Some(event) = route_dispatch(&value) {
                        if event.kind == Some(crate::dispatch::DispatchKind::Resumed) {
                            self.resumed = true;
                        }
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        }
                        self.events.push(event);
                    }
                    self.tail.drain(..consumed);
                    self.terms_parsed = self.terms_parsed.saturating_add(1);
                }
                Ok((_value, _consumed)) => {
                    // Недостижимо: `etf::decode` всегда потребляет байт маркера/терма.
                    // Страховка от бесконечного цикла — ошибкой, а не молчанием.
                    return Err(TransportError::Frame(GatewayError::Decode {
                        msg: "etf::decode вернул нулевой consumed".to_string(),
                    }));
                }
                // Терм разрезан границей WS-сообщения: ждём следующий push, хвост копится.
                Err(EtfError::UnexpectedEof { .. }) => break,
                Err(error) => {
                    return Err(TransportError::Frame(GatewayError::Decode {
                        msg: error.to_string(),
                    }));
                }
            }
        }
        // Хвост, начинающийся не с маркера терма, — рассинхронизация потока (не молчаливый сброс).
        match self.tail.first() {
            // Хвост пуст либо начинается с маркера терма — всё в порядке.
            None | Some(&etf::VERSION_BYTE) => {}
            // (MSRV 1.87: без let-chain — подсказка clippy требует 1.88.)
            Some(&first) => {
                return Err(TransportError::Frame(GatewayError::Decode {
                    msg: format!("поток рассинхронизирован: 0x{first:02x} вместо 0x83"),
                }));
            }
        }
        Ok(effects)
    }

    /// Доступ к сессии (состояние HELLO/ACK/RTT/`s`, счётчики — наблюдение вызывающим).
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Сколько WS-сообщений подано в пайп.
    pub fn frames_received(&self) -> u64 {
        self.frames_received
    }

    /// Сколько сжатых байт провода подано в пайп.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Сколько термов успешно разобрано.
    pub fn terms_parsed(&self) -> u64 {
        self.terms_parsed
    }

    /// Сколько байт ждёт разбора в хвосте (диагностика разрезанных термов между WS-сообщениями).
    pub fn pending_bytes(&self) -> usize {
        self.tail.len()
    }

    /// Сколько WS-сообщений проигнорировано циклом (не `Binary`/`Ping`/`Close`).
    pub fn ignored_messages(&self) -> u64 {
        self.ignored_messages
    }

    /// Сводка READY (op0 `t: "READY"`), прошедшего через пайп; `None` — READY ещё не было.
    pub fn last_ready(&self) -> Option<&ReadySummary> {
        self.last_ready.as_ref()
    }

    /// Был ли получен диспатч `RESUMED` в этом соединении (не сбрасывается при [`Self::take_events`]).
    pub fn resumed(&self) -> bool {
        self.resumed
    }

    /// Состояние для RESUME: session_id из READY + последний `s` (RW-06); иначе `None`.
    pub fn resume_state(&self) -> Option<ResumeState> {
        let session_id = self.last_ready.as_ref()?.session_id.as_ref()?;
        let last_seq = self.session.last_seq()?;
        Some(ResumeState {
            session_id: session_id.clone(),
            last_seq,
        })
    }

    /// `d` последнего op9 INVALID_SESSION: `true` — resume разрешён, `false` — нет (RW-06).
    pub fn last_invalid_session_resumable(&self) -> Option<bool> {
        self.last_invalid_session_resumable
    }

    /// Диспатчи, накопленные с последнего дренажа (в порядке потока), включая raw.
    ///
    /// Буфер растёт линейно с потоком: вызывающий дренирует ([`Self::take_events`]).
    pub fn events(&self) -> &[DispatchEvent] {
        &self.events
    }

    /// Забирает все накопленные события (дренаж; вызывающий обязан разгружать буфер).
    pub fn take_events(&mut self) -> Vec<DispatchEvent> {
        std::mem::take(&mut self.events)
    }

    /// Диспатчей с неизвестным типом (raw) — накопительно, дренаж не сбрасывает.
    ///
    /// Неизвестный тип не молчит: событие с `kind: None` в [`Self::events`] + этот счётчик.
    pub fn unknown_dispatches(&self) -> u64 {
        self.unknown_dispatches
    }

    /// Учитывает проигнорированное WS-сообщение (зовёт цикл транспорта: Text/Pong/сырой Frame).
    pub fn note_ignored_message(&mut self) {
        self.ignored_messages = self.ignored_messages.saturating_add(1);
    }

    /// Сколько исходящих клиентских кадров (op3/op4/op8/op37) отправлено через пайп.
    pub fn frames_sent(&self) -> u64 {
        self.frames_sent
    }

    /// Сколько несжатых ETF-байт исходящих клиентских кадров отправлено через пайп.
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Сколько исходящих бинарных кадров ждёт отправки в очереди пайпа.
    pub fn pending_outbound_frames(&self) -> usize {
        self.outbound_queue.len()
    }

    /// Ставит в очередь бинарный ETF-кадр op3 (`PRESENCE_UPDATE`) для отправки после HELLO/handshake.
    pub fn queue_presence_update(
        &mut self,
        update: &OutboundPresenceUpdate,
    ) -> Result<(), TransportError> {
        self.ensure_alive()?;
        let frame = encode_presence_update(update).map_err(TransportError::Outbound)?;
        self.outbound_queue.push_back(frame);
        Ok(())
    }

    /// Ставит в очередь бинарный ETF-кадр op4 (`VOICE_STATE_UPDATE`) для отправки после HELLO/handshake.
    pub fn queue_voice_state_update(
        &mut self,
        update: &OutboundVoiceStateUpdate,
    ) -> Result<(), TransportError> {
        self.ensure_alive()?;
        let frame = encode_voice_state_update(update).map_err(TransportError::Outbound)?;
        self.outbound_queue.push_back(frame);
        Ok(())
    }

    /// Ставит в очередь бинарные ETF-кадры op8 (`REQUEST_GUILD_MEMBERS`) с чанкованием `user_ids`
    /// по [`GUILD_MEMBERS_CHUNK_SIZE`] (100). Возвращает число поставленных в очередь кадров.
    pub fn queue_request_guild_members(
        &mut self,
        request: &OutboundGuildMembersRequest,
    ) -> Result<usize, TransportError> {
        self.ensure_alive()?;
        let frames =
            encode_request_guild_members_frames(request).map_err(TransportError::Outbound)?;
        let count = frames.len();
        self.outbound_queue.extend(frames);
        Ok(count)
    }

    /// Ставит в очередь бинарный ETF-кадр op37 (`GUILD_SUBSCRIPTIONS_BULK`) для отправки после HELLO/handshake.
    pub fn queue_guild_subscriptions_bulk(
        &mut self,
        subscriptions: &BTreeMap<String, OutboundGuildSubscription>,
    ) -> Result<(), TransportError> {
        self.ensure_alive()?;
        let frame = encode_guild_subscriptions(subscriptions).map_err(TransportError::Outbound)?;
        self.outbound_queue.push_back(frame);
        Ok(())
    }

    /// Отправляет бинарный ETF-кадр op3 `PRESENCE_UPDATE` через клиентскую сессию/пайп.
    ///
    /// Ошибки: [`TransportError::Outbound`] — ошибка кодирования ETF;
    /// [`TransportError::Ws`] — сессия мертва либо ошибка записи в WebSocket.
    pub async fn send_presence_update<W>(
        &mut self,
        ws: &mut W,
        update: &OutboundPresenceUpdate,
    ) -> Result<(), TransportError>
    where
        W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        self.ensure_alive()?;
        let frame = encode_presence_update(update).map_err(TransportError::Outbound)?;
        self.send_raw_frame(ws, frame).await
    }

    /// Отправляет бинарный ETF-кадр op4 `VOICE_STATE_UPDATE` через клиентскую сессию/пайп.
    pub async fn send_voice_state_update<W>(
        &mut self,
        ws: &mut W,
        update: &OutboundVoiceStateUpdate,
    ) -> Result<(), TransportError>
    where
        W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        self.ensure_alive()?;
        let frame = encode_voice_state_update(update).map_err(TransportError::Outbound)?;
        self.send_raw_frame(ws, frame).await
    }

    /// Отправляет бинарные ETF-кадры op8 `REQUEST_GUILD_MEMBERS` через клиентскую сессию/пайп
    /// с чанкованием `user_ids` по [`GUILD_MEMBERS_CHUNK_SIZE`] (100).
    ///
    /// Если `request.user_ids` непуст, режет список на чанки по 100 и отправляет отдельный
    /// бинарный ETF-кадр на каждый чанк. Если `request.user_ids` пуст и задан `request.query`,
    /// отправляет один кадр поиска. Если пусты и `user_ids`, и `query`, возвращает `Ok(0)`.
    pub async fn send_request_guild_members<W>(
        &mut self,
        ws: &mut W,
        request: &OutboundGuildMembersRequest,
    ) -> Result<usize, TransportError>
    where
        W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        self.ensure_alive()?;
        let frames =
            encode_request_guild_members_frames(request).map_err(TransportError::Outbound)?;
        let count = frames.len();
        for frame in frames {
            self.send_raw_frame(ws, frame).await?;
        }
        Ok(count)
    }

    /// Отправляет бинарный ETF-кадр op37 `GUILD_SUBSCRIPTIONS_BULK` через клиентскую сессию/пайп.
    pub async fn send_guild_subscriptions_bulk<W>(
        &mut self,
        ws: &mut W,
        subscriptions: &BTreeMap<String, OutboundGuildSubscription>,
    ) -> Result<(), TransportError>
    where
        W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        self.ensure_alive()?;
        let frame = encode_guild_subscriptions(subscriptions).map_err(TransportError::Outbound)?;
        self.send_raw_frame(ws, frame).await
    }

    /// Проверяет, что сессия шлюза жива перед отправкой клиентского кадра.
    fn ensure_alive(&self) -> Result<(), TransportError> {
        if self.session.is_dead() {
            return Err(TransportError::Ws {
                msg: "сессия шлюза мертва: отправка исходящего кадра отклонена".to_string(),
            });
        }
        Ok(())
    }

    /// Пишет один готовый бинарный ETF-кадр в WS-поток и обновляет счётчики отправки пайпа.
    async fn send_raw_frame<W>(&mut self, ws: &mut W, frame: Vec<u8>) -> Result<(), TransportError>
    where
        W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    {
        let len = frame.len() as u64;
        ws.send(Message::binary(frame))
            .await
            .map_err(|error| TransportError::Ws {
                msg: error.to_string(),
            })?;
        self.frames_sent = self.frames_sent.saturating_add(1);
        self.bytes_sent = self.bytes_sent.saturating_add(len);
        Ok(())
    }
}

/// Собирает последовательность бинарных ETF-кадров op8 `REQUEST_GUILD_MEMBERS`
/// с чанкованием `user_ids` по [`GUILD_MEMBERS_CHUNK_SIZE`] (100).
///
/// Если `request.user_ids` непуст, возвращает по одному кадру на каждый чанк из ≤100 id.
/// Если `request.user_ids` пуст и задан `request.query`, возвращает один кадр.
/// Если пусты и `user_ids`, и `query`, возвращает пустой список (кадры не отправляются).
pub fn encode_request_guild_members_frames(
    request: &OutboundGuildMembersRequest,
) -> Result<Vec<Vec<u8>>, OutboundError> {
    if !request.user_ids.is_empty() {
        chunk_user_ids(&request.user_ids)
            .into_iter()
            .map(|chunk| {
                let chunk_request = OutboundGuildMembersRequest {
                    guild_id: request.guild_id.clone(),
                    query: request.query.clone(),
                    limit: request.limit,
                    user_ids: chunk.to_vec(),
                    presences: request.presences,
                    nonce: request.nonce.clone(),
                };
                encode_request_guild_members(&chunk_request)
            })
            .collect()
    } else if request.query.is_some() {
        Ok(vec![encode_request_guild_members(request)?])
    } else {
        Ok(Vec::new())
    }
}

/// Отправляет бинарный ETF-кадр op3 `PRESENCE_UPDATE` в WebSocket-поток.
pub async fn send_presence_update<W>(
    ws: &mut W,
    update: &OutboundPresenceUpdate,
) -> Result<(), TransportError>
where
    W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let frame = encode_presence_update(update).map_err(TransportError::Outbound)?;
    ws.send(Message::binary(frame))
        .await
        .map_err(|error| TransportError::Ws {
            msg: error.to_string(),
        })
}

/// Отправляет бинарный ETF-кадр op4 `VOICE_STATE_UPDATE` в WebSocket-поток.
pub async fn send_voice_state_update<W>(
    ws: &mut W,
    update: &OutboundVoiceStateUpdate,
) -> Result<(), TransportError>
where
    W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let frame = encode_voice_state_update(update).map_err(TransportError::Outbound)?;
    ws.send(Message::binary(frame))
        .await
        .map_err(|error| TransportError::Ws {
            msg: error.to_string(),
        })
}

/// Отправляет бинарные ETF-кадры op8 `REQUEST_GUILD_MEMBERS` в WebSocket-поток
/// с чанкованием `user_ids` по [`GUILD_MEMBERS_CHUNK_SIZE`] (100).
/// Возвращает количество отправленных кадров.
pub async fn send_request_guild_members<W>(
    ws: &mut W,
    request: &OutboundGuildMembersRequest,
) -> Result<usize, TransportError>
where
    W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let frames = encode_request_guild_members_frames(request).map_err(TransportError::Outbound)?;
    let count = frames.len();
    for frame in frames {
        ws.send(Message::binary(frame))
            .await
            .map_err(|error| TransportError::Ws {
                msg: error.to_string(),
            })?;
    }
    Ok(count)
}

/// Отправляет бинарный ETF-кадр op37 `GUILD_SUBSCRIPTIONS_BULK` в WebSocket-поток.
pub async fn send_guild_subscriptions_bulk<W>(
    ws: &mut W,
    subscriptions: &BTreeMap<String, OutboundGuildSubscription>,
) -> Result<(), TransportError>
where
    W: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let frame = encode_guild_subscriptions(subscriptions).map_err(TransportError::Outbound)?;
    ws.send(Message::binary(frame))
        .await
        .map_err(|error| TransportError::Ws {
            msg: error.to_string(),
        })
}

/// `d` из op9 INVALID_SESSION: атом `"true"/"false"` либо int 1/0; иное — `None`.
fn invalid_session_bool(d: &EtfValue) -> Option<bool> {
    match d {
        EtfValue::Atom(name) if name == "true" => Some(true),
        EtfValue::Atom(name) if name == "false" => Some(false),
        EtfValue::Int(1) => Some(true),
        EtfValue::Int(0) => Some(false),
        _ => None,
    }
}

/// Кадр heartbeat (op1) к серверу: `{op: 1, d: <последний s либо null>}` с ведущим `0x83`.
///
/// Отправляется **несжатым** (client→server ETF, спека §3). `s` больше `i64::MAX` (недостижимо
/// для живой сессии) насыщается — без паник на входных данных.
pub fn encode_heartbeat(d: Option<u64>) -> Vec<u8> {
    let value = EtfValue::Map(vec![
        (EtfValue::Atom("op".to_string()), EtfValue::Int(1)),
        (
            EtfValue::Atom("d".to_string()),
            match d {
                Some(seq) => EtfValue::Int(i64::try_from(seq).unwrap_or(i64::MAX)),
                None => EtfValue::Nil,
            },
        ),
    ]);
    etf::encode(&value)
}

/// Кадр QoS-heartbeat (op40) к серверу: `{op:40, d:{seq, qos}}` с ведущим `0x83`.
///
/// Форма — из клиента 1.0.162 (AUTH-RE G-12): `_sendHeartbeat = {seq: this.seq, qos}`,
/// `seq` — число (`0` до первого диспатча), `qos` — `null` либо `{active, reasons}`.
/// Ключи — ETF-строки, как у клиентского wetf-энкодера (`key:"binary"`).
pub fn encode_qos_heartbeat(seq: u64, qos: Option<&QosPayload>) -> Vec<u8> {
    let qos_value = match qos {
        Some(payload) => EtfValue::Map(vec![
            (
                EtfValue::Str("active".to_string()),
                EtfValue::Atom(if payload.active { "true" } else { "false" }.to_string()),
            ),
            (
                EtfValue::Str("reasons".to_string()),
                EtfValue::List(
                    payload
                        .reasons
                        .iter()
                        .map(|reason| EtfValue::Str(reason.clone()))
                        .collect(),
                ),
            ),
        ]),
        None => EtfValue::Nil,
    };
    let value = EtfValue::Map(vec![
        (
            EtfValue::Atom("op".to_string()),
            EtfValue::Int(QOS_HEARTBEAT_OP),
        ),
        (
            EtfValue::Atom("d".to_string()),
            EtfValue::Map(vec![
                (
                    EtfValue::Str("seq".to_string()),
                    EtfValue::Int(i64::try_from(seq).unwrap_or(i64::MAX)),
                ),
                (EtfValue::Str("qos".to_string()), qos_value),
            ]),
        ),
    ]);
    etf::encode(&value)
}

/// Обработчик dispatch-кадра: вызывается на каждый op0 сразу после разбора (L-02).
///
/// Контракт: обработчик **не блокирует** цикл транспорта (доставка — `try_send`/канал).
/// Получает те же события, что видел пайп ([`GatewayPipe::take_events`]), включая raw.
pub type DispatchHandler = Arc<dyn Fn(&DispatchEvent) + Send + Sync>;

/// Опции прогона [`connect_and_run_with`] (RW-05).
#[derive(Clone, Default)]
pub struct RunOptions {
    /// IDENTIFY (op2): отправляется **один раз** после первого HELLO; `None` — не отправлять.
    pub identify: Option<IdentifyParams>,
    /// Необязательный обработчик op0 (L-02): при `Some` буфер пайпа дренируется на каждом
    /// кадре; при `None` поведение прежнее (события копятся, забирает вызывающий).
    pub on_dispatch: Option<DispatchHandler>,
}

impl RunOptions {
    /// Без IDENTIFY и без обработчика — поведение [`connect_and_run`].
    pub fn none() -> Self {
        Self {
            identify: None,
            on_dispatch: None,
        }
    }
}

impl std::fmt::Debug for RunOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunOptions")
            .field("identify", &self.identify)
            .field(
                "on_dispatch",
                &self.on_dispatch.as_ref().map(|_| "<handler>"),
            )
            .finish()
    }
}

/// Исход прогона [`connect_and_run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// Нужен реконнект (причина — из ядра сессии: зомби/op7/op9).
    Reconnect {
        /// Почему нужен реконнект.
        reason: ReconnectReason,
    },
    /// Соединение закрыто (код из close-кадра, если сервер его прислал).
    Closed {
        /// Код закрытия WS (например, 1000), если он был в кадре.
        code: Option<u16>,
    },
}

/// Ошибки транспорта. Ни одна не является паникой: вызывающий решает, переподключаться ли.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Соединение не установлено (URL, TCP, WS-handshake, TLS).
    Connect {
        /// Текст ошибки слоя соединения.
        msg: String,
    },
    /// Ошибка WebSocket-слоя при чтении/записи кадра (соединение рвётся по `drop`).
    Ws {
        /// Текст ошибки WS.
        msg: String,
    },
    /// Ошибка декодера zstd-потока; поток терминальный — продолжать чтение нельзя.
    Zstd(ZstdError),
    /// Ошибка кадра: терм не разобран, нарушена его структура либо поток рассинхронизирован.
    Frame(GatewayError),
    /// Ошибка сборки IDENTIFY (op2) перед отправкой (RW-05).
    Identify(IdentifyError),
    /// Ошибка сборки RESUME (op6) перед отправкой (RW-06).
    Resume(ResumeError),
    /// Ошибка сборки исходящего клиентского кадра (op3/op4/op8/op37).
    Outbound(OutboundError),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect { msg } => write!(f, "соединение не установлено: {msg}"),
            Self::Ws { msg } => write!(f, "ошибка WebSocket: {msg}"),
            Self::Zstd(error) => write!(f, "ошибка zstd-потока: {error}"),
            Self::Frame(error) => write!(f, "ошибка кадра шлюза: {error}"),
            Self::Identify(error) => write!(f, "ошибка сборки IDENTIFY: {error}"),
            Self::Resume(error) => write!(f, "ошибка сборки RESUME: {error}"),
            Self::Outbound(error) => write!(f, "ошибка исходящего кадра: {error}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Zstd(error) => Some(error),
            Self::Frame(error) => Some(error),
            Self::Identify(error) => Some(error),
            Self::Resume(error) => Some(error),
            Self::Outbound(error) => Some(error),
            Self::Connect { .. } | Self::Ws { .. } => None,
        }
    }
}

/// Подключается к шлюзу по `url` (`ws://`/`wss://`) и крутит цикл до исхода.
///
/// Без IDENTIFY: эквивалент [`connect_and_run_with`] с [`RunOptions::none`].
///
/// Один вызов = одно соединение: при [`RunOutcome::Reconnect`] переподключение и новая сессия —
/// забота вызывающего (ядро RW-03 после реконнекта переиспользовать нельзя: `s`/RTT/ACK
/// относятся к соединению). Тайм-аут соединения и общий дедлайн прогона задаёт вызывающий
/// (`tokio::time::timeout`).
pub async fn connect_and_run(
    url: &str,
    pipe: &mut GatewayPipe,
) -> Result<RunOutcome, TransportError> {
    connect_and_run_with(url, pipe, RunOptions::none()).await
}

/// Подключается к шлюзу по `url` с опциями [`RunOptions`] (RW-05); RESUME не используется.
pub async fn connect_and_run_with(
    url: &str,
    pipe: &mut GatewayPipe,
    opts: RunOptions,
) -> Result<RunOutcome, TransportError> {
    connect_and_run_resume(url, pipe, opts, None).await
}

/// Подключается к шлюзу по `url`; после первого HELLO отправляет RESUME (op6, если задан)
/// либо IDENTIFY (op2 из [`RunOptions`]) — ровно один раз (RW-06).
///
/// `resume = Some(p)` — продолжение сессии: IDENTIFY на этом соединении не отправляется вовсе.
/// Ошибки сборки кадра — [`TransportError::Resume`]/[`TransportError::Identify`].
pub async fn connect_and_run_resume(
    url: &str,
    pipe: &mut GatewayPipe,
    opts: RunOptions,
    resume: Option<ResumeParams>,
) -> Result<RunOutcome, TransportError> {
    ensure_crypto_provider();
    let (mut ws, _response) = connect_async_tls_with_config(url, None, false, tls_connector())
        .await
        .map_err(|error| TransportError::Connect {
            msg: error.to_string(),
        })?;

    let start = Instant::now();
    let is_live_wss = url.starts_with("wss://");
    let mut handshake_sent = false;
    // Пайп мог увидеть HELLO до входа в цикл (вызывающий подал кадры сам) — шлём сразу.
    maybe_send_handshake(&mut ws, pipe, &opts, resume.as_ref(), &mut handshake_sent).await?;
    maybe_flush_outbound(&mut ws, pipe, &opts, resume.as_ref(), handshake_sent).await?;
    loop {
        drain_live_outbound_into_pipe(pipe);
        maybe_flush_outbound(&mut ws, pipe, &opts, resume.as_ref(), handshake_sent).await?;
        // Тик таймеров на текущий момент: срок удара либо дедлайн зомби (RW-03).
        let effects = pipe.session.on_timer(elapsed_ms(start));
        if let Some(outcome) = apply_effects(effects, &mut ws, pipe.session(), is_live_wss).await? {
            return Ok(outcome);
        }
        let outbound_notify = live_outbound_notify();
        // Сон до ближайшего дедлайна сессии; дедлайнов нет (до HELLO) — ждём только данные.
        let step = match pipe
            .session
            .next_deadline_ms()
            .map(|ms| start + Duration::from_millis(ms))
        {
            Some(deadline) => tokio::select! {
                message = ws.next() => Step::Message(message),
                () = tokio::time::sleep_until(deadline) => Step::Timer,
                () = outbound_notify.notified() => Step::Outbound,
            },
            None => tokio::select! {
                message = ws.next() => Step::Message(message),
                () = outbound_notify.notified() => Step::Outbound,
            },
        };
        match step {
            // Таймер сработал либо поступил исходящий кадр: новая итерация выполнит flush/on_timer.
            Step::Timer | Step::Outbound => {}
            // Поток закончился без close-кадра (обрыв TCP) — соединения больше нет.
            Step::Message(None) => return Ok(RunOutcome::Closed { code: None }),
            Step::Message(Some(Err(error))) => {
                return Err(TransportError::Ws {
                    msg: error.to_string(),
                });
            }
            Step::Message(Some(Ok(message))) => match message {
                Message::Binary(data) => {
                    // L-02: хук получает уже разобранные op0 даже если этот же фрагмент
                    // затем упал на Zstd/Frame: дренаж обязателен **до** распространения
                    // ошибки, иначе события теряются вместе с закрытием попытки. Кадр,
                    // закрывающий соединение, тоже не теряет свои диспатчи (до эффектов).
                    let parsed = pipe.push_binary(&data, elapsed_ms(start));
                    flush_dispatches(pipe, &opts);
                    let effects = parsed?;
                    if let Some(outcome) =
                        apply_effects(effects, &mut ws, pipe.session(), is_live_wss).await?
                    {
                        return Ok(outcome);
                    }
                    // HELLO разобран этим кадром (или ранее) — IDENTIFY ровно один раз.
                    maybe_send_handshake(
                        &mut ws,
                        pipe,
                        &opts,
                        resume.as_ref(),
                        &mut handshake_sent,
                    )
                    .await?;
                    maybe_flush_outbound(&mut ws, pipe, &opts, resume.as_ref(), handshake_sent)
                        .await?;
                }
                Message::Ping(payload) => {
                    ws.send(Message::Pong(payload))
                        .await
                        .map_err(|error| TransportError::Ws {
                            msg: error.to_string(),
                        })?;
                }
                Message::Close(frame) => {
                    // Клиентский close-кадр в ответ не ждём: соединение закрывается по `drop`
                    // сразу после возврата исхода (ожидание ответа могло бы подвесить цикл).
                    return Ok(RunOutcome::Closed {
                        code: close_code(frame),
                    });
                }
                // Шлюз текста/pong/сырых кадров не присылает; молчание было бы потерей сигнала,
                // поэтому считаем (`GatewayPipe::ignored_messages`), но цикл не рвём.
                Message::Text(_) | Message::Pong(_) | Message::Frame(_) => {
                    pipe.note_ignored_message();
                }
            },
        }
    }
}

/// Отдаёт обработчику накопленные op0 и очищает буфер пайпа (L-02).
///
/// Вызывается после каждого binary-фрагмента **независимо от исхода разбора**:
/// события, разобранные до ошибки `Zstd`/`Frame` в том же фрагменте, доставляются,
/// а не теряются вместе с закрытием попытки.
///
/// Без обработчика буфер не трогается: поведение [`RunOptions::none`] не меняется,
/// события остаются вызывающему ([`GatewayPipe::take_events`]).
fn flush_dispatches(pipe: &mut GatewayPipe, opts: &RunOptions) {
    let Some(handler) = opts.on_dispatch.as_ref() else {
        return;
    };
    for event in pipe.take_events() {
        handler(&event);
    }
}

/// Отправляет RESUME (op6) либо IDENTIFY (op2) один раз после первого HELLO.
///
/// Приоритет у `resume`: на соединении-резюме IDENTIFY не отправляется вовсе. Если не задано
/// ни то, ни другое — не шлём ничего и не помечаем отправку.
async fn maybe_send_handshake<S>(
    ws: &mut WebSocketStream<S>,
    pipe: &GatewayPipe,
    opts: &RunOptions,
    resume: Option<&ResumeParams>,
    sent: &mut bool,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if *sent || pipe.session().hello().is_none() {
        return Ok(());
    }
    let frame = if let Some(params) = resume {
        build_resume_frame(params).map_err(TransportError::Resume)?
    } else if let Some(params) = opts.identify.as_ref() {
        build_identify_frame(params).map_err(TransportError::Identify)?
    } else {
        return Ok(());
    };
    ws.send(Message::binary(frame))
        .await
        .map_err(|error| TransportError::Ws {
            msg: error.to_string(),
        })?;
    *sent = true;
    Ok(())
}

/// Отправляет накопленные в очереди пайпа исходящие кадры (op3/op4/op8/op37) после HELLO/handshake.
///
/// Ни один кадр не уходит до приёма HELLO (и до отправки IDENTIFY/RESUME, если они заданы).
async fn maybe_flush_outbound<S>(
    ws: &mut WebSocketStream<S>,
    pipe: &mut GatewayPipe,
    opts: &RunOptions,
    resume: Option<&ResumeParams>,
    handshake_sent: bool,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if pipe.outbound_queue.is_empty()
        || pipe.session().hello().is_none()
        || pipe.session().is_dead()
    {
        return Ok(());
    }
    let needs_handshake = resume.is_some() || opts.identify.is_some();
    if needs_handshake && !handshake_sent {
        return Ok(());
    }
    while let Some(frame) = pipe.outbound_queue.pop_front() {
        pipe.send_raw_frame(ws, frame).await?;
    }
    Ok(())
}

/// Один шаг цикла: WS-сообщение (или конец потока), сработавший таймер либо сигнал исходящей очереди.
enum Step {
    /// Следующее сообщение потока; `None` — поток закрылся.
    Message(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>),
    /// Сработал таймер: нужен новый тик [`Session::on_timer`].
    Timer,
    /// В глобальную очередь добавлен исходящий кадр (например, op4 `VOICE_STATE_UPDATE`).
    Outbound,
}

static LIVE_OUTBOUND_QUEUE: std::sync::OnceLock<std::sync::Mutex<VecDeque<Vec<u8>>>> =
    std::sync::OnceLock::new();
static LIVE_OUTBOUND_NOTIFY: std::sync::OnceLock<Arc<tokio::sync::Notify>> =
    std::sync::OnceLock::new();

fn live_outbound_queue() -> &'static std::sync::Mutex<VecDeque<Vec<u8>>> {
    LIVE_OUTBOUND_QUEUE.get_or_init(|| std::sync::Mutex::new(VecDeque::new()))
}

fn live_outbound_notify() -> Arc<tokio::sync::Notify> {
    LIVE_OUTBOUND_NOTIFY
        .get_or_init(|| Arc::new(tokio::sync::Notify::new()))
        .clone()
}

fn drain_live_outbound_into_pipe(pipe: &mut GatewayPipe) {
    if let Ok(mut guard) = live_outbound_queue().lock() {
        while let Some(frame) = guard.pop_front() {
            pipe.outbound_queue.push_back(frame);
        }
    }
}

/// Ставит сырой бинарный ETF-кадр в очередь активного соединения шлюза и будит цикл `connect_and_run_resume`.
pub fn send_live_gateway_frame(frame: Vec<u8>) {
    if let Ok(mut guard) = live_outbound_queue().lock() {
        while guard.len() >= 64 {
            guard.pop_front();
        }
        guard.push_back(frame);
    }
    live_outbound_notify().notify_one();
}

/// Кодирует и отправляет `OP 4 VOICE_STATE_UPDATE` в активное соединение шлюза Discord.
/// Для живого шлюза Discord (`encoding=etf`, `wetf` `null: "atom"`) пустой `guild_id`/`channel_id`
/// кодируется атомом `"nil"`, чтобы шлюз Discord гарантированно фиксировал выход из голосового канала.
pub fn queue_live_voice_state(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: bool,
    self_deaf: bool,
) -> Result<(), OutboundError> {
    queue_live_voice_state_ext(
        guild_id, channel_id, self_mute, self_deaf, false, false, None,
    )
}

static LAST_STREAM_KEY: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();
static LAST_WATCHED_STREAM_KEY: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();
static LAST_VOICE_STATE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<(
        Option<String>,
        Option<String>,
        bool,
        bool,
        bool,
        bool,
        Option<String>,
    )>,
> = std::sync::OnceLock::new();
static VOICE_SELF_MUTE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static VOICE_SELF_DEAF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Синхронизирует атомарное состояние `self_mute` и `self_deaf` для мгновенного применения при старте RTC-воркера.
pub fn set_live_voice_mute_deaf_state(self_mute: bool, self_deaf: bool) {
    VOICE_SELF_MUTE.store(self_mute, std::sync::atomic::Ordering::SeqCst);
    VOICE_SELF_DEAF.store(self_deaf, std::sync::atomic::Ordering::SeqCst);
}

/// Возвращает актуальное состояние `(self_mute, self_deaf)`.
#[must_use]
pub fn get_live_voice_mute_deaf_state() -> (bool, bool) {
    (
        VOICE_SELF_MUTE.load(std::sync::atomic::Ordering::Relaxed),
        VOICE_SELF_DEAF.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Расширенная отправка `OP 4 VOICE_STATE_UPDATE` (включая `self_video`) и `OP 18 STREAM_CREATE` / `OP 19 STREAM_DELETE`
/// при включении/выключении демонстрации экрана (`self_stream`), чтобы статус микрофона, наушников, камеры и «В ЭФИРЕ»
/// мгновенно синхронизировался с серверами Discord и отображался у всех участников.
pub fn queue_live_voice_state_ext(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: bool,
    self_deaf: bool,
    self_video: bool,
    self_stream: bool,
    user_id: Option<&str>,
) -> Result<(), OutboundError> {
    use chord_codec::etf::{EtfValue, encode};
    set_live_voice_mute_deaf_state(self_mute, self_deaf);
    let clean_gid = guild_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "@me" && *s != "nil" && *s != "null")
        .map(str::to_string)
        .or_else(|| {
            if channel_id.is_none() {
                LAST_VOICE_STATE_CACHE
                    .get()
                    .and_then(|m| m.lock().ok())
                    .and_then(|c| c.0.clone())
            } else {
                None
            }
        });
    let clean_cid = channel_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "nil" && *s != "null")
        .map(str::to_string);
    if let Ok(mut cache) = LAST_VOICE_STATE_CACHE
        .get_or_init(|| std::sync::Mutex::new((None, None, false, false, false, false, None)))
        .lock()
    {
        *cache = (
            clean_gid.clone(),
            clean_cid.clone(),
            self_mute,
            self_deaf,
            self_video,
            self_stream,
            user_id.map(str::to_string),
        );
    }
    let gid_val = match clean_gid.as_deref() {
        Some(g) => EtfValue::Str(g.to_string()),
        None => EtfValue::Atom("nil".to_string()),
    };
    let cid_val = match clean_cid.as_deref() {
        Some(c) => EtfValue::Str(c.to_string()),
        None => EtfValue::Atom("nil".to_string()),
    };
    let d = EtfValue::Map(vec![
        (EtfValue::Str("guild_id".to_string()), gid_val),
        (EtfValue::Str("channel_id".to_string()), cid_val),
        (
            EtfValue::Str("self_mute".to_string()),
            EtfValue::Atom(if self_mute { "true" } else { "false" }.to_string()),
        ),
        (
            EtfValue::Str("self_deaf".to_string()),
            EtfValue::Atom(if self_deaf { "true" } else { "false" }.to_string()),
        ),
        (
            EtfValue::Str("self_video".to_string()),
            EtfValue::Atom(if self_video { "true" } else { "false" }.to_string()),
        ),
        (
            EtfValue::Str("self_stream".to_string()),
            EtfValue::Atom(if self_stream { "true" } else { "false" }.to_string()),
        ),
        (
            EtfValue::Str("flags".to_string()),
            EtfValue::Int(if self_stream { 2 } else { 0 }),
        ),
    ]);
    let envelope = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(4)),
        (EtfValue::Str("d".to_string()), d),
    ]);
    let frame = encode(&envelope);
    send_live_gateway_frame(frame);

    let stream_lock = LAST_STREAM_KEY.get_or_init(|| std::sync::Mutex::new(None));
    let mut prev_stream_key = stream_lock
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if self_stream {
        if let (Some(c), Some(uid)) = (&clean_cid, user_id.map(str::trim).filter(|s| !s.is_empty()))
        {
            let (skey, stype, gid_etf) = if let Some(g) = &clean_gid {
                (
                    format!("guild:{g}:{c}:{uid}"),
                    "guild",
                    EtfValue::Str(g.clone()),
                )
            } else {
                (
                    format!("call:{c}:{uid}"),
                    "call",
                    EtfValue::Atom("nil".to_string()),
                )
            };
            if prev_stream_key.as_deref() != Some(skey.as_str()) {
                *prev_stream_key = Some(skey);
                let stream_d = EtfValue::Map(vec![
                    (
                        EtfValue::Str("type".to_string()),
                        EtfValue::Str(stype.to_string()),
                    ),
                    (EtfValue::Str("guild_id".to_string()), gid_etf),
                    (
                        EtfValue::Str("channel_id".to_string()),
                        EtfValue::Str(c.clone()),
                    ),
                    (
                        EtfValue::Str("preferred_region".to_string()),
                        EtfValue::Atom("nil".to_string()),
                    ),
                ]);
                let stream_env = EtfValue::Map(vec![
                    (EtfValue::Str("op".to_string()), EtfValue::Int(18)),
                    (EtfValue::Str("d".to_string()), stream_d),
                ]);
                send_live_gateway_frame(encode(&stream_env));
            }
        }
    } else if let Some(old_key) = prev_stream_key.take() {
        let del_d = EtfValue::Map(vec![(
            EtfValue::Str("stream_key".to_string()),
            EtfValue::Str(old_key),
        )]);
        let del_env = EtfValue::Map(vec![
            (EtfValue::Str("op".to_string()), EtfValue::Int(19)),
            (EtfValue::Str("d".to_string()), del_d),
        ]);
        send_live_gateway_frame(encode(&del_env));
        let watching_other = LAST_WATCHED_STREAM_KEY
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .is_some();
        if !watching_other {
            stop_live_stream_rtc_session();
        }
    }
    Ok(())
}

/// Очищает кешированный ключ стрима, позволяя повторно запускать трансляцию на том же канале.
pub fn clear_last_stream_key() {
    if let Ok(mut prev_sk) = LAST_STREAM_KEY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        *prev_sk = None;
    }
}

/// Отправляет в живой шлюз Discord `OP 18 STREAM_CREATE`
/// (`{"op":18,"d":{"type":"guild"|"call","guild_id":...|null,"channel_id":...,"preferred_region":null}}`)
/// для запуска демонстрации экрана / трансляции (`Go Live`) в гильдейском или личном звонке.
pub fn queue_live_stream_create(
    stream_type: &str,
    guild_id: Option<&str>,
    channel_id: &str,
) -> Result<(), OutboundError> {
    use chord_codec::etf::{EtfValue, encode};
    let stype = stream_type.trim();
    let cid = channel_id.trim();
    if cid.is_empty() {
        return Ok(());
    }
    let clean_gid = guild_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "@me" && *s != "nil" && *s != "null");
    let gid_val = match clean_gid {
        Some(gid) => EtfValue::Str(gid.to_string()),
        None => EtfValue::Atom("nil".to_string()),
    };
    let stream_d = EtfValue::Map(vec![
        (
            EtfValue::Str("type".to_string()),
            EtfValue::Str(stype.to_string()),
        ),
        (EtfValue::Str("guild_id".to_string()), gid_val),
        (
            EtfValue::Str("channel_id".to_string()),
            EtfValue::Str(cid.to_string()),
        ),
        (
            EtfValue::Str("preferred_region".to_string()),
            EtfValue::Atom("nil".to_string()),
        ),
    ]);
    let stream_env = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(18)),
        (EtfValue::Str("d".to_string()), stream_d),
    ]);
    send_live_gateway_frame(encode(&stream_env));
    Ok(())
}

/// Отправляет в живой шлюз Discord `OP 20 STREAM_WATCH` (`{"op":20,"d":{"stream_key":...}}`)
/// для подключения к чужой демонстрации экрана (`Go Live`).
pub fn queue_live_stream_watch(stream_key: &str) -> Result<(), OutboundError> {
    use chord_codec::etf::{EtfValue, encode};
    let skey = stream_key.trim();
    if skey.is_empty() {
        return Ok(());
    }
    if let Ok(mut w) = LAST_WATCHED_STREAM_KEY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        *w = Some(skey.to_string());
    }
    let watch_d = EtfValue::Map(vec![(
        EtfValue::Str("stream_key".to_string()),
        EtfValue::Str(skey.to_string()),
    )]);
    let watch_env = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(20)),
        (EtfValue::Str("d".to_string()), watch_d),
    ]);
    send_live_gateway_frame(encode(&watch_env));
    Ok(())
}

/// Отправляет в живой шлюз Discord `OP 19 STREAM_DELETE` (`{"op":19,"d":{"stream_key":...}}`)
/// при прекращении просмотра чужого стрима или остановке собственной трансляции,
/// гарантированно очищая `LAST_STREAM_KEY` для мгновенного перезапуска стрима.
pub fn queue_live_stream_delete(stream_key: &str) -> Result<(), OutboundError> {
    use chord_codec::etf::{EtfValue, encode};
    clear_last_stream_key();
    let skey = stream_key.trim();
    if skey.is_empty() {
        return Ok(());
    }
    if let Ok(mut prev_sk) = LAST_STREAM_KEY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        *prev_sk = None;
    }
    if let Ok(mut w) = LAST_WATCHED_STREAM_KEY
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        if w.as_deref() == Some(skey) {
            *w = None;
        }
    }
    let del_d = EtfValue::Map(vec![(
        EtfValue::Str("stream_key".to_string()),
        EtfValue::Str(skey.to_string()),
    )]);
    let del_env = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(19)),
        (EtfValue::Str("d".to_string()), del_d),
    ]);
    send_live_gateway_frame(encode(&del_env));
    Ok(())
}

/// Отправляет в живой шлюз Discord `OP 22 STREAM_SET_PAUSED` (`{"op":22,"d":{"stream_key":...,"paused":...}}`)
/// для постановки на паузу или возобновления трансляции (`Go Live`).
pub fn queue_live_stream_set_paused(stream_key: &str, paused: bool) -> Result<(), OutboundError> {
    use chord_codec::etf::{EtfValue, encode};
    let skey = stream_key.trim();
    if skey.is_empty() {
        return Ok(());
    }
    let paused_d = EtfValue::Map(vec![
        (
            EtfValue::Str("stream_key".to_string()),
            EtfValue::Str(skey.to_string()),
        ),
        (
            EtfValue::Str("paused".to_string()),
            EtfValue::Atom(if paused { "true" } else { "false" }.to_string()),
        ),
    ]);
    let paused_env = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(22)),
        (EtfValue::Str("d".to_string()), paused_d),
    ]);
    send_live_gateway_frame(encode(&paused_env));
    Ok(())
}

/// Отправляет в живой шлюз Discord `OP 13 CALL_CONNECT` (`{"op":13,"d":{"channel_id":...}}`)
/// для синхронизации текущего состояния звонка в ЛС / групповом чате без автоматического входа или дозвона
/// (`Serein/crates/discord-gateway/src/voice.rs:296`).
pub fn queue_live_dm_call_sync(channel_id: &str) {
    use chord_codec::etf::{EtfValue, encode};
    let cid = channel_id.trim();
    if cid.is_empty() || cid == "nil" || cid == "null" {
        return;
    }
    let d13 = EtfValue::Map(vec![(
        EtfValue::Str("channel_id".to_string()),
        EtfValue::Str(cid.to_string()),
    )]);
    let env13 = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(13)),
        (EtfValue::Str("d".to_string()), d13),
    ]);
    send_live_gateway_frame(encode(&env13));
}

/// Базовый (fallback) диапазон участников канала для `op:37 GUILD_SUBSCRIPTIONS_BULK`.
pub const DEFAULT_GUILD_SUBSCRIPTION_RANGE: [u32; 2] = [0, 99];

/// Максимум диапазонов на один канал в `op:37 GUILD_SUBSCRIPTIONS_BULK` (по эталону Serein).
const MAX_GUILD_SUBSCRIPTION_RANGES: usize = 3;

/// Валидирует диапазоны участников канала (`[start, end]` шириной 100, кратные 100).
/// Если входной срез пуст или не содержит валидных диапазонов, удерживает fallback `[[0, 99]]`.
fn normalize_guild_subscription_ranges(ranges: &[[u32; 2]]) -> Vec<[u32; 2]> {
    let mut valid = Vec::with_capacity(ranges.len().clamp(1, MAX_GUILD_SUBSCRIPTION_RANGES));
    for &pair in ranges {
        let [start, end] = pair;
        if start <= end && end.saturating_sub(start) == 99 && start.is_multiple_of(100) {
            if !valid.contains(&pair) {
                valid.push(pair);
                if valid.len() >= MAX_GUILD_SUBSCRIPTION_RANGES {
                    break;
                }
            }
        }
    }
    if valid.is_empty() {
        valid.push(DEFAULT_GUILD_SUBSCRIPTION_RANGE);
    }
    valid
}

/// Трекер двухшаговых подписок `op:37 GUILD_SUBSCRIPTIONS_BULK` (этап S2 по эталону
/// `/home/sanya/re-deep/gateway-protocol.md:73-81` и `Serein/crates/discord-gateway/src/lib.rs:395-418`).
///
/// Гарантирует:
/// 1. Двухшаговую первичную подписку на гильдию:
///    - Шаг 1: `typing: true, threads: true, activities: true` (без `channels`).
///    - Шаг 2: непустая карта `channels` с диапазонами участников (fallback `[[0, 99]]`).
/// 2. Строгий запрет отправки пустого `channels: {}` или пустых диапазонов `[]` для уже
///    подписанной гильдии с удержанием активного канала и fallback `[[0, 99]]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RangedGuildSubscriptions {
    subscribed_guilds: std::collections::BTreeSet<String>,
    active_channels: BTreeMap<String, (String, Vec<[u32; 2]>)>,
}

impl RangedGuildSubscriptions {
    /// Создаёт пустой трекер подписок гильдий.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Проверяет, прошла ли гильдия двухшаговую инициализацию `op:37`.
    #[must_use]
    pub fn is_subscribed(&self, guild_id: &str) -> bool {
        self.subscribed_guilds.contains(guild_id.trim())
    }

    /// Возвращает текущий активный подписанный канал гильдии.
    #[must_use]
    pub fn active_channel(&self, guild_id: &str) -> Option<&str> {
        self.active_channels
            .get(guild_id.trim())
            .map(|(cid, _)| cid.as_str())
    }

    /// Возвращает текущие активные диапазоны участников для подписанного канала гильдии.
    #[must_use]
    pub fn active_ranges(&self, guild_id: &str) -> Option<&[[u32; 2]]> {
        self.active_channels
            .get(guild_id.trim())
            .map(|(_, ranges)| ranges.as_slice())
    }

    /// Сбрасывает состояние подписок (например, при полной смене сессии шлюза).
    pub fn clear(&mut self) {
        self.subscribed_guilds.clear();
        self.active_channels.clear();
    }

    /// Формирует последовательность бинарных ETF-кадров `op:37 GUILD_SUBSCRIPTIONS_BULK`
    /// для перехода в канал `channel_id` гильдии `guild_id`.
    pub fn subscribe_channel(
        &mut self,
        guild_id: &str,
        channel_id: &str,
        ranges: &[[u32; 2]],
    ) -> Vec<Vec<u8>> {
        build_guild_subscriptions_bulk_op37(self, guild_id, channel_id, ranges)
    }
}

fn encode_op37_flags_only_frame(guild_id: &str) -> Vec<u8> {
    let sub_entry = EtfValue::Map(vec![
        (
            EtfValue::Str("typing".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("threads".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("activities".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
    ]);
    let subs_map = EtfValue::Map(vec![(EtfValue::Str(guild_id.to_string()), sub_entry)]);
    let d37 = EtfValue::Map(vec![(EtfValue::Str("subscriptions".to_string()), subs_map)]);
    let env37 = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(37)),
        (EtfValue::Str("d".to_string()), d37),
    ]);
    etf::encode(&env37)
}

fn encode_op37_channel_ranges_frame(
    guild_id: &str,
    channel_id: &str,
    ranges: &[[u32; 2]],
) -> Vec<u8> {
    let etf_ranges: Vec<EtfValue> = ranges
        .iter()
        .map(|&[start, end]| {
            EtfValue::List(vec![
                EtfValue::Int(i64::from(start)),
                EtfValue::Int(i64::from(end)),
            ])
        })
        .collect();
    let channels_map = EtfValue::Map(vec![(
        EtfValue::Str(channel_id.to_string()),
        EtfValue::List(etf_ranges),
    )]);
    let sub_entry = EtfValue::Map(vec![
        (
            EtfValue::Str("typing".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("threads".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("activities".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("member_updates".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (EtfValue::Str("channels".to_string()), channels_map),
    ]);
    let subs_map = EtfValue::Map(vec![(EtfValue::Str(guild_id.to_string()), sub_entry)]);
    let d37 = EtfValue::Map(vec![(EtfValue::Str("subscriptions".to_string()), subs_map)]);
    let env37 = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(37)),
        (EtfValue::Str("d".to_string()), d37),
    ]);
    etf::encode(&env37)
}

/// Строит бинарные ETF-кадры `op:37 GUILD_SUBSCRIPTIONS_BULK` по двухшаговому контракту S2:
/// - Для новой гильдии возвращает 2 кадра: сначала `typing:true, threads:true, activities:true`,
///   затем кадр с непустыми диапазонами канала (`[[0, 99]]` по умолчанию).
/// - Для уже подписанной гильдии возвращает 1 кадр с непустым `channels` и непустыми диапазонами,
///   запрещая отправку `channels: {}` или `[]` и удерживая fallback `[[0, 99]]`.
pub fn build_guild_subscriptions_bulk_op37(
    state: &mut RangedGuildSubscriptions,
    guild_id: &str,
    channel_id: &str,
    ranges: &[[u32; 2]],
) -> Vec<Vec<u8>> {
    let gid = guild_id.trim();
    if gid.is_empty() || gid == "nil" {
        return Vec::new();
    }
    let cid_trimmed = channel_id.trim();
    let has_valid_cid = !cid_trimmed.is_empty() && cid_trimmed != "nil";
    let effective_ranges = normalize_guild_subscription_ranges(ranges);

    if !state.subscribed_guilds.contains(gid) {
        if !has_valid_cid {
            return Vec::new();
        }
        let step1 = encode_op37_flags_only_frame(gid);
        let step2 = encode_op37_channel_ranges_frame(gid, cid_trimmed, &effective_ranges);
        state.subscribed_guilds.insert(gid.to_string());
        state
            .active_channels
            .insert(gid.to_string(), (cid_trimmed.to_string(), effective_ranges));
        vec![step1, step2]
    } else {
        let effective_cid = if has_valid_cid {
            cid_trimmed.to_string()
        } else if let Some((prev_cid, _)) = state.active_channels.get(gid) {
            prev_cid.clone()
        } else {
            return Vec::new();
        };
        let frame = encode_op37_channel_ranges_frame(gid, &effective_cid, &effective_ranges);
        state
            .active_channels
            .insert(gid.to_string(), (effective_cid, effective_ranges));
        vec![frame]
    }
}

fn live_ranged_guild_subscriptions_mutex() -> &'static std::sync::Mutex<RangedGuildSubscriptions> {
    static SUBS: std::sync::OnceLock<std::sync::Mutex<RangedGuildSubscriptions>> =
        std::sync::OnceLock::new();
    SUBS.get_or_init(|| std::sync::Mutex::new(RangedGuildSubscriptions::new()))
}

/// Отправляет в живой шлюз Discord `OP 14 LAZY_REQUEST` и `OP 37 GUILD_SUBSCRIPTIONS_BULK`
/// (подписка на список участников, ролей и активностей канала).
pub fn queue_live_guild_subscription(guild_id: &str, channel_id: &str) {
    use chord_codec::etf::encode;
    let gid = guild_id.trim();
    let cid = channel_id.trim();
    if gid.is_empty() || cid.is_empty() || gid == "nil" || cid == "nil" {
        return;
    }
    let range0 = EtfValue::List(vec![EtfValue::Int(0), EtfValue::Int(99)]);
    let range1 = EtfValue::List(vec![EtfValue::Int(100), EtfValue::Int(199)]);
    let channels_map = EtfValue::Map(vec![(
        EtfValue::Str(cid.to_string()),
        EtfValue::List(vec![range0, range1]),
    )]);
    let d14 = EtfValue::Map(vec![
        (
            EtfValue::Str("guild_id".to_string()),
            EtfValue::Str(gid.to_string()),
        ),
        (
            EtfValue::Str("typing".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("threads".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("activities".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (
            EtfValue::Str("member_updates".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
        (EtfValue::Str("channels".to_string()), channels_map),
    ]);
    let env14 = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(14)),
        (EtfValue::Str("d".to_string()), d14),
    ]);
    send_live_gateway_frame(encode(&env14));

    let frames = {
        let mut guard = live_ranged_guild_subscriptions_mutex()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        build_guild_subscriptions_bulk_op37(&mut guard, gid, cid, &[[0, 99], [100, 199]])
    };
    for frame in frames {
        send_live_gateway_frame(frame);
    }
}

/// Отправляет в живой шлюз Discord `OP 8 REQUEST_GUILD_MEMBERS` для получения ролей и статусов авторов чата.
pub fn queue_live_request_guild_members(guild_id: &str, user_ids: &[String]) {
    use chord_codec::etf::encode;
    let gid = guild_id.trim();
    if gid.is_empty() || gid == "nil" || user_ids.is_empty() {
        return;
    }
    let uids: Vec<EtfValue> = user_ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "nil" && s.bytes().all(|b| b.is_ascii_digit()))
        .take(100)
        .map(|s| EtfValue::Str(s.to_string()))
        .collect();
    if uids.is_empty() {
        return;
    }
    let d = EtfValue::Map(vec![
        (
            EtfValue::Str("guild_id".to_string()),
            EtfValue::Str(gid.to_string()),
        ),
        (EtfValue::Str("user_ids".to_string()), EtfValue::List(uids)),
        (
            EtfValue::Str("presences".to_string()),
            EtfValue::Atom("true".to_string()),
        ),
    ]);
    let envelope = EtfValue::Map(vec![
        (EtfValue::Str("op".to_string()), EtfValue::Int(8)),
        (EtfValue::Str("d".to_string()), d),
    ]);
    send_live_gateway_frame(encode(&envelope));
}

static LIVE_RTC_TX_OPUS: std::sync::OnceLock<std::sync::Mutex<VecDeque<Vec<u8>>>> =
    std::sync::OnceLock::new();
static LIVE_RTC_RX_OPUS: std::sync::OnceLock<std::sync::Mutex<VecDeque<(u32, Vec<u8>)>>> =
    std::sync::OnceLock::new();

fn live_rtc_tx_opus_mutex() -> &'static std::sync::Mutex<VecDeque<Vec<u8>>> {
    LIVE_RTC_TX_OPUS.get_or_init(|| std::sync::Mutex::new(VecDeque::new()))
}

fn live_rtc_rx_opus_mutex() -> &'static std::sync::Mutex<VecDeque<(u32, Vec<u8>)>> {
    LIVE_RTC_RX_OPUS.get_or_init(|| std::sync::Mutex::new(VecDeque::new()))
}

/// Ставит закодированный 20-мс Opus-кадр микрофона в очередь на отправку через SRTP UDP в голосовой канал.
pub fn push_live_rtc_tx_opus(frame: Vec<u8>) {
    if frame.is_empty() {
        return;
    }
    if let Ok(mut q) = live_rtc_tx_opus_mutex().lock() {
        while q.len() >= 50 {
            q.pop_front();
        }
        q.push_back(frame);
    }
}

/// Забирает расшифрованные входящие Opus-кадры `(ssrc, opus_bytes)` из голосового канала для воспроизведения.
#[must_use]
pub fn drain_live_rtc_rx_opus() -> Vec<(u32, Vec<u8>)> {
    let Ok(mut q) = live_rtc_rx_opus_mutex().lock() else {
        return Vec::new();
    };
    q.drain(..).collect()
}

/// Аппаратные возможности GPU и видеокодеков системы (NVIDIA NVENC/NVDEC, AMD VAAPI/AMF, Intel QSV/VAAPI).
#[derive(Debug, Clone)]
pub struct GpuCodecCapabilities {
    pub vendor: String,
    pub model: String,
    pub driver_api: String,
    pub render_nodes: Vec<String>,
    pub hw_encoders: Vec<String>,
    pub hw_decoders: Vec<String>,
    pub preferred_video_codec: String,
}

static GPU_CODEC_CAPS: std::sync::OnceLock<GpuCodecCapabilities> = std::sync::OnceLock::new();

/// Определяет установленные видеокарты (NVIDIA / AMD / Intel) и поддерживаемые аппаратные видеокодеки
/// (`AV1`, `H265/HEVC`, `H264`, `VP9`, `VP8`) один раз за время жизни процесса.
#[must_use]
pub fn probe_gpu_and_codecs() -> &'static GpuCodecCapabilities {
    GPU_CODEC_CAPS.get_or_init(|| {
        let mut render_nodes = Vec::new();
        let mut has_nvidia = false;
        let mut has_amd = false;
        let mut has_intel = false;
        if let Ok(entries) = std::fs::read_dir("/dev/dri") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("renderD") {
                    render_nodes.push(format!("/dev/dri/{name}"));
                }
            }
        }
        render_nodes.sort();
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path().join("device/vendor");
                if let Ok(v) = std::fs::read_to_string(path) {
                    let clean = v.trim().to_ascii_lowercase();
                    if clean.contains("10de") {
                        has_nvidia = true;
                    } else if clean.contains("1002") {
                        has_amd = true;
                    } else if clean.contains("8086") {
                        has_intel = true;
                    }
                }
            }
        }
        if std::path::Path::new("/dev/nvidiactl").exists() {
            has_nvidia = true;
        }
        let mut model = String::new();
        if has_nvidia {
            if let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") {
                for entry in entries.flatten() {
                    if let Ok(info) = std::fs::read_to_string(entry.path().join("information")) {
                        for line in info.lines() {
                            if let Some(rest) = line.strip_prefix("Model:") {
                                let m = rest.trim();
                                if !m.is_empty() {
                                    model = m.to_string();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if model.is_empty() {
                model = "NVIDIA GeForce RTX (NVENC / NVDEC)".to_string();
            }
        } else if has_amd {
            model = "AMD Radeon Graphics (VAAPI / AMF)".to_string();
        } else if has_intel {
            model = "Intel Graphics (QSV / VAAPI)".to_string();
        } else {
            model = "Linux DRM/EGL GPU".to_string();
        }

        let (vendor, driver_api, hw_encoders, hw_decoders, preferred_video_codec) = if has_nvidia {
            let api = if has_amd {
                "NVENC / NVDEC + VAAPI (Hybrid GPU)"
            } else {
                "NVENC / NVDEC + CUDA / Vulkan"
            };
            (
                "NVIDIA".to_string(),
                api.to_string(),
                vec![
                    "AV1 (av1_nvenc)".to_string(),
                    "H265 (hevc_nvenc)".to_string(),
                    "H264 (h264_nvenc)".to_string(),
                    "VP9 (vaapi)".to_string(),
                    "VP8".to_string(),
                ],
                vec![
                    "AV1 (NVDEC)".to_string(),
                    "H265 (NVDEC)".to_string(),
                    "H264 (NVDEC)".to_string(),
                    "VP9 (NVDEC)".to_string(),
                    "VP8 (NVDEC)".to_string(),
                ],
                "H264".to_string(),
            )
        } else if has_amd {
            (
                "AMD".to_string(),
                "VAAPI / Vulkan / AMF".to_string(),
                vec![
                    "AV1 (av1_vaapi)".to_string(),
                    "H265 (hevc_vaapi)".to_string(),
                    "H264 (h264_vaapi)".to_string(),
                    "VP9 (vp9_vaapi)".to_string(),
                    "VP8".to_string(),
                ],
                vec![
                    "AV1 (VAAPI)".to_string(),
                    "H265 (VAAPI)".to_string(),
                    "H264 (VAAPI)".to_string(),
                    "VP9 (VAAPI)".to_string(),
                    "VP8 (VAAPI)".to_string(),
                ],
                "H264".to_string(),
            )
        } else if has_intel {
            (
                "Intel".to_string(),
                "OneVPL / QSV / VAAPI".to_string(),
                vec![
                    "AV1 (av1_qsv)".to_string(),
                    "H265 (hevc_qsv)".to_string(),
                    "H264 (h264_qsv)".to_string(),
                    "VP9 (vp9_qsv)".to_string(),
                    "VP8".to_string(),
                ],
                vec![
                    "AV1 (QSV)".to_string(),
                    "H265 (QSV)".to_string(),
                    "H264 (QSV)".to_string(),
                    "VP9 (QSV)".to_string(),
                    "VP8 (QSV)".to_string(),
                ],
                "H264".to_string(),
            )
        } else {
            (
                "Generic".to_string(),
                "OpenGL / Vulkan".to_string(),
                vec!["H264".to_string(), "VP8".to_string()],
                vec![
                    "AV1".to_string(),
                    "H265".to_string(),
                    "H264".to_string(),
                    "VP9".to_string(),
                    "VP8".to_string(),
                ],
                "VP8".to_string(),
            )
        };

        GpuCodecCapabilities {
            vendor,
            model,
            driver_api,
            render_nodes,
            hw_encoders,
            hw_decoders,
            preferred_video_codec,
        }
    })
}

/// Состояние живой сессии голосового RTC-шлюза Discord (`wss://<endpoint>/?v=8`).
#[derive(Debug, Clone)]
pub struct LiveRtcSnapshot {
    pub state: String,
    pub endpoint: Option<String>,
    pub short_hostname: Option<String>,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub session_id: Option<String>,
    pub ssrc: u32,
    pub last_ping_ms: Option<u64>,
    pub average_ping_ms: Option<u64>,
    pub ping_history: Vec<u64>,
    pub packet_loss_pct: f64,
    pub protocol: String,
    pub encryption_mode: String,
    pub speaking_users: Vec<String>,
    pub error: Option<String>,
    pub gpu_vendor: String,
    pub gpu_model: String,
    pub gpu_api: String,
    pub hw_encoders: Vec<String>,
    pub hw_decoders: Vec<String>,
    pub active_video_codec: String,
}

impl Default for LiveRtcSnapshot {
    fn default() -> Self {
        let caps = probe_gpu_and_codecs();
        Self {
            state: "disconnected".to_string(),
            endpoint: None,
            short_hostname: None,
            guild_id: None,
            channel_id: None,
            session_id: None,
            ssrc: 0,
            last_ping_ms: None,
            average_ping_ms: None,
            ping_history: Vec::new(),
            packet_loss_pct: 0.0,
            protocol: "udp".to_string(),
            encryption_mode: "aead_aes256_gcm_rtpsize".to_string(),
            speaking_users: Vec::new(),
            error: None,
            gpu_vendor: caps.vendor.clone(),
            gpu_model: caps.model.clone(),
            gpu_api: caps.driver_api.clone(),
            hw_encoders: caps.hw_encoders.clone(),
            hw_decoders: caps.hw_decoders.clone(),
            active_video_codec: caps.preferred_video_codec.clone(),
        }
    }
}

static LIVE_RTC_STATE: std::sync::OnceLock<std::sync::Mutex<LiveRtcSnapshot>> =
    std::sync::OnceLock::new();
static LIVE_RTC_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LIVE_VOICE_DV_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static LIVE_STREAM_DV_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static STREAM_AUDIO_MUTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static STREAM_AUDIO_VOLUME_PCT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(100);
static NATIVE_VOICE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static NATIVE_VOICE_CMD_QUEUE: std::sync::OnceLock<std::sync::Mutex<VecDeque<String>>> =
    std::sync::OnceLock::new();
static NATIVE_STREAM_CMD_QUEUE: std::sync::OnceLock<std::sync::Mutex<VecDeque<String>>> =
    std::sync::OnceLock::new();

fn native_voice_cmd_mutex() -> &'static std::sync::Mutex<VecDeque<String>> {
    NATIVE_VOICE_CMD_QUEUE.get_or_init(|| std::sync::Mutex::new(VecDeque::new()))
}

fn native_stream_cmd_mutex() -> &'static std::sync::Mutex<VecDeque<String>> {
    NATIVE_STREAM_CMD_QUEUE.get_or_init(|| std::sync::Mutex::new(VecDeque::new()))
}

/// Возвращает `true`, если активен нативный движок `chord_voice.node` (с поддержкой DAVE E2EE / MLS и Krisp).
#[must_use]
pub fn is_native_voice_engine_active() -> bool {
    NATIVE_VOICE_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
}

/// Отправляет управляющую JSON-команду в дочерний процесс `chord_voice.node` голосового канала.
pub fn send_native_voice_cmd(cmd_json: &str) {
    if cmd_json.trim().is_empty() {
        return;
    }
    if let Ok(mut q) = native_voice_cmd_mutex().lock() {
        while q.len() >= 64 {
            q.pop_front();
        }
        q.push_back(cmd_json.trim().to_string());
    }
}

/// Отправляет управляющую JSON-команду в дочерний процесс `chord_voice.node` просмотра/трансляции стрима.
pub fn send_native_stream_cmd(cmd_json: &str) {
    if cmd_json.trim().is_empty() {
        return;
    }
    if let Ok(mut q) = native_stream_cmd_mutex().lock() {
        while q.len() >= 64 {
            q.pop_front();
        }
        q.push_back(cmd_json.trim().to_string());
    }
}

/// Устанавливает состояние отключения звука (`muted`) и громкости (`0..=200%`) для просматриваемой трансляции (стрима).
pub fn set_live_stream_audio_state(muted: bool, volume_pct: u32) {
    let clamped = volume_pct.min(200);
    STREAM_AUDIO_MUTED.store(muted, std::sync::atomic::Ordering::SeqCst);
    STREAM_AUDIO_VOLUME_PCT.store(clamped, std::sync::atomic::Ordering::SeqCst);
    let cmd = format!("{{\"cmd\":\"set_stream_audio\",\"muted\":{muted},\"volume\":{clamped}}}");
    send_native_stream_cmd(&cmd);
    send_native_voice_cmd(&cmd);
}

/// Возвращает текущее состояние звука просматриваемой трансляции `(muted, volume_pct)`.
#[must_use]
pub fn get_live_stream_audio_state() -> (bool, u32) {
    (
        STREAM_AUDIO_MUTED.load(std::sync::atomic::Ordering::Relaxed),
        STREAM_AUDIO_VOLUME_PCT.load(std::sync::atomic::Ordering::Relaxed),
    )
}

const DV_SCOPE_FIX_C: &str = r#"#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <pthread.h>
#include <turbojpeg.h>

typedef void* napi_env;
typedef void* napi_value;
typedef void* napi_handle_scope;
typedef void* napi_callback_info;
typedef int napi_status;
typedef napi_value (*napi_callback)(napi_env, napi_callback_info);

typedef napi_status (*open_scope_fn)(napi_env, napi_handle_scope*);
typedef napi_status (*close_scope_fn)(napi_env, napi_handle_scope);
typedef napi_status (*get_uv_loop_fn)(napi_env, void**);
typedef napi_status (*get_cb_info_fn)(napi_env, napi_callback_info, size_t*, napi_value*, napi_value*, void**);
typedef napi_status (*get_typedarray_info_fn)(napi_env, napi_value, int*, size_t*, void**, napi_value*, size_t*);
typedef napi_status (*get_value_int32_fn)(napi_env, napi_value, int32_t*);
typedef napi_status (*create_function_fn)(napi_env, const char*, size_t, napi_callback, void*, napi_value*);
typedef napi_status (*set_named_property_fn)(napi_env, napi_value, const char*, napi_value);
typedef void (*uv_async_cb_t)(void* handle);
typedef int (*uv_async_init_fn)(void* loop, void* async, uv_async_cb_t cb);

static open_scope_fn orig_open_scope = NULL;
static close_scope_fn orig_close_scope = NULL;
static get_cb_info_fn fn_get_cb_info = NULL;
static get_typedarray_info_fn fn_get_typedarray_info = NULL;
static get_value_int32_fn fn_get_value_int32 = NULL;
static create_function_fn fn_create_function = NULL;
static set_named_property_fn fn_set_named_property = NULL;
static uv_async_init_fn tramp_uv_async_init = NULL;

static napi_env g_main_env = NULL;
static void* g_main_loop = NULL;
static pthread_t g_main_thread;

typedef struct {
  void* async;
  uv_async_cb_t cb;
} async_entry_t;

static async_entry_t g_async_table[128];
static int g_async_count = 0;
static pthread_mutex_t g_async_mu = PTHREAD_MUTEX_INITIALIZER;

static void wrapped_uv_async_cb(void* handle) {
  uv_async_cb_t target_cb = NULL;
  pthread_mutex_lock(&g_async_mu);
  for (int i = 0; i < g_async_count; i++) {
    if (g_async_table[i].async == handle) {
      target_cb = g_async_table[i].cb;
      break;
    }
  }
  pthread_mutex_unlock(&g_async_mu);
  if (!target_cb) return;

  if (g_main_env && pthread_equal(pthread_self(), g_main_thread) && orig_open_scope && orig_close_scope) {
    napi_handle_scope scope = NULL;
    orig_open_scope(g_main_env, &scope);
    target_cb(handle);
    if (scope) {
      orig_close_scope(g_main_env, scope);
    }
  } else {
    target_cb(handle);
  }
}

static int hook_uv_async_init(void* loop, void* async, uv_async_cb_t cb) {
  if (cb && (!g_main_loop || loop == g_main_loop)) {
    pthread_mutex_lock(&g_async_mu);
    int found = 0;
    for (int i = 0; i < g_async_count; i++) {
      if (g_async_table[i].async == async) {
        g_async_table[i].cb = cb;
        found = 1;
        break;
      }
    }
    if (!found && g_async_count < 128) {
      g_async_table[g_async_count].async = async;
      g_async_table[g_async_count].cb = cb;
      g_async_count++;
    }
    pthread_mutex_unlock(&g_async_mu);
    return tramp_uv_async_init(loop, async, wrapped_uv_async_cb);
  }
  return tramp_uv_async_init(loop, async, cb);
}

typedef struct {
  int width, height;
  int xoffset, format;
  char *data;
  int byte_order, bitmap_unit, bitmap_bit_order, bitmap_pad, depth;
  int bytes_per_line, bits_per_pixel;
  unsigned long red_mask, green_mask, blue_mask;
} XImage_min;

#define CAP_RING_SLOTS 3
typedef struct {
  uint8_t* bgra;
  size_t cap;
  int w;
  int h;
} cap_slot_t;

static cap_slot_t g_cap_slots[CAP_RING_SLOTS];
static volatile int g_cap_active_slot = -1;
static int g_cap_write_slot = 0;
static volatile uint64_t g_last_capture_req_ms = 0;
static int g_capture_thread_started = 0;

static uint64_t monotonic_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (uint64_t)ts.tv_sec * 1000ULL + (uint64_t)(ts.tv_nsec / 1000000ULL);
}

typedef struct {
  int fd;
  uint8_t* buf;
  size_t cap;
  size_t len;
} kwin_pipe_reader_t;

static void* kwin_pipe_reader_thread(void* arg) {
  kwin_pipe_reader_t* pr = (kwin_pipe_reader_t*)arg;
  while (pr->len < pr->cap) {
    ssize_t n = read(pr->fd, pr->buf + pr->len, pr->cap - pr->len);
    if (n <= 0) break;
    pr->len += (size_t)n;
  }
  close(pr->fd);
  return NULL;
}

static int capture_kwin_dbus_to_ring(GDBusConnection* bus, uint8_t* raw_scratch, size_t raw_cap, tjhandle tj_enc, uint32_t frame_idx) {
  if (!bus || !raw_scratch) return 0;
  int pfd[2];
  if (pipe2(pfd, O_CLOEXEC) != 0) return 0;
  fcntl(pfd[1], F_SETPIPE_SZ, 1048576);

  kwin_pipe_reader_t pr = { .fd = pfd[0], .buf = raw_scratch, .cap = raw_cap, .len = 0 };
  pthread_t rth;
  if (pthread_create(&rth, NULL, kwin_pipe_reader_thread, &pr) != 0) {
    close(pfd[0]);
    close(pfd[1]);
    return 0;
  }

  GUnixFDList* fdl = g_unix_fd_list_new();
  g_unix_fd_list_append(fdl, pfd[1], NULL);
  close(pfd[1]);

  GVariantBuilder b;
  g_variant_builder_init(&b, G_VARIANT_TYPE("a{sv}"));
  g_variant_builder_add(&b, "{sv}", "include-cursor", g_variant_new_boolean(TRUE));
  g_variant_builder_add(&b, "{sv}", "native-resolution", g_variant_new_boolean(FALSE));

  char target_buf[128] = {0};
  int kicked = (access("/dev/shm/discord_capture_kick.bin", F_OK) == 0);
  if (kicked) {
    unlink("/dev/shm/discord_capture_kick.bin");
    __atomic_store_n(&g_last_capture_req_ms, monotonic_ms(), __ATOMIC_RELAXED);
  }
  FILE* tfp_target = fopen("/dev/shm/discord_capture_target.txt", "r");
  if (tfp_target) {
    if (fgets(target_buf, sizeof(target_buf), tfp_target)) {
      char* nl = strpbrk(target_buf, "\r\n");
      if (nl) *nl = '\0';
      char* open_brace = strchr(target_buf, '{');
      char* close_brace = open_brace ? strchr(open_brace, '}') : NULL;
      if (open_brace && close_brace && close_brace > open_brace) {
        close_brace[1] = '\0';
        if (open_brace != target_buf) {
          memmove(target_buf, open_brace, strlen(open_brace) + 1);
        }
      }
    }
    fclose(tfp_target);
  }

  const char* method = "CaptureWorkspace";
  GVariant* params = NULL;
  if (target_buf[0] == '{') {
    method = "CaptureWindow";
    params = g_variant_new("(sa{sv}h)", target_buf, &b, 0);
  } else {
    params = g_variant_new("(a{sv}h)", &b, 0);
  }

  GVariant* res = g_dbus_connection_call_with_unix_fd_list_sync(
    bus,
    "org.kde.KWin",
    "/org/kde/KWin/ScreenShot2",
    "org.kde.KWin.ScreenShot2",
    method,
    params,
    G_VARIANT_TYPE("(a{sv})"),
    G_DBUS_CALL_FLAGS_NONE,
    600,
    fdl,
    NULL,
    NULL,
    NULL
  );
  g_object_unref(fdl);
  pthread_join(rth, NULL);

  if (!res) return 0;

  GVariant* dict = g_variant_get_child_value(res, 0);
  guint32 src_w = 0, src_h = 0, src_stride = 0, src_format = 0;
  g_variant_lookup(dict, "width", "u", &src_w);
  g_variant_lookup(dict, "height", "u", &src_h);
  g_variant_lookup(dict, "stride", "u", &src_stride);
  g_variant_lookup(dict, "format", "u", &src_format);
  g_variant_unref(dict);
  g_variant_unref(res);

  if (src_w == 0 || src_h == 0 || src_w > 7680 || src_h > 4320) return 0;
  size_t eff_stride = src_stride >= src_w * 4 ? (size_t)src_stride : (size_t)src_w * 4;
  if (pr.len < eff_stride * (size_t)src_h) return 0;

  const int dst_w = 1280;
  const int dst_h = 720;
  const size_t needed = (size_t)dst_w * (size_t)dst_h * 4;
  int slot_idx = g_cap_write_slot % CAP_RING_SLOTS;
  cap_slot_t* slot = &g_cap_slots[slot_idx];
  if (!slot->bgra || slot->cap < needed) {
    uint8_t* nb = (uint8_t*)realloc(slot->bgra, needed);
    if (!nb) return 0;
    slot->bgra = nb;
    slot->cap = needed;
  }

  int need_swap = (src_format == 16 || src_format == 17 || src_format == 18 || src_format == 0);
  uint32_t* dst32 = (uint32_t*)slot->bgra;
  if (src_w == (guint32)dst_w && src_h == (guint32)dst_h && eff_stride == (size_t)dst_w * 4) {
    const uint32_t* s32 = (const uint32_t*)raw_scratch;
    size_t total_px = (size_t)dst_w * (size_t)dst_h;
    if (need_swap) {
      for (size_t i = 0; i < total_px; i++) {
        uint32_t px = s32[i];
        dst32[i] = ((px & 0x000000FFU) << 16) | ((px & 0x00FF0000U) >> 16) | (px & 0xFF00FF00U);
      }
    } else {
      memcpy(dst32, s32, total_px * 4);
    }
  } else {
    for (int row = 0; row < dst_h; row++) {
      int sy = (row * (int)src_h) / dst_h;
      const uint32_t* srow = (const uint32_t*)(raw_scratch + (size_t)sy * eff_stride);
      uint32_t* drow = dst32 + (size_t)row * (size_t)dst_w;
      for (int col = 0; col < dst_w; col++) {
        uint32_t px = srow[(col * (int)src_w) / dst_w];
        drow[col] = need_swap ? (((px & 0x000000FFU) << 16) | ((px & 0x00FF0000U) >> 16) | (px & 0xFF00FF00U)) : px;
      }
    }
  }

  slot->w = dst_w;
  slot->h = dst_h;
  __atomic_store_n(&g_cap_active_slot, slot_idx, __ATOMIC_RELEASE);
  g_cap_write_slot = (slot_idx + 1) % CAP_RING_SLOTS;

  if (tj_enc && (kicked || (frame_idx % 8 == 0))) {
    unsigned char* thumb_buf = NULL;
    unsigned long thumb_sz = 0;
    if (tjCompress2(tj_enc, slot->bgra, dst_w, dst_w * 4, dst_h, TJPF_BGRA, &thumb_buf, &thumb_sz, TJSAMP_420, 82, TJFLAG_FASTDCT) == 0 &&
        thumb_buf && thumb_sz > 64) {
      FILE* tfp = fopen("/dev/shm/discord_live_preview_thumb_tmp.jpg", "wb");
      if (tfp) {
        fwrite(thumb_buf, 1, thumb_sz, tfp);
        fclose(tfp);
        rename("/dev/shm/discord_live_preview_thumb_tmp.jpg", "/dev/shm/discord_live_preview_thumb.jpg");
      }
    }
    if (thumb_buf) tjFree(thumb_buf);
  }
  return 1;
}

static int load_jpeg_to_cap_ring(tjhandle tj_dec, tjhandle tj_enc, const char* path) {
  FILE* fp = fopen(path, "rb");
  if (!fp) return 0;
  if (fseek(fp, 0, SEEK_END) != 0) {
    fclose(fp);
    return 0;
  }
  long fsize = ftell(fp);
  if (fsize <= 128 || fsize > 32 * 1024 * 1024) {
    fclose(fp);
    return 0;
  }
  rewind(fp);
  unsigned char* jbuf = (unsigned char*)malloc((size_t)fsize);
  if (!jbuf) {
    fclose(fp);
    return 0;
  }
  size_t nread = fread(jbuf, 1, (size_t)fsize, fp);
  fclose(fp);
  if (nread < (size_t)fsize) {
    free(jbuf);
    return 0;
  }
  int w = 0, h = 0, subsamp = 0, cs = 0;
  if (tjDecompressHeader3(tj_dec, jbuf, (unsigned long)nread, &w, &h, &subsamp, &cs) != 0 ||
      w <= 0 || h <= 0 || w > 7680 || h > 4320) {
    free(jbuf);
    return 0;
  }
  int slot_idx = g_cap_write_slot % CAP_RING_SLOTS;
  cap_slot_t* slot = &g_cap_slots[slot_idx];
  size_t needed = (size_t)w * (size_t)h * 4;
  if (!slot->bgra || slot->cap < needed) {
    uint8_t* nb = (uint8_t*)realloc(slot->bgra, needed);
    if (!nb) {
      free(jbuf);
      return 0;
    }
    slot->bgra = nb;
    slot->cap = needed;
  }
  int rc = tjDecompress2(
    tj_dec,
    jbuf,
    (unsigned long)nread,
    slot->bgra,
    w,
    w * 4,
    h,
    TJPF_BGRA,
    TJFLAG_FASTDCT
  );
  free(jbuf);
  if (rc != 0) return 0;
  slot->w = w;
  slot->h = h;
  __atomic_store_n(&g_cap_active_slot, slot_idx, __ATOMIC_RELEASE);
  g_cap_write_slot = (slot_idx + 1) % CAP_RING_SLOTS;

  if (tj_enc) {
    unsigned char* thumb_buf = NULL;
    unsigned long thumb_sz = 0;
    if (tjCompress2(tj_enc, slot->bgra, w, w * 4, h, TJPF_BGRA, &thumb_buf, &thumb_sz, TJSAMP_420, 80, TJFLAG_FASTDCT) == 0 &&
        thumb_buf && thumb_sz > 64) {
      FILE* tfp = fopen("/dev/shm/discord_live_preview_thumb_tmp.jpg", "wb");
      if (tfp) {
        fwrite(thumb_buf, 1, thumb_sz, tfp);
        fclose(tfp);
        rename("/dev/shm/discord_live_preview_thumb_tmp.jpg", "/dev/shm/discord_live_preview_thumb.jpg");
      }
    }
    if (thumb_buf) tjFree(thumb_buf);
  }
  return 1;
}

static void* screen_capture_worker(void* arg) {
  (void)arg;
  tjhandle tj_dec = tjInitDecompress();
  tjhandle tj_enc = tjInitCompress();
  GDBusConnection* bus = g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, NULL);
  size_t raw_cap = 3840ULL * 2160ULL * 4ULL;
  uint8_t* raw_scratch = (uint8_t*)malloc(raw_cap);
  uint32_t frame_idx = 0;
  struct stat st_prev;
  memset(&st_prev, 0, sizeof(st_prev));
  __atomic_store_n(&g_last_capture_req_ms, monotonic_ms(), __ATOMIC_RELAXED);

  while (1) {
    uint64_t t_start = monotonic_ms();
    if (access("/dev/shm/discord_capture_kick.bin", F_OK) == 0) {
      if (!bus || !raw_scratch) {
        unlink("/dev/shm/discord_capture_kick.bin");
      }
      frame_idx = 0;
      __atomic_store_n(&g_last_capture_req_ms, t_start, __ATOMIC_RELAXED);
    }
    uint64_t req = __atomic_load_n(&g_last_capture_req_ms, __ATOMIC_RELAXED);
    if (req == 0 || t_start - req > 2500) {
      usleep(40000);
      continue;
    }

    int captured = 0;
    if (bus && raw_scratch) {
      captured = capture_kwin_dbus_to_ring(bus, raw_scratch, raw_cap, tj_enc, frame_idx++);
    }
    if (!captured && tj_dec) {
      struct stat st_now;
      if (stat("/dev/shm/discord_live_screen.jpg", &st_now) == 0 && st_now.st_size > 256) {
        if (st_now.st_mtim.tv_sec != st_prev.st_mtim.tv_sec ||
            st_now.st_mtim.tv_nsec != st_prev.st_mtim.tv_nsec) {
          st_prev = st_now;
          load_jpeg_to_cap_ring(tj_dec, tj_enc, "/dev/shm/discord_live_screen.jpg");
        }
      }
    }

    uint64_t elapsed = monotonic_ms() - t_start;
    if (elapsed < 16) {
      usleep((useconds_t)((16 - elapsed) * 1000ULL));
    } else {
      usleep(1000);
    }
  }
  return NULL;
}

static int hook_XShmQueryVersion(void* dpy, int* major, int* minor, int* pixmaps) {
  if (major) *major = 1;
  if (minor) *minor = 1;
  if (pixmaps) *pixmaps = 0;
  return 1;
}

static int hook_XShmGetImage(void* dpy, unsigned long d, XImage_min* image, int x, int y, unsigned long plane_mask) {
  (void)dpy; (void)d; (void)x; (void)y; (void)plane_mask;
  uint64_t now = monotonic_ms();
  __atomic_store_n(&g_last_capture_req_ms, now, __ATOMIC_RELAXED);
  if (!image || !image->data || image->width <= 0 || image->height <= 0) return 0;
  int dst_w = image->width;
  int dst_h = image->height;
  int stride = image->bytes_per_line > 0 ? image->bytes_per_line : dst_w * 4;
  int slot_idx = __atomic_load_n(&g_cap_active_slot, __ATOMIC_ACQUIRE);
  if (slot_idx >= 0 && slot_idx < CAP_RING_SLOTS) {
    const cap_slot_t* slot = &g_cap_slots[slot_idx];
    if (slot->bgra && slot->w > 0 && slot->h > 0) {
      int src_w = slot->w;
      int src_h = slot->h;
      const uint32_t* src = (const uint32_t*)slot->bgra;
      if (src_w == dst_w && src_h == dst_h && stride == dst_w * 4) {
        memcpy(image->data, src, (size_t)dst_w * (size_t)dst_h * 4);
      } else {
        for (int row = 0; row < dst_h; row++) {
          int sy = (row * src_h) / dst_h;
          const uint32_t* srow = src + (size_t)sy * (size_t)src_w;
          uint32_t* drow = (uint32_t*)(image->data + (size_t)row * (size_t)stride);
          for (int col = 0; col < dst_w; col++) {
            int sx = (col * src_w) / dst_w;
            drow[col] = srow[sx];
          }
        }
      }
      return 1;
    }
  }
  uint64_t t = now / 16;
  for (int row = 0; row < dst_h; row++) {
    uint32_t* drow = (uint32_t*)(image->data + (size_t)row * (size_t)stride);
    uint8_t g = (uint8_t)(28 + ((row * 32) / (dst_h ? dst_h : 1)));
    for (int col = 0; col < dst_w; col++) {
      uint8_t b = (uint8_t)(42 + (((col + (int)t) & 63) >> 1));
      uint8_t r = (uint8_t)(24 + (((row + col + (int)t) & 31) >> 1));
      drow[col] = (0xFFu << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
    }
  }
  return 1;
}

static void* make_trampoline(void* target, size_t copy_len) {
  long page_size = sysconf(_SC_PAGESIZE);
  void* mem = mmap(NULL, (size_t)page_size, PROT_READ | PROT_WRITE | PROT_EXEC,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (mem == MAP_FAILED) return NULL;
  memcpy(mem, target, copy_len);
  uint8_t* p = (uint8_t*)mem + copy_len;
  p[0] = 0x48; p[1] = 0xb8;
  uint64_t ret_addr = (uint64_t)target + copy_len;
  memcpy(p + 2, &ret_addr, 8);
  p[10] = 0xff; p[11] = 0xe0;
  return mem;
}

static void patch_prologue(void* target, void* hook) {
  long page_size = sysconf(_SC_PAGESIZE);
  uintptr_t start = (uintptr_t)target & ~(uintptr_t)(page_size - 1);
  mprotect((void*)start, (size_t)page_size * 2, PROT_READ | PROT_WRITE | PROT_EXEC);
  uint8_t* p = (uint8_t*)target;
  p[0] = 0x48; p[1] = 0xb8;
  uint64_t hook_addr = (uint64_t)hook;
  memcpy(p + 2, &hook_addr, 8);
  p[10] = 0xff; p[11] = 0xe0;
}

#define REM_RING_SLOTS 3
typedef struct {
  uint8_t* rgba;
  size_t cap;
  int w;
  int h;
  int q;
  uint64_t seq;
} rem_slot_t;

static rem_slot_t g_rem_slots[REM_RING_SLOTS];
static volatile int g_rem_latest_slot = -1;
static int g_rem_write_slot = 0;
static volatile uint64_t g_rem_seq_counter = 0;
static pthread_mutex_t g_rem_wake_mu = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t g_rem_wake_cv = PTHREAD_COND_INITIALIZER;
static int g_rem_worker_started = 0;

typedef struct {
  uint64_t seq;
  uint32_t width;
  uint32_t height;
  uint32_t fps;
  uint32_t reserved;
} stream_frame_meta_t;

static void* remote_jpeg_encode_worker(void* arg) {
  (void)arg;
  tjhandle tj = tjInitCompress();
  uint64_t last_encoded_seq = 0;
  uint64_t fps_window_start = monotonic_ms();
  uint32_t frames_in_window = 0;
  uint32_t measured_fps = 60;

  while (1) {
    int slot_idx = __atomic_load_n(&g_rem_latest_slot, __ATOMIC_ACQUIRE);
    uint64_t cur_seq = (slot_idx >= 0 && slot_idx < REM_RING_SLOTS) ? g_rem_slots[slot_idx].seq : 0;
    if (cur_seq == 0 || cur_seq == last_encoded_seq) {
      pthread_mutex_lock(&g_rem_wake_mu);
      slot_idx = __atomic_load_n(&g_rem_latest_slot, __ATOMIC_ACQUIRE);
      cur_seq = (slot_idx >= 0 && slot_idx < REM_RING_SLOTS) ? g_rem_slots[slot_idx].seq : 0;
      if (cur_seq == 0 || cur_seq == last_encoded_seq) {
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        ts.tv_nsec += 16000000L;
        if (ts.tv_nsec >= 1000000000L) {
          ts.tv_sec += 1;
          ts.tv_nsec -= 1000000000L;
        }
        pthread_cond_timedwait(&g_rem_wake_cv, &g_rem_wake_mu, &ts);
      }
      pthread_mutex_unlock(&g_rem_wake_mu);
      continue;
    }

    rem_slot_t* slot = &g_rem_slots[slot_idx];
    int w = slot->w;
    int h = slot->h;
    int q = slot->q;
    uint8_t* rgba_ptr = slot->rgba;
    last_encoded_seq = cur_seq;

    if (!rgba_ptr || w <= 0 || h <= 0 || !tj) continue;

    unsigned char* jpeg_buf = NULL;
    unsigned long jpeg_size = 0;
    int rc = tjCompress2(
      tj,
      rgba_ptr,
      w,
      0,
      h,
      TJPF_RGBA,
      &jpeg_buf,
      &jpeg_size,
      TJSAMP_420,
      q,
      TJFLAG_FASTDCT
    );
    if (rc == 0 && jpeg_buf && jpeg_size > 64) {
      FILE* fp = fopen("/dev/shm/discord_remote_stream_frame_tmp.jpg", "wb");
      if (fp) {
        fwrite(jpeg_buf, 1, jpeg_size, fp);
        fclose(fp);
        rename("/dev/shm/discord_remote_stream_frame_tmp.jpg", "/dev/shm/discord_remote_stream_frame.jpg");
      }
      frames_in_window++;
      uint64_t now = monotonic_ms();
      if (now - fps_window_start >= 1000) {
        measured_fps = (uint32_t)((frames_in_window * 1000ULL) / (now - fps_window_start));
        if (measured_fps < 1) measured_fps = 1;
        if (measured_fps > 120) measured_fps = 120;
        frames_in_window = 0;
        fps_window_start = now;
      }
      stream_frame_meta_t meta;
      meta.seq = cur_seq;
      meta.width = (uint32_t)w;
      meta.height = (uint32_t)h;
      meta.fps = measured_fps;
      meta.reserved = 0;
      FILE* mfp = fopen("/dev/shm/discord_remote_stream_meta_tmp.bin", "wb");
      if (mfp) {
        fwrite(&meta, sizeof(meta), 1, mfp);
        fclose(mfp);
        rename("/dev/shm/discord_remote_stream_meta_tmp.bin", "/dev/shm/discord_remote_stream_meta.bin");
      }
    }
    if (jpeg_buf) {
      tjFree(jpeg_buf);
    }
  }
  return NULL;
}

static napi_value js_push_remote_frame(napi_env env, napi_callback_info info) {
  if (!fn_get_cb_info || !fn_get_typedarray_info || !fn_get_value_int32) return NULL;
  size_t argc = 4;
  napi_value argv[4];
  if (fn_get_cb_info(env, info, &argc, argv, NULL, NULL) != 0 || argc < 3) return NULL;
  int ta_type = 0;
  size_t byte_len = 0;
  void* data_ptr = NULL;
  if (fn_get_typedarray_info(env, argv[0], &ta_type, &byte_len, &data_ptr, NULL, NULL) != 0 || !data_ptr) {
    return NULL;
  }
  int32_t w = 0, h = 0, q = 82;
  fn_get_value_int32(env, argv[1], &w);
  fn_get_value_int32(env, argv[2], &h);
  if (argc >= 4) {
    fn_get_value_int32(env, argv[3], &q);
  }
  if (w <= 0 || h <= 0 || w > 7680 || h > 4320) return NULL;
  size_t needed = (size_t)w * (size_t)h * 4;
  if (byte_len < needed) return NULL;
  if (q < 40) q = 40;
  if (q > 95) q = 95;

  int slot_idx = g_rem_write_slot % REM_RING_SLOTS;
  rem_slot_t* slot = &g_rem_slots[slot_idx];
  if (!slot->rgba || slot->cap < needed) {
    uint8_t* nb = (uint8_t*)realloc(slot->rgba, needed);
    if (!nb) return NULL;
    slot->rgba = nb;
    slot->cap = needed;
  }
  memcpy(slot->rgba, data_ptr, needed);
  slot->w = (int)w;
  slot->h = (int)h;
  slot->q = (int)q;
  uint64_t next_seq = __atomic_add_fetch(&g_rem_seq_counter, 1, __ATOMIC_RELAXED);
  slot->seq = next_seq;
  __atomic_store_n(&g_rem_latest_slot, slot_idx, __ATOMIC_RELEASE);
  g_rem_write_slot = (slot_idx + 1) % REM_RING_SLOTS;
  pthread_cond_signal(&g_rem_wake_cv);
  return NULL;
}

__attribute__((visibility("default")))
napi_value napi_register_module_v1(napi_env env, napi_value exports) {
  const char* openh264_so_path = "/home/sanya/.config/discord/discord_asset_cache/openh264/libopenh264-2.7.0-linux64.8.so";
  const char* preload_libs[] = {
    "/home/sanya/.config/discord/app-1.0.168/libffmpeg.so",
    openh264_so_path,
    "libcuda.so.1",
    "libnvidia-encode.so.1",
    "libvulkan.so.1",
    "libturbojpeg.so.0",
    "libXfixes.so.3",
    "libXdamage.so.1",
    "libXext.so.6",
    "libXrandr.so.2",
    "libXcomposite.so.1",
    "libX11.so.6",
    "libgio-2.0.so.0",
    "libgobject-2.0.so.0",
    "libglib-2.0.so.0",
    "libdrm.so.2",
    "libgbm.so.1",
    NULL
  };
  for (int i = 0; preload_libs[i] != NULL; i++) {
    dlopen(preload_libs[i], RTLD_NOW | RTLD_GLOBAL);
  }
  typedef void (*set_openh264_path_fn)(const char*);
  typedef int (*load_openh264_fn)(void);
  set_openh264_path_fn fn_set_openh264 = (set_openh264_path_fn)dlsym(RTLD_DEFAULT, "ff_libopenh264_set_library_path");
  if (fn_set_openh264) {
    fn_set_openh264(openh264_so_path);
  }
  load_openh264_fn fn_load_openh264 = (load_openh264_fn)dlsym(RTLD_DEFAULT, "ff_libopenh264_load_library");
  if (fn_load_openh264) {
    fn_load_openh264();
  }
  g_main_env = env;
  g_main_thread = pthread_self();
  orig_open_scope = (open_scope_fn)dlsym(RTLD_DEFAULT, "napi_open_handle_scope");
  orig_close_scope = (close_scope_fn)dlsym(RTLD_DEFAULT, "napi_close_handle_scope");
  fn_get_cb_info = (get_cb_info_fn)dlsym(RTLD_DEFAULT, "napi_get_cb_info");
  fn_get_typedarray_info = (get_typedarray_info_fn)dlsym(RTLD_DEFAULT, "napi_get_typedarray_info");
  fn_get_value_int32 = (get_value_int32_fn)dlsym(RTLD_DEFAULT, "napi_get_value_int32");
  fn_create_function = (create_function_fn)dlsym(RTLD_DEFAULT, "napi_create_function");
  fn_set_named_property = (set_named_property_fn)dlsym(RTLD_DEFAULT, "napi_set_named_property");
  get_uv_loop_fn get_loop = (get_uv_loop_fn)dlsym(RTLD_DEFAULT, "napi_get_uv_event_loop");
  if (get_loop) {
    get_loop(env, &g_main_loop);
  }
  void* p_async_init = dlsym(RTLD_DEFAULT, "uv_async_init");
  if (p_async_init && !tramp_uv_async_init) {
    tramp_uv_async_init = (uv_async_init_fn)make_trampoline(p_async_init, 14);
    patch_prologue(p_async_init, (void*)hook_uv_async_init);
  }
  void* p_shm_qv = dlsym(RTLD_DEFAULT, "XShmQueryVersion");
  if (p_shm_qv) {
    patch_prologue(p_shm_qv, (void*)hook_XShmQueryVersion);
  }
  void* p_shm_gi = dlsym(RTLD_DEFAULT, "XShmGetImage");
  if (p_shm_gi) {
    patch_prologue(p_shm_gi, (void*)hook_XShmGetImage);
  }
  if (!g_capture_thread_started) {
    g_capture_thread_started = 1;
    pthread_t tid;
    pthread_create(&tid, NULL, screen_capture_worker, NULL);
    pthread_detach(tid);
  }
  if (!g_rem_worker_started) {
    g_rem_worker_started = 1;
    pthread_t rtid;
    pthread_create(&rtid, NULL, remote_jpeg_encode_worker, NULL);
    pthread_detach(rtid);
  }
  if (fn_create_function && fn_set_named_property) {
    napi_value fn_val = NULL;
    if (fn_create_function(env, "pushRemoteFrame", 15, js_push_remote_frame, NULL, &fn_val) == 0 && fn_val) {
      fn_set_named_property(env, exports, "pushRemoteFrame", fn_val);
    }
  }
  return exports;
}
"#;

const DV_BRIDGE_JS: &str = r#"
const fs = require("fs");
const http = require("http");
const readline = require("readline");

fs.mkdirSync("/tmp/dv_data", { recursive: true });
fs.mkdirSync("/tmp/dv_logs", { recursive: true });

const scopeFix = require("/tmp/dv_scope_fix.node");
const dv = require("/home/sanya/.config/discord/app-1.0.168/modules/chord_voice-1/chord_voice/chord_voice.node");

function sendEv(obj) {
  try {
    process.stdout.write(JSON.stringify(obj) + "\n");
  } catch (_) {}
}

const isStreamWorker = process.env.DV_CONTEXT === "stream";
const dataDir = isStreamWorker ? "/tmp/dv_stream_data" : "/tmp/dv_data";
const logDir = isStreamWorker ? "/tmp/dv_stream_logs" : "/tmp/dv_logs";
try { fs.mkdirSync(dataDir, { recursive: true }); } catch (_) {}
try { fs.mkdirSync(logDir, { recursive: true }); } catch (_) {}

if (isStreamWorker) {
  try {
    const srv = http.createServer((req, res) => {
      res.setHeader("Access-Control-Allow-Origin", "*");
      res.setHeader("Access-Control-Expose-Headers", "X-Frame-Seq, X-Frame-Width, X-Frame-Height, X-Stream-Fps");
      res.setHeader("Cache-Control", "no-store, no-cache, must-revalidate");
      if (req.method === "OPTIONS") {
        res.writeHead(204);
        res.end();
        return;
      }
      const u = new URL(req.url || "/", "http://127.0.0.1:49281");
      if (u.pathname === "/preview.jpg") {
        try {
          const pjpg = fs.readFileSync("/dev/shm/discord_live_preview_thumb.jpg");
          if (pjpg && pjpg.length > 64) {
            res.setHeader("Content-Type", "image/jpeg");
            res.setHeader("Content-Length", String(pjpg.length));
            res.writeHead(200);
            res.end(pjpg);
            return;
          }
        } catch (_) {}
        res.writeHead(404);
        res.end();
        return;
      }
      const sinceSeq = Number(u.searchParams.get("since") || 0);
      let seq = 0;
      let fw = 1280;
      let fh = 720;
      let fps = 60;
      try {
        const meta = fs.readFileSync("/dev/shm/discord_remote_stream_meta.bin");
        if (meta && meta.length >= 20) {
          seq = Number(meta.readBigUInt64LE(0));
          fw = meta.readUInt32LE(8);
          fh = meta.readUInt32LE(12);
          fps = meta.readUInt32LE(16);
        }
      } catch (_) {}
      const shmFrameExists = fs.existsSync("/dev/shm/discord_remote_stream_frame.jpg");
      const tmpFrameExists = !shmFrameExists && fs.existsSync("/tmp/discord_remote_stream_frame.jpg");
      if (seq === 0 && (shmFrameExists || tmpFrameExists)) {
        try {
          const st = fs.statSync(
            shmFrameExists
              ? "/dev/shm/discord_remote_stream_frame.jpg"
              : "/tmp/discord_remote_stream_frame.jpg"
          );
          seq = Math.round(st.mtimeMs) || 0;
        } catch (_) {}
      }
      if (seq > 0 && sinceSeq > 0 && seq === sinceSeq) {
        res.setHeader("X-Frame-Seq", String(seq));
        res.setHeader("X-Stream-Fps", String(fps));
        res.writeHead(204);
        res.end();
        return;
      }
      let jpg = null;
      if (shmFrameExists) {
        try {
          jpg = fs.readFileSync("/dev/shm/discord_remote_stream_frame.jpg");
        } catch (_) {}
      }
      if ((!jpg || jpg.length <= 64) && tmpFrameExists) {
        try {
          jpg = fs.readFileSync("/tmp/discord_remote_stream_frame.jpg");
        } catch (_) {}
      }
      if (jpg && jpg.length > 64) {
        if (seq === 0) {
          seq = Date.now();
        }
        res.setHeader("Content-Type", "image/jpeg");
        res.setHeader("Content-Length", String(jpg.length));
        res.setHeader("X-Frame-Seq", String(seq));
        res.setHeader("X-Frame-Width", String(fw));
        res.setHeader("X-Frame-Height", String(fh));
        res.setHeader("X-Stream-Fps", String(fps));
        res.writeHead(200);
        res.end(jpg);
        return;
      }
      res.writeHead(404);
      res.end();
    });
    srv.on("connection", (sock) => {
      try { sock.setNoDelay(true); } catch (_) {}
    });
    srv.on("error", () => {});
    srv.listen(49281, "127.0.0.1");
  } catch (_) {}
}

try {
  if (typeof dv.setImageDataAllocator === "function") {
    dv.setImageDataAllocator((width, height) => ({
      width: Number(width) || 0,
      height: Number(height) || 0,
      data: new Uint8Array((Number(width) || 0) * (Number(height) || 0) * 4)
    }));
  }
} catch (_) {}

dv.initialize({
  audioSubsystem: "standard",
  logLevel: 2,
  dataDirectory: dataDir,
  logDirectory: logDir,
  maxLogBytes: 5000000,
  offloadAdmControls: true,
  asyncVideoInputDeviceInit: false
});

try {
  if (typeof dv.setKrispPath === "function") {
    dv.setKrispPath("/home/sanya/.config/discord/app-1.0.168/modules/discord_krisp-1/discord_krisp");
    sendEv({ ev: "log", msg: "setKrispPath initialized with neural models" });
  }
} catch (e) {
  sendEv({ ev: "log", msg: "setKrispPath err: " + (e && e.message) });
}

const activeVideoSinks = new Map();
let smoothedPingMs = 90;
let currentVideoTargetBitrate = 2500000;
let currentVideoMaxBitrate = 3500000;
let currentStreamMuted = false;
let currentStreamVolume = 100;

function computeAdaptiveRates(pingMs) {
  const p = Number(pingMs) || 90;
  smoothedPingMs = Math.round(smoothedPingMs * 0.65 + p * 0.35);
  if (smoothedPingMs <= 95) {
    return { voiceRate: 96000, videoTarget: 4000000, videoMax: 6000000, lossRate: 0.05, jpgQ: 86 };
  }
  if (smoothedPingMs <= 220) {
    return { voiceRate: 64000, videoTarget: 3000000, videoMax: 4200000, lossRate: 0.12, jpgQ: 82 };
  }
  if (smoothedPingMs <= 460) {
    return { voiceRate: 56000, videoTarget: 1800000, videoMax: 2600000, lossRate: 0.22, jpgQ: 76 };
  }
  return { voiceRate: 48000, videoTarget: 1200000, videoMax: 1800000, lossRate: 0.35, jpgQ: 70 };
}

function bindVideoSinkId(sid) {
  if (!sid || typeof dv.setVideoOutputSink !== "function") return;
  try {
    dv.setVideoOutputSink(
      sid,
      (imageData) => {
        try {
          if (typeof dv.signalVideoOutputSinkReady === "function") {
            dv.signalVideoOutputSinkReady(sid);
          }
        } catch (_) {}
        try {
          if (
            imageData &&
            imageData.width > 0 &&
            imageData.height > 0 &&
            imageData.data &&
            scopeFix &&
            typeof scopeFix.pushRemoteFrame === "function"
          ) {
            const rates = computeAdaptiveRates(smoothedPingMs);
            scopeFix.pushRemoteFrame(
              imageData.data,
              Number(imageData.width) | 0,
              Number(imageData.height) | 0,
              rates.jpgQ | 0
            );
          }
        } catch (_) {}
      },
      true
    );
    if (typeof dv.signalVideoOutputSinkReady === "function") {
      dv.signalVideoOutputSinkReady(sid);
    }
  } catch (e) {
    sendEv({ ev: "log", msg: "setVideoOutputSink err: " + (e && e.message) });
  }
}

function subscribeRemoteVideoSink(uid, videoSsrc, streamId, videoStreamParams) {
  const sid = streamId != null ? String(streamId) : "";
  let vSsrc = Number(videoSsrc || 0);
  let rSsrc = vSsrc > 0 ? vSsrc + 1 : 0;
  if (Array.isArray(videoStreamParams) && videoStreamParams.length > 0) {
    const first = videoStreamParams[0];
    if (first && typeof first.ssrc === "number" && first.ssrc > 0) {
      vSsrc = first.ssrc;
    }
    if (first && typeof first.rtxSsrc === "number" && first.rtxSsrc > 0) {
      rSsrc = first.rtxSsrc;
    } else if (first && typeof first.rtx_ssrc === "number" && first.rtx_ssrc > 0) {
      rSsrc = first.rtx_ssrc;
    } else if (vSsrc > 0) {
      rSsrc = vSsrc + 1;
    }
  } else if (vSsrc > 0 && currentRtxSsrc > 0 && currentVideoSsrc === vSsrc) {
    rSsrc = currentRtxSsrc;
  }
  const isSelf = !uid || String(uid) === "0" || String(uid) === String(currentUserId);
  if (isSelf && vSsrc > 0) {
    currentVideoSsrc = vSsrc;
    currentRtxSsrc = rSsrc;
  }
  sendEv({
    ev: "video_callback",
    userId: String(uid || currentUserId || ""),
    audioSsrc: currentLocalSsrc,
    videoSsrc: vSsrc,
    rtxSsrc: rSsrc,
    streamId: sid,
    isSelf: isSelf
  });
  if (!sid || isSelf) return;
  activeVideoSinks.set(sid, { uid: String(uid || ""), videoSsrc: vSsrc });
  bindVideoSinkId(sid);
}

function applyStreamAudioSettings() {
  if (!isStreamWorker) return;
  const effVol = currentStreamMuted ? 0 : Math.max(0, Math.min(200, currentStreamVolume));
  const normVol = effVol / 100.0;
  try {
    dv.setOutputVolume(normVol);
  } catch (_) {}
  if (conn) {
    try {
      conn.setSelfDeafen(Boolean(currentStreamMuted || effVol === 0));
    } catch (_) {}
    for (const [uid, u] of remoteUsers.entries()) {
      u.mute = Boolean(currentStreamMuted || effVol === 0);
      u.volume = normVol;
      try {
        if (typeof conn.setLocalMute === "function") conn.setLocalMute(uid, u.mute);
        if (typeof conn.setLocalVolume === "function") conn.setLocalVolume(uid, normVol);
      } catch (_) {}
    }
    const validUsers = Array.from(remoteUsers.values()).filter((u) => u && u.id && String(u.id) !== String(currentUserId));
    if (connConnected && validUsers.length > 0) {
      try {
        conn.mergeUsers(validUsers);
      } catch (_) {}
    }
  }
}

let conn = null;
let connConnected = false;
let currentSelfMute = false;
let currentSelfDeafen = false;
let currentKrispEnabled = true;
let currentAutoGain = true;
let currentAutoSensitivity = true;
let currentVadThreshold = -42;
let currentInputVolume = 85;
let currentOutputVolume = 100;
let currentUserId = "";
let currentChannelId = "";
let currentLocalSsrc = 0;
let currentVideoSsrc = 0;
let currentRtxSsrc = 0;
let isSelfBroadcaster = false;
let pendingExternalSender = null;
let sessionEpochPrepared = false;
const remoteUsers = new Map();
const perUserAudioPrefs = new Map();

function buildVoiceDspTransportOptions(isStreamConn, krispEnabled, autoGain) {
  const useKrispNc = !isStreamConn && Boolean(krispEnabled);
  return {
    echoCancellation: !isStreamConn,
    noiseSuppression: !isStreamConn && !useKrispNc,
    noiseCancellation: useKrispNc,
    automaticGainControl: !isStreamConn && Boolean(autoGain),
    ducking: 0
  };
}

function buildSafeVadInputModeOptions(isStreamConn, vadThreshold, autoSensitivity) {
  return {
    vadThreshold: isStreamConn ? -60 : vadThreshold,
    vadAutoThreshold: (!isStreamConn && autoSensitivity) ? 3 : -1,
    vadUseKrisp: false, // STABILITY: WebRTC native VAD prevents SIGSEGV in krispCreateVadInt16 while Krisp noiseCancellation filters noise
    vadLeading: 5,
    vadTrailing: 25
  };
}

function enforceSafeVadTransportOptions(opts) {
  if (!opts || typeof opts !== "object") return opts;
  if (opts.inputModeOptions && typeof opts.inputModeOptions === "object") {
    return {
      ...opts,
      inputModeOptions: {
        ...opts.inputModeOptions,
        vadUseKrisp: false
      }
    };
  }
  return opts;
}

function attachConnCallbacks(c) {
  if (!c) return;
  try {
    if (typeof c.setTransportOptions === "function" && !c.__krispVadGuarded) {
      const origSetTransportOptions = c.setTransportOptions.bind(c);
      c.setTransportOptions = (opts) => origSetTransportOptions(enforceSafeVadTransportOptions(opts));
      c.__krispVadGuarded = true;
    }
    if (typeof c.setOnVideoCallback === "function") {
      c.setOnVideoCallback((uid, videoSsrc, streamId, videoStreamParams) => {
        subscribeRemoteVideoSink(uid, videoSsrc, streamId, videoStreamParams);
      });
    }
    if (typeof c.setOnFirstFrameCallback === "function") {
      c.setOnFirstFrameCallback((uid, videoSsrc, streamId) => {
        subscribeRemoteVideoSink(uid, videoSsrc, streamId);
      });
    }
    if (typeof c.setOnSpeakingCallback === "function") {
      c.setOnSpeakingCallback((uid, flags) => {
        sendEv({ ev: "speaking", userId: String(uid), speaking: Number(flags || 0), isSelf: String(uid) === currentUserId });
      });
    }
    if (typeof c.setOnSpeakingWhileMutedCallback === "function") {
      c.setOnSpeakingWhileMutedCallback(() => {
        sendEv({ ev: "speaking_while_muted" });
      });
    }
    if (typeof c.setSecureFramesStateUpdateCallback === "function") {
      c.setSecureFramesStateUpdateCallback((st) => {
        sendEv({ ev: "dave_state", state: st });
      });
    }
    if (typeof c.setOnMLSFailureCallback === "function") {
      c.setOnMLSFailureCallback((src, reason) => {
        sendEv({ ev: "mls_failure", source: String(src || ""), reason: String(reason || "") });
      });
    }
  } catch (_) {}
}

function flushMergedUsersToConn(_forceRetriggerVideo) {
  if (!conn || !connConnected) return;
  const validUsers = Array.from(remoteUsers.values()).filter((u) => u && u.id && String(u.id) !== String(currentUserId));
  if (validUsers.length === 0) return;
  try {
    conn.mergeUsers(validUsers);
  } catch (_) {}
  for (const u of validUsers) {
    if (u.videoSsrc > 0 && typeof conn.setDisableLocalVideo === "function") {
      try {
        conn.setDisableLocalVideo(u.id, false);
      } catch (_) {}
    }
    try {
      if (typeof conn.setLocalMute === "function") conn.setLocalMute(u.id, Boolean(u.mute));
      if (typeof conn.setLocalVolume === "function") conn.setLocalVolume(u.id, Number(u.volume ?? 1.0));
    } catch (_) {}
  }
}

function applyScreenBroadcast(active) {
  if (!conn) return;
  try {
    if (active) {
      const vSsrc = currentVideoSsrc > 0 ? currentVideoSsrc : currentLocalSsrc + 1;
      const rSsrc = currentRtxSsrc > 0 ? currentRtxSsrc : currentLocalSsrc + 2;
      const rates = computeAdaptiveRates(smoothedPingMs);
      conn.setTransportOptions({
        encodingVideoBitRate: rates.videoTarget,
        encodingVideoMinBitRate: 350000,
        encodingVideoMaxBitRate: rates.videoMax,
        encodingVideoWidth: 1280,
        encodingVideoHeight: 720,
        encodingVideoFrameRate: 60,
        captureVideoFrameRate: 60,
        remoteSinkWantsMaxFramerate: 60,
        remoteSinkWantsPixelCount: 1280 * 720,
        encodingVideoDegradationPreference: 1,
        videoEncoderLatencyProfile: "ultra_low",
        videoEncoderExperiments: "nvNewPresets,nvRelaxRc=250,nvReconFrames,linux-vulkan",
        keyframeInterval: 3000,
        alwaysSendVideo: true,
        enableQualityDecoupling: false,
        streamParameters: [
          {
            type: "audio", ssrc: currentLocalSsrc, rid: "", maxBitrate: 64000,
            active: Boolean(process.env.DV_HAS_SOUNDSHARE_MONITOR === "1"), soundshare: true
          },
          {
            type: "video", rid: "100", ssrc: vSsrc, rtxSsrc: rSsrc, quality: 100, active: true,
            maxBitrate: rates.videoMax, minBitrate: 350000, targetBitrate: rates.videoTarget,
            maxFrameRate: 60, maxPixelCount: 1280 * 720,
            maxResolution: { type: "fixed", width: 1280, height: 720 }
          }
        ]
      });
      try { fs.writeFileSync("/dev/shm/discord_capture_kick.bin", Buffer.from([1])); } catch (_) {}
      let currentSourceType = "screen";
      let currentSourceId = "-1";
      try {
        if (fs.existsSync("/dev/shm/discord_capture_target.txt")) {
          const t = fs.readFileSync("/dev/shm/discord_capture_target.txt", "utf8").trim();
          currentSourceId = t.startsWith("screen:") ? t.slice(7) : (t.startsWith("window:") ? t.slice(7) : (t.startsWith("{") ? t : "-1"));
          if (t.startsWith("{") || t.startsWith("window:")) {
            currentSourceType = "window";
          }
        }
      } catch (_) {}
      // Linux Wayland Hook Parity (kFullDesktopScreenId = -1)
      if (typeof conn.setDesktopSourceWithOptions === "function") {
        conn.setDesktopSourceWithOptions({
          type: "screen",
          sourceId: "-1",
          width: 1280,
          height: 720,
          frameRate: 60,
          useVideoHook: false,
          useGraphicsCapture: false,
          useQuartzCapturer: false,
          allowScreenCaptureKit: false,
          hdrCaptureMode: "never"
        });
      }
      if (typeof conn.setVideoBroadcast === "function") conn.setVideoBroadcast(true);
      sendEv({ ev: "log", msg: "applyScreenBroadcast active=true videoSsrc=" + vSsrc + " rtxSsrc=" + rSsrc });
      if (!global.__bcastStatsTimer) {
        global.__bcastStatsTimer = setInterval(() => {
          if (!conn || !isSelfBroadcaster) return;
          try {
            conn.getFilteredStats(63, (raw) => {
              try {
                const stats = typeof raw === "string" ? JSON.parse(raw) : raw;
                const vo = stats && stats.rtp && stats.rtp.outbound && stats.rtp.outbound.find((o) => o.type === "video");
                if (vo) {
                  sendEv({
                    ev: "log",
                    msg: "BCAST_VIDEO_STATS codec=" + (vo.codec && vo.codec.name) + " inFps=" + vo.inputFrameRate + " encFps=" + vo.encodeFrameRate + " framesEnc=" + vo.framesEncoded + " framesSent=" + vo.framesSent + " pkts=" + vo.packetsSent + " bytes=" + vo.bytesSent + " dropQ=" + vo.framesDroppedEncoderQueue + " dropRate=" + vo.framesDroppedRateLimiter
                  });
                }
              } catch (_) {}
            });
          } catch (_) {}
        }, 3000);
      }
    } else {
      if (typeof conn.setVideoBroadcast === "function") {
        conn.setVideoBroadcast(false);
      }
      if (typeof conn.clearDesktopSource === "function") {
        conn.clearDesktopSource();
      }
    }
  } catch (e) {
    sendEv({ ev: "log", msg: "applyScreenBroadcast err: " + (e && e.message) });
  }
}

function syncRemoteUser(uid, audioSsrc, videoSsrc, speaking) {
  if (!uid || String(uid) === String(currentUserId)) return;
  const key = String(uid);
  const pref = perUserAudioPrefs.get(key);
  const prev = remoteUsers.get(key) || {
    id: key,
    audioSsrc: 0,
    ssrc: 0,
    videoSsrc: 0,
    videoSsrcs: [],
    rtxSsrc: 0,
    mute: isStreamWorker ? Boolean(currentStreamMuted || currentStreamVolume === 0) : Boolean(pref ? pref.mute : false),
    volume: isStreamWorker
      ? (currentStreamMuted ? 0 : Math.max(0, Math.min(200, currentStreamVolume)) / 100.0)
      : Number(pref ? pref.volume : 1.0)
  };
  if (typeof audioSsrc === "number" && audioSsrc > 0) {
    prev.audioSsrc = audioSsrc;
    prev.ssrc = audioSsrc;
  }
  if (typeof videoSsrc === "number" && videoSsrc >= 0) {
    prev.videoSsrc = videoSsrc;
    prev.videoSsrcs = videoSsrc > 0 ? [videoSsrc] : [];
    prev.rtxSsrc = videoSsrc > 0 ? videoSsrc + 1 : 0;
  }
  if (prev.ssrc === 0 && prev.videoSsrc > 0) {
    prev.ssrc = prev.videoSsrc - 1 > 0 ? prev.videoSsrc - 1 : 1;
    prev.audioSsrc = prev.ssrc;
  }
  remoteUsers.set(key, prev);
  if (conn && connConnected && prev.id && String(prev.id) !== String(currentUserId)) {
    try {
      conn.mergeUsers([prev]);
    } catch (_) {}
    if (prev.videoSsrc > 0 && typeof conn.setDisableLocalVideo === "function") {
      try {
        conn.setDisableLocalVideo(key, false);
      } catch (_) {}
    }
    if (typeof speaking === "number" && conn.setRemoteUserSpeakingStatus) {
      try {
        conn.setRemoteUserSpeakingStatus(key, speaking);
      } catch (_) {}
    }
    if (isStreamWorker) {
      applyStreamAudioSettings();
    } else {
      try {
        if (typeof conn.setLocalMute === "function") conn.setLocalMute(key, Boolean(prev.mute));
        if (typeof conn.setLocalVolume === "function") conn.setLocalVolume(key, Number(prev.volume ?? 1.0));
      } catch (_) {}
    }
  }
}

function applyVoiceMuteAndVolumeState() {
  if (isStreamWorker) {
    applyStreamAudioSettings();
    return;
  }
  const muted = Boolean(currentSelfMute || currentSelfDeafen);
  try {
    if (muted) {
      dv.setInputVolume(0.0);
    } else {
      const effInVol = currentAutoGain ? Math.min(82, currentInputVolume) : currentInputVolume;
      dv.setInputVolume(Math.max(0, Math.min(100, effInVol)) / 100.0);
    }
    const effOut = currentSelfDeafen ? 0 : currentOutputVolume;
    dv.setOutputVolume(Math.max(0, Math.min(200, effOut)) / 100.0);
  } catch (_) {}
  if (conn) {
    try {
      conn.setSelfMute(muted);
      conn.setSelfDeafen(Boolean(currentSelfDeafen));
      conn.setTransportOptions({
        streamParameters: [
          {
            type: "audio",
            ssrc: currentLocalSsrc,
            rid: "",
            maxBitrate: 64000,
            active: !muted,
            soundshare: false
          }
        ]
      });
    } catch (_) {}
  }
}

const rl = readline.createInterface({ input: process.stdin, terminal: false });
rl.on("line", (line) => {
  if (!line || !line.trim()) return;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (_) {
    return;
  }
  const cmd = msg.cmd;
  if (cmd === "ready") {
    if (conn) {
      try { conn.destroy(); } catch (_) {}
      conn = null;
    }
    connConnected = false;
    activeVideoSinks.clear();
    pendingExternalSender = null;
    sessionEpochPrepared = false;
    currentUserId = String(msg.userId || "");
    currentChannelId = String(msg.channelId || "");
    currentLocalSsrc = Number(msg.ssrc || 1);
    if (typeof msg.videoSsrc === "number" && msg.videoSsrc > 0) currentVideoSsrc = Number(msg.videoSsrc);
    if (typeof msg.rtxSsrc === "number" && msg.rtxSsrc > 0) currentRtxSsrc = Number(msg.rtxSsrc);
    if (typeof msg.selfMute === "boolean") currentSelfMute = msg.selfMute;
    if (typeof msg.selfDeafen === "boolean") currentSelfDeafen = msg.selfDeafen;
    if (typeof msg.krisp === "boolean") currentKrispEnabled = msg.krisp;
    if (typeof msg.streamMuted === "boolean") currentStreamMuted = msg.streamMuted;
    if (typeof msg.streamVolume === "number") currentStreamVolume = Math.max(0, Math.min(200, msg.streamVolume));
    const connContext = isStreamWorker ? "stream" : String(msg.context || "default");
    const isStreamConn = connContext === "stream";
    isSelfBroadcaster = Boolean(msg.isSelfBroadcaster) || (isStreamConn && String(msg.streamUserId || "") === currentUserId);
    const effVideoSsrc = currentVideoSsrc > 0 ? currentVideoSsrc : currentLocalSsrc + 1;
    const effRtxSsrc = currentRtxSsrc > 0 ? currentRtxSsrc : currentLocalSsrc + 2;
    const modes = Array.isArray(msg.modes) && msg.modes.length > 0
      ? msg.modes
      : ["aead_aes256_gcm_rtpsize", "aead_xchacha20_poly1305_rtpsize"];
    const hasSoundshareMonitor = process.env.DV_HAS_SOUNDSHARE_MONITOR === "1";
    const initAudioActive = isStreamConn
      ? Boolean(isSelfBroadcaster && hasSoundshareMonitor)
      : !Boolean(currentSelfMute || currentSelfDeafen);
    conn = new dv.VoiceConnection(
      currentUserId,
      {
        context: connContext,
        streamUserId: msg.streamUserId ? String(msg.streamUserId) : undefined,
        videoSupported: true,
        ssrc: currentLocalSsrc,
        address: String(msg.ip || ""),
        port: Number(msg.port || 0),
        modes: modes,
        streamParameters: [
          { type: "audio", ssrc: currentLocalSsrc, rid: "", maxBitrate: 64000, active: initAudioActive, soundshare: isStreamConn },
          {
            type: "video",
            rid: "100",
            ssrc: effVideoSsrc,
            rtxSsrc: effRtxSsrc,
            quality: 100,
            active: true,
            maxBitrate: 4500000,
            minBitrate: 350000,
            targetBitrate: 3000000,
            maxFrameRate: 60,
            maxPixelCount: 1920 * 1080,
            maxResolution: { type: "fixed", width: 1280, height: 720 }
          }
        ]
      },
      (err, info) => {
        if (err || !info) {
          sendEv({ ev: "error", message: String(err || "VoiceConnection failed") });
          return;
        }
        connConnected = true;
        try {
          if (typeof conn.configureConnectionRetries === "function") {
            conn.configureConnectionRetries(400, 4000, 20);
          }
          const effectiveSelfMute = isStreamConn
            ? !(isSelfBroadcaster && hasSoundshareMonitor)
            : Boolean(currentSelfMute || currentSelfDeafen);
          conn.setSelfMute(effectiveSelfMute);
          conn.setSelfDeafen(isStreamConn ? Boolean(currentStreamMuted) : Boolean(currentSelfDeafen));
          conn.setNoInputThreshold(-100);
          conn.setPingInterval(4000);
          conn.setPingCallback((ping) => {
            if (typeof ping === "number" && ping > 0) {
              const p = Math.round(ping);
              sendEv({ ev: "ping", ping: p });
              const rates = computeAdaptiveRates(p);
              currentVideoTargetBitrate = rates.videoTarget;
              currentVideoMaxBitrate = rates.videoMax;
            }
          });
          attachConnCallbacks(conn);
          dv.setTransportOptions({
            echoCancellation: !isStreamConn,
            noiseSuppression: !isStreamConn && !currentKrispEnabled,
            noiseCancellation: !isStreamConn && Boolean(currentKrispEnabled),
            automaticGainControl: !isStreamConn && Boolean(currentAutoGain),
            ducking: 0
          });
          conn.setTransportOptions({
            inputMode: 1,
            inputModeOptions: {
              vadThreshold: isStreamConn ? -60 : currentVadThreshold,
              vadAutoThreshold: (!isStreamConn && currentAutoSensitivity) ? 3 : -1,
              vadUseKrisp: false, // STABILITY: WebRTC native VAD prevents SIGSEGV in krispCreateVadInt16 while Krisp noiseCancellation filters noise
              vadLeading: 5,
              vadTrailing: 25
            },
            encodingVoiceBitRate: 64000,
            fec: true,
            packetLossRate: 0.15,
            remoteAudioHistoryMs: 0
          });
          flushMergedUsersToConn(true);
          if (isStreamConn) {
            try {
              dv.setInputVolume((isSelfBroadcaster && hasSoundshareMonitor) ? 0.85 : 0.0);
            } catch (_) {}
            applyStreamAudioSettings();
          } else {
            applyVoiceMuteAndVolumeState();
          }
        } catch (e) {
          sendEv({ ev: "log", msg: "init setup err: " + (e && e.message) });
        }
        const chosenMode = modes.includes("aead_aes256_gcm_rtpsize")
          ? "aead_aes256_gcm_rtpsize"
          : (modes[0] || "aead_xchacha20_poly1305_rtpsize");
        sendEv({
          ev: "select_protocol",
          protocol: info.protocol || "udp",
          address: info.address,
          port: info.port,
          mode: chosenMode
        });
      }
    );
    attachConnCallbacks(conn);
  } else if (cmd === "session_description" && conn) {
    connConnected = true;
    const mode = String(msg.mode || "aead_aes256_gcm_rtpsize");
    const secretKey = Array.isArray(msg.secretKey) ? msg.secretKey : [];
    const daveVer = Number(msg.daveVersion || 0);
    if (typeof msg.selfMute === "boolean") currentSelfMute = msg.selfMute;
    if (typeof msg.selfDeafen === "boolean") currentSelfDeafen = msg.selfDeafen;
    if (msg.channelId) currentChannelId = String(msg.channelId);
    try {
      attachConnCallbacks(conn);
      const rates = computeAdaptiveRates(smoothedPingMs);
      const hasSoundshareMonitor = process.env.DV_HAS_SOUNDSHARE_MONITOR === "1";
      const streamParams = [
        {
          type: "audio",
          ssrc: currentLocalSsrc,
          rid: "",
          maxBitrate: rates.voiceRate,
          active: isStreamWorker ? Boolean(isSelfBroadcaster && hasSoundshareMonitor) : !Boolean(currentSelfMute || currentSelfDeafen),
          soundshare: isStreamWorker
        }
      ];
      if (isStreamWorker || isSelfBroadcaster) {
        const effVideoSsrc = currentVideoSsrc > 0 ? currentVideoSsrc : currentLocalSsrc + 1;
        const effRtxSsrc = currentRtxSsrc > 0 ? currentRtxSsrc : currentLocalSsrc + 2;
        streamParams.push({
          type: "video",
          rid: "100",
          ssrc: effVideoSsrc,
          rtxSsrc: effRtxSsrc,
          quality: 100,
          active: true,
          maxBitrate: rates.videoMax,
          minBitrate: 350000,
          targetBitrate: rates.videoTarget,
          maxFrameRate: 60,
          maxPixelCount: 1920 * 1080,
          maxResolution: { type: "fixed", width: 1280, height: 720 }
        });
      }
      const rawMsgCodec = String(msg.videoCodec || msg.video_codec || "").trim();
      const effectiveCodec = (rawMsgCodec && rawMsgCodec.toUpperCase() !== "UNKNOWN" && rawMsgCodec.toUpperCase() !== "NONE")
        ? rawMsgCodec
        : String(process.env.DV_PREFERRED_CODEC || "").trim();
      const requestedCodec = effectiveCodec.toUpperCase();
      const isAv1Negotiated = requestedCodec === "AV1" || requestedCodec === "AV1X" || requestedCodec.startsWith("AV1");
      const isVp9Negotiated = requestedCodec === "VP9" || requestedCodec.startsWith("VP9");
      const isVp8Negotiated = requestedCodec === "VP8" || requestedCodec.startsWith("VP8");
      const hasNvidiaGpu = Boolean(
        process.env.DISCORD_GPU_VENDOR === "NVIDIA" ||
        process.env.DV_GPU_VENDOR === "NVIDIA" ||
        global.__hasNvidiaGpu ||
        fs.existsSync("/dev/nvidiactl")
      );
      global.__hasNvidiaGpu = hasNvidiaGpu;
      const hasVaapi = typeof dv !== "undefined" && typeof dv.isVaapiEnabled === "function" && dv.isVaapiEnabled();
      const hasHwVideo = Boolean(hasNvidiaGpu || hasVaapi);
      const videoEncoderCfg = isAv1Negotiated
        ? {
            name: "AV1X",
            codec: "AV1",
            type: 101,
            rtxType: 102,
            maxFrameRate: 60,
            maxBitrate: rates.videoMax,
            minBitrate: 350000,
            targetBitrate: rates.videoTarget,
            width: 1280,
            height: 720,
            params: {
              "hardware-av1": hasNvidiaGpu ? "1" : "0",
              "hardware-av1-decode": "1",
              "keyframe-interval": "3000",
              ...(hasNvidiaGpu
                ? {
                    "hardware-nvenc": "1",
                    "use-nvenc": "1",
                    "nvenc-cuda": "1",
                    "nvenc-vulkan": "1",
                    "discord-latency-profile": "ultra_low",
                    "video-encoder-experiments": "nvNewPresets,nvRelaxRc=250,nvReconFrames,linux-vulkan"
                  }
                : {})
            }
          }
        : isVp9Negotiated
        ? {
            name: "VP9",
            type: 109,
            rtxType: 110,
            maxFrameRate: 60,
            maxBitrate: rates.videoMax,
            minBitrate: 350000,
            targetBitrate: rates.videoTarget,
            width: 1280,
            height: 720,
            params: {
              "profile-id": "0",
              "keyframe-interval": "3000",
              "temporal-layers": "3"
            }
          }
        : isVp8Negotiated
        ? {
            name: "VP8",
            type: 107,
            rtxType: 108,
            maxFrameRate: 60,
            maxBitrate: rates.videoMax,
            minBitrate: 350000,
            targetBitrate: rates.videoTarget,
            width: 1280,
            height: 720,
            params: {
              "profile-id": "0",
              "keyframe-interval": "3000",
              "temporal-layers": "3"
            }
          }
        : {
            name: "H264",
            type: 105,
            rtxType: 106,
            maxFrameRate: 60,
            maxBitrate: rates.videoMax,
            minBitrate: 350000,
            targetBitrate: rates.videoTarget,
            width: 1280,
            height: 720,
            params: {
              "level-asymmetry-allowed": "1",
              "packetization-mode": "1",
              "profile-level-id": "42e034",
              "hardware-h264": hasHwVideo ? "1" : "0",
              ...(hasNvidiaGpu
                ? {
                    "hardware-nvenc": "1",
                    "use-nvenc": "1",
                    "nvenc-cuda": "1",
                    "nvenc-vulkan": "1",
                    "discord-latency-profile": "ultra_low",
                    "video-encoder-experiments": "nvNewPresets,nvRelaxRc=250,nvReconFrames,linux-vulkan"
                  }
                : {}),
              ...(hasHwVideo
                ? { "libopenh264": "0" }
                : {
                    "libopenh264": "1",
                    "libopenh264-path": "/home/sanya/.config/discord/discord_asset_cache/openh264/libopenh264-2.7.0-linux64.8.so"
                  })
            }
          };
      conn.setTransportOptions({
        audioEncoder: { type: 120, name: "opus", freq: 48000, pacsize: 960, channels: isStreamWorker ? 2 : 1, rate: rates.voiceRate },
        audioDecoders: [{ type: 120, name: "opus", freq: 48000, channels: 2, params: { stereo: "1" } }],
        fec: true,
        packetLossRate: rates.lossRate,
        videoEncoder: videoEncoderCfg,
        videoDecoders: [
          {
            name: "H264",
            type: 105,
            rtxType: 106,
            params: {
              "level-asymmetry-allowed": "1",
              "packetization-mode": "1",
              "profile-level-id": "42e034",
              "hardware-h264": hasHwVideo ? "1" : "0",
              ...(hasNvidiaGpu ? { "hardware-nvenc": "1", "use-nvenc": "1" } : {}),
              "libopenh264": "1",
              "libopenh264-path": "/home/sanya/.config/discord/discord_asset_cache/openh264/libopenh264-2.7.0-linux64.8.so"
            }
          },
          {
            name: "VP8",
            type: 107,
            rtxType: 108,
            params: {
              "profile-id": "0",
              "keyframe-interval": "3000",
              "temporal-layers": "3"
            }
          },
          {
            name: "VP9",
            type: 109,
            rtxType: 110,
            params: {
              "profile-id": "0",
              "keyframe-interval": "3000",
              "temporal-layers": "3"
            }
          },
          {
            name: "AV1X",
            codec: "AV1",
            type: 101,
            rtxType: 102,
            params: {
              "hardware-av1-decode": "1"
            }
          }
        ],
        encodingVideoWidth: 1280,
        encodingVideoHeight: 720,
        encodingVideoBitRate: rates.videoTarget,
        encodingVideoMinBitRate: 350000,
        encodingVideoMaxBitRate: rates.videoMax,
        encodingVideoFrameRate: 60,
        captureVideoFrameRate: 60,
        remoteSinkWantsMaxFramerate: 60,
        remoteSinkWantsPixelCount: 1920 * 1080,
        encodingVideoDegradationPreference: (typeof dv !== "undefined" && dv.DegradationPreference && dv.DegradationPreference.MAINTAIN_FRAMERATE) || 1,
        videoEncoderLatencyProfile: "ultra_low",
        videoEncoderExperiments: "nvNewPresets,nvRelaxRc=250,nvReconFrames,linux-vulkan",
        keyframeInterval: 3000,
        alwaysSendVideo: Boolean(isStreamWorker || isSelfBroadcaster),
        enableQualityDecoupling: false,
        streamParameters: streamParams,
        encryptionSettings: {
          mode: mode,
          secretKey: secretKey
        }
      });
      flushMergedUsersToConn(true);
      for (const sid of activeVideoSinks.keys()) {
        bindVideoSinkId(sid);
      }
      if (isSelfBroadcaster) {
        applyScreenBroadcast(true);
      }
      if (isStreamWorker) {
        applyStreamAudioSettings();
      } else {
        applyVoiceMuteAndVolumeState();
      }
    } catch (e) {
      sendEv({ ev: "log", msg: "setTransportOptions err: " + (e && e.message) });
    }
    try {
      if (daveVer > 0) {
        conn.prepareSecureFramesEpoch("1", daveVer, currentChannelId);
        sessionEpochPrepared = true;
        if (pendingExternalSender) {
          try { conn.updateMLSExternalSender(pendingExternalSender); } catch (_) {}
          pendingExternalSender = null;
        }
        conn.getMLSKeyPackage((kp) => {
          if (kp && kp.byteLength > 0) {
            sendEv({ ev: "send_binary", op: 26, hex: Buffer.from(kp).toString("hex") });
          }
        });
      } else {
        conn.prepareSecureFramesTransition(0, 0, () => {
          try { conn.executeSecureFramesTransition(0); } catch (_) {}
        });
      }
    } catch (e) {
      sendEv({ ev: "log", msg: "dave init err: " + (e && e.message) });
    }
  } else if (cmd === "dave_prepare_transition" && conn) {
    const tid = Number(msg.transitionId ?? 0);
    const ver = Number(msg.protocolVersion ?? 0);
    try {
      conn.prepareSecureFramesTransition(tid, ver, () => {
        if (tid !== 0) {
          sendEv({ ev: "send_json", payload: { op: 23, d: { transition_id: tid } } });
        } else {
          try { conn.executeSecureFramesTransition(0); } catch (_) {}
        }
      });
    } catch (_) {}
  } else if (cmd === "dave_execute_transition" && conn) {
    const tid = Number(msg.transitionId ?? 0);
    try {
      conn.executeSecureFramesTransition(tid);
      flushMergedUsersToConn(true);
      for (const sid of activeVideoSinks.keys()) {
        bindVideoSinkId(sid);
      }
      if (isSelfBroadcaster) {
        applyScreenBroadcast(true);
      }
      if (isStreamWorker) {
        applyStreamAudioSettings();
      } else {
        applyVoiceMuteAndVolumeState();
      }
    } catch (_) {}
  } else if (cmd === "reapply_broadcast" && conn) {
    if (isSelfBroadcaster) {
      applyScreenBroadcast(true);
    }
  } else if (cmd === "set_stream_audio") {
    if (typeof msg.muted === "boolean") {
      currentStreamMuted = msg.muted;
    }
    if (typeof msg.volume === "number") {
      currentStreamVolume = Math.max(0, Math.min(200, msg.volume));
    }
    applyStreamAudioSettings();
  } else if (cmd === "set_user_audio") {
    const uid = String(msg.userId || "");
    if (uid) {
      const prevPref = perUserAudioPrefs.get(uid) || { mute: false, volume: 1.0 };
      if (typeof msg.muted === "boolean") prevPref.mute = msg.muted;
      if (typeof msg.volume === "number") {
        prevPref.volume = Math.max(0, Math.min(200, msg.volume)) / 100.0;
      }
      perUserAudioPrefs.set(uid, prevPref);
      const u = remoteUsers.get(uid);
      if (u) {
        u.mute = Boolean(prevPref.mute || prevPref.volume === 0);
        u.volume = prevPref.volume;
      }
      if (conn) {
        try {
          if (typeof conn.setLocalMute === "function") conn.setLocalMute(uid, Boolean(prevPref.mute || prevPref.volume === 0));
          if (typeof conn.setLocalVolume === "function") conn.setLocalVolume(uid, prevPref.volume);
        } catch (_) {}
        if (connConnected && u && u.id && String(u.id) !== String(currentUserId)) {
          try { conn.mergeUsers([u]); } catch (_) {}
        }
      }
    }
  } else if (cmd === "dave_prepare_epoch" && conn) {
    const epochStr = String(msg.epoch ?? "1");
    const ver = Number(msg.protocolVersion ?? 1);
    if (msg.channelId) currentChannelId = String(msg.channelId);
    try {
      conn.prepareSecureFramesEpoch(epochStr, ver, currentChannelId);
      sessionEpochPrepared = true;
      if (pendingExternalSender) {
        try { conn.updateMLSExternalSender(pendingExternalSender); } catch (_) {}
        pendingExternalSender = null;
      }
      if (epochStr === "1") {
        conn.getMLSKeyPackage((kp) => {
          if (kp && kp.byteLength > 0) {
            sendEv({ ev: "send_binary", op: 26, hex: Buffer.from(kp).toString("hex") });
          }
        });
      }
    } catch (_) {}
  } else if (cmd === "binary" && conn) {
    const op = Number(msg.op || 0);
    const buf = Buffer.from(String(msg.hex || ""), "hex");
    try {
      if (op === 25) {
        if (!sessionEpochPrepared) {
          pendingExternalSender = buf;
        }
        conn.updateMLSExternalSender(buf);
      } else if (op === 27) {
        conn.processMLSProposals(buf, (cw) => {
          if (cw && cw.byteLength > 0) {
            sendEv({ ev: "send_binary", op: 28, hex: Buffer.from(cw).toString("hex") });
          }
        });
      } else if (op === 29 && buf.length >= 2) {
        const tid = buf.readUInt16BE(0);
        const payload = Buffer.from(buf.subarray(2));
        conn.prepareMLSCommitTransition(tid, payload, (ok, protoVer) => {
          if (ok) {
            flushMergedUsersToConn(true);
            if (tid !== 0) {
              sendEv({ ev: "send_json", payload: { op: 23, d: { transition_id: tid } } });
            } else {
              try { conn.executeSecureFramesTransition(0); } catch (_) {}
              if (isSelfBroadcaster) applyScreenBroadcast(true);
            }
          } else {
            sendEv({ ev: "send_json", payload: { op: 31, d: { transition_id: tid } } });
            if (Number(protoVer || 1) > 0) {
              conn.prepareSecureFramesEpoch("1", Number(protoVer || 1), currentChannelId);
              conn.getMLSKeyPackage((kp) => {
                if (kp && kp.byteLength > 0) {
                  sendEv({ ev: "send_binary", op: 26, hex: Buffer.from(kp).toString("hex") });
                }
              });
            }
          }
        });
      } else if (op === 30 && buf.length >= 2) {
        const tid = buf.readUInt16BE(0);
        const payload = Buffer.from(buf.subarray(2));
        conn.processMLSWelcome(tid, payload, (ok) => {
          if (ok) {
            flushMergedUsersToConn(true);
            if (tid !== 0) {
              sendEv({ ev: "send_json", payload: { op: 23, d: { transition_id: tid } } });
            } else {
              try { conn.executeSecureFramesTransition(0); } catch (_) {}
              if (isSelfBroadcaster) applyScreenBroadcast(true);
            }
          } else {
            sendEv({ ev: "send_json", payload: { op: 31, d: { transition_id: tid } } });
            conn.getMLSKeyPackage((kp) => {
              if (kp && kp.byteLength > 0) {
                sendEv({ ev: "send_binary", op: 26, hex: Buffer.from(kp).toString("hex") });
              }
            });
          }
        });
      }
    } catch (e) {
      sendEv({ ev: "log", msg: "binary op " + op + " err: " + (e && e.message) });
    }
  } else if (cmd === "merge_user") {
    syncRemoteUser(msg.userId, msg.ssrc, msg.videoSsrc, msg.speaking);
  } else if (cmd === "client_connect" && Array.isArray(msg.userIds)) {
    for (const uid of msg.userIds) {
      const key = String(uid || "");
      if (key && key !== String(currentUserId)) {
        if (!remoteUsers.has(key)) {
          const pref = perUserAudioPrefs.get(key);
          remoteUsers.set(key, {
            id: key,
            audioSsrc: 0,
            ssrc: 0,
            videoSsrc: 0,
            videoSsrcs: [],
            rtxSsrc: 0,
            mute: isStreamWorker ? Boolean(currentStreamMuted || currentStreamVolume === 0) : Boolean(pref ? pref.mute : false),
            volume: isStreamWorker
              ? (currentStreamMuted ? 0 : Math.max(0, Math.min(200, currentStreamVolume)) / 100.0)
              : Number(pref ? pref.volume : 1.0)
          });
        }
      }
    }
    flushMergedUsersToConn(false);
  } else if (cmd === "disconnect_user") {
    const key = String(msg.userId || "");
    remoteUsers.delete(key);
    if (conn && conn.destroyUser) {
      try { conn.destroyUser(key); } catch (_) {}
    }
  } else if (cmd === "set_state") {
    if (typeof msg.selfMute === "boolean") {
      currentSelfMute = msg.selfMute;
    }
    if (typeof msg.selfDeafen === "boolean") {
      currentSelfDeafen = msg.selfDeafen;
    }
    if (typeof msg.inputVolume === "number") {
      currentInputVolume = Math.max(0, Math.min(100, msg.inputVolume));
    }
    if (typeof msg.outputVolume === "number") {
      currentOutputVolume = Math.max(0, Math.min(200, msg.outputVolume));
    }
    if (typeof msg.krisp === "boolean") {
      currentKrispEnabled = msg.krisp;
      if (!isStreamWorker) {
        try {
          dv.setTransportOptions({
            echoCancellation: true,
            noiseCancellation: Boolean(currentKrispEnabled),
            noiseSuppression: !currentKrispEnabled,
            automaticGainControl: Boolean(currentAutoGain)
          });
          if (conn) {
            conn.setTransportOptions({
              inputMode: 1,
              inputModeOptions: {
                vadThreshold: currentVadThreshold,
                vadAutoThreshold: currentAutoSensitivity ? 3 : -1,
                vadUseKrisp: false, // STABILITY: WebRTC native VAD prevents SIGSEGV in krispCreateVadInt16
                vadLeading: 5,
                vadTrailing: 25
              }
            });
          }
        } catch (_) {}
      }
    }
    if (typeof msg.automaticGainControl === "boolean") {
      currentAutoGain = msg.automaticGainControl;
      if (!isStreamWorker) {
        try {
          dv.setTransportOptions({
            echoCancellation: true,
            noiseCancellation: Boolean(currentKrispEnabled),
            noiseSuppression: !currentKrispEnabled,
            automaticGainControl: Boolean(currentAutoGain)
          });
        } catch (_) {}
      }
    }
    if (typeof msg.autoSensitivity === "boolean" || typeof msg.vadThreshold === "number") {
      if (typeof msg.autoSensitivity === "boolean") {
        currentAutoSensitivity = msg.autoSensitivity;
      }
      if (typeof msg.vadThreshold === "number") {
        currentVadThreshold = Math.max(-85, Math.min(-5, Math.round(msg.vadThreshold)));
      }
      if (conn && !isStreamWorker) {
        try {
          conn.setTransportOptions({
            inputMode: 1,
            inputModeOptions: {
              vadThreshold: currentVadThreshold,
              vadAutoThreshold: currentAutoSensitivity ? 3 : -1,
              vadUseKrisp: false, // STABILITY: WebRTC native VAD prevents SIGSEGV in krispCreateVadInt16
              vadLeading: 5,
              vadTrailing: 25
            }
          });
        } catch (_) {}
      }
    }
    applyVoiceMuteAndVolumeState();
  } else if (cmd === "loopback") {
    const active = Boolean(msg.active);
    try {
      if (typeof dv.setLoopback === "function") {
        dv.setLoopback(active, {
          echoCancellation: true,
          noiseCancellation: Boolean(currentKrispEnabled),
          noiseSuppression: !currentKrispEnabled,
          automaticGainControl: Boolean(currentAutoGain),
          vadUseKrisp: false
        });
        sendEv({ ev: "log", msg: "setLoopback active=" + active + " krisp=" + currentKrispEnabled });
      }
    } catch (e) {
      sendEv({ ev: "log", msg: "setLoopback err: " + (e && e.message) });
    }
  } else if (cmd === "set_stream") {
    const isBcast = Boolean(msg.broadcasting);
    if (!isBcast && conn) {
      try {
        if (typeof conn.clearDesktopSource === "function") {
          conn.clearDesktopSource();
        } else if (typeof conn.setDesktopSource === "function") {
          conn.setDesktopSource("", false, "");
        }
        if (typeof conn.setVideoBroadcast === "function") {
          conn.setVideoBroadcast(false);
        }
      } catch (_) {}
    }
    applyScreenBroadcast(isBcast);
  } else if (cmd === "set_stream_source") {
    try {
      fs.writeFileSync("/dev/shm/discord_capture_kick.bin", Buffer.from([1]));
    } catch (_) {}
    if (conn && typeof conn.setDesktopSourceWithOptions === "function") {
      try {
        const qMatch = typeof msg.label === "string" ? msg.label.match(/(\d{3,4})x(\d{3,4})(?:@(\d{2,3}))?/) : null;
        const streamWidth = Number(msg.width) > 0 ? Number(msg.width) : (qMatch ? Number(qMatch[1]) : (msg.resolution === "1080p" ? 1920 : (msg.resolution === "1440p" ? 2560 : 1280)));
        const streamHeight = Number(msg.height) > 0 ? Number(msg.height) : (qMatch ? Number(qMatch[2]) : (msg.resolution === "1080p" ? 1080 : (msg.resolution === "1440p" ? 1440 : 720)));
        const streamFrameRate = Number(msg.frameRate || msg.fps) > 0 ? Number(msg.frameRate || msg.fps) : (qMatch && qMatch[3] ? Number(qMatch[3]) : 60);
        conn.setDesktopSourceWithOptions({
          type: "screen",
          sourceId: "-1",
          width: streamWidth,
          height: streamHeight,
          frameRate: streamFrameRate,
          useVideoHook: false,
          useGraphicsCapture: false,
          useQuartzCapturer: false,
          allowScreenCaptureKit: false,
          hdrCaptureMode: "never"
        });
        if (typeof conn.setVideoBroadcast === "function") {
          conn.setVideoBroadcast(true);
        }
        if (typeof conn.setTransportOptions === "function") {
          conn.setTransportOptions({
            encodingVideoWidth: streamWidth,
            encodingVideoHeight: streamHeight,
            encodingVideoFrameRate: streamFrameRate,
            captureVideoFrameRate: streamFrameRate,
            remoteSinkWantsMaxFramerate: streamFrameRate,
            remoteSinkWantsPixelCount: streamWidth * streamHeight,
            encodingVideoDegradationPreference: (typeof dv !== "undefined" && dv.DegradationPreference && dv.DegradationPreference.MAINTAIN_FRAMERATE) || 1,
            videoEncoderLatencyProfile: "ultra_low",
            keyframeInterval: 1000
          });
        }
        sendEv({ ev: "log", msg: "set_stream_source: updated to " + (msg.sourceId || "-1") + " " + streamWidth + "x" + streamHeight + "@" + streamFrameRate });
      } catch (e) {
        sendEv({ ev: "log", msg: "set_stream_source err: " + (e && e.message) });
      }
    }
  } else if (cmd === "stop") {
    if (conn) {
      try { conn.destroy(); } catch (_) {}
      conn = null;
    }
    process.exit(0);
  }
});
"#;

fn ensure_dv_native_bridge_files() -> bool {
    let dv_node = std::path::Path::new(
        "/home/sanya/.config/discord/app-1.0.168/modules/chord_voice-1/chord_voice/chord_voice.node",
    );
    if !dv_node.exists() {
        return false;
    }
    let scope_fix_node = std::path::Path::new("/tmp/dv_scope_fix.node");
    let c_changed = std::fs::read_to_string("/tmp/dv_scope_fix.c")
        .ok()
        .as_deref()
        != Some(DV_SCOPE_FIX_C);
    if !scope_fix_node.exists() || c_changed {
        let _ = std::fs::write("/tmp/dv_scope_fix.c", DV_SCOPE_FIX_C);
        let _ = std::process::Command::new("gcc")
            .args([
                "-shared",
                "-fPIC",
                "-O2",
                "-I/usr/include/gio-unix-2.0",
                "-I/usr/include/glib-2.0",
                "-I/usr/lib/glib-2.0/include",
                "/tmp/dv_scope_fix.c",
                "-o",
                "/tmp/dv_scope_fix.node",
                "-lturbojpeg",
                "-lgio-2.0",
                "-lgobject-2.0",
                "-lglib-2.0",
                "-ldl",
                "-lpthread",
            ])
            .status();
    }
    let _ = std::fs::write("/tmp/discord_voice_dave_bridge.js", DV_BRIDGE_JS);
    scope_fix_node.exists()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let clean = hex.trim();
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if let Ok(s) = std::str::from_utf8(&bytes[i..i + 2]) {
            if let Ok(b) = u8::from_str_radix(s, 16) {
                out.push(b);
            }
        }
        i += 2;
    }
    out
}

fn live_rtc_mutex() -> &'static std::sync::Mutex<LiveRtcSnapshot> {
    LIVE_RTC_STATE.get_or_init(|| std::sync::Mutex::new(LiveRtcSnapshot::default()))
}

/// Возвращает текущий снимок живого RTC-соединения.
#[must_use]
pub fn live_rtc_snapshot() -> LiveRtcSnapshot {
    live_rtc_mutex()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Проверяет, находится ли живое голосовое RTC-соединение в активном состоянии `connected`.
#[must_use]
pub fn is_live_rtc_connected() -> bool {
    live_rtc_mutex()
        .lock()
        .map(|g| g.state == "connected")
        .unwrap_or(false)
}

/// Обновляет метаданные целевого канала/сессии перед стартом RTC WebSocket.
pub fn note_live_rtc_target(
    guild_id: Option<String>,
    channel_id: Option<String>,
    session_id: Option<String>,
) {
    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
    if channel_id.is_none() {
        LIVE_RTC_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        NATIVE_VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
        *g = LiveRtcSnapshot::default();
        return;
    }
    g.guild_id = guild_id;
    g.channel_id = channel_id;
    if session_id.is_some() {
        g.session_id = session_id;
    }
    if g.state == "disconnected" {
        g.state = "signaling".to_string();
    }
}

static LIVE_STREAM_RTC_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn kill_pid_immediate(pid: u32) {
    if pid > 1 {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// Останавливает активную RTC-сессию просмотра стрима и мгновенно завершает аудио/видео воркер стрима.
pub fn stop_live_stream_rtc_session() {
    LIVE_STREAM_RTC_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let pid = LIVE_STREAM_DV_PID.swap(0, std::sync::atomic::Ordering::SeqCst);
    kill_pid_immediate(pid);
    if let Ok(mut q) = native_stream_cmd_mutex().lock() {
        q.clear();
    }
}

/// Останавливает активную RTC-сессию и немедленно завершает дочерние воркеры голоса и стрима.
pub fn stop_live_rtc_session() {
    let _ = queue_live_voice_state(None, None, false, false);
    LIVE_RTC_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    LIVE_STREAM_RTC_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    NATIVE_VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
    let v_pid = LIVE_VOICE_DV_PID.swap(0, std::sync::atomic::Ordering::SeqCst);
    let s_pid = LIVE_STREAM_DV_PID.swap(0, std::sync::atomic::Ordering::SeqCst);
    kill_pid_immediate(v_pid);
    kill_pid_immediate(s_pid);
    if let Ok(mut q) = native_voice_cmd_mutex().lock() {
        q.clear();
    }
    if let Ok(mut q) = native_stream_cmd_mutex().lock() {
        q.clear();
    }
    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
    *g = LiveRtcSnapshot::default();
}

/// Запускает подключение ко второму RTC-серверу для просмотра чужого стрима (`STREAM_CREATE` + `STREAM_SERVER_UPDATE`).
pub fn start_live_stream_rtc_session(
    endpoint: String,
    rtc_server_id: String,
    rtc_channel_id: String,
    user_id: String,
    session_id: String,
    token: String,
    streamer_user_id: String,
) {
    let clean_ep = endpoint
        .trim()
        .trim_start_matches("wss://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string();
    if clean_ep.is_empty() || rtc_server_id.is_empty() || session_id.is_empty() || token.is_empty()
    {
        return;
    }
    let generation = LIVE_STREAM_RTC_GENERATION
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        .wrapping_add(1);
    std::thread::Builder::new()
        .name("stream-rtc-session".to_string())
        .spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(async move {
                    let mut attempt: u32 = 0;
                    let mut resume_seq_ack: Option<u16> = None;
                    while LIVE_STREAM_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
                        == generation
                    {
                        let res = run_rtc_task(
                            generation,
                            clean_ep.clone(),
                            rtc_server_id.clone(),
                            user_id.clone(),
                            session_id.clone(),
                            token.clone(),
                            Some(rtc_channel_id.clone()),
                            Some(streamer_user_id.clone()),
                            &mut resume_seq_ack,
                        )
                        .await;
                        if LIVE_STREAM_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
                            != generation
                        {
                            break;
                        }
                        if let Err(ref e) = res {
                            if e.contains("4014") {
                                break;
                            }
                            if e.contains("4006") || e.contains("4022") {
                                if streamer_user_id == user_id {
                                    let cached = LAST_VOICE_STATE_CACHE
                                        .get_or_init(|| {
                                            std::sync::Mutex::new((
                                                None, None, false, false, false, false, None,
                                            ))
                                        })
                                        .lock()
                                        .ok()
                                        .map(|c| c.clone());
                                    if let Some((gid, Some(cid), sm, sd, sv, true, uid_opt)) =
                                        cached
                                    {
                                        println!(
                                            "[VOICE-RTC] Broadcaster Stream RTC {e} expired; re-requesting OP 18 STREAM_CREATE..."
                                        );
                                        if let Ok(mut prev_sk) = LAST_STREAM_KEY
                                            .get_or_init(|| std::sync::Mutex::new(None))
                                            .lock()
                                        {
                                            *prev_sk = None;
                                        }
                                        let _ = queue_live_voice_state_ext(
                                            gid,
                                            Some(cid),
                                            sm,
                                            sd,
                                            sv,
                                            true,
                                            uid_opt.as_deref(),
                                        );
                                    }
                                } else if let Some(skey) = LAST_WATCHED_STREAM_KEY
                                    .get_or_init(|| std::sync::Mutex::new(None))
                                    .lock()
                                    .ok()
                                    .and_then(|g| g.clone())
                                {
                                    println!(
                                        "[VOICE-RTC] Stream RTC {e} expired; re-requesting OP 20 STREAM_WATCH for {skey}"
                                    );
                                    let _ = queue_live_stream_watch(&skey);
                                }
                                break;
                            }
                        }
                        attempt = attempt.saturating_add(1);
                        let backoff_ms = (1_000u64 * u64::from(attempt.min(4))).min(4_000);
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    }
                });
            }));
        })
        .ok();
}

/// Запускает в фоновом потоке реальное подключение к RTC-серверу Discord (`wss://{endpoint}/?v=8`),
/// выполняет `OP 0 IDENTIFY`, `OP 2 READY`, 74-байтовый UDP IP Discovery, `OP 1 SELECT_PROTOCOL`,
/// принимает `OP 4 SESSION_DESCRIPTION` и ведёт цикл `OP 3 HEARTBEAT` / `OP 6 HEARTBEAT_ACK` с замером реального RTT.
pub fn start_live_rtc_session(
    endpoint: String,
    server_id: String,
    user_id: String,
    session_id: String,
    token: String,
    channel_id: Option<String>,
) {
    let clean_ep = endpoint
        .trim()
        .trim_start_matches("wss://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string();
    if clean_ep.is_empty() || server_id.is_empty() || session_id.is_empty() || token.is_empty() {
        return;
    }
    let short_host = clean_ep
        .split(':')
        .next()
        .unwrap_or(&clean_ep)
        .split('.')
        .next()
        .unwrap_or(&clean_ep)
        .to_string();
    let generation = LIVE_RTC_GENERATION
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        .wrapping_add(1);
    {
        let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
        g.state = "connecting".to_string();
        g.endpoint = Some(clean_ep.clone());
        g.short_hostname = Some(short_host);
        g.guild_id = Some(server_id.clone());
        if channel_id.is_some() {
            g.channel_id = channel_id.clone();
        }
        g.session_id = Some(session_id.clone());
        g.error = None;
    }
    std::thread::Builder::new()
        .name("voice-rtc-session".to_string())
        .spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(async move {
                    let mut attempt: u32 = 0;
                    let mut resume_seq_ack: Option<u16> = None;
                    while LIVE_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
                        == generation
                    {
                        let res = run_rtc_task(
                            generation,
                            clean_ep.clone(),
                            server_id.clone(),
                            user_id.clone(),
                            session_id.clone(),
                            token.clone(),
                            channel_id.clone(),
                            None,
                            &mut resume_seq_ack,
                        )
                        .await;
                        if LIVE_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
                            != generation
                        {
                            break;
                        }
                        if let Err(ref e) = res {
                            if e.contains("4014") {
                                {
                                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                    g.state = "disconnected".to_string();
                                }
                                break;
                            }
                            if e.contains("4006") || e.contains("4022") {
                                {
                                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                    g.state = "reconnecting".to_string();
                                }
                                let cached = LAST_VOICE_STATE_CACHE
                                    .get_or_init(|| {
                                        std::sync::Mutex::new((
                                            None, None, false, false, false, false, None,
                                        ))
                                    })
                                    .lock()
                                    .ok()
                                    .map(|c| c.clone());
                                if let Some((gid, Some(cid), sm, sd, sv, ss, uid_opt)) = cached {
                                    println!(
                                        "[VOICE-RTC] Voice session {e} expired while channel {cid} active; requesting fresh Gateway OP 4 session..."
                                    );
                                    if let Ok(mut prev_sk) = LAST_STREAM_KEY
                                        .get_or_init(|| std::sync::Mutex::new(None))
                                        .lock()
                                    {
                                        *prev_sk = None;
                                    }
                                    let _ = queue_live_voice_state_ext(
                                        gid,
                                        Some(cid),
                                        sm,
                                        sd,
                                        sv,
                                        ss,
                                        uid_opt.as_deref(),
                                    );
                                }
                                break;
                            }
                        }
                        {
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            g.state = "reconnecting".to_string();
                        }
                        attempt = attempt.saturating_add(1);
                        let backoff_ms = (1_200u64 * u64::from(attempt.min(5))).min(5_000);
                        println!(
                            "[VOICE-RTC] Session loop exited (attempt={attempt}), auto-reconnecting in {backoff_ms}ms..."
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    }
                });
            }));
        })
        .ok();
}

/// Порог последовательных ошибок отправки UDP в цикле Voice WS `v=8` (этап V4).
/// При достижении 10 ошибок подряд сессия завершается с переподключением.
pub const VOICE_UDP_MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Счётчик последовательных сбоев отправки UDP-пакетов (`UdpFailures`, этап V4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UdpFailures {
    consecutive_failures: u32,
}

impl UdpFailures {
    /// Создаёт новый счётчик без накопленных ошибок.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consecutive_failures: 0,
        }
    }

    /// Текущее число последовательных ошибок отправки UDP.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Возвращает `true`, если достигнут порог [`VOICE_UDP_MAX_CONSECUTIVE_FAILURES`] (`10`).
    #[must_use]
    pub const fn should_reconnect(&self) -> bool {
        self.consecutive_failures >= VOICE_UDP_MAX_CONSECUTIVE_FAILURES
    }

    /// Сбрасывает счётчик последовательных ошибок после успешной отправки пакета.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Регистрирует одну ошибку отправки UDP и возвращает `true`, если требуется переподключение (`>= 10`).
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.should_reconnect()
    }

    /// Обрабатывает результат отправки UDP-пакета: успех сбрасывает счётчик,
    /// а 10-я ошибка подряд возвращает `Err("udp_failures:10")`.
    pub fn record_send_result<T, E>(&mut self, result: Result<T, E>) -> Result<(), String> {
        match result {
            Ok(_) => {
                self.record_success();
                Ok(())
            }
            Err(_) => {
                if self.record_failure() {
                    Err(format!("udp_failures:{}", self.consecutive_failures))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Отправляет пакет через `tokio::net::UdpSocket`, учитывая `UdpFailures` (10 ошибок подряд -> `Err`).
    pub async fn send_packet(
        &mut self,
        sock: &tokio::net::UdpSocket,
        packet: &[u8],
    ) -> Result<(), String> {
        let res = sock.send(packet).await;
        self.record_send_result(res)
    }
}

/// Обновляет `seq_ack: Option<u16>` из входящего текстового JSON-кадра Voice WS `v=8` (поле `"seq"`).
pub fn update_voice_v8_seq_ack_from_text(seq_ack: &mut Option<u16>, text: &str) -> Option<u16> {
    let parsed = chord_json::parse(text).ok()?;
    update_voice_v8_seq_ack_from_json(seq_ack, &parsed)
}

fn update_voice_v8_seq_ack_from_json(
    seq_ack: &mut Option<u16>,
    parsed: &chord_json::JsonValue,
) -> Option<u16> {
    let seq_i64 = parsed
        .get("seq")
        .and_then(chord_json::JsonValue::as_i64)?;
    let seq_u16 = u16::try_from(seq_i64).ok()?;
    *seq_ack = Some(seq_u16);
    Some(seq_u16)
}

/// Обновляет `seq_ack: Option<u16>` из входящего бинарного кадра Voice WS `v=8`
/// (`u16::from_be_bytes([bin[0], bin[1]])` при `bin.len() >= 3`).
/// Возвращает `Some((seq, bin_op, payload))`.
pub fn update_voice_v8_seq_ack_from_binary<'a>(
    seq_ack: &mut Option<u16>,
    bin: &'a [u8],
) -> Option<(u16, u8, &'a [u8])> {
    if bin.len() < 3 {
        return None;
    }
    let seq = u16::from_be_bytes([bin[0], bin[1]]);
    *seq_ack = Some(seq);
    Some((seq, bin[2], &bin[3..]))
}

/// Строит JSON-кадр `op:3 Heartbeat` для Voice WS `v=8` с передачей `seq_ack: Option<u16>`.
#[must_use]
pub fn build_voice_v8_heartbeat_op3(nonce: u64, seq_ack: Option<u16>) -> String {
    let seq_val = seq_ack.map_or(-1i64, i64::from);
    chord_voice::VoiceHeartbeat {
        t: nonce,
        seq_ack: seq_val,
    }
    .to_json()
}

/// Строит JSON-кадр `op:7 Resume` для Voice WS `v=8` с передачей `seq_ack: Option<u16>`.
#[must_use]
pub fn build_voice_v8_resume_op7(
    server_id: &str,
    channel_id: &str,
    session_id: &str,
    token: &str,
    seq_ack: Option<u16>,
) -> String {
    let seq_val = seq_ack.map_or(-1i64, i64::from);
    chord_voice::VoiceResume {
        server_id: server_id.to_string(),
        channel_id: channel_id.to_string(),
        session_id: session_id.to_string(),
        token: token.to_string(),
        seq_ack: seq_val,
    }
    .to_json()
}

/// Строит JSON-кадр `op:16 VoiceBackendVersion / RTC Ping` для Voice WS `v=8`.
#[must_use]
pub fn build_voice_v8_op16() -> String {
    "{\"op\":16,\"d\":{}}".to_string()
}

/// Строит JSON-кадр `op:12 Video` для Voice WS `v=8` с маппингом `audio_ssrc`, `video_ssrc` и `rtx_ssrc`.
#[must_use]
pub fn build_voice_v8_video_op12(
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    active: bool,
) -> String {
    if active && video_ssrc > 0 {
        let eff_rtx = if rtx_ssrc > 0 {
            rtx_ssrc
        } else {
            video_ssrc.wrapping_add(1)
        };
        format!(
            "{{\"op\":12,\"d\":{{\"audio_ssrc\":{audio_ssrc},\"video_ssrc\":{video_ssrc},\"rtx_ssrc\":{eff_rtx},\"streams\":[{{\"type\":\"video\",\"rid\":\"100\",\"ssrc\":{video_ssrc},\"active\":true,\"quality\":100,\"rtx_ssrc\":{eff_rtx},\"max_bitrate\":6000000,\"max_framerate\":60,\"max_resolution\":{{\"type\":\"fixed\",\"width\":1280,\"height\":720}}}}]}}}}"
        )
    } else {
        format!(
            "{{\"op\":12,\"d\":{{\"audio_ssrc\":{audio_ssrc},\"video_ssrc\":0,\"rtx_ssrc\":0,\"streams\":[]}}}}"
        )
    }
}

/// Исходящий кадр сигнализации Voice WS `v=8`, порождённый pure-Rust обработчиком DAVE/RTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceV8OutboundFrame {
    /// Текстовый JSON-кадр (например, `op:23 ReadyForTransition` или `op:31 InvalidCommitWelcome`).
    Text(String),
    /// Бинарный MLS-кадр (например, `op:26 KeyPackage` или `op:28 CommitWelcome`).
    Binary(Vec<u8>),
}

/// Задержка первого `OP 3 HEARTBEAT` после приёма `OP 8 HELLO` в Voice WS `v=8` (60 мс вместо ожидания полного `13_750` мс).
pub const VOICE_V8_INITIAL_HEARTBEAT_DELAY_MS: u64 = 60;

/// Стандартный интервал `heartbeat_interval` в Voice WS `v=8` (13.75 с).
pub const VOICE_V8_DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 13_750;

/// Состояние pure-Rust ветки Voice WS `v=8` (`when dv_stdin.is_none()`):
/// отслеживает `seq_ack: Option<u16>`, `OP 8 HELLO` (интервал + мгновенный 60 мс первый `OP 3 HEARTBEAT`),
/// `OP 6 HEARTBEAT_ACK` (замер реального RTT), `OP 16` (версии бэкенда и ответный heartbeat/ping),
/// `OP 12 VIDEO` (`video_ssrc` и `rtx_ssrc`), управляет движком `DaveMlsEngine` (`op:4 -> op:26`,
/// `op:25`, `op:27 -> op:28`, `op:29/30 -> op:23 | op:31 + op:26`) и считает `UdpFailures`.
#[derive(Debug)]
pub struct PureRustVoiceV8State {
    server_id: String,
    channel_id_str: String,
    own_user_id: u64,
    channel_id: u64,
    session_id: String,
    token: String,
    seq_ack: Option<u16>,
    heartbeat_interval_ms: u64,
    initial_heartbeat_delay_ms: Option<u64>,
    hb_counter: u64,
    last_hb_sent: Option<(u64, Instant)>,
    last_rtt_ms: Option<u64>,
    local_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    is_broadcaster: bool,
    video_codec: Option<String>,
    voice_backend_version: Option<String>,
    rtc_worker_version: Option<String>,
    dave_engine: Option<chord_voice::srtp::DaveMlsEngine>,
    udp_failures: UdpFailures,
}

impl PureRustVoiceV8State {
    /// Создаёт новое состояние pure-Rust Voice WS `v=8`.
    #[must_use]
    pub fn new(
        server_id: &str,
        channel_id: &str,
        user_id: &str,
        session_id: &str,
        token: &str,
    ) -> Self {
        let own_user_id = user_id
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|&v| v > 0)
            .unwrap_or(1);
        let cid_num = channel_id
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|&v| v > 0)
            .or_else(|| server_id.trim().parse::<u64>().ok().filter(|&v| v > 0))
            .unwrap_or(1);
        Self {
            server_id: server_id.to_string(),
            channel_id_str: channel_id.to_string(),
            own_user_id,
            channel_id: cid_num,
            session_id: session_id.to_string(),
            token: token.to_string(),
            seq_ack: None,
            heartbeat_interval_ms: VOICE_V8_DEFAULT_HEARTBEAT_INTERVAL_MS,
            initial_heartbeat_delay_ms: None,
            hb_counter: 1,
            last_hb_sent: None,
            last_rtt_ms: None,
            local_ssrc: 1,
            video_ssrc: 2,
            rtx_ssrc: 3,
            is_broadcaster: false,
            video_codec: None,
            voice_backend_version: None,
            rtc_worker_version: None,
            dave_engine: None,
            udp_failures: UdpFailures::new(),
        }
    }

    /// Возвращает локальный `audio_ssrc` из `OP 2 READY`.
    #[must_use]
    pub const fn local_ssrc(&self) -> u32 {
        self.local_ssrc
    }

    /// Возвращает назначенный `video_ssrc` из `OP 2 READY` (`d.streams`).
    #[must_use]
    pub const fn video_ssrc(&self) -> u32 {
        self.video_ssrc
    }

    /// Возвращает назначенный `rtx_ssrc` из `OP 2 READY` (`d.streams`).
    #[must_use]
    pub const fn rtx_ssrc(&self) -> u32 {
        self.rtx_ssrc
    }

    /// Включает или выключает режим вещателя (`Go Live` / видео), при котором на `OP 4` отправляется активный `OP 12 VIDEO`.
    pub fn set_broadcaster(&mut self, is_broadcaster: bool) {
        self.is_broadcaster = is_broadcaster;
    }

    /// Возвращает `true`, если сессия находится в режиме вещателя (`Go Live` / видео).
    #[must_use]
    pub const fn is_broadcaster(&self) -> bool {
        self.is_broadcaster
    }

    /// Возвращает согласованный видеокодек из входящего `OP 4 SESSION_DESCRIPTION` (например, `"AV1"`).
    #[must_use]
    pub fn video_codec(&self) -> Option<&str> {
        self.video_codec.as_deref()
    }

    /// Возвращает версию голосового бэкенда из входящего `OP 16`.
    #[must_use]
    pub fn voice_backend_version(&self) -> Option<&str> {
        self.voice_backend_version.as_deref()
    }

    /// Возвращает версию `rtc_worker` из входящего `OP 16`.
    #[must_use]
    pub fn rtc_worker_version(&self) -> Option<&str> {
        self.rtc_worker_version.as_deref()
    }

    /// Формирует JSON `op:12 Video` с текущими `local_ssrc`, `video_ssrc` и `rtx_ssrc`.
    #[must_use]
    pub fn build_video_op12(&self, active: bool) -> String {
        build_voice_v8_video_op12(self.local_ssrc, self.video_ssrc, self.rtx_ssrc, active)
    }

    /// Возвращает интервал `heartbeat_interval` из `OP 8 HELLO` (по умолчанию `13_750` мс).
    #[must_use]
    pub const fn heartbeat_interval_ms(&self) -> u64 {
        self.heartbeat_interval_ms
    }

    /// Возвращает задержку до первого `OP 3 HEARTBEAT` после `OP 8 HELLO` (`Some(60)` мс).
    #[must_use]
    pub const fn initial_heartbeat_delay_ms(&self) -> Option<u64> {
        self.initial_heartbeat_delay_ms
    }

    /// Возвращает последний измеренный RTT по паре `OP 3 HEARTBEAT` -> `OP 6 HEARTBEAT_ACK`.
    #[must_use]
    pub const fn last_rtt_ms(&self) -> Option<u64> {
        self.last_rtt_ms
    }

    /// Возвращает текущий `seq_ack: Option<u16>`.
    #[must_use]
    pub const fn seq_ack(&self) -> Option<u16> {
        self.seq_ack
    }

    /// Устанавливает `seq_ack: Option<u16>`.
    pub fn set_seq_ack(&mut self, seq: Option<u16>) {
        self.seq_ack = seq;
    }

    /// Ссылка на счётчик `UdpFailures`.
    #[must_use]
    pub const fn udp_failures(&self) -> &UdpFailures {
        &self.udp_failures
    }

    /// Мутабельная ссылка на счётчик `UdpFailures`.
    pub fn udp_failures_mut(&mut self) -> &mut UdpFailures {
        &mut self.udp_failures
    }

    /// Ссылка на инициализированный движок `DaveMlsEngine`.
    #[must_use]
    pub const fn dave_engine(&self) -> Option<&chord_voice::srtp::DaveMlsEngine> {
        self.dave_engine.as_ref()
    }

    /// Формирует JSON `op:3 Heartbeat` с текущим `seq_ack`.
    #[must_use]
    pub fn build_heartbeat_op3(&self, nonce: u64) -> String {
        build_voice_v8_heartbeat_op3(nonce, self.seq_ack)
    }

    /// Формирует JSON `op:7 Resume` с текущим `seq_ack`.
    #[must_use]
    pub fn build_resume_op7(&self) -> String {
        build_voice_v8_resume_op7(
            &self.server_id,
            &self.channel_id_str,
            &self.session_id,
            &self.token,
            self.seq_ack,
        )
    }

    /// Инициализирует DAVE v1 MLS при получении `op:4 SessionDescription` (`dave_protocol_version >= 1`)
    /// и возвращает бинарный кадр `op:26 KeyPackage`.
    pub fn on_op4_session_description(
        &mut self,
        mode: &str,
        dave_protocol_version: i64,
        secret_key: &[u8; 32],
    ) -> Option<Vec<u8>> {
        if dave_protocol_version < 1 {
            self.dave_engine = None;
            return None;
        }
        let enc_mode = chord_voice::srtp::EncryptionMode::from_engine_name(mode)
            .unwrap_or(chord_voice::srtp::EncryptionMode::Aes256GcmRtpSize);
        let (engine, op26) = chord_voice::srtp::DaveMlsEngine::init_on_op4(
            self.own_user_id,
            self.channel_id,
            1,
            secret_key,
            enc_mode,
        )
        .ok()?;
        self.dave_engine = Some(engine);
        Some(op26)
    }

    fn ensure_dave_engine(&mut self) -> &mut chord_voice::srtp::DaveMlsEngine {
        if self.dave_engine.is_none() {
            let default_key = [0x42u8; 32];
            if let Ok((engine, _)) = chord_voice::srtp::DaveMlsEngine::init_on_op4(
                self.own_user_id,
                self.channel_id,
                1,
                &default_key,
                chord_voice::srtp::EncryptionMode::Aes256GcmRtpSize,
            ) {
                self.dave_engine = Some(engine);
            }
        }
        self.dave_engine
            .as_mut()
            .expect("DaveMlsEngine initialized")
    }

    /// Обрабатывает входящий текстовый JSON-кадр Voice WS `v=8` в pure-Rust режиме.
    pub fn on_text_frame(&mut self, text: &str) -> Result<Vec<VoiceV8OutboundFrame>, String> {
        let parsed = chord_json::parse(text).map_err(|e| e.to_string())?;
        let _ = update_voice_v8_seq_ack_from_json(&mut self.seq_ack, &parsed);
        let Some(op) = parsed.get("op").and_then(chord_json::JsonValue::as_i64) else {
            return Ok(Vec::new());
        };
        let d = parsed.get("d");
        let mut out = Vec::new();
        match op {
            8 => {
                if let Some(interval) = d
                    .and_then(|d| d.get("heartbeat_interval"))
                    .and_then(chord_json::JsonValue::as_i64)
                    && interval > 500
                {
                    self.heartbeat_interval_ms = (interval as u64).clamp(5_000, 15_000);
                }
                self.initial_heartbeat_delay_ms = Some(VOICE_V8_INITIAL_HEARTBEAT_DELAY_MS);
                let nonce = 1_700_000_000_000u64 + self.hb_counter;
                self.hb_counter = self.hb_counter.saturating_add(1);
                let hb_json = self.build_heartbeat_op3(nonce);
                self.last_hb_sent = Some((nonce, Instant::now()));
                out.push(VoiceV8OutboundFrame::Text(hb_json));
            }
            2 => {
                let ssrc = d
                    .and_then(|d| d.get("ssrc"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .filter(|&v| v > 0)
                    .unwrap_or(1) as u32;
                self.local_ssrc = ssrc;
                let mut v_ssrc = ssrc.wrapping_add(1);
                let mut r_ssrc = ssrc.wrapping_add(2);
                if let Some(streams_arr) = d
                    .and_then(|d| d.get("streams"))
                    .and_then(chord_json::JsonValue::as_array)
                {
                    for s in streams_arr {
                        let stype = s
                            .get("type")
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("");
                        if stype == "video" || stype == "screen" || stype.is_empty() {
                            if let Some(v) = s
                                .get("ssrc")
                                .and_then(chord_json::JsonValue::as_i64)
                                .filter(|&v| v > 0)
                            {
                                v_ssrc = v as u32;
                                r_ssrc = v_ssrc.wrapping_add(1);
                            }
                            if let Some(r) = s
                                .get("rtx_ssrc")
                                .or_else(|| s.get("rtxSsrc"))
                                .and_then(chord_json::JsonValue::as_i64)
                                .filter(|&v| v > 0)
                            {
                                r_ssrc = r as u32;
                            }
                        }
                    }
                }
                self.video_ssrc = v_ssrc;
                self.rtx_ssrc = r_ssrc;
                out.push(VoiceV8OutboundFrame::Text(build_voice_v8_op16()));
            }
            16 => {
                if let Some(v) = d
                    .and_then(|d| d.get("voice"))
                    .and_then(chord_json::JsonValue::as_str)
                {
                    self.voice_backend_version = Some(v.to_string());
                }
                if let Some(w) = d
                    .and_then(|d| d.get("rtc_worker"))
                    .and_then(chord_json::JsonValue::as_str)
                {
                    self.rtc_worker_version = Some(w.to_string());
                }
                let nonce = d
                    .and_then(|d| {
                        d.get("t")
                            .or_else(|| d.get("ping"))
                            .or_else(|| d.get("nonce"))
                    })
                    .and_then(chord_json::JsonValue::as_i64)
                    .and_then(|v| u64::try_from(v).ok())
                    .unwrap_or_else(|| {
                        let n = 1_700_000_000_000u64 + self.hb_counter;
                        self.hb_counter = self.hb_counter.saturating_add(1);
                        n
                    });
                let hb_json = self.build_heartbeat_op3(nonce);
                self.last_hb_sent = Some((nonce, Instant::now()));
                out.push(VoiceV8OutboundFrame::Text(hb_json));
            }
            6 => {
                if let Some((_, sent_at)) = self.last_hb_sent.take() {
                    let rtt = u64::try_from(sent_at.elapsed().as_millis())
                        .unwrap_or(1)
                        .max(1);
                    self.last_rtt_ms = Some(rtt);
                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                    g.last_ping_ms = Some(rtt);
                    g.ping_history.push(rtt);
                    if g.ping_history.len() > 40 {
                        g.ping_history.remove(0);
                    }
                    let sum: u64 = g.ping_history.iter().sum();
                    g.average_ping_ms = Some(sum / (g.ping_history.len() as u64));
                }
            }
            4 => {
                let mode = d
                    .and_then(|d| d.get("mode"))
                    .and_then(chord_json::JsonValue::as_str)
                    .unwrap_or("aead_aes256_gcm_rtpsize");
                if let Some(vc) = d
                    .and_then(|d| d.get("video_codec").or_else(|| d.get("videoCodec")))
                    .and_then(chord_json::JsonValue::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("unknown"))
                {
                    let upper = vc.to_ascii_uppercase();
                    let normalized =
                        if upper == "AV1" || upper == "AV1X" || upper.starts_with("AV1") {
                            "AV1".to_string()
                        } else {
                            upper
                        };
                    self.video_codec = Some(normalized.clone());
                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                    g.active_video_codec = normalized;
                }
                let dave_ver = d
                    .and_then(|d| d.get("dave_protocol_version"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .unwrap_or(0);
                let mut key_bytes = [0x42u8; 32];
                if let Some(arr) = d
                    .and_then(|d| d.get("secret_key"))
                    .and_then(chord_json::JsonValue::as_array)
                {
                    if arr.len() == 32 {
                        for (i, v) in arr.iter().enumerate() {
                            key_bytes[i] = v.as_i64().unwrap_or(0).clamp(0, 255) as u8;
                        }
                    }
                }
                if self.is_broadcaster {
                    out.push(VoiceV8OutboundFrame::Text(self.build_video_op12(true)));
                }
                if let Some(op26) = self.on_op4_session_description(mode, dave_ver, &key_bytes) {
                    out.push(VoiceV8OutboundFrame::Binary(op26));
                }
            }
            11 => {
                if let Some(arr) = d
                    .and_then(|d| d.get("user_ids"))
                    .and_then(chord_json::JsonValue::as_array)
                {
                    let peer_ids: Vec<u64> = arr
                        .iter()
                        .filter_map(|v| v.as_str()?.parse::<u64>().ok())
                        .filter(|&id| id > 0)
                        .collect();
                    if !peer_ids.is_empty()
                        && let Some(engine) = self.dave_engine.as_mut()
                    {
                        let _ = engine.on_clients_connect_op11(&peer_ids);
                    }
                }
            }
            13 => {
                if let Some(uid) = d
                    .and_then(|d| d.get("user_id"))
                    .and_then(chord_json::JsonValue::as_str)
                    .and_then(|s| s.parse::<u64>().ok())
                    && let Some(engine) = self.dave_engine.as_mut()
                {
                    let _ = engine.on_client_disconnect_op13(uid);
                }
            }
            21 => {
                let tid = d
                    .and_then(|d| d.get("transition_id"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .unwrap_or(0);
                let proto_ver = d
                    .and_then(|d| d.get("protocol_version"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .unwrap_or(1);
                if let Some(engine) = self.dave_engine.as_mut()
                    && let Ok(Some(op23_json)) = engine.on_prepare_transition_op21(
                        u16::try_from(tid.max(0)).unwrap_or(0),
                        u8::try_from(proto_ver.max(0)).unwrap_or(1),
                    )
                {
                    out.push(VoiceV8OutboundFrame::Text(op23_json));
                } else if tid > 0 {
                    let op23_json = chord_voice::dave::DaveReadyForTransition {
                        transition_id: u16::try_from(tid).unwrap_or(0),
                    }
                    .to_json();
                    out.push(VoiceV8OutboundFrame::Text(op23_json));
                }
            }
            22 => {
                let tid = d
                    .and_then(|d| d.get("transition_id"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .unwrap_or(0);
                if let Some(engine) = self.dave_engine.as_mut() {
                    let _ =
                        engine.on_execute_transition_op22(u16::try_from(tid.max(0)).unwrap_or(0));
                }
            }
            24 => {
                let epoch = d
                    .and_then(|d| d.get("epoch"))
                    .and_then(chord_json::JsonValue::as_i64)
                    .unwrap_or(1);
                if epoch == 1 {
                    let engine = self.ensure_dave_engine();
                    out.push(VoiceV8OutboundFrame::Binary(engine.key_package_op26()));
                }
            }
            _ => {}
        }
        Ok(out)
    }

    /// Обрабатывает входящий бинарный кадр DAVE (`op:25`, `op:27`, `op:29`, `op:30`) в pure-Rust режиме.
    pub fn on_binary_frame(&mut self, bin: &[u8]) -> Result<Vec<VoiceV8OutboundFrame>, String> {
        let Some((_, bin_op, payload)) =
            update_voice_v8_seq_ack_from_binary(&mut self.seq_ack, bin)
        else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        match bin_op {
            25 => {
                let engine = self.ensure_dave_engine();
                engine
                    .on_external_sender_op25(bin)
                    .map_err(|e| e.to_string())?;
            }
            27 => {
                let engine = self.ensure_dave_engine();
                let mut res = engine.on_proposals_op27(bin);
                if matches!(res, Ok(None)) && !payload.is_empty() {
                    let _ = engine.on_external_sender_op25(&[0, 0, 25, 1, 2, 3, 4]);
                    res = engine.on_proposals_op27(bin);
                }
                if let Ok(Some(op28)) = res {
                    out.push(VoiceV8OutboundFrame::Binary(op28));
                } else if !payload.is_empty() {
                    let mut normalized = Vec::with_capacity(4 + payload.len());
                    normalized.extend_from_slice(&bin[0..2]);
                    normalized.push(27);
                    normalized.push(0);
                    normalized.extend_from_slice(payload);
                    if let Ok(Some(op28)) = engine.on_proposals_op27(&normalized) {
                        out.push(VoiceV8OutboundFrame::Binary(op28));
                    }
                }
            }
            29 | 30 => {
                let engine = self.ensure_dave_engine();
                match engine.on_commit_or_welcome_op29_op30(bin) {
                    Ok(chord_voice::srtp::DaveTransitionOutcome::SendTransitionReadyOp23 {
                        op23_json,
                        ..
                    }) => {
                        out.push(VoiceV8OutboundFrame::Text(op23_json));
                    }
                    Ok(chord_voice::srtp::DaveTransitionOutcome::ExecutedImmediate {
                        ..
                    }) => {}
                    Ok(chord_voice::srtp::DaveTransitionOutcome::InvalidCommitWelcome {
                        op31_json,
                        fresh_key_package_op26,
                        ..
                    }) => {
                        out.push(VoiceV8OutboundFrame::Text(op31_json));
                        out.push(VoiceV8OutboundFrame::Binary(fresh_key_package_op26));
                    }
                    Err(_) if payload.len() >= 2 => {
                        let tid = u16::from_be_bytes([payload[0], payload[1]]);
                        let op31_json =
                            chord_voice::dave::DaveInvalidCommitWelcome { transition_id: tid }
                                .to_json();
                        let fresh_op26 = engine.key_package_op26();
                        out.push(VoiceV8OutboundFrame::Text(op31_json));
                        out.push(VoiceV8OutboundFrame::Binary(fresh_op26));
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            _ => {}
        }
        Ok(out)
    }
}

/// Читает и обрабатывает один входящий кадр из `WebSocketStream` в pure-Rust режиме Voice WS `v=8`,
/// отправляя сформированные ответные кадры (`op:26`, `op:28`, `op:23`, `op:31 + op:26`) обратно в сокет.
pub async fn drive_voice_v8_ws_step<S>(
    ws: &mut WebSocketStream<S>,
    state: &mut PureRustVoiceV8State,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Some(msg_res) = ws.next().await else {
        return Err("ws_closed".to_string());
    };
    let msg = msg_res.map_err(|e| e.to_string())?;
    let outbound = match msg {
        Message::Text(t) => state.on_text_frame(&t)?,
        Message::Binary(b) => state.on_binary_frame(&b)?,
        Message::Ping(p) => {
            ws.send(Message::Pong(p)).await.map_err(|e| e.to_string())?;
            Vec::new()
        }
        _ => Vec::new(),
    };
    for frame in outbound {
        match frame {
            VoiceV8OutboundFrame::Text(txt) => {
                ws.send(Message::Text(txt.into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            VoiceV8OutboundFrame::Binary(bin) => {
                ws.send(Message::Binary(bin.into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

async fn run_rtc_task(
    generation: u64,
    clean_ep: String,
    server_id: String,
    user_id: String,
    session_id: String,
    token: String,
    channel_id: Option<String>,
    stream_owner_id: Option<String>,
    resume_seq_ack: &mut Option<u16>,
) -> Result<(), String> {
    let is_stream_conn = stream_owner_id.is_some();
    let is_cached_broadcaster = LAST_VOICE_STATE_CACHE
        .get()
        .and_then(|m| m.lock().ok())
        .is_some_and(|c| c.5);
    let is_watching_other = LAST_WATCHED_STREAM_KEY
        .get()
        .and_then(|m| m.lock().ok())
        .is_some_and(|w| w.is_some());
    let user_id =
        if (user_id.is_empty() || user_id == "me") && is_cached_broadcaster && !is_watching_other {
            stream_owner_id.clone().unwrap_or(user_id)
        } else {
            user_id
        };
    let is_self_broadcaster = stream_owner_id.as_deref() == Some(user_id.as_str())
        || (is_stream_conn && is_cached_broadcaster && !is_watching_other);
    ensure_crypto_provider();
    let connect_t = Instant::now();
    let url = format!("wss://{clean_ep}/?v=8");
    let (mut ws, _) = connect_async_tls_with_config(&url, None, false, tls_connector())
        .await
        .map_err(|e| {
            if !is_stream_conn
                && LIVE_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst) == generation
            {
                let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                g.state = "error".to_string();
                g.error = Some(format!("RTC WS connect: {e}"));
            }
            e.to_string()
        })?;

    let ws_handshake_rtt = u64::try_from(connect_t.elapsed().as_millis())
        .unwrap_or(42)
        .clamp(1, 999);
    if !is_stream_conn {
        let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
        g.last_ping_ms = Some(ws_handshake_rtt);
        g.average_ping_ms = Some(ws_handshake_rtt);
        if g.ping_history.len() >= 40 {
            g.ping_history.remove(0);
        }
        g.ping_history.push(ws_handshake_rtt);
        g.state = "negotiating".to_string();
    }

    let cid = channel_id.as_deref().unwrap_or(&server_id);
    struct ChildKillGuard {
        child: Option<std::process::Child>,
        is_stream: bool,
    }
    impl Drop for ChildKillGuard {
        fn drop(&mut self) {
            if let Some(mut c) = self.child.take() {
                let pid = c.id();
                let _ = c.kill();
                let _ = c.wait();
                if self.is_stream {
                    let _ = LIVE_STREAM_DV_PID.compare_exchange(
                        pid,
                        0,
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                } else {
                    let _ = LIVE_VOICE_DV_PID.compare_exchange(
                        pid,
                        0,
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                }
            }
        }
    }
    let mut dv_guard = ChildKillGuard {
        child: None,
        is_stream: is_stream_conn,
    };
    let mut dv_stdin: Option<std::process::ChildStdin> = None;
    let (dv_line_tx, mut dv_line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    if ensure_dv_native_bridge_files() {
        let gpu_caps = probe_gpu_and_codecs();
        let mut cmd_builder = std::process::Command::new("node");
        cmd_builder
            .arg("/tmp/discord_voice_dave_bridge.js")
            .env("XDG_SESSION_TYPE", "x11")
            .env("DV_GPU_VENDOR", &gpu_caps.vendor)
            .env("DV_PREFERRED_CODEC", &gpu_caps.preferred_video_codec)
            .env("PULSE_LATENCY_MSEC", "20");
        if let Ok(disp) = std::env::var("DISPLAY") {
            cmd_builder.env("DISPLAY", disp);
        } else {
            cmd_builder.env("DISPLAY", ":0");
        }
        cmd_builder
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        if is_stream_conn {
            cmd_builder.env("DV_CONTEXT", "stream");
            cmd_builder.env(
                "PULSE_PROP",
                "application.name=DiscordStreamAudio application.id=lab.discord.stream.audio media.role=video",
            );
            // Строгая изоляция микрофона от потока трансляции (Stream Worker):
            // Стрим-воркер НИКОГДА не должен открывать физический микрофон пользователя,
            // иначе WebRTC ADM перехватывает/глушит захват микрофона основного голосового канала.
            let mut monitor_src: Option<String> = None;
            if is_self_broadcaster {
                if let Ok(out) = std::process::Command::new("pactl")
                    .arg("get-default-sink")
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output()
                {
                    if out.status.success() {
                        let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !sink.is_empty() && !sink.contains("auto_null") {
                            monitor_src = Some(format!("{sink}.monitor"));
                        }
                    }
                }
            }
            if let Some(ref mon) = monitor_src {
                cmd_builder.env("PULSE_SOURCE", mon);
                cmd_builder.env("DV_HAS_SOUNDSHARE_MONITOR", "1");
            } else {
                cmd_builder.env("PULSE_SOURCE", "__discord_stream_no_mic__");
                cmd_builder.env("DV_HAS_SOUNDSHARE_MONITOR", "0");
            }
        } else {
            cmd_builder.env(
                "PULSE_PROP",
                "application.name=DiscordVoiceMic application.id=lab.discord.voice.mic media.role=phone",
            );
            let mut phys_mic: Option<String> = None;
            if let Ok(out) = std::process::Command::new("pactl")
                .arg("get-default-source")
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
            {
                if out.status.success() {
                    let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !src.is_empty() && !src.ends_with(".monitor") && !src.contains("auto_null") {
                        phys_mic = Some(src);
                    }
                }
            }
            if phys_mic.is_none() {
                if let Ok(out) = std::process::Command::new("pactl")
                    .args(["list", "short", "sources"])
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output()
                {
                    if out.status.success() {
                        for line in String::from_utf8_lossy(&out.stdout).lines() {
                            let cols: Vec<&str> = line.split_whitespace().collect();
                            if let Some(&name) = cols.get(1) {
                                if !name.ends_with(".monitor") && !name.contains("auto_null") {
                                    phys_mic = Some(name.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if let Some(ref mic_src) = phys_mic {
                cmd_builder.env("PULSE_SOURCE", mic_src);
            }
        }
        if let Ok(mut child) = cmd_builder.spawn() {
            let pid = child.id();
            if is_stream_conn {
                let prev = LIVE_STREAM_DV_PID.swap(pid, std::sync::atomic::Ordering::SeqCst);
                if prev > 1 && prev != pid {
                    kill_pid_immediate(prev);
                }
            } else {
                let prev = LIVE_VOICE_DV_PID.swap(pid, std::sync::atomic::Ordering::SeqCst);
                if prev > 1 && prev != pid {
                    kill_pid_immediate(prev);
                }
            }
            dv_stdin = child.stdin.take();
            if let Some(out) = child.stdout.take() {
                std::thread::spawn(move || {
                    use std::io::BufRead as _;
                    let reader = std::io::BufReader::new(out);
                    for line in reader.lines().map_while(Result::ok) {
                        if dv_line_tx.send(line).is_err() {
                            break;
                        }
                    }
                });
            }
            dv_guard.child = Some(child);
            if !is_stream_conn {
                NATIVE_VOICE_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            println!(
                "[VOICE-RTC] Spawned native chord_voice.node + DAVE E2EE worker pid={pid} (stream={is_stream_conn})"
            );
        }
    }
    let has_native_voice = dv_stdin.is_some();
    let max_dave_ver = if has_native_voice { 1 } else { 0 };
    let mut seq_ack: Option<u16> = *resume_seq_ack;
    let mut v8_state = PureRustVoiceV8State::new(&server_id, cid, &user_id, &session_id, &token);
    v8_state.set_seq_ack(seq_ack);
    let mut udp_failures = UdpFailures::new();

    if seq_ack.is_some() {
        let resume_json = build_voice_v8_resume_op7(&server_id, cid, &session_id, &token, seq_ack);
        println!(
            "[VOICE-RTC] WS reconnected to {url} (seq_ack={seq_ack:?}), sending OP 7 RESUME server={server_id} channel={cid}"
        );
        ws.send(Message::Text(resume_json.into()))
            .await
            .map_err(|e| e.to_string())?;
    } else {
        println!(
            "[VOICE-RTC] WS connected to {url} (rtt={ws_handshake_rtt}ms, native_voice={has_native_voice}, max_dave={max_dave_ver}), sending OP 0 IDENTIFY server={server_id} channel={cid} user={user_id}"
        );
        let identify_streams = if !is_stream_conn || is_self_broadcaster {
            "[{\"type\":\"video\",\"rid\":\"100\",\"quality\":100}]"
        } else {
            "[]"
        };
        let identify = format!(
            "{{\"op\":0,\"d\":{{\"server_id\":\"{server_id}\",\"channel_id\":\"{cid}\",\"user_id\":\"{user_id}\",\"session_id\":\"{session_id}\",\"token\":\"{token}\",\"max_dave_protocol_version\":{max_dave_ver},\"video\":true,\"streams\":{identify_streams}}}}}"
        );
        ws.send(Message::Text(identify.into()))
            .await
            .map_err(|e| e.to_string())?;
        let _ = ws.send(Message::Text("{\"op\":16,\"d\":{}}".into())).await;
    }

    let mut hb_interval_ms: u64 = 13_750;
    let mut last_hb_sent: Option<(u64, Instant)> = None;
    let mut hb_counter: u64 = 1;
    let mut next_hb = Instant::now() + Duration::from_millis(5_000);
    let mut udp_sock: Option<tokio::net::UdpSocket> = None;
    let mut udp_keepalive_tick = Instant::now() + Duration::from_secs(5);
    let mut tx_audio_tick = Instant::now() + Duration::from_millis(15);
    let mut keepalive_counter: u32 = 1;
    let mut udp_recv_buf = [0u8; 2048];
    let mut ssrc_to_user: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut rtp_speaking_until: std::collections::HashMap<String, Instant> =
        std::collections::HashMap::new();
    let mut srtp_ctx: Option<chord_voice::srtp::SrtpContext> = None;
    let mut local_ssrc: u32 = 1;
    let mut sfu_video_ssrc: Option<u32> = None;
    let mut sfu_rtx_ssrc: Option<u32> = None;
    let mut known_video_ssrcs: Vec<i64> = Vec::new();
    let mut session_desc_received = false;
    let mut rtp_seq: u16 = 1;
    let mut rtp_ts: u32 = 960;
    let mut local_tx_speaking = false;
    let mut last_local_tx_at = Instant::now();
    let mut rx_rtp_count: u64 = 0;
    let mut rx_decrypted_count: u64 = 0;

    loop {
        let cur_gen = if is_stream_conn {
            LIVE_STREAM_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
        } else {
            LIVE_RTC_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
        };
        if cur_gen != generation {
            println!("[VOICE-RTC] generation superseded ({generation}), closing RTC WS");
            drop(dv_guard);
            let _ = ws.close(None).await;
            return Ok(());
        }
        tokio::select! {
            msg = ws.next() => {
                let Some(msg_res) = msg else {
                    println!("[VOICE-RTC] WS stream ended (None)");
                    break;
                };
                let msg = match msg_res {
                    Ok(m) => m,
                    Err(e) => {
                        println!("[VOICE-RTC] WS recv error: {e}");
                        break;
                    }
                };
                let text = match msg {
                    Message::Text(t) => t.to_string(),
                    Message::Binary(b) => {
                        if let Some((seq, bin_op, payload)) =
                            update_voice_v8_seq_ack_from_binary(&mut seq_ack, &b)
                        {
                            *resume_seq_ack = seq_ack;
                            v8_state.set_seq_ack(seq_ack);
                            println!(
                                "[VOICE-RTC] WS RX BINARY op={bin_op} seq={seq} payload_len={}",
                                payload.len()
                            );
                            if let Some(stdin) = dv_stdin.as_mut() {
                                use std::io::Write as _;
                                let cmd = format!(
                                    "{{\"cmd\":\"binary\",\"op\":{bin_op},\"hex\":\"{}\"}}\n",
                                    bytes_to_hex(payload)
                                );
                                let _ = stdin.write_all(cmd.as_bytes());
                                let _ = stdin.flush();
                            } else if let Ok(out_frames) = v8_state.on_binary_frame(&b) {
                                for frame in out_frames {
                                    match frame {
                                        VoiceV8OutboundFrame::Binary(bin_out) => {
                                            let _ = ws.send(Message::Binary(bin_out.into())).await;
                                        }
                                        VoiceV8OutboundFrame::Text(txt_out) => {
                                            let _ = ws.send(Message::Text(txt_out.into())).await;
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        String::from_utf8_lossy(&b).to_string()
                    }
                    Message::Ping(p) => {
                        let _ = ws.send(Message::Pong(p)).await;
                        continue;
                    }
                    Message::Close(cf) => {
                        println!("[VOICE-RTC] WS closed by server: {cf:?}");
                        if let Some(frame) = cf {
                            let code: u16 = frame.code.into();
                            if code == 4014 || code == 4006 || code == 4022 {
                                *resume_seq_ack = None;
                                drop(dv_guard);
                                return Err(format!("closed:{code}"));
                            }
                        }
                        break;
                    }
                    _ => continue,
                };
                let Ok(parsed) = chord_json::parse(&text) else { continue; };
                if update_voice_v8_seq_ack_from_json(&mut seq_ack, &parsed).is_some() {
                    *resume_seq_ack = seq_ack;
                    v8_state.set_seq_ack(seq_ack);
                }
                let Some(op) = parsed.get("op").and_then(chord_json::JsonValue::as_i64) else {
                    continue;
                };
                if op != 6 && op != 4 {
                    println!(
                        "[VOICE-RTC] WS RX op={op} seq={} payload={text}",
                        seq_ack.map_or(-1i64, i64::from)
                    );
                }
                let d = parsed.get("d");
                match op {
                    8 => {
                        if let Some(interval) = d
                            .and_then(|d| d.get("heartbeat_interval"))
                            .and_then(chord_json::JsonValue::as_i64)
                        {
                            if interval > 500 {
                                hb_interval_ms = (interval as u64).clamp(5_000, 15_000);
                            }
                        }
                        println!("[VOICE-RTC] OP 8 HELLO heartbeat_interval={hb_interval_ms}ms");
                        next_hb = Instant::now() + Duration::from_millis(60);
                    }
                    2 => {
                        let ssrc = d
                            .and_then(|d| d.get("ssrc"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(1) as u32;
                        local_ssrc = ssrc;
                        if let Some(streams_arr) = d
                            .and_then(|d| d.get("streams"))
                            .and_then(chord_json::JsonValue::as_array)
                        {
                            for s in streams_arr {
                                let stype = s
                                    .get("type")
                                    .and_then(chord_json::JsonValue::as_str)
                                    .unwrap_or("");
                                if stype == "video" || stype == "screen" {
                                    if let Some(v) = s
                                        .get("ssrc")
                                        .and_then(chord_json::JsonValue::as_i64)
                                    {
                                        sfu_video_ssrc = Some(v as u32);
                                    }
                                    if let Some(r) = s
                                        .get("rtx_ssrc")
                                        .or_else(|| s.get("rtxSsrc"))
                                        .and_then(chord_json::JsonValue::as_i64)
                                    {
                                        if r > 0 {
                                            sfu_rtx_ssrc = Some(r as u32);
                                        }
                                    }
                                }
                            }
                        }
                        if sfu_rtx_ssrc.is_none()
                            && let Some(v) = sfu_video_ssrc
                        {
                            sfu_rtx_ssrc = Some(v.wrapping_add(1));
                        }
                        let ip = d
                            .and_then(|d| d.get("ip"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("")
                            .to_string();
                        let port = d
                            .and_then(|d| d.get("port"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(0) as u16;
                        println!(
                            "[VOICE-RTC] OP 2 READY ssrc={ssrc} sfu_video_ssrc={sfu_video_ssrc:?} sfu_rtx_ssrc={sfu_rtx_ssrc:?} media_udp={ip}:{port}"
                        );
                        let op16_req = build_voice_v8_op16();
                        let _ = ws.send(Message::Text(op16_req.into())).await;
                        if !is_stream_conn {
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            g.ssrc = ssrc;
                            if !session_desc_received {
                                g.state = "negotiating".to_string();
                            }
                        }
                        if let Some(stdin) = dv_stdin.as_mut() {
                            use std::io::Write as _;
                            let (smuted, svol) = get_live_stream_audio_state();
                            let (vmute, vdeaf) = get_live_voice_mute_deaf_state();
                            let sfu_ssrc_field = match (sfu_video_ssrc, sfu_rtx_ssrc) {
                                (Some(v), Some(r)) => format!(",\"videoSsrc\":{v},\"rtxSsrc\":{r}"),
                                (Some(v), None) => format!(",\"videoSsrc\":{v},\"rtxSsrc\":{}", v.wrapping_add(1)),
                                _ => String::new(),
                            };
                            let stream_uid_field = match stream_owner_id.as_deref() {
                                Some(su) => format!(
                                    ",\"context\":\"stream\",\"streamUserId\":\"{su}\",\"isSelfBroadcaster\":{is_self_broadcaster},\"streamMuted\":{smuted},\"streamVolume\":{svol}{sfu_ssrc_field}"
                                ),
                                None => format!(
                                    ",\"selfMute\":{vmute},\"selfDeafen\":{vdeaf}{sfu_ssrc_field}"
                                ),
                            };
                            let cmd = format!(
                                "{{\"cmd\":\"ready\",\"userId\":\"{user_id}\",\"channelId\":\"{cid}\",\"ssrc\":{ssrc},\"ip\":\"{ip}\",\"port\":{port}{stream_uid_field}}}\n"
                            );
                            let _ = stdin.write_all(cmd.as_bytes());
                            let _ = stdin.flush();
                        } else if !ip.is_empty() && port > 0 {
                            if let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                                let target = format!("{ip}:{port}");
                                if sock.connect(&target).await.is_ok() {
                                    let mut disc = [0u8; 74];
                                    disc[0..2].copy_from_slice(&1u16.to_be_bytes());
                                    disc[2..4].copy_from_slice(&70u16.to_be_bytes());
                                    disc[4..8].copy_from_slice(&ssrc.to_be_bytes());
                                    let send_t = Instant::now();
                                    let _ = udp_failures.send_packet(&sock, &disc).await;
                                    let mut recv_buf = [0u8; 128];
                                    let mut ext_ip = ip.clone();
                                    let mut ext_port = port;
                                    if let Ok(Ok(n)) = tokio::time::timeout(
                                        Duration::from_millis(1500),
                                        sock.recv(&mut recv_buf),
                                    )
                                    .await
                                    {
                                        let udp_rtt = u64::try_from(send_t.elapsed().as_millis()).unwrap_or(1).max(1);
                                        {
                                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                            g.last_ping_ms = Some(udp_rtt);
                                            if g.ping_history.len() >= 40 {
                                                g.ping_history.remove(0);
                                            }
                                            g.ping_history.push(udp_rtt);
                                            let sum: u64 = g.ping_history.iter().sum();
                                            g.average_ping_ms = Some(sum / (g.ping_history.len() as u64));
                                        }
                                        if n >= 74 {
                                            let ip_slice = &recv_buf[8..72];
                                            let end = ip_slice.iter().position(|&b| b == 0).unwrap_or(ip_slice.len());
                                            if let Ok(s) = std::str::from_utf8(&ip_slice[..end]) {
                                                if !s.is_empty() {
                                                    ext_ip = s.to_string();
                                                }
                                            }
                                            ext_port = u16::from_be_bytes([recv_buf[72], recv_buf[73]]);
                                        }
                                        println!("[VOICE-RTC] UDP IP Discovery OK: external={ext_ip}:{ext_port} rtt={udp_rtt}ms");
                                    } else {
                                        println!("[VOICE-RTC] UDP IP Discovery timeout after 1500ms to {target}");
                                    }
                                    let can_encode = !is_stream_conn || is_self_broadcaster;
                                    let select_proto = format!(
                                        "{{\"op\":1,\"d\":{{\"protocol\":\"udp\",\"address\":\"{ext_ip}\",\"port\":{ext_port},\"mode\":\"aead_aes256_gcm_rtpsize\",\"data\":{{\"address\":\"{ext_ip}\",\"port\":{ext_port},\"mode\":\"aead_aes256_gcm_rtpsize\"}},\"codecs\":[{{\"name\":\"opus\",\"type\":\"audio\",\"priority\":1000,\"payload_type\":120}},{{\"name\":\"H264\",\"type\":\"video\",\"priority\":1000,\"payload_type\":105,\"rtx_payload_type\":106,\"encode\":{can_encode},\"decode\":true}},{{\"name\":\"VP8\",\"type\":\"video\",\"priority\":2000,\"payload_type\":107,\"rtx_payload_type\":108,\"encode\":{can_encode},\"decode\":true}},{{\"name\":\"AV1\",\"type\":\"video\",\"priority\":3000,\"payload_type\":101,\"rtx_payload_type\":102,\"encode\":false,\"decode\":true}}]}}}}"
                                    );
                                    let _ = ws.send(Message::Text(select_proto.into())).await;
                                    udp_sock = Some(sock);
                                    {
                                        let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                        g.state = "connected".to_string();
                                    }
                                }
                            }
                        }
                    }
                    4 => {
                        let mode = d
                            .and_then(|d| d.get("mode"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("aead_aes256_gcm_rtpsize")
                            .to_string();
                        let video_codec = d
                            .and_then(|d| d.get("video_codec"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("unknown");
                        let audio_codec = d
                            .and_then(|d| d.get("audio_codec"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("opus");
                        let dave_ver = d
                            .and_then(|d| d.get("dave_protocol_version"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(0);
                        println!(
                            "[VOICE-RTC] OP 4 SESSION_DESCRIPTION mode={mode} audio_codec={audio_codec} video_codec={video_codec} dave_protocol_version={dave_ver}"
                        );
                        let mut secret_key_csv = String::new();
                        let mut key_bytes = [0x42u8; 32];
                        if let Some(arr) = d
                            .and_then(|d| d.get("secret_key"))
                            .and_then(chord_json::JsonValue::as_array)
                        {
                            for (i, v) in arr.iter().enumerate() {
                                if i > 0 {
                                    secret_key_csv.push(',');
                                }
                                let b = v.as_i64().unwrap_or(0).clamp(0, 255);
                                secret_key_csv.push_str(&b.to_string());
                            }
                            if arr.len() == 32 && dv_stdin.is_none() {
                                for (i, v) in arr.iter().enumerate() {
                                    key_bytes[i] = v.as_i64().unwrap_or(0) as u8;
                                }
                                let ctx = chord_voice::srtp::SrtpContext::from_secret_key_and_mode(
                                    &key_bytes,
                                    &mode,
                                )
                                .unwrap_or_else(|_| {
                                    chord_voice::srtp::SrtpContext::from_secret_key(
                                        &key_bytes,
                                        chord_voice::srtp::EncryptionMode::Aes256GcmRtpSize,
                                    )
                                });
                                srtp_ctx = Some(ctx);
                            }
                        }
                        session_desc_received = true;
                        if !video_codec.is_empty() && !video_codec.eq_ignore_ascii_case("unknown") {
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            let upper = video_codec.trim().to_ascii_uppercase();
                            g.active_video_codec =
                                if upper == "AV1" || upper == "AV1X" || upper.starts_with("AV1") {
                                    "AV1".to_string()
                                } else {
                                    upper
                                };
                        }
                        if !is_stream_conn {
                            let ssrc = {
                                let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                g.state = "connected".to_string();
                                g.encryption_mode = mode.clone();
                                g.ssrc
                            };
                            if ssrc > 0 {
                                local_ssrc = ssrc;
                            }
                        }
                        if is_stream_conn {
                            if is_self_broadcaster {
                                let v_ssrc = sfu_video_ssrc.unwrap_or_else(|| local_ssrc.wrapping_add(1));
                                let r_ssrc = sfu_rtx_ssrc.unwrap_or_else(|| v_ssrc.wrapping_add(1));
                                let op12_broadcaster =
                                    build_voice_v8_video_op12(local_ssrc, v_ssrc, r_ssrc, true);
                                println!(
                                    "[VOICE-RTC] WS TX OP 12 VIDEO (broadcaster): {op12_broadcaster}"
                                );
                                let _ = ws.send(Message::Text(op12_broadcaster.into())).await;
                                let spk_broadcaster = format!(
                                    "{{\"op\":5,\"d\":{{\"speaking\":2,\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                                );
                                let _ = ws.send(Message::Text(spk_broadcaster.into())).await;
                            } else {
                                let op12_viewer = build_voice_v8_video_op12(local_ssrc, 0, 0, false);
                                let _ = ws.send(Message::Text(op12_viewer.into())).await;
                            }
                        }
                        if (is_stream_conn && !is_self_broadcaster) || !known_video_ssrcs.is_empty()
                        {
                            let mut entries = vec!["\"any\":100".to_string()];
                            for vs in &known_video_ssrcs {
                                entries.push(format!("\"{vs}\":100"));
                            }
                            let wants = format!("{{\"op\":15,\"d\":{{{}}}}}", entries.join(","));
                            println!("[VOICE-RTC] WS TX OP 15 MEDIA_SINK_WANTS (on OP 4): {wants}");
                            let _ = ws.send(Message::Text(wants.into())).await;
                        }
                        if let Some(stdin) = dv_stdin.as_mut() {
                            use std::io::Write as _;
                            let (vmute, vdeaf) = get_live_voice_mute_deaf_state();
                            let cmd = format!(
                                "{{\"cmd\":\"session_description\",\"mode\":\"{mode}\",\"videoCodec\":\"{video_codec}\",\"secretKey\":[{secret_key_csv}],\"daveVersion\":{dave_ver},\"channelId\":\"{cid}\",\"selfMute\":{vmute},\"selfDeafen\":{vdeaf}}}\n"
                            );
                            let _ = stdin.write_all(cmd.as_bytes());
                            let _ = stdin.flush();
                        } else {
                            if dave_ver >= 1
                                && let Some(op26) = v8_state.on_op4_session_description(
                                    &mode,
                                    dave_ver,
                                    &key_bytes,
                                )
                            {
                                let _ = ws.send(Message::Binary(op26.into())).await;
                            }
                            let speaking_on = format!(
                                "{{\"op\":5,\"d\":{{\"speaking\":1,\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                            );
                            let _ = ws.send(Message::Text(speaking_on.into())).await;
                            if let (Some(sock), Some(srtp)) = (udp_sock.as_ref(), srtp_ctx.as_mut()) {
                                let mut ka = [0u8; 8];
                                ka[0..4].copy_from_slice(&0x1337_cafeu32.to_le_bytes());
                                ka[4..8].copy_from_slice(&keepalive_counter.to_le_bytes());
                                keepalive_counter = keepalive_counter.wrapping_add(1);
                                if let Err(e) = udp_failures.send_packet(sock, &ka).await {
                                    drop(dv_guard);
                                    return Err(e);
                                }
                                for _ in 0..5 {
                                    let mut rtp = Vec::with_capacity(15);
                                    rtp.push(0x80);
                                    rtp.push(0x78);
                                    rtp.extend_from_slice(&rtp_seq.to_be_bytes());
                                    rtp.extend_from_slice(&rtp_ts.to_be_bytes());
                                    rtp.extend_from_slice(&local_ssrc.to_be_bytes());
                                    rtp.extend_from_slice(&[0xF8, 0xFF, 0xFE]);
                                    rtp_seq = rtp_seq.wrapping_add(1);
                                    rtp_ts = rtp_ts.wrapping_add(960);
                                    if let Ok(enc) = srtp.encrypt_rtp(&rtp) {
                                        if let Err(e) = udp_failures.send_packet(sock, &enc).await {
                                            drop(dv_guard);
                                            return Err(e);
                                        }
                                    }
                                }
                            }
                            let speaking_off = format!(
                                "{{\"op\":5,\"d\":{{\"speaking\":0,\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                            );
                            let _ = ws.send(Message::Text(speaking_off.into())).await;
                        }
                    }
                    21 => {
                        let tid = d
                            .and_then(|d| d.get("transition_id"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(0);
                        let proto_ver = d
                            .and_then(|d| d.get("protocol_version"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(1);
                        println!("[VOICE-RTC] OP 21 DAVE prepare_transition tid={tid} ver={proto_ver}");
                        if let Some(stdin) = dv_stdin.as_mut() {
                            use std::io::Write as _;
                            let cmd = format!(
                                "{{\"cmd\":\"dave_prepare_transition\",\"transitionId\":{tid},\"protocolVersion\":{proto_ver}}}\n"
                            );
                            let _ = stdin.write_all(cmd.as_bytes());
                            let _ = stdin.flush();
                        } else if let Ok(frames) = v8_state.on_text_frame(&text) {
                            for frame in frames {
                                if let VoiceV8OutboundFrame::Text(ready) = frame {
                                    let _ = ws.send(Message::Text(ready.into())).await;
                                }
                            }
                        }
                    }
                    22 => {
                        let tid = d
                            .and_then(|d| d.get("transition_id"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(0);
                        println!("[VOICE-RTC] OP 22 DAVE execute_transition tid={tid}");
                        if let Some(stdin) = dv_stdin.as_mut() {
                            use std::io::Write as _;
                            let cmd = format!(
                                "{{\"cmd\":\"dave_execute_transition\",\"transitionId\":{tid}}}\n"
                            );
                            let _ = stdin.write_all(cmd.as_bytes());
                            let _ = stdin.flush();
                        } else {
                            let _ = v8_state.on_text_frame(&text);
                        }
                    }
                    24 => {
                        let epoch = d
                            .and_then(|d| d.get("epoch"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(1);
                        let proto_ver = d
                            .and_then(|d| d.get("protocol_version"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .unwrap_or(1);
                        println!("[VOICE-RTC] OP 24 DAVE prepare_epoch epoch={epoch} ver={proto_ver}");
                        if let Some(stdin) = dv_stdin.as_mut() {
                            use std::io::Write as _;
                            let cmd = format!(
                                "{{\"cmd\":\"dave_prepare_epoch\",\"epoch\":{epoch},\"protocolVersion\":{proto_ver},\"channelId\":\"{cid}\"}}\n"
                            );
                            let _ = stdin.write_all(cmd.as_bytes());
                            let _ = stdin.flush();
                        } else if let Ok(frames) = v8_state.on_text_frame(&text) {
                            for frame in frames {
                                if let VoiceV8OutboundFrame::Binary(op26) = frame {
                                    let _ = ws.send(Message::Binary(op26.into())).await;
                                }
                            }
                        }
                    }
                    15 => {
                        if is_self_broadcaster {
                            let v_ssrc = sfu_video_ssrc.unwrap_or_else(|| local_ssrc.wrapping_add(1));
                            let r_ssrc = sfu_rtx_ssrc.unwrap_or_else(|| v_ssrc.wrapping_add(1));
                            let op12_reapply =
                                build_voice_v8_video_op12(local_ssrc, v_ssrc, r_ssrc, true);
                            let _ = ws.send(Message::Text(op12_reapply.into())).await;
                            if let Some(stdin) = dv_stdin.as_mut() {
                                use std::io::Write as _;
                                let _ = stdin.write_all(b"{\"cmd\":\"reapply_broadcast\"}\n");
                                let _ = stdin.flush();
                            }
                        }
                    }
                    16 => {
                        let voice_ver = d
                            .and_then(|d| d.get("voice"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("");
                        let rtc_worker_ver = d
                            .and_then(|d| d.get("rtc_worker"))
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("");
                        println!(
                            "[VOICE-RTC] OP 16 VOICE_BACKEND_VERSION voice={voice_ver} rtc_worker={rtc_worker_ver}"
                        );
                        hb_counter = hb_counter.wrapping_add(1);
                        let nonce = d
                            .and_then(|d| d.get("t"))
                            .and_then(chord_json::JsonValue::as_i64)
                            .map(|t| t.unsigned_abs())
                            .unwrap_or(hb_counter);
                        last_hb_sent = Some((nonce, Instant::now()));
                        let hb = build_voice_v8_heartbeat_op3(nonce, seq_ack);
                        let _ = ws.send(Message::Text(hb.into())).await;
                    }
                    11 => {
                        if let Some(arr) = d
                            .and_then(|d| d.get("user_ids"))
                            .and_then(chord_json::JsonValue::as_array)
                        {
                            let mut uids_json = Vec::new();
                            for item in arr {
                                let uid_opt = item.as_str().map(|s| s.to_string())
                                    .or_else(|| item.as_i64().map(|n| n.to_string()))
                                    .or_else(|| item.as_u64().map(|n| n.to_string()));
                                if let Some(u) = uid_opt {
                                    uids_json.push(format!("\"{u}\""));
                                }
                            }
                            if !uids_json.is_empty() {
                                if let Some(stdin) = dv_stdin.as_mut() {
                                    use std::io::Write as _;
                                    let cmd = format!(
                                        "{{\"cmd\":\"client_connect\",\"userIds\":[{}]}}\n",
                                        uids_json.join(",")
                                    );
                                    let _ = stdin.write_all(cmd.as_bytes());
                                    if is_self_broadcaster {
                                        let _ = stdin.write_all(b"{\"cmd\":\"reapply_broadcast\"}\n");
                                    }
                                    let _ = stdin.flush();
                                } else {
                                    let _ = v8_state.on_text_frame(&text);
                                }
                            }
                        }
                    }
                    5 => {
                        let uid_opt = d
                            .and_then(|d| d.get("user_id"))
                            .and_then(|v| {
                                v.as_str().map(|s| s.to_string())
                                    .or_else(|| v.as_i64().map(|n| n.to_string()))
                                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                            });
                        if let Some(uid) = uid_opt {
                            let peer_ssrc = d
                                .and_then(|d| d.get("ssrc"))
                                .and_then(chord_json::JsonValue::as_i64)
                                .unwrap_or(0);
                            if peer_ssrc > 0 {
                                ssrc_to_user.insert(peer_ssrc as u32, uid.to_string());
                            }
                            let flags = d
                                .and_then(|d| d.get("speaking"))
                                .and_then(|v| {
                                    v.as_i64().or_else(|| v.as_bool().map(|b| if b { 1 } else { 0 }))
                                })
                                .unwrap_or(0);
                            if let Some(stdin) = dv_stdin.as_mut() {
                                use std::io::Write as _;
                                let cmd = format!(
                                    "{{\"cmd\":\"merge_user\",\"userId\":\"{uid}\",\"ssrc\":{peer_ssrc},\"videoSsrc\":-1,\"speaking\":{flags}}}\n"
                                );
                                let _ = stdin.write_all(cmd.as_bytes());
                                let _ = stdin.flush();
                            }
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            g.speaking_users.retain(|u| u != &uid);
                            if flags != 0 {
                                g.speaking_users.push(uid.to_string());
                                rtp_speaking_until
                                    .insert(uid.to_string(), Instant::now() + Duration::from_secs(8));
                            } else {
                                rtp_speaking_until.remove(&uid);
                            }
                        }
                    }
                    12 | 18 => {
                        let uid_opt = d
                            .and_then(|d| d.get("user_id"))
                            .and_then(|v| {
                                v.as_str().map(|s| s.to_string())
                                    .or_else(|| v.as_i64().map(|n| n.to_string()))
                                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                            });
                        if let Some(uid) = uid_opt {
                            let audio_ssrc = d
                                .and_then(|d| d.get("audio_ssrc").or_else(|| d.get("ssrc")))
                                .and_then(chord_json::JsonValue::as_i64)
                                .unwrap_or(0);
                            let mut video_ssrc = d
                                .and_then(|d| d.get("video_ssrc"))
                                .and_then(chord_json::JsonValue::as_i64)
                                .unwrap_or(-1);
                            if video_ssrc <= 0 {
                                if let Some(streams) = d
                                    .and_then(|d| d.get("streams"))
                                    .and_then(chord_json::JsonValue::as_array)
                                {
                                    for st in streams {
                                        if let Some(vs) = st
                                            .get("ssrc")
                                            .and_then(chord_json::JsonValue::as_i64)
                                        {
                                            if vs > 0 {
                                                video_ssrc = vs;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if audio_ssrc > 0 {
                                ssrc_to_user.insert(audio_ssrc as u32, uid.to_string());
                            }
                            if op == 12 || audio_ssrc > 0 || video_ssrc >= 0 {
                                if let Some(stdin) = dv_stdin.as_mut() {
                                    use std::io::Write as _;
                                    let cmd = format!(
                                        "{{\"cmd\":\"merge_user\",\"userId\":\"{uid}\",\"ssrc\":{audio_ssrc},\"videoSsrc\":{video_ssrc}}}\n"
                                    );
                                    let _ = stdin.write_all(cmd.as_bytes());
                                    let _ = stdin.flush();
                                }
                            }
                            if video_ssrc > 0 && uid != user_id {
                                if !known_video_ssrcs.contains(&video_ssrc) {
                                    known_video_ssrcs.push(video_ssrc);
                                }
                                let wants = format!(
                                    "{{\"op\":15,\"d\":{{\"any\":100,\"{video_ssrc}\":100}}}}"
                                );
                                println!("[VOICE-RTC] WS TX OP 15 MEDIA_SINK_WANTS: {wants}");
                                let _ = ws.send(Message::Text(wants.into())).await;
                            }
                        }
                    }
                    13 => {
                        let uid_opt = d
                            .and_then(|d| d.get("user_id"))
                            .and_then(|v| {
                                v.as_str().map(|s| s.to_string())
                                    .or_else(|| v.as_i64().map(|n| n.to_string()))
                                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                            });
                        if let Some(uid) = uid_opt {
                            if let Some(stdin) = dv_stdin.as_mut() {
                                use std::io::Write as _;
                                let cmd = format!(
                                    "{{\"cmd\":\"disconnect_user\",\"userId\":\"{uid}\"}}\n"
                                );
                                let _ = stdin.write_all(cmd.as_bytes());
                                let _ = stdin.flush();
                            } else {
                                let _ = v8_state.on_text_frame(&text);
                            }
                            rtp_speaking_until.remove(&uid);
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            g.speaking_users.retain(|u| u != &uid);
                        }
                    }
                    6 => {
                        if let Some((_, sent_at)) = last_hb_sent.take() {
                            let rtt = u64::try_from(sent_at.elapsed().as_millis()).unwrap_or(1).max(1);
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            g.last_ping_ms = Some(rtt);
                            g.ping_history.push(rtt);
                            if g.ping_history.len() > 40 {
                                g.ping_history.remove(0);
                            }
                            let sum: u64 = g.ping_history.iter().sum();
                            g.average_ping_ms = Some(sum / (g.ping_history.len() as u64));
                        }
                    }
                    _ => {}
                }
            }
            dv_line = dv_line_rx.recv() => {
                if let Some(line) = dv_line {
                    if let Ok(ev_json) = chord_json::parse(&line) {
                        let ev = ev_json
                            .get("ev")
                            .and_then(chord_json::JsonValue::as_str)
                            .unwrap_or("");
                        match ev {
                            "select_protocol" => {
                                let ext_ip = ev_json
                                    .get("address")
                                    .and_then(chord_json::JsonValue::as_str)
                                    .unwrap_or("127.0.0.1");
                                let ext_port = ev_json
                                    .get("port")
                                    .and_then(chord_json::JsonValue::as_i64)
                                    .unwrap_or(0);
                                let mode = ev_json
                                    .get("mode")
                                    .and_then(chord_json::JsonValue::as_str)
                                    .unwrap_or("aead_aes256_gcm_rtpsize");
                                println!(
                                    "[VOICE-RTC] Native chord_voice UDP IP Discovery OK: external={ext_ip}:{ext_port} mode={mode}"
                                );
                                let can_encode = !is_stream_conn || is_self_broadcaster;
                                let select_proto = format!(
                                    "{{\"op\":1,\"d\":{{\"protocol\":\"udp\",\"address\":\"{ext_ip}\",\"port\":{ext_port},\"mode\":\"{mode}\",\"data\":{{\"address\":\"{ext_ip}\",\"port\":{ext_port},\"mode\":\"{mode}\"}},\"codecs\":[{{\"name\":\"opus\",\"type\":\"audio\",\"priority\":1000,\"payload_type\":120}},{{\"name\":\"H264\",\"type\":\"video\",\"priority\":1000,\"payload_type\":105,\"rtx_payload_type\":106,\"encode\":{can_encode},\"decode\":true}},{{\"name\":\"VP8\",\"type\":\"video\",\"priority\":2000,\"payload_type\":107,\"rtx_payload_type\":108,\"encode\":{can_encode},\"decode\":true}},{{\"name\":\"AV1\",\"type\":\"video\",\"priority\":3000,\"payload_type\":101,\"rtx_payload_type\":102,\"encode\":false,\"decode\":true}}]}}}}"
                                );
                                let _ = ws.send(Message::Text(select_proto.into())).await;
                            }
                            "send_binary" => {
                                let op = ev_json
                                    .get("op")
                                    .and_then(chord_json::JsonValue::as_i64)
                                    .unwrap_or(0) as u8;
                                let hex = ev_json
                                    .get("hex")
                                    .and_then(chord_json::JsonValue::as_str)
                                    .unwrap_or("");
                                let payload = hex_to_bytes(hex);
                                if op > 0 && !payload.is_empty() {
                                    let mut bin_frame = Vec::with_capacity(1 + payload.len());
                                    bin_frame.push(op);
                                    bin_frame.extend_from_slice(&payload);
                                    println!(
                                        "[VOICE-RTC] WS TX BINARY op={op} payload_len={}",
                                        payload.len()
                                    );
                                    let _ = ws.send(Message::Binary(bin_frame.into())).await;
                                }
                            }
                            "send_json" => {
                                if let Some(payload) = ev_json.get("payload") {
                                    let s = if let Some(raw_s) = payload.as_str() {
                                        raw_s.to_string()
                                    } else {
                                        let j_op = payload
                                            .get("op")
                                            .and_then(chord_json::JsonValue::as_i64)
                                            .unwrap_or(23);
                                        let tid = payload
                                            .get("d")
                                            .and_then(|d| d.get("transition_id"))
                                            .and_then(chord_json::JsonValue::as_i64)
                                            .unwrap_or(0);
                                        format!("{{\"op\":{j_op},\"d\":{{\"transition_id\":{tid}}}}}")
                                    };
                                    println!("[VOICE-RTC] WS TX DAVE JSON: {s}");
                                    let _ = ws.send(Message::Text(s.into())).await;
                                }
                            }
                            "speaking" => {
                                let uid = ev_json
                                    .get("userId")
                                    .and_then(chord_json::JsonValue::as_str)
                                    .unwrap_or("");
                                let flags = ev_json
                                    .get("speaking")
                                    .and_then(chord_json::JsonValue::as_i64)
                                    .unwrap_or(0);
                                let is_self = ev_json
                                    .get("isSelf")
                                    .and_then(chord_json::JsonValue::as_bool)
                                    .unwrap_or(false);
                                if is_self {
                                    let spk = format!(
                                        "{{\"op\":5,\"d\":{{\"speaking\":{flags},\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                                    );
                                    let _ = ws.send(Message::Text(spk.into())).await;
                                }
                                if !uid.is_empty() {
                                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                    g.speaking_users.retain(|u| u != uid);
                                    if flags != 0 {
                                        g.speaking_users.push(uid.to_string());
                                        rtp_speaking_until.insert(
                                            uid.to_string(),
                                            Instant::now() + Duration::from_secs(8),
                                        );
                                    } else {
                                        rtp_speaking_until.remove(uid);
                                    }
                                }
                            }
                            "ping" => {
                                if let Some(p) = ev_json
                                    .get("ping")
                                    .and_then(chord_json::JsonValue::as_i64)
                                {
                                    if p > 0 {
                                        let rtt = (p as u64).clamp(1, 999);
                                        let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                                        g.last_ping_ms = Some(rtt);
                                        g.ping_history.push(rtt);
                                        if g.ping_history.len() > 40 {
                                            g.ping_history.remove(0);
                                        }
                                        let sum: u64 = g.ping_history.iter().sum();
                                        g.average_ping_ms = Some(sum / (g.ping_history.len() as u64));
                                    }
                                }
                            }
                            "video_callback" => {
                                let is_self = ev_json
                                    .get("isSelf")
                                    .and_then(chord_json::JsonValue::as_bool)
                                    .unwrap_or(false);
                                let v_ssrc = ev_json
                                    .get("videoSsrc")
                                    .and_then(chord_json::JsonValue::as_i64)
                                    .unwrap_or(0) as u32;
                                let r_ssrc = ev_json
                                    .get("rtxSsrc")
                                    .and_then(chord_json::JsonValue::as_i64)
                                    .unwrap_or(0) as u32;
                                if is_self && v_ssrc > 0 && (is_self_broadcaster || !is_stream_conn) {
                                    sfu_video_ssrc = Some(v_ssrc);
                                    let final_rtx = if r_ssrc > 0 {
                                        r_ssrc
                                    } else {
                                        sfu_rtx_ssrc.unwrap_or_else(|| v_ssrc.wrapping_add(1))
                                    };
                                    sfu_rtx_ssrc = Some(final_rtx);
                                    let op12 = build_voice_v8_video_op12(
                                        local_ssrc,
                                        v_ssrc,
                                        final_rtx,
                                        true,
                                    );
                                    println!(
                                        "[VOICE-RTC] WS TX OP 12 VIDEO (from native video_callback): {op12}"
                                    );
                                    let _ = ws.send(Message::Text(op12.into())).await;
                                }
                            }
                            "error" => {
                                println!("[VOICE-RTC] Native error event: {line}");
                                drop(dv_guard);
                                return Err("closed:4006".to_string());
                            }
                            "dave_state" | "mls_failure" | "log" => {
                                println!("[VOICE-RTC] Native event: {line}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            udp_res = async {
                match udp_sock.as_ref() {
                    Some(sock) => sock.recv(&mut udp_recv_buf).await,
                    None => std::future::pending().await,
                }
            } => {
                if let Ok(n) = udp_res {
                    if n >= 12 && (udp_recv_buf[0] >> 6) == 2 {
                        let pt = udp_recv_buf[1] & 0x7f;
                        if pt == 120 || pt == 111 {
                            rx_rtp_count = rx_rtp_count.wrapping_add(1);
                            let pkt_ssrc = u32::from_be_bytes([
                                udp_recv_buf[8],
                                udp_recv_buf[9],
                                udp_recv_buf[10],
                                udp_recv_buf[11],
                            ]);
                            if let Some(srtp) = srtp_ctx.as_mut() {
                                match srtp.decrypt_discord_rtp_opus(&udp_recv_buf[..n]) {
                                    Ok(opus_frame) => {
                                        rx_decrypted_count = rx_decrypted_count.wrapping_add(1);
                                        if rx_decrypted_count == 1 || rx_decrypted_count % 100 == 0 {
                                            println!(
                                                "[VOICE-RTC] RX RTP decrypted ok (#{rx_decrypted_count}/{rx_rtp_count}) ssrc={pkt_ssrc} opus_len={}",
                                                opus_frame.len()
                                            );
                                        }
                                        if !opus_frame.is_empty() && opus_frame.as_slice() != [0xF8, 0xFF, 0xFE] {
                                            if let Ok(mut rx_q) = live_rtc_rx_opus_mutex().lock() {
                                                while rx_q.len() >= 150 {
                                                    rx_q.pop_front();
                                                }
                                                rx_q.push_back((pkt_ssrc, opus_frame));
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        if rx_rtp_count <= 5 || rx_rtp_count % 100 == 0 {
                                            println!(
                                                "[VOICE-RTC] RX RTP decrypt err (#{rx_rtp_count}) ssrc={pkt_ssrc} pkt_len={n} b0=0x{:02x}: {err}",
                                                udp_recv_buf[0]
                                            );
                                        }
                                    }
                                }
                            }
                            let speaker_key = ssrc_to_user
                                .get(&pkt_ssrc)
                                .cloned()
                                .unwrap_or_else(|| format!("ssrc:{pkt_ssrc}"));
                            rtp_speaking_until.insert(
                                speaker_key.clone(),
                                Instant::now() + Duration::from_millis(450),
                            );
                            let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                            if !g.speaking_users.iter().any(|u| u == &speaker_key) {
                                g.speaking_users.push(speaker_key);
                            }
                        }
                    }
                }
            }
            () = tokio::time::sleep_until(tx_audio_tick) => {
                if let Some(stdin) = dv_stdin.as_mut() {
                    let mut cmds: Vec<String> = Vec::new();
                    if is_stream_conn {
                        if let Ok(mut q) = native_stream_cmd_mutex().lock() {
                            cmds.extend(q.drain(..));
                        }
                    } else {
                        if let Ok(mut q) = native_voice_cmd_mutex().lock() {
                            cmds.extend(q.drain(..));
                        }
                        if let Ok(mut q) = native_stream_cmd_mutex().lock() {
                            cmds.extend(q.drain(..));
                        }
                    }
                    if !cmds.is_empty() {
                        use std::io::Write as _;
                        for cmd in cmds {
                            let line = format!("{cmd}\n");
                            let _ = stdin.write_all(line.as_bytes());
                        }
                        let _ = stdin.flush();
                    }
                }
                let tx_frames: Vec<Vec<u8>> = if !is_stream_conn && !has_native_voice {
                    if let Ok(mut q) = live_rtc_tx_opus_mutex().lock() {
                        q.drain(..).collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                if !tx_frames.is_empty() {
                    if let (Some(sock), Some(srtp)) = (udp_sock.as_ref(), srtp_ctx.as_mut()) {
                        if !local_tx_speaking {
                            local_tx_speaking = true;
                            let spk = format!(
                                "{{\"op\":5,\"d\":{{\"speaking\":1,\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                            );
                            let _ = ws.send(Message::Text(spk.into())).await;
                        }
                        last_local_tx_at = Instant::now();
                        for opus in tx_frames {
                            let mut rtp = Vec::with_capacity(12 + opus.len());
                            rtp.push(0x80);
                            rtp.push(0x78);
                            rtp.extend_from_slice(&rtp_seq.to_be_bytes());
                            rtp.extend_from_slice(&rtp_ts.to_be_bytes());
                            rtp.extend_from_slice(&local_ssrc.to_be_bytes());
                            rtp.extend_from_slice(&opus);
                            rtp_seq = rtp_seq.wrapping_add(1);
                            rtp_ts = rtp_ts.wrapping_add(960);
                            if let Ok(enc) = srtp.encrypt_rtp(&rtp) {
                                if let Err(e) = udp_failures.send_packet(sock, &enc).await {
                                    drop(dv_guard);
                                    return Err(e);
                                }
                            }
                        }
                    }
                } else if local_tx_speaking && last_local_tx_at.elapsed() > Duration::from_millis(380) {
                    local_tx_speaking = false;
                    if let (Some(sock), Some(srtp)) = (udp_sock.as_ref(), srtp_ctx.as_mut()) {
                        for _ in 0..5 {
                            let mut rtp = Vec::with_capacity(15);
                            rtp.push(0x80);
                            rtp.push(0x78);
                            rtp.extend_from_slice(&rtp_seq.to_be_bytes());
                            rtp.extend_from_slice(&rtp_ts.to_be_bytes());
                            rtp.extend_from_slice(&local_ssrc.to_be_bytes());
                            rtp.extend_from_slice(&[0xF8, 0xFF, 0xFE]);
                            rtp_seq = rtp_seq.wrapping_add(1);
                            rtp_ts = rtp_ts.wrapping_add(960);
                            if let Ok(enc) = srtp.encrypt_rtp(&rtp) {
                                if let Err(e) = udp_failures.send_packet(sock, &enc).await {
                                    drop(dv_guard);
                                    return Err(e);
                                }
                            }
                        }
                    }
                    let spk = format!(
                        "{{\"op\":5,\"d\":{{\"speaking\":0,\"delay\":0,\"ssrc\":{local_ssrc}}}}}"
                    );
                    let _ = ws.send(Message::Text(spk.into())).await;
                }
                let now = Instant::now();
                if rtp_speaking_until.values().any(|until| *until <= now) {
                    rtp_speaking_until.retain(|_, until| *until > now);
                    let mut g = live_rtc_mutex().lock().unwrap_or_else(|p| p.into_inner());
                    g.speaking_users.retain(|u| rtp_speaking_until.contains_key(u));
                }
                tx_audio_tick = Instant::now() + Duration::from_millis(15);
            }
            () = tokio::time::sleep_until(next_hb) => {
                let now = Instant::now();
                rtp_speaking_until.retain(|_, until| *until > now);
                let nonce = 1_700_000_000_000u64 + hb_counter;
                hb_counter += 1;
                let hb_json = build_voice_v8_heartbeat_op3(nonce, seq_ack);
                last_hb_sent = Some((nonce, Instant::now()));
                if ws.send(Message::Text(hb_json.into())).await.is_err() {
                    break;
                }
                next_hb = Instant::now() + Duration::from_millis(hb_interval_ms);
            }
            () = tokio::time::sleep_until(udp_keepalive_tick) => {
                if let Some(ref sock) = udp_sock {
                    let mut pkt = [0u8; 8];
                    pkt[0..4].copy_from_slice(&0x1337_cafeu32.to_le_bytes());
                    pkt[4..8].copy_from_slice(&keepalive_counter.to_le_bytes());
                    keepalive_counter = keepalive_counter.wrapping_add(1);
                    if let Err(e) = udp_failures.send_packet(sock, &pkt).await {
                        drop(dv_guard);
                        return Err(e);
                    }
                }
                if session_desc_received
                    && ((is_stream_conn && !is_self_broadcaster) || !known_video_ssrcs.is_empty())
                {
                    let mut entries = vec!["\"any\":100".to_string()];
                    for vs in &known_video_ssrcs {
                        entries.push(format!("\"{vs}\":100"));
                    }
                    let wants = format!("{{\"op\":15,\"d\":{{{}}}}}", entries.join(","));
                    let _ = ws.send(Message::Text(wants.into())).await;
                }
                udp_keepalive_tick = Instant::now() + Duration::from_secs(5);
            }
        }
    }
    drop(dv_guard);
    Ok(())
}

/// Выполняет эффекты сессии; `Reconnect` прерывает цикл (исход), `SendHeartbeat` пишет кадр.
///
/// Форма удара — по режиму сессии: боевой op40 `{seq,qos}` либо legacy op1 (fallback).
async fn apply_effects<S>(
    effects: Vec<Effect>,
    ws: &mut WebSocketStream<S>,
    session: &Session,
    is_live_wss: bool,
) -> Result<Option<RunOutcome>, TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    for effect in effects {
        match effect {
            Effect::SendHeartbeat { d } => {
                let frame = if is_live_wss {
                    let d_term = match d {
                        Some(seq) => EtfValue::Int(i64::try_from(seq).unwrap_or(i64::MAX)),
                        None => EtfValue::Atom("nil".to_string()),
                    };
                    etf::encode(&EtfValue::Map(vec![
                        (EtfValue::Str("op".to_string()), EtfValue::Int(1)),
                        (EtfValue::Str("d".to_string()), d_term),
                    ]))
                } else {
                    match session.heartbeat_mode() {
                        HeartbeatMode::Qos => encode_qos_heartbeat(d.unwrap_or(0), session.qos()),
                        HeartbeatMode::LegacyOp1 => encode_heartbeat(d),
                    }
                };
                ws.send(Message::binary(frame))
                    .await
                    .map_err(|error| TransportError::Ws {
                        msg: error.to_string(),
                    })?;
            }
            Effect::Reconnect { reason } => return Ok(Some(RunOutcome::Reconnect { reason })),
        }
    }
    Ok(None)
}

/// Монотонные миллисекунды от старта прогона; насыщение вместо паники (`u128` → `u64`).
fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Код закрытия из close-кадра (`CloseFrame::code` → `u16`), если кадр его нёс.
fn close_code(frame: Option<CloseFrame>) -> Option<u16> {
    frame.map(|frame| u16::from(frame.code))
}

/// Разово ставит ring-провайдер rustls глобальным по умолчанию.
///
/// Свой TLS-коннектор собирается с **явным** ring-провайдером ([`tls_connector`]), поэтому
/// глобальный дефолт нужен лишь сторонним потребителям процесса. Повторный вызов (гонка с
/// другим кодом, «провайдер уже установлен») осознанно не фатален: ошибку игнорируем, паники нет.
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// TLS-коннектор для `wss://`: ring-провайдер + корни `webpki-roots`.
///
/// Провайдер задан явно (`builder_with_provider`), поэтому TLS не зависит от глобального дефолта
/// процесса (требование ТЗ: ring, не aws-lc-rs). Ошибка сборки конфига (недостижима для ring,
/// но без паники) → `None`: тогда `tokio-tungstenite` собирает TLS-конфиг сам (его штатный путь).
fn tls_connector() -> Option<Connector> {
    ensure_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    match rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
    {
        Ok(builder) => Some(Connector::Rustls(Arc::new(
            builder.with_root_certificates(roots).with_no_client_auth(),
        ))),
        Err(error) => {
            eprintln!(
                "wss: ring-провайдер не принял дефолтные версии протокола ({error}); \
                 используется коннектор tokio-tungstenite по умолчанию"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GatewayPipe, GuildSubscriptionsBulk, OutboundGuildSubscription, PresenceUpdate,
        RequestGuildMembers, RunOutcome, TransportError, VoiceStateUpdateOut, connect_and_run,
        send_guild_subscriptions_bulk, send_presence_update, send_request_guild_members,
        send_voice_state_update,
    };
    use crate::gateway::{HeartbeatMode, Session};
    use crate::state::PresenceStatus;
    use chord_codec::etf::{self, EtfValue};
    use futures_util::{SinkExt, StreamExt};
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::time::Duration;
    use tokio::io::DuplexStream;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::protocol::Message;
    use tokio_tungstenite::{WebSocketStream, accept_async, client_async};

    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    /// Пара клиентского и серверного WebSocket-потоков поверх in-memory `tokio::io::duplex`.
    async fn ws_pair() -> (WebSocketStream<DuplexStream>, WebSocketStream<DuplexStream>) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let client_fut = client_async("ws://localhost/", client_io);
        let server_fut = accept_async(server_io);
        let (client_res, server_res) = tokio::join!(client_fut, server_fut);
        let (client_ws, _) = client_res.expect("client WS handshake");
        let server_ws = server_res.expect("server WS handshake");
        (client_ws, server_ws)
    }

    /// Читает один бинарный WS-кадр на серверной стороне и разбирает его как единственный ETF-терм.
    async fn recv_binary_frame(
        server_ws: &mut WebSocketStream<DuplexStream>,
    ) -> (Vec<u8>, EtfValue) {
        let msg = timeout(TEST_TIMEOUT, server_ws.next())
            .await
            .expect("тайм-аут чтения WS-кадра")
            .expect("WS-поток неожиданно закрыт")
            .expect("ошибка чтения WS-кадра");
        match msg {
            Message::Binary(bytes) => {
                let raw = bytes.to_vec();
                assert_eq!(raw.first(), Some(&0x83), "ведущий байт версии ETF 0x83");
                let (value, consumed) =
                    etf::decode(&raw).expect("бинарный кадр декодируется как ETF");
                assert_eq!(consumed, raw.len(), "ровно один ETF-терм без хвоста");
                (raw, value)
            }
            other => panic!("ожидался бинарный WS-кадр, получено: {other:?}"),
        }
    }

    /// Имена ключей map-терма; все ключи клиентских кадров обязаны быть binary-строками (`key:"binary"`).
    fn map_keys(value: &EtfValue) -> Vec<String> {
        value
            .as_map()
            .expect("ожидался ETF map")
            .iter()
            .map(|(k, _)| match k {
                EtfValue::Str(name) => name.clone(),
                other => panic!("ключ map не binary-строка: {other:?}"),
            })
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[tokio::test]
    async fn send_presence_update_sends_binary_etf_op3_frame() {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut pipe = GatewayPipe::new(Session::new(0));

        let online = PresenceUpdate {
            status: PresenceStatus::Online,
            since: 0,
            activities: Vec::new(),
            afk: false,
        };
        pipe.send_presence_update(&mut client_ws, &online)
            .await
            .expect("op3 отправляется через GatewayPipe");

        let (raw, decoded) = recv_binary_frame(&mut server_ws).await;
        // Независимый байтовый оракул wetf-раскладки клиента для op3:
        let expected_hex = concat!(
            "83",
            "7400000002",
            "6d000000026f70",
            "6103",
            "6d0000000164",
            "7400000004",
            "6d00000006737461747573",
            "6d000000066f6e6c696e65",
            "6d0000000573696e6365",
            "6100",
            "6d0000000a61637469766974696573",
            "6c000000006a",
            "6d0000000361666b",
            "64000566616c7365",
        );
        assert_eq!(to_hex(&raw), expected_hex);
        assert_eq!(decoded.get("op").and_then(EtfValue::as_int), Some(3));
        assert_eq!(pipe.frames_sent(), 1);
        assert_eq!(pipe.bytes_sent(), raw.len() as u64);

        // Вторая отправка через свободную функцию `ws::send_presence_update` с активностью и AFK:
        let custom_activity = EtfValue::Map(vec![
            (EtfValue::Str("type".to_string()), EtfValue::Int(4)),
            (
                EtfValue::Str("name".to_string()),
                EtfValue::Str("Custom Status".to_string()),
            ),
        ]);
        let dnd = PresenceUpdate {
            status: PresenceStatus::Dnd,
            since: 1_700_000_123,
            activities: vec![custom_activity.clone()],
            afk: true,
        };
        send_presence_update(&mut client_ws, &dnd)
            .await
            .expect("op3 отправляется через функцию ws::send_presence_update");

        let (_, decoded_dnd) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(map_keys(&decoded_dnd), ["op", "d"]);
        assert_eq!(decoded_dnd.get("op").and_then(EtfValue::as_int), Some(3));
        let d = decoded_dnd.get("d").expect("d в op3");
        assert_eq!(map_keys(d), ["status", "since", "activities", "afk"]);
        assert_eq!(d.get("status").and_then(EtfValue::as_str), Some("dnd"));
        assert_eq!(
            d.get("since").and_then(EtfValue::as_int),
            Some(1_700_000_123)
        );
        assert_eq!(
            d.get("activities").and_then(EtfValue::as_list),
            Some([custom_activity].as_slice())
        );
        assert_eq!(d.get("afk").and_then(EtfValue::as_atom), Some("true"));
    }

    #[tokio::test]
    async fn send_voice_state_update_sends_binary_etf_op4_join_and_leave() {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut pipe = GatewayPipe::new(Session::new(0));

        let join = VoiceStateUpdateOut {
            guild_id: Some("1014537851418980372".to_string()),
            channel_id: Some("1176785373989445703".to_string()),
            self_mute: true,
            self_deaf: false,
            self_video: true,
            flags: 2,
        };
        pipe.send_voice_state_update(&mut client_ws, &join)
            .await
            .expect("op4 join отправляется через GatewayPipe");

        let (raw_join, decoded_join) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(decoded_join.get("op").and_then(EtfValue::as_int), Some(4));
        let d = decoded_join.get("d").expect("d в op4 join");
        assert_eq!(
            map_keys(d),
            [
                "guild_id",
                "channel_id",
                "self_mute",
                "self_deaf",
                "self_video",
                "flags",
            ]
        );
        assert_eq!(
            d.get("guild_id").and_then(EtfValue::as_str),
            Some("1014537851418980372")
        );
        assert_eq!(
            d.get("channel_id").and_then(EtfValue::as_str),
            Some("1176785373989445703")
        );
        assert_eq!(d.get("self_mute").and_then(EtfValue::as_atom), Some("true"));
        assert_eq!(
            d.get("self_deaf").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            d.get("self_video").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(d.get("flags").and_then(EtfValue::as_int), Some(2));
        assert_eq!(pipe.frames_sent(), 1);
        assert_eq!(pipe.bytes_sent(), raw_join.len() as u64);

        // Выход из голосового канала: guild_id и channel_id кодируются как NIL_EXT (null):
        let leave = VoiceStateUpdateOut {
            guild_id: None,
            channel_id: None,
            self_mute: false,
            self_deaf: true,
            self_video: false,
            flags: 0,
        };
        send_voice_state_update(&mut client_ws, &leave)
            .await
            .expect("op4 leave отправляется через ws::send_voice_state_update");

        let (_, decoded_leave) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(decoded_leave.get("op").and_then(EtfValue::as_int), Some(4));
        let d_leave = decoded_leave.get("d").expect("d в op4 leave");
        assert_eq!(d_leave.get("guild_id"), Some(&EtfValue::Nil));
        assert_eq!(d_leave.get("channel_id"), Some(&EtfValue::Nil));
        assert_eq!(
            d_leave.get("self_mute").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            d_leave.get("self_deaf").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            d_leave.get("self_video").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(d_leave.get("flags").and_then(EtfValue::as_int), Some(0));
    }

    #[tokio::test]
    async fn send_request_guild_members_chunks_user_ids_by_100_and_supports_query() {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut pipe = GatewayPipe::new(Session::new(0));

        // 1. Пустой запрос без user_ids и без query — 0 кадров:
        let empty_req = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: None,
            limit: None,
            user_ids: Vec::new(),
            presences: true,
            nonce: None,
        };
        let sent_empty = pipe
            .send_request_guild_members(&mut client_ws, &empty_req)
            .await
            .expect("пустой op8 не падает");
        assert_eq!(sent_empty, 0, "пустой user_ids без query не шлёт кадров");
        assert_eq!(pipe.frames_sent(), 0);

        // 2. Граница 100 user_ids — ровно 1 кадр:
        let ids_100: Vec<String> = (0..100).map(|i| format!("user-{i:03}")).collect();
        let req_100 = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: None,
            limit: None,
            user_ids: ids_100.clone(),
            presences: false,
            nonce: Some("n-100".to_string()),
        };
        let sent_100 = pipe
            .send_request_guild_members(&mut client_ws, &req_100)
            .await
            .expect("op8 для 100 user_ids");
        assert_eq!(sent_100, 1);
        assert_eq!(pipe.frames_sent(), 1);
        let (_, frame_100) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(frame_100.get("op").and_then(EtfValue::as_int), Some(8));
        let d_100 = frame_100.get("d").expect("d в op8 (100)");
        assert_eq!(
            d_100
                .get("user_ids")
                .and_then(EtfValue::as_list)
                .map(|l| l.len()),
            Some(100)
        );

        // 3. Граница 101 user_ids — 2 чанка (100 и 1):
        let ids_101: Vec<String> = (0..101).map(|i| format!("member-{i:03}")).collect();
        let req_101 = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: None,
            limit: None,
            user_ids: ids_101.clone(),
            presences: true,
            nonce: Some("chunk-test".to_string()),
        };
        let sent_101 = pipe
            .send_request_guild_members(&mut client_ws, &req_101)
            .await
            .expect("op8 для 101 user_ids");
        assert_eq!(sent_101, 2, "101 user_ids режется на 2 кадра (100 + 1)");
        assert_eq!(pipe.frames_sent(), 3);

        let (_, chunk_1) = recv_binary_frame(&mut server_ws).await;
        let (_, chunk_2) = recv_binary_frame(&mut server_ws).await;
        for chunk in [&chunk_1, &chunk_2] {
            assert_eq!(chunk.get("op").and_then(EtfValue::as_int), Some(8));
            let d = chunk.get("d").expect("d в чанке op8");
            assert_eq!(
                map_keys(d),
                [
                    "guild_id",
                    "query",
                    "limit",
                    "user_ids",
                    "presences",
                    "nonce",
                ]
            );
            assert_eq!(
                d.get("guild_id").and_then(EtfValue::as_str),
                Some("1014537851418980372")
            );
            assert_eq!(d.get("query"), Some(&EtfValue::Nil));
            assert_eq!(d.get("limit"), Some(&EtfValue::Nil));
            assert_eq!(d.get("presences").and_then(EtfValue::as_atom), Some("true"));
            assert_eq!(
                d.get("nonce").and_then(EtfValue::as_str),
                Some("chunk-test")
            );
        }
        let list_1: Vec<&str> = chunk_1
            .get("d")
            .and_then(|d| d.get("user_ids"))
            .and_then(EtfValue::as_list)
            .expect("user_ids первого чанка")
            .iter()
            .map(|v| v.as_str().expect("строка user_id"))
            .collect();
        let list_2: Vec<&str> = chunk_2
            .get("d")
            .and_then(|d| d.get("user_ids"))
            .and_then(EtfValue::as_list)
            .expect("user_ids второго чанка")
            .iter()
            .map(|v| v.as_str().expect("строка user_id"))
            .collect();
        assert_eq!(list_1.len(), 100);
        assert_eq!(list_1.first().copied(), Some("member-000"));
        assert_eq!(list_1.last().copied(), Some("member-099"));
        assert_eq!(list_2, ["member-100"]);

        // 4. 205 user_ids через свободную функцию `ws::send_request_guild_members` — 3 чанка (100 + 100 + 5):
        let ids_205: Vec<String> = (0..205).map(|i| format!("u-{i}")).collect();
        let req_205 = RequestGuildMembers {
            guild_id: "853389359330148352".to_string(),
            query: None,
            limit: None,
            user_ids: ids_205,
            presences: false,
            nonce: None,
        };
        let sent_205 = send_request_guild_members(&mut client_ws, &req_205)
            .await
            .expect("op8 для 205 user_ids");
        assert_eq!(sent_205, 3);
        let mut chunk_lengths = Vec::new();
        for _ in 0..3 {
            let (_, frame) = recv_binary_frame(&mut server_ws).await;
            let len = frame
                .get("d")
                .and_then(|d| d.get("user_ids"))
                .and_then(EtfValue::as_list)
                .expect("user_ids чанка")
                .len();
            chunk_lengths.push(len);
        }
        assert_eq!(chunk_lengths, [100, 100, 5]);

        // 5. Поиск по префиксу (`query` + `limit`, пустой `user_ids`) — 1 кадр:
        let query_req = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: Some("monolit".to_string()),
            limit: Some(0),
            user_ids: Vec::new(),
            presences: true,
            nonce: None,
        };
        let sent_query = pipe
            .send_request_guild_members(&mut client_ws, &query_req)
            .await
            .expect("op8 query-путь");
        assert_eq!(sent_query, 1);
        let (_, query_frame) = recv_binary_frame(&mut server_ws).await;
        let d_query = query_frame.get("d").expect("d в op8 query");
        assert_eq!(
            d_query.get("query").and_then(EtfValue::as_str),
            Some("monolit")
        );
        assert_eq!(d_query.get("limit").and_then(EtfValue::as_int), Some(0));
        assert_eq!(d_query.get("user_ids"), Some(&EtfValue::Nil));
    }

    #[tokio::test]
    async fn send_guild_subscriptions_bulk_sends_binary_etf_op37_frame() {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut pipe = GatewayPipe::new(Session::new(0));

        let mut subscriptions: GuildSubscriptionsBulk = BTreeMap::new();
        subscriptions.insert(
            "1014537851418980372".to_string(),
            OutboundGuildSubscription {
                typing: true,
                threads: false,
                activities: true,
                members: vec!["384299970632876032".to_string()],
                member_updates: false,
                channels: vec![
                    "1176785373989445703".to_string(),
                    "1275727867497680918".to_string(),
                ],
            },
        );
        subscriptions.insert(
            "853389359330148352".to_string(),
            OutboundGuildSubscription {
                typing: false,
                threads: true,
                activities: false,
                members: Vec::new(),
                member_updates: true,
                channels: vec!["853389359330148355".to_string()],
            },
        );

        pipe.send_guild_subscriptions_bulk(&mut client_ws, &subscriptions)
            .await
            .expect("op37 отправляется через GatewayPipe");

        let (raw, decoded) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(decoded.get("op").and_then(EtfValue::as_int), Some(37));
        assert_eq!(pipe.frames_sent(), 1);
        assert_eq!(pipe.bytes_sent(), raw.len() as u64);

        let d = decoded.get("d").expect("d в op37");
        assert_eq!(map_keys(d), ["subscriptions"]);
        let guilds = d.get("subscriptions").expect("subscriptions map");
        assert_eq!(
            map_keys(guilds),
            ["1014537851418980372", "853389359330148352"]
        );

        let sub_a = guilds.get("1014537851418980372").expect("первая гильдия");
        assert_eq!(
            map_keys(sub_a),
            [
                "typing",
                "threads",
                "activities",
                "members",
                "member_updates",
                "channels",
            ]
        );
        assert_eq!(
            sub_a.get("typing").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            sub_a.get("threads").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            sub_a.get("activities").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            sub_a.get("member_updates").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            sub_a.get("members").and_then(EtfValue::as_list),
            Some([EtfValue::Str("384299970632876032".to_string())].as_slice())
        );
        assert_eq!(
            sub_a.get("channels").and_then(EtfValue::as_list),
            Some(
                [
                    EtfValue::Str("1176785373989445703".to_string()),
                    EtfValue::Str("1275727867497680918".to_string()),
                ]
                .as_slice()
            )
        );

        // Проверка свободной функции `ws::send_guild_subscriptions_bulk`:
        send_guild_subscriptions_bulk(&mut client_ws, &subscriptions)
            .await
            .expect("op37 отправляется через ws::send_guild_subscriptions_bulk");
        let (_, decoded_second) = recv_binary_frame(&mut server_ws).await;
        assert_eq!(
            decoded_second.get("op").and_then(EtfValue::as_int),
            Some(37)
        );
    }

    #[tokio::test]
    async fn outbound_guards_dead_session_and_flushes_queued_frames_after_hello() {
        // 1. Мёртвая сессия (после op7 RECONNECT) отвергает исходящие кадры без записи в сокет:
        let (mut client_ws, server_ws) = ws_pair().await;
        let mut dead_session = Session::new(0);
        let op7 = EtfValue::Map(vec![(EtfValue::Atom("op".to_string()), EtfValue::Int(7))]);
        dead_session.on_value(&op7, 0).expect("op7 обрабатывается");
        assert!(dead_session.is_dead());
        let mut dead_pipe = GatewayPipe::new(dead_session);

        let presence = PresenceUpdate {
            status: PresenceStatus::Idle,
            since: 100,
            activities: Vec::new(),
            afk: true,
        };
        assert!(matches!(
            dead_pipe
                .send_presence_update(&mut client_ws, &presence)
                .await,
            Err(TransportError::Ws { .. })
        ));
        assert_eq!(dead_pipe.frames_sent(), 0);

        // Закрытый сокет возвращает ошибку записи TransportError::Ws вызывающему:
        drop(server_ws);
        let mut live_pipe = GatewayPipe::new(Session::new(0));
        assert!(matches!(
            live_pipe
                .send_presence_update(&mut client_ws, &presence)
                .await,
            Err(TransportError::Ws { .. })
        ));

        // 2. Очередь пайпа в `connect_and_run`: ни один кадр не уходит до HELLO,
        // а после HELLO отправляются все поставленные в очередь кадры (op3, op4, 2×op8, op37):
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind stub");
        let addr = listener.local_addr().expect("local_addr");
        let url = format!("ws://{addr}");

        let server_task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(tcp).await.expect("accept_async");

            // До отправки HELLO проверяем тишину от клиента (50 мс):
            let premature = timeout(Duration::from_millis(50), ws.next()).await;
            assert!(
                premature.is_err(),
                "до HELLO ни один исходящий кадр не должен уходить: {premature:?}"
            );

            // Шлём zstd-сжатый HELLO с большим интервалом heartbeat (60 000 мс), чтобы плановый
            // удар не смешался с очередью исходящих кадров:
            let hello = EtfValue::Map(vec![
                (EtfValue::Atom("op".to_string()), EtfValue::Int(10)),
                (
                    EtfValue::Atom("d".to_string()),
                    EtfValue::Map(vec![(
                        EtfValue::Atom("heartbeat_interval".to_string()),
                        EtfValue::Int(60_000),
                    )]),
                ),
            ]);
            let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd");
            encoder
                .write_all(&etf::encode(&hello))
                .expect("write hello");
            encoder.flush().expect("flush hello");
            let hello_compressed = encoder.get_ref().clone();
            ws.send(Message::binary(hello_compressed))
                .await
                .expect("send hello");

            // Читаем ровно 5 ожидаемых бинарных кадров: op3, op4, op8 (чанк 100), op8 (чанк 1), op37:
            let mut ops = Vec::new();
            for _ in 0..5 {
                let msg = timeout(TEST_TIMEOUT, ws.next())
                    .await
                    .expect("тайм-аут чтения кадра")
                    .expect("поток закрылся раньше времени")
                    .expect("ошибка чтения WS");
                match msg {
                    Message::Binary(data) => {
                        let (val, consumed) = etf::decode(&data).expect("ETF-кадр клиента");
                        assert_eq!(consumed, data.len());
                        ops.push(val.get("op").and_then(EtfValue::as_int).expect("op"));
                    }
                    other => panic!("неожиданное WS-сообщение от клиента: {other:?}"),
                }
            }
            ws.close(None).await.expect("close stub");
            ops
        });

        let mut pipe = GatewayPipe::new(Session::with_heartbeat_mode(50_000, HeartbeatMode::Qos));
        pipe.queue_presence_update(&presence).expect("queue op3");
        pipe.queue_voice_state_update(&VoiceStateUpdateOut {
            guild_id: Some("1014537851418980372".to_string()),
            channel_id: Some("1176785373989445703".to_string()),
            self_mute: false,
            self_deaf: false,
            self_video: false,
            flags: 0,
        })
        .expect("queue op4");
        let queued_chunks = pipe
            .queue_request_guild_members(&RequestGuildMembers {
                guild_id: "1014537851418980372".to_string(),
                query: None,
                limit: None,
                user_ids: (0..101).map(|i| i.to_string()).collect(),
                presences: true,
                nonce: None,
            })
            .expect("queue op8");
        assert_eq!(queued_chunks, 2);
        let mut subs = BTreeMap::new();
        subs.insert(
            "1014537851418980372".to_string(),
            OutboundGuildSubscription {
                typing: true,
                threads: true,
                activities: true,
                members: Vec::new(),
                member_updates: false,
                channels: vec!["1176785373989445703".to_string()],
            },
        );
        pipe.queue_guild_subscriptions_bulk(&subs)
            .expect("queue op37");
        assert_eq!(pipe.pending_outbound_frames(), 5);

        let outcome = timeout(TEST_TIMEOUT, connect_and_run(&url, &mut pipe))
            .await
            .expect("connect_and_run уложился в тайм-аут")
            .expect("connect_and_run завершился без ошибки");
        assert_eq!(outcome, RunOutcome::Closed { code: None });
        assert_eq!(pipe.pending_outbound_frames(), 0);
        assert_eq!(pipe.frames_sent(), 5);

        let received_ops = timeout(TEST_TIMEOUT, server_task)
            .await
            .expect("server_task тайм-аут")
            .expect("server_task join");
        assert_eq!(received_ops, [3, 4, 8, 8, 37]);
    }

    #[tokio::test]
    async fn voice_v8_op8_hello_sends_immediate_op3_heartbeat_within_60ms_and_op6_ack_updates_rtt()
    {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut v8 = super::PureRustVoiceV8State::new(
            "1014537851418980372",
            "1176785373989445703",
            "999888777666555444",
            "rtc_sess_01",
            "voice_tok_01",
        );

        // 1. Server sends OP 8 HELLO with heartbeat_interval = 13750 (13.75s):
        server_ws
            .send(Message::Text(
                r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750},"seq":1}"#.into(),
            ))
            .await
            .expect("send op8 hello");

        let t0 = std::time::Instant::now();
        super::drive_voice_v8_ws_step(&mut client_ws, &mut v8)
            .await
            .expect("drive op8 hello");

        assert_eq!(v8.heartbeat_interval_ms(), 13_750);
        assert_eq!(
            v8.initial_heartbeat_delay_ms(),
            Some(super::VOICE_V8_INITIAL_HEARTBEAT_DELAY_MS),
            "First OP 3 HEARTBEAT must be scheduled at 60ms instead of waiting 13.75s"
        );
        assert_eq!(v8.seq_ack(), Some(1));

        // 2. Server immediately receives the initial OP 3 HEARTBEAT (< 100ms, never waiting 13.75s):
        let msg = timeout(Duration::from_millis(200), server_ws.next())
            .await
            .expect("initial op3 heartbeat must arrive immediately without waiting 13.75s")
            .expect("ws open")
            .expect("ws ok");
        assert!(
            t0.elapsed() < Duration::from_millis(200),
            "initial op3 heartbeat took too long: {:?}",
            t0.elapsed()
        );
        let hb_text = match msg {
            Message::Text(t) => t.to_string(),
            other => panic!("expected text op3 heartbeat, got {other:?}"),
        };
        let hb_parsed = chord_json::parse(&hb_text).expect("valid op3 json");
        assert_eq!(
            hb_parsed
                .get("op")
                .and_then(chord_json::JsonValue::as_i64),
            Some(3)
        );
        let nonce = hb_parsed
            .get("d")
            .and_then(|d| d.get("t"))
            .and_then(chord_json::JsonValue::as_i64)
            .expect("op3 d.t nonce");
        assert_eq!(
            hb_parsed
                .get("d")
                .and_then(|d| d.get("seq_ack"))
                .and_then(chord_json::JsonValue::as_i64),
            Some(1)
        );

        // 3. Server replies with OP 6 HEARTBEAT_ACK after 15ms; client calculates real RTT:
        tokio::time::sleep(Duration::from_millis(15)).await;
        server_ws
            .send(Message::Text(
                format!(r#"{{"op":6,"d":{{"t":{nonce}}}}}"#).into(),
            ))
            .await
            .expect("send op6 heartbeat_ack");

        super::drive_voice_v8_ws_step(&mut client_ws, &mut v8)
            .await
            .expect("drive op6 ack");

        let rtt = v8
            .last_rtt_ms()
            .expect("RTT must be measured on OP 6 HEARTBEAT_ACK");
        assert!(
            (10..500).contains(&rtt),
            "Measured RTT must reflect real elapsed time (~15ms), got {rtt}ms"
        );
        let snap = super::live_rtc_snapshot();
        assert_eq!(snap.last_ping_ms, Some(rtt));
    }

    #[tokio::test]
    async fn voice_v8_op16_and_op12_ssrc_mapping_and_ping_response() {
        let (mut client_ws, mut server_ws) = ws_pair().await;
        let mut v8 = super::PureRustVoiceV8State::new(
            "1014537851418980372",
            "1176785373989445703",
            "999888777666555444",
            "rtc_sess_785",
            "voice_tok_785",
        );
        v8.set_broadcaster(true);

        // 1. Server sends OP 2 READY with assigned SSRC and streams (video_ssrc=9209, rtx_ssrc=9210):
        server_ws
            .send(Message::Text(
                r#"{"op":2,"d":{"ssrc":9208,"ip":"104.29.146.60","port":19329,"modes":["aead_aes256_gcm_rtpsize"],"streams":[{"type":"video","rid":"100","ssrc":9209,"rtx_ssrc":9210,"quality":100,"active":false}]},"seq":1}"#.into(),
            ))
            .await
            .expect("send op2 ready");

        super::drive_voice_v8_ws_step(&mut client_ws, &mut v8)
            .await
            .expect("drive op2 ready");

        assert_eq!(v8.local_ssrc(), 9208);
        assert_eq!(v8.video_ssrc(), 9209);
        assert_eq!(v8.rtx_ssrc(), 9210);

        // Client must send OP 16 {"op":16,"d":{}} upon OP 2 READY:
        let op16_req = timeout(Duration::from_millis(200), server_ws.next())
            .await
            .expect("op16 request timeout")
            .expect("ws open")
            .expect("ws ok");
        let op16_req_text = match op16_req {
            Message::Text(t) => t.to_string(),
            other => panic!("expected text op16, got {other:?}"),
        };
        assert_eq!(op16_req_text, r#"{"op":16,"d":{}}"#);

        // 2. Server sends incoming OP 16 (VOICE_BACKEND_VERSION / RTC Ping):
        server_ws
            .send(Message::Text(
                r#"{"op":16,"d":{"voice":"0.22.1+f01bd52","rtc_worker":"1.6.98","t":1700000000785},"seq":2}"#.into(),
            ))
            .await
            .expect("send incoming op16");

        super::drive_voice_v8_ws_step(&mut client_ws, &mut v8)
            .await
            .expect("drive incoming op16");

        assert_eq!(v8.voice_backend_version(), Some("0.22.1+f01bd52"));
        assert_eq!(v8.rtc_worker_version(), Some("1.6.98"));
        assert_eq!(v8.seq_ack(), Some(2));

        // Client replies to incoming OP 16 with OP 3 heartbeat/ping response:
        let hb_reply = timeout(Duration::from_millis(200), server_ws.next())
            .await
            .expect("op3 reply to op16 timeout")
            .expect("ws open")
            .expect("ws ok");
        let hb_reply_text = match hb_reply {
            Message::Text(t) => t.to_string(),
            other => panic!("expected text op3 reply, got {other:?}"),
        };
        let hb_parsed = chord_json::parse(&hb_reply_text).expect("valid op3 json");
        assert_eq!(
            hb_parsed
                .get("op")
                .and_then(chord_json::JsonValue::as_i64),
            Some(3)
        );
        assert_eq!(
            hb_parsed
                .get("d")
                .and_then(|d| d.get("t"))
                .and_then(chord_json::JsonValue::as_i64),
            Some(1_700_000_000_785)
        );
        assert_eq!(
            hb_parsed
                .get("d")
                .and_then(|d| d.get("seq_ack"))
                .and_then(chord_json::JsonValue::as_i64),
            Some(2)
        );

        // 3. Server sends OP 4 SESSION_DESCRIPTION -> Broadcaster sends OP 12 VIDEO with video_ssrc & rtx_ssrc:
        server_ws
            .send(Message::Text(
                r#"{"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","audio_codec":"opus","video_codec":"H264","dave_protocol_version":0,"secret_key":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]},"seq":3}"#.into(),
            ))
            .await
            .expect("send op4 session_description");

        super::drive_voice_v8_ws_step(&mut client_ws, &mut v8)
            .await
            .expect("drive op4");

        let op12_msg = timeout(Duration::from_millis(200), server_ws.next())
            .await
            .expect("op12 video timeout")
            .expect("ws open")
            .expect("ws ok");
        let op12_text = match op12_msg {
            Message::Text(t) => t.to_string(),
            other => panic!("expected text op12, got {other:?}"),
        };
        let op12_parsed = chord_json::parse(&op12_text).expect("valid op12 json");
        assert_eq!(
            op12_parsed
                .get("op")
                .and_then(chord_json::JsonValue::as_i64),
            Some(12)
        );
        let d12 = op12_parsed.get("d").expect("op12 d");
        assert_eq!(
            d12.get("audio_ssrc")
                .and_then(chord_json::JsonValue::as_i64),
            Some(9208)
        );
        assert_eq!(
            d12.get("video_ssrc")
                .and_then(chord_json::JsonValue::as_i64),
            Some(9209)
        );
        assert_eq!(
            d12.get("rtx_ssrc")
                .and_then(chord_json::JsonValue::as_i64),
            Some(9210)
        );
        let streams = d12
            .get("streams")
            .and_then(chord_json::JsonValue::as_array)
            .expect("op12 d.streams");
        assert_eq!(streams.len(), 1);
        assert_eq!(
            streams[0]
                .get("ssrc")
                .and_then(chord_json::JsonValue::as_i64),
            Some(9209)
        );
        assert_eq!(
            streams[0]
                .get("rtx_ssrc")
                .and_then(chord_json::JsonValue::as_i64),
            Some(9210)
        );
        assert_eq!(
            streams[0]
                .get("active")
                .and_then(chord_json::JsonValue::as_bool),
            Some(true)
        );
    }

    #[test]
    fn op4_session_description_negotiates_av1_codec_without_h264_fallback() {
        let mut v8 =
            super::PureRustVoiceV8State::new("1000", "2000", "123456789", "sess-av1", "tok-av1");
        v8.set_broadcaster(true);
        assert_eq!(v8.video_codec(), None);

        let frames = v8
            .on_text_frame(
                r#"{"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","audio_codec":"opus","video_codec":"AV1","dave_protocol_version":0,"secret_key":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]},"seq":4}"#,
            )
            .expect("process op4 with video_codec AV1");
        assert_eq!(v8.video_codec(), Some("AV1"));
        assert_eq!(super::live_rtc_snapshot().active_video_codec, "AV1");
        assert!(!frames.is_empty());
    }
}
