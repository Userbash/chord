# REPORT RW-29 — B10 headless: P4-04 + P4-12 + P7-01e

Волна: RW-29. Дата: 2026-10-22. Статус: **принято**.
Контракты: `BATCH-TZ.md`, `TZ-W29a.md`, `TZ-W29b.md` (+ §6 уточнения после ревью), `TZ-W29c.md`.

## Состав и результаты

| Карточка | Воркер | Тесты | Мутации | Отчёт |
|---|---|---:|---:|---|
| P4-04 RTP-медиа (PT120/0xBEDE, DTX/CN, джиттер) | VOICE-1 / W29b | 13 | 9/9 | `REPORT-W29b.md` |
| P4-12 reconnect/resume (бэкофф, op7/op9) | VOICE-1 / W29c | 10 + 2 unit | 6/6 | `REPORT-W29c.md` |
| P7-01e Discord-таймстемпы `<t:…>` | CLIENT-1 / W29a | 14 | 8/8 | `REPORT-W29a.md` |

## Оракулы

- **RTP**: `fixtures/voice/rtp-pt120-oracle.json` (md5 `798dd8aea8dd14d4429ac00b1c1d0a12`) —
  10 пакетов PT120 из golden-pcap `reference-oracle/voice/voice-udp4.pcap`; генератор
  `gen-rtp-oracle.py`, воспроизводимость проверена независимым парсером ревьюера (0 расхождений).
- **Таймстемпы**: `rust/crates/discord-i18n/tests/data/timestamp-oracle.json` (558 строк:
  31 локаль × 6 стилей × 3 момента) + R-оракул Node 24 ICU; генератор `gen-timestamp-data.mjs`
  с самопроверкой JS-модели против ICU.
- **Reconnect**: `docs/re/gateway-protocol.md` §4/§9 (op7, 3× RESUME→RESUMED, `s` 95/664/332);
  ограничения — `RE-CONNECT.md`.

## Гейт и ревью

- `waves/RW-29/gate-cargo.log`: `FMT_EXIT=0`, `CLIPPY_EXIT=0`, `TEST_EXIT=0`,
  workspace **953 passed / 0 failed**.
- `waves/RW-29/gate-worker.log`: passed, `.gate/worker-2026-10-22T17-12-46-556Z.json`.
- Независимое ревью voice-части: **принято**, P0/P1/P2 = 0, 8 P3 в бэклог (`STATE.md` §7);
  уточнения контракта занесены в `TZ-W29b.md` §6.

## Изменённые файлы (кратко)

`rust/crates/discord-voice/src/media/{mod,rtp,jitter}.rs`, `src/reconnect.rs`,
`src/lib.rs` (+2 строки); `rust/crates/discord-i18n/src/{timestamp,timestamp_data}.rs`,
`src/format.rs` (аддитивно), `src/lib.rs` (+1 строка); тесты
`media_contract.rs`, `reconnect_contract.rs`, `timestamp_contract.rs`;
фикстура RTP и генераторы; `waves/RW-29/*`.

## Ограничения

- Opus-SDK не использован: CN определяется по длине шифротекста (эмпирика capture).
- Wire-совместимость SRTP/reconnect не заявляется (ключевой материал/точные задержки
  не измерены).
- Таймстемпы рендерятся в UTC; NNBSP/NBSP нормализуются в пробел; `f` — по ТЗ, не по
  фактической раскладке клиента.
