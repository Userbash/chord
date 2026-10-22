//! Генератор нагрузки и замеры для T-HARN-04 (карточка P7-02).
//!
//! Харнесс строит детерминированную нагрузку целевого масштаба (24 229 участников,
//! 100 000 сообщений, 512 каналов) и измеряет её на **реальном** редьюсере
//! `discord_core::state::apply`. Ни сети, ни SQLite, ни UI: только события, редьюсер
//! и чтение состояния.
//!
//! # Детерминизм
//!
//! Генератор — собственный PRNG [`SplitMix64`] (приращение состояния
//! `0x9E37_79B9_7F4A_7C15`, две перетасовки `xorshift`+`multiply`, финальный `xorshift`).
//! Единственный источник случайности — `WorkloadSpec::seed`: ни `SystemTime`, ни `random`,
//! ни времени чтения. Один и тот же seed ⇒ побайтово тот же набор событий.
//!
//! # Digest
//!
//! `digest` — FNV-1a 64 (смещение `0xcbf2_9ce4_8422_2325`, множитель
//! `0x0000_0100_0000_01b3`) по фиксированному порядку обхода:
//!
//! 1. события в порядке `Workload::events`;
//! 2. `GuildMemberListUpdate` — по `ops`, по элементам op'а; для участника — 8 байт
//!    `member.user_id` (little-endian);
//! 3. `MessageCreate` — 8 байт `id`, 8 байт `channel_id`, затем байты `content`.
//!
//! # Метрики
//!
//! - `ColdStart` — построение состояния с нуля: базовый READY + все события нагрузки;
//! - `Ingest` — только применение сообщений к состоянию с уже загруженными участниками;
//! - `Selector` — суммарное время read-only выборок из готового состояния (мкс).
//!
//! Метрики — реальное время (`std::time::Instant`), а не константы. Счётчики
//! `members_loaded`/`messages_ingested` берутся из фактического состояния, чтобы
//! «молчащая» потеря данных была видна.

use std::time::{Duration, Instant};

use discord_core::codec::etf::EtfValue;
use discord_core::ready::{GuildMember, Ready, ReadyError, User};
use discord_core::state::{
    Event, EventPayload, MemberListEntry, MemberListGroup, MemberListId, MemberListOp,
    MemberListRange, MemberListUpdate, MemberPresence, Message, State, StateDiagnostics, apply,
};
use serde::{Deserialize, Serialize};

/// Гильдия референсной нагрузки.
pub const PERF_GUILD_ID: u64 = 1_000_000_000;

/// Raw-id списка участников референсной нагрузки.
pub const PERF_MEMBER_LIST_ID: &str = "everyone";

/// База id каналов референсной нагрузки.
pub const PERF_CHANNEL_BASE: u64 = 2_000_000_000;

/// База id участников гильдии.
pub const PERF_USER_BASE: u64 = 3_000_000_000;

/// База id сообщений.
pub const PERF_MESSAGE_BASE: u64 = 4_000_000_000;

/// Id собственного пользователя харнеса (не участник нагрузки).
pub const PERF_SELF_USER_ID: u64 = 9_000_000_000;

/// Смещение FNV-1a 64.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Простой множитель FNV-1a 64.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Описание нагрузки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadSpec {
    /// Число участников (референс: 24 229).
    pub members: usize,
    /// Число сообщений (референс: 100 000).
    pub messages: usize,
    /// Число каналов (референс: 512).
    pub channels: usize,
    /// Seed генератора (детерминизм обязателен).
    pub seed: u64,
}

impl WorkloadSpec {
    /// Референсная нагрузка P7-02.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            members: 24_229,
            messages: 100_000,
            channels: 512,
            seed: 0x00c0_ffee_0000_0001,
        }
    }
}

/// Сгенерированная нагрузка.
pub struct Workload {
    /// Описание, по которому нагрузка построена.
    pub spec: WorkloadSpec,
    /// События для редьюсера: участники (`GuildMemberListUpdate`) и сообщения
    /// (`MessageCreate`) в фиксированном порядке.
    pub events: Vec<Event>,
}

/// Метрика, по которой сравнивают прогоны.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Полное построение состояния с нуля.
    ColdStart,
    /// Только применение сообщений.
    Ingest,
    /// Время выборок из состояния.
    Selector,
}

