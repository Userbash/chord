//! # Сессия шлюза Discord: детерминированное ядро (RW-03, T-GW-01…03)
//!
//! Обработка HELLO, планирование heartbeat (интервал 41 250 мс; первый удар — jitter от сида),
//! учёт ACK/RTT, детектор зомби, отслеживание `s`-последовательности и реакции на op7/op9.
//! **Транспорта здесь нет** (сокет/TLS/WS — RW-04): часы виртуальные — `now_ms: u64` передаёт
//! вызывающий, `std::time` не используется, поведение детерминировано и воспроизводимо.
//!
//! Контракт (ТЗ RW-03; проверен реплеем реального таймлайна `95974.1389` в
//! `tests/gateway_session.rs`):
//! 1. op10 HELLO: `d.heartbeat_interval` → [`Hello`]; первый удар — `now + jitter_seed % interval`;
//!    повторный HELLO до первого удара пере-планирует расписание, после первого удара —
//!    игнорируется;
//! 2. op11 ACK: закрывает удар (`rtt = now − момент удара`), следующий удар — `ACK + interval`;
//!    ACK без отправленного удара («лишний» ACK между ударами в захвате) игнорируется;
//! 3. [`Session::on_timer`]: нет ACK за интервал → `Reconnect{Zombie}` (сессия мертва); иначе —
//!    удар по сроку, `d` = последний `s` (до первого диспатча — `null`);
//! 4. op0 DISPATCH: `s` (int) → последняя последовательность; `t`/`d` не анализируются;
//! 5. op7 → `Reconnect{ServerRequest}`, op9 → `Reconnect{InvalidSession}`; сессия мертва;
//! 6. прочие опкоды и значения без целого `op` — без эффектов (счётчик
//!    [`Session::ignored_values`]); мусорные байты → [`GatewayError::Decode`], нарушенная
//!    структура разобранного терма → [`GatewayError::Malformed`];
//! 7. [`Session::next_deadline_ms`]: min(срок удара, дедлайн зомби) либо `None`.
//!
//! Паник на входных данных нет: арифметика — saturating, нулевой интервал HELLO отвергается как
//! `Malformed` (деление по модулю не выполняется), `unsafe` запрещён линтом крейта. Ошибки не
//! глотаются молча: [`Session::on_term`] разбирает **ровно один** терм на кадр (хвост —
//! `Malformed`) и остаётся честным даже у мёртвой сессии; [`Session::on_value`] мёртвой сессии
//! возвращает `Ok` без эффектов — это документированное «молчание» мёртвой сессии.

use discord_codec::etf::{self, EtfValue};

pub mod supervisor;

/// Оп-код QoS-heartbeat (`QOS_HEARTBEAT`) из таблицы клиента 1.0.162 (AUTH-RE G-12).
///
/// В [`Op`] кода 40 нет намеренно: сервер его не присылает, а `Op::from_code` — контракт
/// RW-03 (`Op::from_code(40) == Op::Unknown(40)`). Клиентская сторона различает heartbeat по
/// [`HeartbeatMode`].
pub const QOS_HEARTBEAT_OP: i64 = 40;

/// Какой кадр клиент шлёт на удар: боевой op40 или legacy op1 (rw-03/04).
///
/// `Session::new` оставляет [`HeartbeatMode::LegacyOp1`] — обратная совместимость ядра RW-03;
/// боевой путь (супервайзер, транспорт RW-39) включает [`HeartbeatMode::Qos`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartbeatMode {
    /// Legacy op1 `{op:1, d:seq}` — старый контракт RW-03/04.
    #[default]
    LegacyOp1,
    /// Боевой op40 `{op:40, d:{seq, qos}}` (AUTH-RE G-12).
    Qos,
}

/// QoS-полезная нагрузка op40: `{active, reasons}` (из `heartbeatQOSState` клиента).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosPayload {
    /// Пользователь активен (клиент шлёт activity-состояние).
    pub active: bool,
    /// Причины активности (в клиенте — отсортированный уникальный список).
    pub reasons: Vec<String>,
}

impl QosPayload {
    /// Неактивное состояние с пустым списком причин.
    pub fn inactive() -> Self {
        Self {
            active: false,
            reasons: Vec::new(),
        }
    }
}

/// Оп-код шлюза: известные из захвата плюс «всё прочее» числом.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// op0 — DISPATCH (несёт `s`-последовательность).
    Dispatch,
    /// op1 — HEARTBEAT (запрос сервера; в `Qos`-режиме — немедленный удар, G-12).
    Heartbeat,
    /// op7 — RECONNECT.
    Reconnect,
    /// op9 — INVALID_SESSION.
    InvalidSession,
    /// op10 — HELLO.
    Hello,
    /// op11 — HEARTBEAT_ACK.
    HeartbeatAck,
    /// Любой иной оп-код (op2/op40/…): без эффектов.
    Unknown(i64),
}

