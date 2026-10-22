# TZ-W26a — P4-07: RTCP-статистика и приём RTP (крейт `discord-voice`)

Ты — **W26a** (seat CORE-V1), кодовый воркер волны B07/RW-26. Пиши **только** файлы §1.
`src/lib.rs`, `Cargo.toml`, доки, `waves/*`, чужие модули — не трогать (интеграцию
`pub mod` делает TL-2). Сначала red-тесты (зафиксируй команду и exit code), затем green.

## 1. Файлы

Пишешь:
- `rust/crates/discord-voice/src/rtcp.rs`
- `rust/crates/discord-voice/src/reception.rs`
- `rust/crates/discord-voice/src/stats.rs`
- `rust/crates/discord-voice/tests/rtcp_contract.rs`
- `rust/crates/discord-voice/tests/reception_contract.rs`
- `rust/crates/discord-voice/tests/stats_contract.rs`

Читаешь: `src/lib.rs`, `src/udp.rs`, `waves/RW-26/research/RE-VOICE-STATS.md`,
`waves/RW-26/research/fixtures/`, `docs/re/voice-deep.md` §3.

Запрещено: `unsafe`, внешние зависимости, сеть, golden, `git commit/push`, полный
workspace/clippy/gate (это batch-лейн).

## 2. Оракул (RE, capture s3)

- `waves/RW-26/research/fixtures/rtcp-voice-udp4-server.bin` — 20 реальных RTCP-пакетов
  сервера с 4-байтным BE-префиксом длины; каждый = `[RTCP 32 Б][SRTCP tag 16 Б]`.
- `waves/RW-26/research/fixtures/rtcp-voice-udp4-server.json` — ожидаемый разбор
  (SR/RR, RC, length, trailer_len=16). **Тело report block зашифровано** — значения
  блоков из capture не проверяются; для них только RFC-векторы.
- `waves/RW-26/research/fixtures/rtp-udp4-client.json` — 8 открытых RTP-заголовков
  (SSRC 9208, PT 120, seq 17466…17473, ts +960, приходы) и оракул джиттера
  **222.141** ед. / **4.628 мс**, loss 0.

Ключевые факты:
1. RTCP-длина = `(length_words + 1) * 4`; auth tag в неё не входит.
2. SRTCP-трейлер: 16 Б (RR) или 20 Б (SR + counter 4 LE) — парсер возвращает
   `trailer_len`, а не ошибку.
3. `parse_compound` не должен паниковать на мусоре; первый пакет с `V != 2` — ошибка,
   последующий — начало трейлера.
4. RTP-заголовок открыт в `*_rtpsize`: 12 Б фикс + optional ext; для статистики
   достаточно seq/ts/ssrc/PT.

## 3. Замороженный API (сигнатуры буквально)

### `src/rtcp.rs`

```rust
pub const RTCP_SR: u8 = 200;
pub const RTCP_RR: u8 = 201;
pub const RTCP_SDES: u8 = 202;
pub const RTCP_BYE: u8 = 203;
pub const RTCP_APP: u8 = 204;
pub const MAX_COMPOUND_PACKETS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: i32,
    pub extended_highest_seq: u32,
    pub jitter: u32,
    pub last_sr: u32,
    pub delay_since_last_sr: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderInfo {
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpPacket {
    SenderReport { sender_ssrc: u32, info: SenderInfo, reports: Vec<ReportBlock> },
    ReceiverReport { sender_ssrc: u32, reports: Vec<ReportBlock> },
    /// SDES/BYE/APP и неизвестные PT — прозрачное тело без интерпретации.
    Other { payload_type: u8, report_count: u8, body: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compound {
    pub packets: Vec<RtcpPacket>,
    pub trailer_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcpError {
    Truncated { needed: usize, available: usize },
    InvalidVersion(u8),
    InvalidLength { payload_type: u8, length_words: u16 },
    TooManyPackets,
    UnsupportedPadding,
}

pub fn parse_compound(bytes: &[u8]) -> Result<Compound, RtcpError>;
```

Правила:
- пустой вход → `Ok(Compound { packets: vec![], trailer_len: 0 })`;
- `needed` в `Truncated` — полный размер пакета по заголовку;
- `InvalidLength` — если заявленная длина меньше минимума для PT (SR < 28, RR < 8,
  прочие < 4) или больше остатка при `trailer_len ∉ {16, 20}`;
- если `V == 2`, но длина выходит за остаток и остаток 16/20 — это трейлер, не ошибка;
- P-бит (padding) → `UnsupportedPadding` (RFC-паддинг не поддерживаем в этой карточке);
- лимит `MAX_COMPOUND_PACKETS`, затем `TooManyPackets`.

### `src/reception.rs`

```rust
pub const OPUS_CLOCK_RATE: u32 = 48_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    pub payload_type: u8,
    pub marker: bool,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

/// None — не RTP (V != 2 или короче 12 Б).
pub fn parse_rtp_header(bytes: &[u8]) -> Option<RtpHeader>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    InOrder,
    Reordered,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceptionSnapshot {
    pub received: u64,
    pub expected: u64,
    pub lost: u64,
    pub duplicates: u64,
    pub highest_sequence: Option<u16>,
    pub jitter_units: f64,
    pub jitter_ms: f64,
}

#[derive(Debug, Clone)]
pub struct ReceptionStats { /* приватные поля */ }

impl ReceptionStats {
    pub fn new(clock_rate: u32) -> Self;
    pub fn observe(&mut self, header: &RtpHeader, arrival_ms: f64) -> Observation;
    pub fn snapshot(&self) -> ReceptionSnapshot;
}
```

