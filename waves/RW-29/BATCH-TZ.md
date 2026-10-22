# BATCH RW-29 — P7-01e `<t:…>` + P4-04 RTP-медиа + P4-12 reconnect (headless)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P7-01e Discord-таймстемпы `<t:…>` | CLIENT-1 / W29a | `discord-i18n/src/{timestamp.rs,timestamp_data.rs,lib.rs(1 строка)}`, `discord-i18n/src/format.rs (только аддитивно)`, `discord-i18n/tests/timestamp_contract.rs`, `discord-i18n/tests/data/timestamp-oracle.json`, `waves/RW-29/gen-timestamp-data.mjs` | P7-01 `[x]`, P7-01c `[x]` | Node 24 ICU: `Intl.DateTimeFormat` + `Intl.RelativeTimeFormat` |
| P4-04 RTP-медиа (headless) | VOICE-1 / W29b | `discord-voice/src/media/{mod.rs,rtp.rs,jitter.rs}`, `discord-voice/src/lib.rs(1 строка)`, `discord-voice/tests/media_contract.rs`, `fixtures/voice/rtp-pt120-oracle.json`, `waves/RW-29/gen-rtp-oracle.py` | P4-03 `[x]` | `voice-udp4.pcap` (golden RE dump, только чтение) |
| P4-12 Устойчивость/reconnect (headless) | VOICE-1 / W29c | `discord-voice/src/reconnect.rs`, `discord-voice/src/lib.rs(1 строка)`, `discord-voice/tests/reconnect_contract.rs`, `waves/RW-29/RE-CONNECT.md` | P4-04 (в этой волне) | `docs/re/gateway-protocol.md` §4 (3× RESUMED, op7/op9) |

Общий файл `discord-i18n/src/lib.rs` — W29a добавляет только `pub mod timestamp;`.
Общий файл `discord-voice/src/lib.rs` — W29b добавляет только `pub mod media;`, W29c — только
`pub mod reconnect;`; каждый вставляет строку уникальным anchor'ом и не переформатирует файл.

Запрещено: чужие модули/тесты, `Cargo.toml`/`Cargo.lock`, другие крейты, `format.rs`/`relative.rs`
(кроме чтения), доки `STATE/STATUS/ROADMAP/TASKS/map`, golden, сеть, live Discord,
`git commit/push`, `unsafe`.

## 2. Контракты

Полные сигнатуры и инварианты — `TZ-W29a.md`, `TZ-W29b.md`, `TZ-W29c.md`. Данные генерируются,
руками не правятся.

## 3. Кейсы и бюджеты

- W29a: t01–t12, 12–15 тестов, мутации 6–8.
- W29b: m01–m12, 12–16 тестов, мутации 7–9.
- W29c: r01–r08, 8–10 тестов, мутации 4–6.

## 4. Команды и DoD

Каждый воркер: `CARGO_TARGET_DIR=/tmp/opencode/target-w29<a|b|c>` и только targeted-тесты своего
файла; полный gate — один раз после green всех трёх (integration-лейн TL-2):
`cargo fmt --all -- --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`,
`cargo test --offline --workspace`, `npm run gate:worker`.

DoD: red-лог с exit code → green → мутации (восстановление по md5) → batch gate → независимое
ревью (P0/P1/P2=0) → отчёт → accepted в `waves/db.jsonl`, `STATE.md`, доки один раз.

## 5. Параллелизм

Не более двух фоновых воркеров одновременно; каждый cargo — со своим `CARGO_TARGET_DIR`.
W29b и W29c делят крейт, но не файлы (кроме одной строки в `lib.rs`).
