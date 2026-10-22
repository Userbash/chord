# RW-22 / B03 — план волны

Обновлено: 2026-10-21. Статус: **принято** (B03). Контракты были заморожены до кода;
уточнение P7-04 по TZ-W22c зафиксировано в §6.

## 1. Состав

| Карточка | Что закрываем | Тесты | Файлы (владелец) |
|---|---|---|---|
| P3-15 | i18n runtime base: xxh64→base64[0..6], выбор locale, en/ru, AST, fallback, rich-теги/базовый markdown | T-I18N-01…03 | `rust/crates/discord-i18n/**` (W22a) |
| P4-01 | RTC signaling headless: op 0/1/2/4/8, Identify, Ready, Session Description, region selection, `VOICE_SERVER_UPDATE`→endpoint | T-VOICE-01…03 | `rust/crates/discord-voice/**` (W22b) |
| P7-04 | read-only API surface: quests/promotions/entitlements/library/saved messages, только GET, точные пути без query | T-REST-09 | `rust/crates/discord-core/src/rest/resources.rs`, `rest/mod.rs`, `tests/rest_resources.rs`, `discord-core/Cargo.toml` (W22c) |
| P8-03 | CI: fmt/clippy/test/deny-if-available/bench/release artifact, без сети по умолчанию | T-HARN-06 | `scripts/ci.sh`, `scripts/release.sh`, `rust/deny.toml`, `.github/workflows/ci.yml`, `rust/crates/discord-codec/examples/etf_bench.rs` (C) |
| foundation | строгий JSON-парсер без внешних зависимостей для i18n/voice/API | unit + реальные каталоги/кадры | `rust/crates/discord-json/**`, `rust/Cargo.toml` (C) |

## 2. Файловая матрица (пересечения запрещены)

- **C / integration lane:** `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/crates/discord-json/**`,
  `scripts/**`, `.github/workflows/**`, `rust/deny.toml`,
  `rust/crates/discord-codec/examples/etf_bench.rs`, доки и журнал после приёмки.
- **W22a:** только `rust/crates/discord-i18n/**` (включая его `Cargo.toml`).
- **W22b:** только `rust/crates/discord-voice/**` (включая его `Cargo.toml`).
- **W22c:** только `rust/crates/discord-core/src/rest/resources.rs`, `rest/mod.rs`,
  `rust/crates/discord-core/tests/rest_resources.rs`, `rust/crates/discord-core/Cargo.toml`.
- Общие `discord-core/src/lib.rs`, `state/*`, `PLAN.md`, `STATUS.md`, `ROADMAP.md`, `TASKS.md`,
  `docs/map.md`, `waves/*` воркеры не трогают.

## 3. Контракты foundation

`discord-json` — строгий RFC 8259 parser, без `unsafe`, без паник и без внешних зависимостей.

```rust
pub const MAX_DEPTH: usize = 128;
pub const MAX_NODES: usize = 4_000_000;

pub enum JsonValue { Null, Bool(bool), Number(JsonNumber), String(String),
                     Array(Vec<JsonValue>), Object(BTreeMap<String, JsonValue>) }
pub struct JsonNumber(String); // raw, as_i64/as_u64/as_f64
pub struct JsonError { pub offset: usize, pub kind: JsonErrorKind }
pub fn parse(input: &str) -> Result<JsonValue, JsonError>;
```

Обязательно: отклонять duplicate keys, control-символы в строках, неверные escape/surrogate,
leading zero, `+1`, `.5`, `1.`, trailing data; ограничивать глубину и число узлов; сохранять
unicode (включая `\uD83D\uDE00`). Ошибка содержит байтовое смещение и не раскрывает вход.

## 4. Контракты P3-15

- `xxh64(&[u8], seed) -> u64`, `message_key(&str) -> String` (6 символов base64 от LE-digest).
- `LocaleCode` / `resolve_locale(requested, available, default)`: exact → case-insensitive →
  язык (`ru-RU`→`ru`) → default → `en-US`; отключённые (`pt-PT, sk, he, ar, id`) не выбираются.
- `Catalog::parse(&JsonValue)` — map key→AST; `I18n` хранит locale→catalog, умеет `translate`.
- AST узлы: literal(0), argument(1), number(2), date(3), time(4), select(5), plural(6),
  pound(7), tag(8). Для base обязательны literal/argument/plural/tag/pound; date/time/number
  могут возвращать типизированную ошибку, но не паниковать.
