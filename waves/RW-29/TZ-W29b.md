# TZ W29b — P4-04 RTP-медиа: заголовок/расширение, DTX/CN, джиттер-буфер (headless)

Волна: RW-29. Дата: 2026-10-22. Крейт: `discord-voice`. Сеть/live/Opus-SDK не используются:
контракт — байтовый RTP-слой и детерминированный буфер. Оракул — golden pcap (только чтение).

## 1. Замороженный контракт

```rust
pub const OPUS_PAYLOAD_TYPE: u8 = 120;
pub const OPUS_CLOCK_RATE: u32 = 48_000;
pub const FRAME_DURATION_MS: u32 = 20;
pub const FRAME_TICKS: u32 = 960;
pub const EXTENSION_PROFILE: u16 = 0xBEDE;

pub struct RtpHeader { pub marker: bool, pub payload_type: u8, pub sequence: u16, pub timestamp: u32, pub ssrc: u32 }
pub struct RtpExtension<'a> { pub profile: u16, pub data: &'a [u8], pub standard: bool }
pub struct RtpPacket<'a> { pub header: RtpHeader, pub extension: Option<RtpExtension<'a>>, pub payload: &'a [u8] }

pub enum RtpError { TooShort, BadVersion, TruncatedExtension, PaddingOutOfRange, ExtensionNotAligned }
impl RtpError { pub fn as_str(&self) -> &'static str; }

pub fn parse_rtp(bytes: &[u8]) -> Result<RtpPacket<'_>, RtpError>;

pub struct RtpSender { /* ssrc, sequence, timestamp, marker_pending */ }
impl RtpSender {
    pub fn new(ssrc: u32) -> Self;                                  // seq=0, ts=0, marker_pending=true
    pub fn sequence(&self) -> u16;
    pub fn timestamp(&self) -> u32;
    pub fn next_packet(&mut self, payload: &[u8], ext: Option<&[u8]>) -> Result<Vec<u8>, RtpError>;
    pub fn silence(&mut self, elapsed_frames: u32);                 // DTX: ts += 960 * frames, seq не растёт
}

pub enum AudioFrameKind { Speech, ComfortNoise, Empty }
pub fn classify_opus_payload(encrypted_len: usize) -> AudioFrameKind; // 0 → Empty, 1..=3 → ComfortNoise, иначе Speech

pub enum PushOutcome { Queued, Duplicate, Late, Overflow }
pub enum PopOutcome { Frame { sequence: u16, payload: Vec<u8> }, Missing { sequence: u16 }, Empty }
pub struct JitterStats { pub received: u64, pub duplicates: u64, pub late: u64, pub missing: u64, pub overflow: u64 }

pub struct JitterBuffer { /* ... */ }
impl JitterBuffer {
    pub fn new(capacity: usize) -> Self; // capacity == 0 трактуется как 1
    pub fn capacity(&self) -> usize;
    pub fn buffered(&self) -> usize;
    pub fn stats(&self) -> JitterStats;
    pub fn push(&mut self, sequence: u16, payload: Vec<u8>) -> PushOutcome;
    pub fn pop(&mut self) -> PopOutcome;
}
```

Инварианты:

- `RtpHeader::encode` (свободная функция или метод — на усмотрение, но с той же раскладкой):
  байт 0 `0x90` (V=2, P=0, X=1, CC=0), байт 1 `marker<<7 | payload_type`, затем `sequence`,
  `timestamp`, `ssrc` big-endian. При `ext = Some(data)`: `profile=0xBEDE` BE, `words = len/4` BE,
  затем данные. `len % 4 != 0` или `len > 60` → `Err(ExtensionNotAligned)`.
- `parse_rtp`: V==2 обязателен (`BadVersion`), длина ≥ 12; при X=1 читать 4 байта расширения;
  `words*4` не должно выходить за границу (`TruncatedExtension`) — **кроме нестандартного профиля
  (≠0xBEDE)**: там `words` длиной не является, весь хвост отдаётся как opaque `data`, payload пуст
  (зафиксировано по ревью, см. §6); профиль ≠ 0xBEDE допустим, но `standard=false`; P=1 — снять
  паддинг по последнему байту (0 или > payload → `PaddingOutOfRange`). CC>0 — пропустить 4*CC байт
  CSRC (границы проверяются).