Правила:
- loss — RFC 3550 A.3: `expected = highest_ext - base_ext + 1`, `lost = expected - received`
  (saturating), wrap 16-битного seq и реордеринг учитываются;
- duplicate не увеличивает `received`;
- jitter — RFC 3550 §6.4.1: `D = (R - R_prev) - (S - S_prev)`,
  `J += (|D| - J) / 16`, где `R` — приход в тиках часов; вычитай базовый приход, чтобы
  не терять точность на больших timestamp;
- `clock_rate == 0` → `new` не паникует: считать джиттер в мс как `D` в мс (или вернуть
  `jitter_ms = 0`; выбери и задокументируй).

### `src/stats.rs`

```rust
use crate::rtcp::ReportBlock;

pub fn ntp_middle_32(ntp_timestamp: u64) -> u32;

#[derive(Debug, Clone, Default)]
pub struct RttEstimator { /* приватные поля */ }

impl RttEstimator {
    pub fn new() -> Self;
    pub fn note_sender_report(&mut self, ntp_middle: u32, sent_at_ms: u64);
    pub fn on_report_block(&mut self, block: &ReportBlock, received_at_ms: u64) -> Option<f64>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatsSample {
    pub at_ms: u64,
    pub rtt_ms: Option<f64>,
    pub fraction_lost: f64,
    pub cumulative_lost: i64,
    pub jitter_ms: f64,
}

#[derive(Debug, Clone)]
pub struct StatsWindow { /* приватные поля */ }

impl StatsWindow {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, sample: StatsSample);
    pub fn latest(&self) -> Option<&StatsSample>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn average_rtt_ms(&self) -> Option<f64>;
    pub fn max_jitter_ms(&self) -> Option<f64>;
    pub fn loss_delta(&self) -> Option<i64>;
}
```

Правила:
- RTT = `(received_at_ms - sent_at_ms) - dlsr / 65.536`; отсутствующий LSR (0) или
  неизвестный SR → `None`; отрицательный RTT клампится в `0.0`;
- `StatsWindow::new(0)` трактуется как 1 (без паники);
- `loss_delta` = последний `cumulative_lost` − первый в окне; `< 2` сэмплов → `None`;
  отрицательная дельта (перезапуск потока) возвращается как есть — не «исправляй».

## 4. Тесты (red-first)

Файлы тестов — только свои. Используй реальные фикстуры через
`include_bytes!("../../../waves/RW-26/research/fixtures/...")` **нельзя** (путь вне
крейта) — скопируй нужные байты/числа в тест как hex-константы с провенансом в
комментарии (md5/sha256 артефакта и команда `python3 waves/RW-26/research/rtcp-extract.py`).
Либо читай файл по абсолютному пути в тесте — но тогда тест не переносим; предпочтительны
hex-константы.

Обязательные кейсы (16–24):
1. реальный RR: 32 Б + tag → 1 RR, RC=1, SSRC `0x000023f8`, `trailer_len=16`;
2. реальный SR: 28 Б + tag+counter → 1 SR, NTP/счётчики, `trailer_len=20`;
3. синтетический RR: fraction 64, cumulative −1, EHS `0x0001_0000`, jitter `0x0000_1234`,
   LSR/DLSR — точные значения;
4. compound RR+SDES → 2 пакета, SDES в `Other`;
5. V=1 → `InvalidVersion`; обрезанный заголовок → `Truncated`; P-бит → `UnsupportedPadding`;
6. `parse_rtp_header`: 8 реальных заголовков (12 Б минимум, ext игнорируется);
7. `ReceptionStats` на фикстуре: jitter 222.141 ± 0.01, jitter_ms 4.628 ± 0.001, loss 0;
8. wrap seq `65534,65535,0,1` → loss 0; пропуск `65534,0` → lost 1;
9. reorder `1,3,2,4` → InOrder/Reordered/Reordered/InOrder, loss 0;
10. duplicate → `Duplicate`, received не растёт;
11. RTT: SR в 1000 мс, RR в 1100 мс, DLSR 6554 → 0 мс; без DLSR → 100 мс;
    LSR=0 → None; неизвестный LSR → None; отрицательный → 0.0;
12. окно: capacity 3, 4 сэмпла → len 3, latest, average/max, loss_delta.
13. `ntp_middle_32` = младшие 32 бита старшего слова NTP.

## 5. Сдача

- red-лог: `waves/RW-26/workers/W26a/red.log` (команда + exit 101);
- green-лог: `waves/RW-26/workers/W26a/green.log`;
- md5 всех файлов; риски/вопросы — в отчёт `waves/RW-26/workers/W26a/REPORT.md`
  (шаблон `waves/_TEMPLATES/WORKER-REPORT.md`).