impl Op {
    /// Оп-код числа → вариант; неизвестные коды сохраняются.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Dispatch,
            1 => Self::Heartbeat,
            7 => Self::Reconnect,
            9 => Self::InvalidSession,
            10 => Self::Hello,
            11 => Self::HeartbeatAck,
            other => Self::Unknown(other),
        }
    }
}

/// Полезная нагрузка HELLO (op10): интервал heartbeat из `d.heartbeat_interval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// Интервал heartbeat в миллисекундах (в захвате — 41 250).
    pub heartbeat_interval_ms: u64,
}

/// Причина реконнекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectReason {
    /// Нет ACK за интервал — сервер «зомби».
    Zombie,
    /// Сервер прислал op7 RECONNECT.
    ServerRequest,
    /// Сервер прислал op9 INVALID_SESSION.
    InvalidSession,
}

/// Эффект, который сессия просит выполнить снаружи (транспорт — вне этой волны).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Отправить heartbeat; `d` — последняя `s`-последовательность (до первого диспатча — `null`).
    SendHeartbeat {
        /// Последний `s` из op0 либо `None` (до первого диспатча).
        d: Option<u64>,
    },
    /// Переподключиться с указанной причиной.
    Reconnect {
        /// Почему нужен реконнект.
        reason: ReconnectReason,
    },
}

/// Ошибки сессии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayError {
    /// Байты не разбираются как ETF-терм.
    Decode {
        /// Текст ошибки декодера.
        msg: String,
    },
    /// Терм разобран, но его структура нарушена (например, op10 без `heartbeat_interval`).
    Malformed {
        /// Описание нарушения.
        msg: String,
    },
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode { msg } => write!(f, "терм не декодирован: {msg}"),
            Self::Malformed { msg } => write!(f, "нарушена структура терма: {msg}"),
        }
    }
}

impl std::error::Error for GatewayError {}

/// Состояние одной сессии шлюза.
pub struct Session {
    /// Сид детерминированного jitter-а первого удара.
    jitter_seed: u64,
    /// HELLO, если уже пришёл (единственный источник интервала).
    hello: Option<Hello>,
    /// Срок ближайшего запланированного удара (ждём прихода виртуальных часов).
    next_beat_ms: Option<u64>,
    /// Момент отправки удара, ожидающего ACK (неответный удар).
    pending_beat_ms: Option<u64>,
    /// Последняя `s`-последовательность из op0.
    last_seq: Option<u64>,
    /// Последний измеренный RTT (ACK минус удар).
    rtt_ms: Option<u64>,
    /// Сколько heartbeat-ударов отправлено.
    beats_emitted: u64,
    /// Сколько значений проигнорировано (неизвестные/необрабатываемые опкоды).
    ignored_values: u64,
    /// Мёртва ли сессия (зомби либо оп7/оп9).
    dead: bool,
    /// Форма heartbeat: op40 (боевая) либо op1 (legacy).
    heartbeat_mode: HeartbeatMode,
    /// Текущая QoS-полезная нагрузка для op40 (`None` до первого active-состояния).
    qos: Option<QosPayload>,
}

impl Session {
    /// Новая сессия с заданным сидом jitter-а.
    pub fn new(jitter_seed: u64) -> Self {
        Self {
            jitter_seed,
            hello: None,
            next_beat_ms: None,
            pending_beat_ms: None,
            last_seq: None,
            rtt_ms: None,
            beats_emitted: 0,
            ignored_values: 0,
            dead: false,
            heartbeat_mode: HeartbeatMode::LegacyOp1,
            qos: None,
        }
    }

    /// Новая сессия с явной формой heartbeat (боевая — [`HeartbeatMode::Qos`]).
    pub fn with_heartbeat_mode(jitter_seed: u64, heartbeat_mode: HeartbeatMode) -> Self {
        let mut session = Self::new(jitter_seed);
        session.heartbeat_mode = heartbeat_mode;
        session
    }

    /// Форма heartbeat этой сессии.
    pub fn heartbeat_mode(&self) -> HeartbeatMode {
        self.heartbeat_mode
    }

    /// Переключает форму heartbeat (например, fallback на op1 без пересоздания сессии).
    pub fn set_heartbeat_mode(&mut self, heartbeat_mode: HeartbeatMode) {
        self.heartbeat_mode = heartbeat_mode;
    }

