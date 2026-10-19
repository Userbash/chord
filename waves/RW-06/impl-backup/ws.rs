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
//! - client→server: **несжатые** ETF-термы; heartbeat — `{op: 1, d}` ([`encode_heartbeat`]).
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
//! [`Session::ignored_values`]), [`GatewayPipe::pending_bytes`] — байты в хвосте.
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
//! ## Известное ограничение (RW-02: удержание окна ruzstd)
//!
//! Пока zstd-кадр не завершён, [`ZstdStream`] отдаёт не более `распаковано − окно`. Для снимка
//! `gw-proto/zstd-frames-95974_1389.hexl` (окно 65 536) это 759 280 Б из 824 816: пайп видит
//! 44 полных терма (последний `s = 36`) и 704 Б обрезанного терма в хвосте, тогда как в потоке
//! есть 113 диспатчей (`s = 1..113`, независимый оракул — таймлайн того же сокета). Высвободить
//! удержанный хвост может только завершение кадра (P1-02) — это вне файлов RW-04.

use crate::gateway::{Effect, GatewayError, ReconnectReason, Session};
use crate::identify::{IdentifyError, IdentifyParams, build_identify_frame};
use crate::resume::{ResumeError, ResumeParams, ResumeState, build_resume_frame};
use crate::ready::ReadySummary;
use discord_codec::etf::{self, EtfError, EtfValue};
use discord_codec::zstd::{ZstdError, ZstdStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::{Connector, WebSocketStream, connect_async_tls_with_config};

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
    /// `d` последнего op9 INVALID_SESSION: `Some(true)` — resume разрешён (RW-06).
    last_invalid_session_resumable: Option<bool>,
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
            last_invalid_session_resumable: None,
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

    /// Сколько байт ждёт разбора в хвосте (диагностика разрезанных термов и удержания окна).
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

    /// Учитывает проигнорированное WS-сообщение (зовёт цикл транспорта: Text/Pong/сырой Frame).
    pub fn note_ignored_message(&mut self) {
        self.ignored_messages = self.ignored_messages.saturating_add(1);
    }
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

/// Опции прогона [`connect_and_run_with`] (RW-05).
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// IDENTIFY (op2): отправляется **один раз** после первого HELLO; `None` — не отправлять.
    pub identify: Option<IdentifyParams>,
}

impl RunOptions {
    /// Без IDENTIFY — поведение [`connect_and_run`].
    pub fn none() -> Self {
        Self { identify: None }
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
    let mut handshake_sent = false;
    // Пайп мог увидеть HELLO до входа в цикл (вызывающий подал кадры сам) — шлём сразу.
    maybe_send_handshake(&mut ws, pipe, &opts, resume.as_ref(), &mut handshake_sent).await?;
    loop {
        // Тик таймеров на текущий момент: срок удара либо дедлайн зомби (RW-03).
        let effects = pipe.session.on_timer(elapsed_ms(start));
        if let Some(outcome) = apply_effects(effects, &mut ws).await? {
            return Ok(outcome);
        }
        // Сон до ближайшего дедлайна сессии; дедлайнов нет (до HELLO) — ждём только данные.
        let step = match pipe
            .session
            .next_deadline_ms()
            .map(|ms| start + Duration::from_millis(ms))
        {
            Some(deadline) => tokio::select! {
                message = ws.next() => Step::Message(message),
                () = tokio::time::sleep_until(deadline) => Step::Timer,
            },
            None => Step::Message(ws.next().await),
        };
        match step {
            // Таймер сработал: новая итерация сделает `on_timer` на новое время.
            Step::Timer => {}
            // Поток закончился без close-кадра (обрыв TCP) — соединения больше нет.
            Step::Message(None) => return Ok(RunOutcome::Closed { code: None }),
            Step::Message(Some(Err(error))) => {
                return Err(TransportError::Ws {
                    msg: error.to_string(),
                });
            }
            Step::Message(Some(Ok(message))) => match message {
                Message::Binary(data) => {
                    let effects = pipe.push_binary(&data, elapsed_ms(start))?;
                    if let Some(outcome) = apply_effects(effects, &mut ws).await? {
                        return Ok(outcome);
                    }
                    // HELLO разобран этим кадром (или ранее) — IDENTIFY ровно один раз.
                    maybe_send_handshake(&mut ws, pipe, &opts, resume.as_ref(), &mut handshake_sent)
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

/// Один шаг цикла: WS-сообщение (или конец потока) либо сработавший таймер.
enum Step {
    /// Следующее сообщение потока; `None` — поток закрылся.
    Message(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>),
    /// Сработал таймер: нужен новый тик [`Session::on_timer`].
    Timer,
}

/// Выполняет эффекты сессии; `Reconnect` прерывает цикл (исход), `SendHeartbeat` пишет кадр.
async fn apply_effects<S>(
    effects: Vec<Effect>,
    ws: &mut WebSocketStream<S>,
) -> Result<Option<RunOutcome>, TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    for effect in effects {
        match effect {
            Effect::SendHeartbeat { d } => {
                let frame = encode_heartbeat(d);
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