- `RtpSender::next_packet`: собирает пакет с `marker = marker_pending`, затем `marker_pending=false`,
  `sequence = sequence.wrapping_add(1)`, `timestamp = timestamp.wrapping_add(960)`; следующий вызов
  после `silence()` снова ставит marker. `silence(n)` двигает только timestamp (`960*n`), marker
  ставится на следующий пакет, seq не растёт.
- `classify_opus_payload`: порог 3 байта — эмпирика захвата (шифротекст CN = 3 Б, см. §2).
- `JitterBuffer`: окно по модулю 2^16, «впереди» = `wrapping_sub` < 32768. Первый `push` задаёт
  ожидаемый seq. `Duplicate` — seq уже в буфере или уже отдан. `Late` — seq позади ожидаемого
  (уже отдан/пропущен). `Overflow` — буфер полон (новый пакет не принимается, счётчик растёт).
  `pop`: если в буфере есть ожидаемый — отдать и сдвинуть окно через пропуски (каждый пропуск
  `Missing` + `stats.missing += 1`); если буфер пуст — `Empty`. Паники/`unwrap` в продакшн-пути нет.

## 2. Данные и оракул

`fixtures/voice/rtp-pt120-oracle.json` (уже сгенерирован `waves/RW-29/gen-rtp-oracle.py` из
`/home/sanya/re-deep/voice/voice-udp4.pcap`, golden, только чтение): 10 пакетов PT120
(8 валидных с `0xBEDE`/2 слова/23 Б остатка/счётчики `0x80000000|n`, 2 аномальных 43 Б без
валидного расширения), seq `17464..17473`, дельты ts `960`, marker только на первом.

Источник фактов: `docs/re/voice-deep.md` §2.3, `docs/rust-client/FEATURES.md` F-VOICE-08.
Фикстуру и генератор не менять; при необходимости дополнить — только через TL.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| m01 | parse 8 валидных пакетов | seq/ts/ssrc/marker/PT, profile 0xBEDE, words 2, payload 23 |
| m02 | аномальные 2 пакета | `standard=false`, профиль не 0xBEDE, без ошибки |
| m03 | encode заголовка без расширения | 12 Б, `0x80|PT`, BE-поля |
| m04 | encode с расширением 8 Б | 24 Б: `0xBEDE`, words=2, данные дословно |
| m05 | encode ext не кратен 4 / >60 Б | `ExtensionNotAligned` |
| m06 | parse: короткий/битый V/обрезанное расширение | `TooShort`/`BadVersion`/`TruncatedExtension` |
| m07 | parse: P=1 корректный и P>payload | payload без паддинга / `PaddingOutOfRange` |
| m08 | sender: seq/ts инкремент, marker один раз | seq+1, ts+960, marker на первом |
| m09 | sender: `silence(3)` → следующий пакет | ts+2880, seq+1, marker=1 |
| m10 | classify: 0/1/3/4 Б | Empty/ComfortNoise/ComfortNoise/Speech |
| m11 | jitter: реордер 3→1→2 | выдача 1,2,3 |
| m12 | jitter: дубль/опоздавший/потеря | Duplicate/Late/Missing один раз, затем продолжение |
| m13 | jitter: wrap 65535→0 и capacity=1 | порядок сохранён, overflow считается |

Бюджет: 12–16 тестов; мутации 7–9.

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29b cargo test --offline -p discord-voice --test media_contract
```

Полный gate — один раз в integration-лейне (BATCH-TZ §4).

## 5. DoD

red-лог с exit code → green → мутации (продукт восстановлен по md5) → отчёт
`waves/RW-29/REPORT-W29b.md` → ревью → accepted.

## 6. Уточнения по итогам ревью (2026-10-22)

- **P3-1 (нестандартный профиль).** В golden-capture аномальные 43-байтные пакеты имеют профиль
  `0x19f5`/`0x2d6d` и поле `words` 59783/28435 при 43 байтах — это не длина. Правило: для профиля
  ≠ `0xBEDE` весь остаток пакета трактуется как opaque `extension.data`, `payload` пуст,
  ошибки нет. Для `0xBEDE` выход `words*4` за границу — `TruncatedExtension`.
- **P3-2 (окно джиттера).** До первой выдачи пакет позади старта расширяет окно назад (нужно для
  реордера 3→1→2); после первой выдачи граница окна = фронт доставки, seq позади неё — `Late`
  (или `Duplicate`, если уже отдан и ещё в истории 512 seq).
- Остальные P3 ревью (история дублей, мутационные скрипты, пробелы тестов) — в бэклоге
  `waves/STATE.md` §7 без переделки продукта.