impl Metric {
    /// Стабильное имя метрики (для отчётов и CI-истории).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ColdStart => "cold_start",
            Self::Ingest => "ingest",
            Self::Selector => "selector",
        }
    }
}

/// Отчёт одного прогона.
///
/// Метрики времени — обычные JSON-числа (`f64`): `serde_json` с feature
/// `float_roundtrip` разбирает их корректно, поэтому round-trip побитово точный.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfReport {
    /// Нагрузка, на которой снят отчёт.
    pub spec: WorkloadSpec,
    /// Построение состояния с нуля, мс.
    pub cold_start_ms: f64,
    /// Применение сообщений, мс.
    pub ingest_ms: f64,
    /// Выборки из состояния, мкс.
    pub selector_us: f64,
    /// Сколько участников реально попало в состояние.
    pub members_loaded: usize,
    /// Сколько сообщений реально применено.
    pub messages_ingested: usize,
    /// FNV-1a 64 по содержимому нагрузки — детерминизм проверяется по нему.
    pub digest: u64,
}

/// Порог регрессии T-HARN-04, %.
pub const REGRESSION_THRESHOLD_PCT: f64 = 10.0;

/// Результат сравнения прогона с базовым.
#[derive(Debug, Clone, PartialEq)]
pub enum Regression {
    /// Регрессии нет.
    None,
    /// Метрика ухудшилась больше чем на порог.
    Regressed {
        /// Ухудшившаяся метрика.
        metric: Metric,
        /// Значение базы.
        previous: f64,
        /// Значение текущего прогона.
        current: f64,
        /// Ухудшение в процентах.
        delta_pct: f64,
    },
}

/// SplitMix64 — детерминированный генератор псевдослучайных чисел.
///
/// Шаги зафиксированы: приращение состояния `0x9E37_79B9_7F4A_7C15`, затем две
/// перетасовки `xorshift`+`multiply` и финальный `xorshift`. Внешних зависимостей нет.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Новый генератор с заданным seed.
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Следующее 64-битное число.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// Число в диапазоне `[0, bound)`; при `bound == 0` — ноль.
    fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            0
        } else {
            self.next_u64() % bound
        }
    }
}

/// Детерминированно строит нагрузку. Один и тот же seed ⇒ побайтово тот же результат.
#[must_use]
pub fn build_workload(spec: &WorkloadSpec) -> Workload {
    let mut rng = SplitMix64::new(spec.seed);
    let mut events = Vec::with_capacity(spec.messages + spec.members + 1);

    // Участники: один SYNC со всем плоским списком `group|member` — так список
    // приходит в проводе, и редьюсер кладёт его в состояние целиком.
    //
    // Presence получают **все** участники: редьюсер ищет записи пользователя по диапазону
    // ключа `(user_id, scope)` (P7-02/I1), поэтому нагрузка не вырождается в квадратичную.
    let entries = build_member_entries(spec);
    let update = MemberListUpdate {
        guild_id: PERF_GUILD_ID,
        id: MemberListId::new(PERF_MEMBER_LIST_ID),
        member_count: Some(spec.members as i64),
        online_count: Some(spec.members as i64),
        groups: vec![
            MemberListGroup {
                id: "online".to_string(),
                count: u32::try_from(spec.members).unwrap_or(u32::MAX),
            },
            MemberListGroup {
                id: "offline".to_string(),
                count: 0,
            },
        ],
        ops: vec![MemberListOp::Sync {
            items: entries,
            range: Some(MemberListRange {
                start: 0,
                end: u32::try_from(spec.members.saturating_sub(1)).unwrap_or(u32::MAX),
            }),
        }],
    };
    events.push(Event {
        seq: Some(1),
        payload: EventPayload::GuildMemberListUpdate(update),
        diagnostics: StateDiagnostics::default(),
    });

    // Сообщения: монотонная `s`-последовательность, иначе сработает stale-политика.
    let channel_count = spec.channels.max(1);
    for index in 0..spec.messages {
        let channel_id = PERF_CHANNEL_BASE + (index % channel_count) as u64;
        let author_index = rng.below(spec.members.max(1) as u64);
        let author_id = PERF_USER_BASE + author_index;
        let mentions = if rng.below(8) == 0 {
            vec![author_id]
        } else {
            Vec::new()
        };
        // Токен в тексте делает содержимое зависимым от seed: иначе digest был бы
        // одинаков для любых seed (id и channel_id от seed не зависят).
        let token = rng.below(1_000_000);
        let message = Message {
            id: PERF_MESSAGE_BASE + index as u64,
            channel_id,
            guild_id: Some(PERF_GUILD_ID),
            author: Some(build_user(author_index)),
            content: format!(
                "perf-сообщение {index} от {author_id} в канале {channel_id} #{token}"
            ),
            timestamp: Some("2026-01-01T00:00:00.000Z".to_string()),
            edited_timestamp: None,
            message_type: 0,
            flags: 0,
            pinned: false,
            tts: false,
            mention_everyone: false,
            mention_user_ids: mentions,
            mention_role_ids: Vec::new(),
            attachment_count: rng.below(3) as usize,
            embed_count: rng.below(2) as usize,
        };
        events.push(Event {
            seq: Some(2 + index as u64),
            payload: EventPayload::MessageCreate(message),
            diagnostics: StateDiagnostics::default(),
        });
    }

    Workload {
        spec: spec.clone(),
        events,
    }
}

