# TZ-W26b — P4-11: регионы и латентность (крейт `discord-voice`)

Ты — **W26b** (seat CORE-V2), кодовый воркер волны B07/RW-26. Пиши **только** файлы §1.
`src/lib.rs`, `Cargo.toml`, доки, `waves/*`, чужие модули — не трогать. Сначала red-тесты
(зафиксируй команду и exit code), затем минимальный green.

## 1. Файлы

Пишешь:
- `rust/crates/discord-voice/src/regions.rs`
- `rust/crates/discord-voice/tests/regions_contract.rs`

Читаешь: `src/lib.rs` (там уже есть `select_region` и `VoiceError`),
`waves/RW-26/research/RE-REGIONS.md`, `waves/RW-26/research/fixtures/regions-rtc.json`,
`docs/re/voice-deep.md` §1.

Запрещено: `unsafe`, внешние зависимости, сеть, golden, `git commit/push`, полный
workspace/clippy/gate, правка `lib.rs`.

## 2. Оракул (RE)

`waves/RW-26/research/fixtures/regions-rtc.json` (716 Б, md5
`ffe52792025bf702696cfa38db06995f`) — точный массив, который Discord передаёт в
`RegionLatencyTester::PerformTest`:

```json
[{"region":"frankfurt","ips":["104.29.146.223:19308", ...5 шт]},
 {"region":"brussels",...}, {"region":"rotterdam",...}, {"region":"paris",...},
 {"region":"milan",...}]
```

`geo_ordered_rtc_regions` из READY:
`["frankfurt","brussels","rotterdam","paris","milan"]`.

Реальный результат ранжирования (`engine-log-session1.txt:2`):
`frankfurt,rotterdam,paris,milan,brussels` — измеренная задержка важнее geo-порядка.

**Честная граница:** payload latency-пробы и метрика ранжирования Discord в дампе не
подтверждены. Не изобретай wire-байты. Модель самосогласована и помечена в doc-comment
как «not wire-confirmed».

## 3. Замороженный API (сигнатуры буквально)

```rust
use crate::VoiceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionEndpoint {
    pub region: String,
    pub ips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionError {
    Empty,
    InvalidJson,
    InvalidShape,
    MissingRegion,
    MissingIps,
    DuplicateRegion(String),
    InvalidEndpoint(String),
    TooManyRegions,
    NoEndpoints,
    UnavailableRegion(String),
}

/// Строгий парсер JSON-массива `[{region, ips:[host:port,...]}]`.
/// - пустой массив → Empty; дубликат региона → DuplicateRegion;
/// - endpoint обязан быть `IPv4:port` или `[IPv6]:port`, порт 1..=65535, без схемы;
/// - больше 64 регионов или больше 32 endpoint'ов на регион → TooManyRegions;
/// - дубликаты endpoint внутри региона схлопываются (порядок сохраняется).
pub fn parse_rtc_regions(json: &str) -> Result<Vec<RegionEndpoint>, RegionError>;

#[derive(Debug, Clone, Default)]
pub struct RegionLatency { /* приватные поля */ }

impl RegionLatency {
    pub fn new() -> Self;
    pub fn record(&mut self, region: &str, rtt_ms: u64);
    pub fn median_ms(&self, region: &str) -> Option<u64>;
    pub fn samples(&self, region: &str) -> &[u64];
    /// Пары (region, медиана) в порядке возрастания медианы; без сэмплов — пусто.
    pub fn measured(&self) -> Vec<(String, u32)>;
}

/// Ранжирование: измеренные регионы по медиане (возрастание), затем geo-порядок,
/// затем лексикографический порядок. Регионы без измерений — после измеренных,
/// в geo-порядке (если есть в geo), иначе лексикографически.
pub fn ranked_region_names(latency: &RegionLatency, geo: &[String]) -> Vec<String>;

/// Обёртка над crate::select_region: manual проверяется по объединению geo и измеренных;
/// при равенстве медиан — geo-порядок, затем лексика.
pub fn select_measured_region(
    latency: &RegionLatency,
    geo: &[String],
    manual: Option<&str>,
) -> Result<String, VoiceError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionProbe {
    pub region: String,
    pub endpoint: String,
    pub sequence: u64,
    pub sent_at_ms: u64,
}

/// Детерминированный план проб: для каждого региона (в порядке входа) выбирается
/// endpoint `endpoints[sequence as usize % len]`; `max_per_region` ограничивает число
/// проб на регион (0 → NoEndpoints не нужен, просто пустой план).
pub fn plan_probes(
    regions: &[RegionEndpoint],
    sequence: u64,
    now_ms: u64,
    max_per_region: usize,
) -> Vec<RegionProbe>;

/// Завершение пробы: Some(rtt_ms), если ответ пришёл в пределах timeout_ms;
/// None — таймаут/ответ раньше отправки.
pub fn complete_probe(probe: &RegionProbe, received_at_ms: u64, timeout_ms: u64) -> Option<u64>;
```

## 4. Тесты (red-first, 10–16)

1. реальный fixture: 5 регионов, по 5 endpoint'ов, порядок сохранён;
2. дубликат региона → `DuplicateRegion`;
3. битый JSON / не массив / элемент без `region` / без `ips` → соответствующие ошибки;
4. endpoint без порта, порт 0, порт 70000, схема `udp://` → `InvalidEndpoint`;
5. IPv6 `[::1]:19308` валиден; дубликат endpoint схлопывается;
6. `median_ms`: чётное/нечётное число сэмплов, порядок не важен;
7. ранжирование по engine-логу: RTT `frankfurt=45, rotterdam=55, paris=60, milan=70,
   brussels=90` при geo `frankfurt,brussels,rotterdam,paris,milan` →
   `frankfurt,rotterdam,paris,milan,brussels`;
8. равенство медиан → geo-порядок; регион вне geo → лексика;
9. регион без измерений идёт после измеренных;
10. `select_measured_region`: manual доступен/недоступен → Ok/Err;
11. `plan_probes`: sequence 0..2 выбирает 0-й/1-й/2-й endpoint по кругу;
12. `plan_probes` с `max_per_region=2` даёт по 2 пробы на регион;
13. `complete_probe`: rtt, таймаут, `received < sent` → None.

Данные fixture копируй в тест как строковую константу с комментарием провенанса
(путь, md5) — не читай файл из `waves/` во время теста.

## 5. Сдача

- red-лог: `waves/RW-26/workers/W26b/red.log` (команда + exit 101);
- green-лог: `waves/RW-26/workers/W26b/green.log`;
- md5 файлов и краткий отчёт `waves/RW-26/workers/W26b/REPORT.md`.

Команда targeted (свой target-dir, чтобы не блокировать соседний лейн):

```sh
cd rust
CARGO_TARGET_DIR=/home/sanya/.cache/discord-lab-target-rw26b \
  cargo test --offline -p discord-voice --test regions_contract
```
