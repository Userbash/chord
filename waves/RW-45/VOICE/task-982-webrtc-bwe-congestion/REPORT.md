# Отчёт воркера — TASK-982: Контроль перегрузки WebRTC BWE (GCC, REMB/TWCC 500..8000 kbps)

- **Карточка / Задача**: `TASK-982` (`task_079b1f5fbabe`, спринт `Sprint 4: Screen Share & Video Pipeline`)
- **Отдел**: `G5 Voice / Platform` (`webrtc-bwe-congestion`)
- **Дата**: `2026-11-11`

## 1. Сделано
1. По **Strict TDD** сначала создан контрактный тест [`tests/unit/webrtc-bwe-congestion-982.test.mjs`](file:///home/sanya/orca/project/tests/unit/webrtc-bwe-congestion-982.test.mjs) (6 подтестов `SUB-982-0..SUB-982-5`) и зафиксирован `RED`-прогон (`0 pass / 6 fail`) в [`red.log`](file:///home/sanya/orca/project/waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/red.log).
2. Разработан модуль контроля перегрузки сети [`rust/discord-voice/src/congestion.rs`](file:///home/sanya/orca/project/rust/discord-voice/src/congestion.rs) (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, `< 50` строк на функцию, `792 < 800` строк на файл):
   - Кодирование и разбор пакетов `RTCP REMB` (`PT=206, FMT=15, "REMB"`, 6-бит экспонента + 18-бит мантисса, список SSRC).
   - Кодирование и разбор пакетов `RTCP TWCC` (`PT=205, FMT=15`, `draft-holmer-rmcat-transport-wide-cc-extensions-01`, Run-Length и 1/2-бит Status-Vector чанки, дельты по `250 мкс`, корректный переход `u16` sequence `65535 -> 0`).
   - Контроллер `GccCongestionController` (Trendline фильтр градиента задержки + AIMD в диапазоне `500..=8000 kbps`, ограничение максимального снижения битрейта за один шаг `<= 15%` без разрыва или переподключения `connection_alive == true`, `reconnect_count == 0`).
   - Профили разрешения `VideoResolutionTier` (`1080p60`, `1080p30`, `720p30`, `480p20`), интерактивные состояния `bwe-graph` (`probing`, `limit bandwidth`, `adapted bitrate`), маскирование секретов `redact_rtc_secrets` и очистка ключевого материала `zeroize_key_material`.
3. Разработан скрипт верификации [`scripts/verify-bwe.mjs`](file:///home/sanya/orca/project/scripts/verify-bwe.mjs) (`// @ts-check`, коды выхода `0 / 1 / 2`, сверка с RE-оракулами `docs/re/native-re.md`, `docs/re/voice-deep.md`, `fixtures/voice/rtc-frames.jsonl`, `waves/kanban-board.json`).

## 2. Red-прогон (Strict TDD)
- Команда: `node --test tests/unit/webrtc-bwe-congestion-982.test.mjs`
- Лог: [`waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/red.log`](file:///home/sanya/orca/project/waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/red.log)
- Результат: `tests 6 | pass 0 | fail 6` (до создания продуктовых файлов).

## 3. Green-прогон и Targeted проверки
- Команда: `node --test tests/unit/webrtc-bwe-congestion-982.test.mjs`
- Лог: [`waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/green.log`](file:///home/sanya/orca/project/waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/green.log)
- Результат: `tests 6 | pass 6 | fail 0` (`SUB-982-0..SUB-982-5` зелёные, включая `4/4` внутренних Rust-тестов `rustc --test --edition 2024 -D warnings` и `5/5` мутационный гейт).
- CLI верификация: `node scripts/verify-bwe.mjs --out-json waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/bwe-summary.json --out-md waves/RW-45/QUALITY/WEBRTC-BWE-REPORT.md` -> код `0`.

## 4. Файлы и контрольные суммы MD5
| Файл | MD5 |
|---|---|
| `rust/discord-voice/src/congestion.rs` | `5447cc03a9806dbefa3ceef351bd633e` |
| `scripts/verify-bwe.mjs` | `6ebf68704ce37a28001283f23ecb3a56` |
| `tests/unit/webrtc-bwe-congestion-982.test.mjs` | `8156a8416d99b750a8351027c03128e5` |
| `waves/RW-45/VOICE/task-982-webrtc-bwe-congestion/bwe-summary.json` | `0b497f22af0cf258bcf508d9cf78de0a` |
| `waves/RW-45/QUALITY/WEBRTC-BWE-REPORT.md` | `555387e2d3ef32f21ccf3600caf742f8` |

## 5. Риски и ограничения
- Чужие файлы и общие манифесты (`lib.rs`, `kanban-board.json`) не затрагивались согласно правилам изоляции воркеров `AGENTS.md` §1.2.
- Рисков регрессии не выявлено (`P0=0, P1=0`, мутационный гейт `5/5 = 100%`).