/// Плоский список участников: заголовок группы «online», все участники, заголовок
/// пустой группы «offline».
///
/// Presence есть у каждого участника — нагрузка проверяет и путь presence в редьюсере.
fn build_member_entries(spec: &WorkloadSpec) -> Vec<MemberListEntry> {
    let mut entries = Vec::with_capacity(spec.members + 2);
    entries.push(MemberListEntry::Group {
        id: "online".to_string(),
    });
    for index in 0..spec.members {
        entries.push(member_entry(index));
    }
    entries.push(MemberListEntry::Group {
        id: "offline".to_string(),
    });
    entries
}

/// Элемент списка: участник с его пользователем и presence «online».
fn member_entry(index: usize) -> MemberListEntry {
    MemberListEntry::Member {
        member: GuildMember {
            user_id: PERF_USER_BASE + index as u64,
            nick: Some(format!("nick-{index}")),
            ..GuildMember::default()
        },
        user: build_user(index as u64),
        presence: MemberPresence {
            status: Some("online".to_string()),
            ..MemberPresence::default()
        },
    }
}

/// Участник нагрузки как пользователь Discord.
fn build_user(index: u64) -> User {
    User {
        id: PERF_USER_BASE + index,
        username: Some(format!("perf-user-{index}")),
        global_name: Some(format!("Perf {index}")),
        display_name: None,
        discriminator: Some("0".to_string()),
        avatar: None,
        bot: false,
        public_flags: None,
    }
}

/// Базовое состояние для замера: одна гильдия и `spec.channels` каналов.
///
/// Публичный API нужен тестам: они применяют нагрузку к состоянию самостоятельно и
/// сверяют результат через публичные селекторы редьюсера, а не через внутренние поля.
pub fn base_state(spec: &WorkloadSpec) -> Result<State, ReadyError> {
    Ok(State::from_ready(&base_ready(spec)?))
}

/// Базовый READY для замера: одна гильдия с каналами и собственный пользователь.
fn base_ready(spec: &WorkloadSpec) -> Result<Ready, ReadyError> {
    let channel_count = spec.channels.max(1);
    let channels: Vec<EtfValue> = (0..channel_count)
        .map(|index| {
            EtfValue::Map(vec![
                entry(
                    "id",
                    EtfValue::Int((PERF_CHANNEL_BASE + index as u64) as i64),
                ),
                entry("type", EtfValue::Int(0)),
                entry("name", EtfValue::Str(format!("perf-channel-{index}"))),
                entry("position", EtfValue::Int(index as i64)),
            ])
        })
        .collect();

    let guild = EtfValue::Map(vec![
        entry("id", EtfValue::Int(PERF_GUILD_ID as i64)),
        entry("name", EtfValue::Str("perf-guild".to_string())),
        entry("member_count", EtfValue::Int(spec.members as i64)),
        entry("lazy", EtfValue::Atom("true".to_string())),
        entry("channels", EtfValue::List(channels)),
    ]);

    let user = EtfValue::Map(vec![
        entry("id", EtfValue::Int(PERF_SELF_USER_ID as i64)),
        entry("username", EtfValue::Str("perf-harness".to_string())),
    ]);

    let payload = EtfValue::Map(vec![
        entry("guilds", EtfValue::List(vec![guild])),
        entry("user", user),
        entry("session_id", EtfValue::Str("perf-session".to_string())),
        entry("users", EtfValue::List(Vec::new())),
    ]);

    Ready::parse(&payload)
}

