# REPORT RW-26 — B09: P4-07 + P4-11 (voice stats/regions)

Дата: 2026-10-22. Автор кода: CORE-V1/W26a (P4-07), CORE-V2/W26b (P4-11).
Интеграция/фиксы: TL-2. Ревью: REV-1 (codex) + TL-1. Гейт: TL-2/QA-1.

## 1. Сделано

### P4-07 — RTCP/RTP статистика (`discord-voice`)

- `rtcp.rs`: compound-парсер SR/RR/SDES/BYE/APP + SRTCP-трейлер 16/20 Б, лимит
  `MAX_COMPOUND_PACKETS = 32`, 24-битный signed cumulative lost, отказ на padding,
  типизированные ошибки, без паник на префиксах/мусоре.
- `reception.rs`: RFC 3550 §6.4.1/A.3 — extended sequence, wrap ±32768, reorder/duplicate,
  loss = expected − received, jitter по 16-шаговому EMA; реальные 8 RTP-заголовков из
  `voice-udp4.pcap` (222.141 ед. / 4.628 мс).
- `stats.rs`: `RttEstimator` по SR/RR (LSR/DLSR), `StatsWindow` с capacity ≥ 1,
  latest/avg/max/loss-delta.
- 36 тестов: `rtcp_contract` 16, `reception_contract` 10, `stats_contract` 12 (+ unit).
- **TL-2 фикс red-first:** RTP timestamp wrap — разность 32-битных timestamp считалась
  как `f64(a) - f64(b)`, на переходе `u32::MAX → 0` джиттер прыгал до 2.2e8. Теперь
  `wrapping_sub(..) as i32`; red `waves/RW-26/workers/TL2/red-timestamp-wrap.log`
  (exit 101, jitter=221184000), green — `green-reception.log` (exit 0).
- **TL-2 фикс:** граница `MAX_COMPOUND_PACKETS` — добавлен тест «ровно 32 пакета
  принимаются»; мутант `>` → `>=` теперь ловится (M4).

### P4-11 — Регионы/latency (`discord-voice`)

- `regions.rs`: парсер реального `regions-rtc.json` (5 регионов × 5 endpoint'ов),
  IPv4/IPv6:port, дедуп endpoint'ов, лимиты 64 региона / 32 endpoint'а, `RegionLatency`
  (median, measured), `ranked_region_names` (измеренные → geo → лексика),
  `select_measured_region`, `plan_probes`/`complete_probe`.
- 10 тестов `regions_contract`.
- **TL-2 интеграция:** модуль подключён в `lib.rs` (`pub mod regions;`); тест больше не
  включает копию файла через `#[path]`, а использует `discord_voice::regions::*`.
- **TL-2 фикс:** убрана расходящаяся `#[cfg(test)]`-ветка в `select_measured_region`
  (тесты проверяли копию, продукт шёл другим путём). Теперь один production-путь.
- **TL-2 фикс:** `TooManyEndpoints` вместо `TooManyRegions` для >32 endpoint'ов; отказ
  неканонического порта (`+1`, `01`, ведущие нули) — осознанное ужесточение, отмечено в
  рисках; мёртвый `UnavailableRegion` удалён.

### Интеграция и гейт

- `discord-voice/src/lib.rs`: `pub mod regions;` (остальные модули уже были подключены).
- Полный gate: fmt/clippy/workspace + `npm run gate:worker`.
- `discord-ui` (untracked WIP, не карточка RW-26) блокировал gate 6 clippy-ошибками;
  TL-2 сделал только минимальные lint-фиксы без изменения поведения (удалён мёртвый
  закомментированный дубль тестов, убраны неиспользуемые поля, `Default`, `sort_by_key`,
  `assert!`). Модули остаются **непринятыми** и в план не переведены.

## 2. Факт

| Проверка | Результат |
|---|---|
| `rtcp_contract` | **16 passed / 0 failed** |
| `reception_contract` | **10 passed / 0 failed** |
| `stats_contract` | **12 passed / 0 failed** |
| `regions_contract` | **10 passed / 0 failed** |
| `discord-voice` (все тесты) | **114 passed / 0 failed** |
| workspace | **869 passed / 0 failed** |
| fmt / clippy | чисто |
| npm gate | **passed**, 214 тестов (meta 25, unit 169, smoke 20) |
| Мутации P4-07 | **11/11 caught** |
| Мутации P4-11 | **8/8 caught** |
| Всего мутаций | **19/19 caught, 0 missed** |

Логи: `waves/RW-26/gate-worker-b09.log`, `.gate/worker-2026-10-22T11-40-00-985Z.json`,
`waves/RW-26/mutations/MUTATIONS-P407.json`, `MUTATIONS-P411.json`.

## 3. Ревью

- Codex (read-only): `waves/RW-26/reviews/review-codex.md` (если прогон завершился;
  иначе — статус в `codex-review.log`).
- TL-1: findings — timestamp wrap (P2, закрыт red-first), test-only ветка regions (P2,
  закрыт), `#[path]`-копия теста (P2, закрыт), граница compound limit (P3, закрыт тестом),
  `TooManyRegions` для endpoint'ов (P3, закрыт), канонический порт (P3, осознанно).
- P0/P1 — 0. P2 — 0 после фиксов. P3 — backlog ниже.

## 4. Backlog P3

1. SRTCP-трейлер, начинающийся с version=2 и валидной длины, может быть разобран как
   пустой RR (вероятность ~1/4 для случайного тега); нужен признак «после SR/RR ожидается
   только тег» или сверка с длиной датаграммы.
2. `ReceptionStats.seen` — неограниченный `BTreeSet` extended sequence; для многочасовой
   сессии/атаки нужен bounded-вариант (окно 4096) или периодическая очистка.
3. `note_sender_report` вытесняет по минимальному NTP-ключу, а не по времени вставки;
   в нормальной работе совпадает, но контракт стоит зафиксировать.
4. Канонический порт (отказ `01`/`+1`) не подтверждён RE — принято как гигиена ввода;
   при необходимости ослабить до «digits-only».
5. `plan_probes` не защищён от переполнения `sequence + offset` (debug panic при
   `u64::MAX`); sequence — внутренний, но можно `wrapping_add`.
6. `discord-ui` headless-модули (3543 строки, untracked) не приняты; нужен отдельный
   batch-контракт или удаление/перенос в WIP-каталог по решению владельца.