- Fallback: requested → default(en-US) → ошибка/пустая строка с диагностикой. Не подменять
  отсутствующий аргумент пустой строкой.
- Rich tag в i18n: `$b`, `$i`, `$del`, `$code`, `$link`, `$p` → markdown; неизвестный `$tag` —
  ошибка; hook-теги без `$` рендерят детей (без исполнения колбэков).
- Markdown messages: безопасный AST/render для `**`, `*`/`_`, `__`, `~~`, `||`, `` ` ``, fenced
  code, `[t](u)`, escape; HTML не интерпретируется.

## 5. Контракты P4-01

- `VoiceOp` (u8) 0/1/2/3/4/6/8/15/16; неизвестный op — ошибка/диагностика, не паника.
- `VoiceFrame::parse(&str)` через `discord-json`; payload `d` обязателен для известных ops.
- `Identify` op0: server/channel/user/session/token, `max_dave_protocol_version=1`, video,
  streams `rid="100"`, `quality=100`.
- `SelectProtocol` op1: protocol `udp`, address/port/mode, codecs opus PT120 priority1000 и AV1.
- `Ready` op2: ssrc/port/ip/modes/experiments/streams; `SessionDescription` op4: modes,
  `secret_key` 32 bytes (в Debug — `<redacted>`), media session id, DAVE/secure versions,
  codecs. Секрет не попадает в `Debug`/`Display`/логи.
- `VOICE_SERVER_UPDATE` (`token`, `guild_id`, `endpoint`) → endpoint `wss://<endpoint>/?v=9`
  (или `wss://` уже с схемой), без выполнения сети; пустой/подозрительный endpoint — ошибка.
- `select_region`: ранжирование по latency (меньше — лучше), tie-break по `geo_ordered_rtc_regions`,
  затем лексикографически; без измерений — geo/default; ручной выбор валидируется.
- Только headless: никаких DNS/TCP/UDP/сети в коде и тестах.

## 6. Контракты P7-04

- `ResourceKind`: Quest, QuestDecision, QuestGetDecisions, Promotion, Entitlements,
  ApplicationEntitlements, Library, SavedMessages.
- `ReadOnlyResource { kind, path_template, observed_statuses }`; каталог содержит только
  подтверждённые D-02 GET-маршруты, а GET-принадлежность проверяется по `routes::ROUTES`
  (отдельного write-метода в структуре нет). `ResourceQuery` **не вводится**: D-02 не
  подтверждает query у этих восьми маршрутов (уточнение TZ-W22c).
- `path`: id только ASCII-цифры 1..=20; для `ApplicationEntitlements` обязателен, для
  остальных — `UnexpectedId`; missing/invalid/extra id — типизированная ошибка.
- saved messages: только чтение, без выдуманной пагинации; 404 из capture не превращается в
  «есть данные».
- Абсолютные URL не строятся; путь относительный, без `/api/v9`, без query — как в
  `routes::ROUTES` (уточнение TZ-W22c).

## 7. Контракты P8-03

- `scripts/ci.sh`: `fmt → clippy → test → bench-example → release-build`; `cargo-deny` только
  если бинарь установлен, иначе явный skip с причиной; `--dry-run` печатает план без запуска;
  сеть не используется; коды выхода 0/1.
- `scripts/release.sh`: release build, `sha256sum`, binary в `dist/`; не публикует.
- CI workflow повторяет локальный план, без секретов и live.
- Метрика производительности — `etf_bench` example; порог регрессии 10% — P7-02/T-HARN-04,
  не выдумывается в этой карточке.

## 8. Red-first и приёмка

1. Каждый воркер пишет тесты первыми, сохраняет red-лог с exit 101 в `waves/RW-22/`.
2. Targeted-прогоны — через `flock /tmp/opencode/discord-cargo.lock cargo test --offline -p <crate>`.
3. Мутации — по каждой карточке, минимум 2 на смысловой контракт; логи в `waves/RW-22/mutations/`.
4. Один общий `gate:worker` после интеграции; независимое review Codex CLI (read-only).
5. Только после accepted — синхронизация `STATE/STATUS/ROADMAP/TASKS/map` и `waves/db.jsonl`.