    /// Текущая QoS-нагрузка op40 (`None` — `qos:null`, как у клиента до active-состояния).
    pub fn qos(&self) -> Option<&QosPayload> {
        self.qos.as_ref()
    }

    /// Заменяет QoS-нагрузку для следующих ударов.
    pub fn set_qos(&mut self, qos: Option<QosPayload>) {
        self.qos = qos;
    }

    /// Байты провода (с ведущим `0x83`) → эффекты.
    ///
    /// Кадр обязан содержать **ровно один** терм: хвост после разобранного терма — `Malformed`
    /// (ошибки не глотаются). Мусорные байты — `Decode`; паники на входе нет.
    pub fn on_term(&mut self, term: &[u8], now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        let (value, consumed) = etf::decode(term).map_err(|error| GatewayError::Decode {
            msg: error.to_string(),
        })?;
        if consumed != term.len() {
            return Err(GatewayError::Malformed {
                msg: format!(
                    "кадр {} Б: разобран один терм на {consumed} Б (лишний хвост)",
                    term.len()
                ),
            });
        }
        self.on_value(&value, now_ms)
    }

    /// Разобранный терм → эффекты.
    pub fn on_value(&mut self, value: &EtfValue, now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        if self.dead {
            // Мёртвая сессия молчит: транспорт уже переподключается.
            return Ok(Vec::new());
        }
        let Some(code) = value.get("op").and_then(EtfValue::as_int) else {
            // Значение без целого `op` — не кадр шлюза: считаем и молчим (паники нет).
            self.ignored_values += 1;
            return Ok(Vec::new());
        };
        match Op::from_code(code) {
            Op::Hello => self.on_hello(value, now_ms),
            Op::HeartbeatAck => {
                self.on_ack(now_ms);
                Ok(Vec::new())
            }
            Op::Dispatch => self.on_dispatch(value),
            Op::Reconnect => Ok(vec![self.kill(ReconnectReason::ServerRequest)]),
            Op::InvalidSession => Ok(vec![self.kill(ReconnectReason::InvalidSession)]),
            Op::Heartbeat => match self.heartbeat_mode {
                // G-12: op1 от сервера — триггер немедленного heartbeat (боевой режим).
                HeartbeatMode::Qos => Ok(self.on_server_heartbeat(now_ms)),
                // Legacy RW-03: op1 сервер не присылает — значение игнорируется как раньше.
                HeartbeatMode::LegacyOp1 => {
                    self.ignored_values += 1;
                    Ok(Vec::new())
                }
            },
            Op::Unknown(_) => {
                // Сервер op40 не присылает; неизвестные коды не обрабатываются (RW-03).
                self.ignored_values += 1;
                Ok(Vec::new())
            }
        }
    }