/// Пара карты ETF: ключ-атом и значение.
fn entry(key: &str, value: EtfValue) -> (EtfValue, EtfValue) {
    (EtfValue::Atom(key.to_string()), value)
}

/// Сравнивает текущий прогон с базовым по порогу `threshold_pct`.
///
/// Метрики проверяются в фиксированном порядке: холодный старт, ingest, селектор.
///
/// Правила по базовой метрике:
/// - база `> 0` — обычное сравнение по порогу (ухудшение больше `threshold_pct` — регрессия);
/// - база `0.0` и текущая `0.0` — регрессии нет (замера не было, сравнивать нечего);
/// - база `0.0` и текущая `> 0.0` — **регрессия**: метрика «завелась» с нуля, это и есть
///   деградация (например, селектор вообще не вызвался). Процент от нуля не определён,
///   поэтому `delta_pct` = `f64::INFINITY`.
#[must_use]
pub fn compare(current: &PerfReport, baseline: &PerfReport, threshold_pct: f64) -> Regression {
    let metrics = [
        (
            Metric::ColdStart,
            baseline.cold_start_ms,
            current.cold_start_ms,
        ),
        (Metric::Ingest, baseline.ingest_ms, current.ingest_ms),
        (Metric::Selector, baseline.selector_us, current.selector_us),
    ];
    for (metric, previous, value) in metrics {
        if previous <= 0.0 {
            if value > 0.0 {
                return Regression::Regressed {
                    metric,
                    previous,
                    current: value,
                    delta_pct: f64::INFINITY,
                };
            }
            continue;
        }
        let delta_pct = (value - previous) / previous * 100.0;
        if delta_pct > threshold_pct {
            return Regression::Regressed {
                metric,
                previous,
                current: value,
                delta_pct,
            };
        }
    }
    Regression::None
}

/// Замеряет нагрузку на реальном редьюсере `discord_core::state::apply`.
///
/// Если базовое READY не собралось (внутренняя ошибка фикстуры), в stderr печатается
/// причина, а отчёт содержит нулевые метрики и нулевые счётчики: «успех» с нулями
/// врёт, поэтому такой отчёт ломает тест формы отчёта.
#[must_use]
pub fn measure(workload: &Workload) -> PerfReport {
    match measure_checked(workload) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("discord-perf: базовое READY не разобрано ({error}); замер невозможен");
            PerfReport {
                spec: workload.spec.clone(),
                cold_start_ms: 0.0,
                ingest_ms: 0.0,
                selector_us: 0.0,
                members_loaded: 0,
                messages_ingested: 0,
                digest: 0,
            }
        }
    }
}

/// Замер с явной ошибкой базового состояния.
fn measure_checked(workload: &Workload) -> Result<PerfReport, ReadyError> {
    let spec = &workload.spec;
    let ready = base_ready(spec)?;

    // Копии событий для двух проходов измерения готовятся вне измеряемых интервалов:
    // `apply` забирает событие по значению, а клонирование — артефакт API, а не работа
    // редьюсера.
    let (members, messages) = clone_events(workload);

    // Проход ingest: состояние с участниками, измеряется только применение сообщений.
    let mut state = State::from_ready(&ready);
    for event in members {
        let (next, _effects) = apply(state, event);
        state = next;
    }
    let started = Instant::now();
    for event in messages {
        let (next, _effects) = apply(state, event);
        state = next;
    }
    let ingest_ms = millis(started.elapsed());
    drop(state);

    // Проход холодного старта: состояние строится с нуля.
    let (members, messages) = clone_events(workload);
    let started = Instant::now();
    let mut state = State::from_ready(&ready);
    for event in members {
        let (next, _effects) = apply(state, event);
        state = next;
    }
    for event in messages {
        let (next, _effects) = apply(state, event);
        state = next;
    }
    let cold_start_ms = millis(started.elapsed());

    let members_loaded = count_members(&state);
    let messages_ingested = state.message_count();
    let selector_us = measure_selector(&state, spec);
    let digest = digest_of(workload);

    Ok(PerfReport {
        spec: spec.clone(),
        cold_start_ms,
        ingest_ms,
        selector_us,
        members_loaded,
        messages_ingested,
        digest,
    })
}