    /// Тик виртуальных часов.
    ///
    /// Порядок проверок: мёртвая сессия → дедлайн зомби → срок удара. Зомби важнее удара:
    /// сервер, не ответивший на прошлый удар, не должен получать новый.
    pub fn on_timer(&mut self, now_ms: u64) -> Vec<Effect> {
        if self.dead {
            return Vec::new();
        }
        if self
            .zombie_deadline_ms()
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.dead = true;
            self.pending_beat_ms = None;
            self.next_beat_ms = None;
            return vec![Effect::Reconnect {
                reason: ReconnectReason::Zombie,
            }];
        }
        if self.next_beat_ms.is_some_and(|due| now_ms >= due) {
            self.beats_emitted += 1;
            self.pending_beat_ms = Some(now_ms);
            self.next_beat_ms = None;
            return vec![Effect::SendHeartbeat { d: self.last_seq }];
        }
        Vec::new()
    }

    /// Ближайший дедлайн (удар или зомби).
    pub fn next_deadline_ms(&self) -> Option<u64> {
        if self.dead {
            return None;
        }
        [self.next_beat_ms, self.zombie_deadline_ms()]
            .into_iter()
            .flatten()
            .min()
    }

    /// HELLO, если уже пришёл.
    pub fn hello(&self) -> Option<&Hello> {
        self.hello.as_ref()
    }

    /// Последняя `s`-последовательность из op0.
    pub fn last_seq(&self) -> Option<u64> {
        self.last_seq
    }

    /// Последний измеренный RTT (ACK минус удар).
    pub fn rtt_ms(&self) -> Option<u64> {
        self.rtt_ms
    }

    /// Сколько heartbeat-ударов отправлено.
    pub fn beats_emitted(&self) -> u64 {
        self.beats_emitted
    }

    /// Мёртва ли сессия (зомби либо оп7/оп9).
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Сколько значений проигнорировано (неизвестные/необрабатываемые опкоды).
    pub fn ignored_values(&self) -> u64 {
        self.ignored_values
    }

    /// Интервал heartbeat из принятого HELLO (до HELLO — `None`).
    fn interval_ms(&self) -> Option<u64> {
        self.hello.map(|hello| hello.heartbeat_interval_ms)
    }

    /// Дедлайн зомби: момент удара + интервал, пока ACK не пришёл; до первого удара — `None`.
    ///
    /// Инвариант: неответный удар появляется только после HELLO, поэтому интервал здесь всегда
    /// есть — второе `?` недостижимо, но не паникует.
    fn zombie_deadline_ms(&self) -> Option<u64> {
        let sent = self.pending_beat_ms?;
        Some(sent.saturating_add(self.interval_ms()?))
    }

    /// op10 HELLO: запомнить интервал и запланировать первый удар.
    fn on_hello(&mut self, value: &EtfValue, now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        // Повторный HELLO после первого удара игнорируется: расписание уже принято.
        if self.beats_emitted > 0 {
            return Ok(Vec::new());
        }
        let interval = value
            .get("d")
            .and_then(|d| d.get("heartbeat_interval"))
            .and_then(EtfValue::as_int)
            .and_then(|raw| u64::try_from(raw).ok())
            .filter(|interval| *interval > 0)
            .ok_or_else(|| GatewayError::Malformed {
                msg: "op10 HELLO: d.heartbeat_interval — положительное целое".to_string(),
            })?;
        self.hello = Some(Hello {
            heartbeat_interval_ms: interval,
        });
        // Первый удар — детерминированный jitter: seed % interval (замена random из клиента).
        self.next_beat_ms = Some(now_ms.saturating_add(self.jitter_seed % interval));
        Ok(Vec::new())
    }

    /// op11 HEARTBEAT_ACK: закрыть неответный удар и перенести расписание от ACK.
    fn on_ack(&mut self, now_ms: u64) {
        // ACK без отправленного удара — «лишний» ACK захвата (парные суммы ≈ интервал): игнорируем.
        let (Some(sent), Some(interval)) = (self.pending_beat_ms, self.interval_ms()) else {
            return;
        };
        self.pending_beat_ms = None;
        self.rtt_ms = Some(now_ms.saturating_sub(sent));
        // Следующий удар — через интервал от ACK (а не от расписания прошлого удара).
        self.next_beat_ms = Some(now_ms.saturating_add(interval));
    }

    /// op1 от сервера в боевом режиме: немедленный удар и перезапуск интервала (G-12/G-13).
    ///
    /// Клиент (`_handleHeartbeatReceive`) шлёт heartbeat сразу и пересоздаёт `setInterval`,
    /// поэтому следующий плановый удар — `now + interval`. Отправленный удар становится
    /// ожидающим ACK (дедлайн зомби — `now + interval`).
    fn on_server_heartbeat(&mut self, now_ms: u64) -> Vec<Effect> {
        self.beats_emitted = self.beats_emitted.saturating_add(1);
        if let Some(interval) = self.interval_ms() {
            self.pending_beat_ms = Some(now_ms);
            self.next_beat_ms = Some(now_ms.saturating_add(interval));
        }
        vec![Effect::SendHeartbeat { d: self.last_seq }]
    }

    /// op0 DISPATCH: обновить `s`-последовательность (идёт в `d` следующего удара).
    fn on_dispatch(&mut self, value: &EtfValue) -> Result<Vec<Effect>, GatewayError> {
        let seq = value
            .get("s")
            .and_then(EtfValue::as_int)
            .and_then(|raw| u64::try_from(raw).ok())
            .ok_or_else(|| GatewayError::Malformed {
                msg: "op0 DISPATCH: s — неотрицательное целое".to_string(),
            })?;
        self.last_seq = Some(seq);
        Ok(Vec::new())
    }

    /// Помечает сессию мёртвой (зомби уже случился/сервер распорядился) и снимает расписание.
    fn kill(&mut self, reason: ReconnectReason) -> Effect {
        self.dead = true;
        self.pending_beat_ms = None;
        self.next_beat_ms = None;
        Effect::Reconnect { reason }
    }
}

#[cfg(test)]
mod tests {
    use super::Op;

    #[test]
    fn op_codes_match_protocol_table() {
        assert_eq!(Op::from_code(0), Op::Dispatch);
        assert_eq!(Op::from_code(1), Op::Heartbeat);
        assert_eq!(Op::from_code(11), Op::HeartbeatAck);
        assert_eq!(Op::from_code(-7), Op::Unknown(-7));
    }
}