/// Разделяет события нагрузки на участников и сообщения, копируя их.
fn clone_events(workload: &Workload) -> (Vec<Event>, Vec<Event>) {
    let mut members = Vec::new();
    let mut messages = Vec::new();
    for event in &workload.events {
        match &event.payload {
            EventPayload::GuildMemberListUpdate(_) => members.push(event.clone()),
            EventPayload::MessageCreate(_) => messages.push(event.clone()),
            _ => {}
        }
    }
    (members, messages)
}

/// Считает участников в состоянии (плоские элементы списка).
fn count_members(state: &State) -> usize {
    state
        .member_lists
        .values()
        .flat_map(|list| list.items.iter())
        .filter(|entry| matches!(entry, MemberListEntry::Member { .. }))
        .count()
}

/// Время read-only выборок из состояния, мкс.
fn measure_selector(state: &State, spec: &WorkloadSpec) -> f64 {
    let list_id = MemberListId::new(PERF_MEMBER_LIST_ID);
    let channel_count = spec.channels.max(1);
    let member_count = spec.members.max(1);
    let message_count = spec.messages.max(1);
    let mut hits = 0usize;

    let started = Instant::now();
    for index in 0..256u64 {
        let id = PERF_MESSAGE_BASE + index % message_count as u64;
        if state.message(id).is_some() {
            hits += 1;
        }
    }
    for index in 0..64u64 {
        let id = PERF_CHANNEL_BASE + index % channel_count as u64;
        if state.channel(id).is_some() {
            hits += 1;
        }
    }
    for index in 0..256u64 {
        let position = (index % member_count as u64) as usize;
        if state
            .member_list_member_at(PERF_GUILD_ID, &list_id, position)
            .is_some()
        {
            hits += 1;
        }
    }
    std::hint::black_box(hits);

    micros(started.elapsed())
}

/// Digest нагрузки: FNV-1a 64 по фиксированному порядку обхода (см. модуль).
fn digest_of(workload: &Workload) -> u64 {
    let mut hash = FNV_OFFSET;
    for event in &workload.events {
        match &event.payload {
            EventPayload::GuildMemberListUpdate(update) => {
                for op in &update.ops {
                    match op {
                        MemberListOp::Sync { items, .. } => {
                            for entry in items {
                                if let MemberListEntry::Member { member, .. } = entry {
                                    hash = fnv_u64(hash, member.user_id);
                                }
                            }
                        }
                        MemberListOp::Insert { item, .. } | MemberListOp::Update { item, .. } => {
                            if let MemberListEntry::Member { member, .. } = item {
                                hash = fnv_u64(hash, member.user_id);
                            }
                        }
                        MemberListOp::Delete { .. } => {}
                    }
                }
            }
            EventPayload::MessageCreate(message) => {
                hash = fnv_u64(hash, message.id);
                hash = fnv_u64(hash, message.channel_id);
                hash = fnv_bytes(hash, message.content.as_bytes());
            }
            _ => {}
        }
    }
    hash
}

/// FNV-1a 64 по байтам.
fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// FNV-1a 64 по `u64` в little-endian.
fn fnv_u64(hash: u64, value: u64) -> u64 {
    fnv_bytes(hash, &value.to_le_bytes())
}

/// Миллисекунды из длительности.
fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Микросекунды из длительности.
fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

/// Сериализация отчёта в JSON.
pub fn report_to_json(report: &PerfReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// Разбор отчёта из JSON.
pub fn report_from_json(text: &str) -> Result<PerfReport, serde_json::Error> {
    serde_json::from_str(text)
}
