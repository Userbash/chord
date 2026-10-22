# RW-21 / TEST-MATRIX-P2-08 — settings-proto

Дата: 2026-10-21. Контракт: `DESIGN-P2-08.md` §2–§4. Оракул: `oracle.py`, READY/UPDATE
из `/home/sanya/re-deep` (read-only). Бюджет M: 16 тестов, 10 мутаций.

## Фикстуры

| Файл | Источник | base64 | raw B | sha256 raw |
|---|---|---:|---:|---|
| `tests/data/settings-preloaded-ready-4400.b64` | `gateway/by-type/002-READY.json` → `d.user_settings_proto` | 4400 | 3300 | `c79fca65e321cae5135401f22ee9041809958a34e52e959d0a1a63a04f7c5672` |
| `tests/data/settings-frecency-update-8269.b64` | `gw-proto/msgs-run2-95974_8269.jsonl` → type 2 | — | 13077 | — |
| `tests/data/settings-frecency-update-8608.b64` | `gw-proto/msgs-run2-95974_8608.jsonl` | — | 13077 | — |
| `tests/data/settings-frecency-update-13072.b64` | `gw-proto/msgs-run2-95974_13072.jsonl` | 17436 | 13077 | `8cd02970195054bbf5bfc481486264df9027213101432548f4a1b26a8d1531e5` |

## `tests/settings_proto.rs` (16)

| ID | Проверка | Ожидание | Оракул |
|---|---|---|---|
| p08t01 | READY base64 → type 1, raw length | `Preloaded`; `raw.fields.len()==18`; base64/bytes длины | oracle READY |
| p08t02 | top-level номера READY | `1,2,3,4,5,6,7,8,9,10,11,12,13,14,16,19,23,25` в порядке | oracle |
| p08t03 | `versions` + `inbox` | client 21, server None, data 3027; current_tab 2, viewed_tutorial None | oracle |
| p08t04 | appearance | theme 3, developer true, hour 2, density 1; отсутствующие None | oracle |
| p08t05 | voice | always true, afk 600, stream true, native true, disable false, preset None | oracle |
| p08t06 | text/images (16 полей) | значения §3 RESEARCH; `animate_stickers=2`, `explicit=2`, `dm_spam=3` | oracle |
| p08t07 | notifications | notify false, acked `1041362756533751819`, quiet None | oracle |
| p08t08 | status + localization | status `invisible`, show false, created `1766220108588`; locale `ru`, offset `-300` | oracle |
| p08t09 | raw сохраняет неизвестные | top-level 3 (guilds), 16 (audio context), 19, 23, 25 присутствуют; длины >0; `raw.field(13).is_some()` | READY |
| p08t10 | три UPDATE type 2 | `Frecency`; top-level `1,2,4,6,7,9,11,12,13`; raw 13077; versions client 10/data 7547; не `Preloaded` | oracle + все 3 capture |
| p08t11 | unsupported type 3 | `Unsupported { proto_type: 3 }`; generic field 1 varint 7 сохранён; не ошибка | протокол |
| p08t12 | base64 errors | `A` (length), `!!!!` (char), `AA=A` (padding) → `Base64` | base64 oracle |
| p08t13 | wire errors | truncated varint `0x80`; field 0 `0x00`; wire 3 `0x0b`; length overflow `0x0a 0x05 0x00`; oversize 1 MiB+1 | wire protocol |
| p08t14 | known field wrong wire | `Preloaded` field 5 wire 0 `0x28 0x01` → `WrongWireType{field:5,expected:2,actual:0}` | schema |
| p08t15 | empty wrappers vs absent | type 1 bytes field 5 len 0 (`0x2a 0x00`) → `voice_and_video Some`, `disable_stream_previews`? (поле 10 пустой wrapper) → `Some(false)`; пустой top-level → секции None | schema |
| p08t16 | Debug redaction | `format!("{:?}", decoded)` не содержит `invisible`, `ru`, `1041362756533751819`, `ru`-строку; содержит только counts/presence | security |

`p08t15` использует synthetic bytes, а не фикстуру; проверяет различие «поле отсутствует» и
«пустой BoolValue = false». Для пустого `PreloadedUserSettings` (`&[]`) все секции `None`.

## Мутации (10)

| ID | Мутант | Цель |
|---|---|---|
| M1 | `MAX_PROTO_BYTES + 1` не проверяется | p08t13 |
| M2 | varint разрешает 11-й байт | p08t13 |
| M3 | field number 0 принимается | p08t13 |
| M4 | wire 3 принимается как length-delimited | p08t13 |
| M5 | length не проверяется на остаток | p08t13 |
| M6 | last-wins заменён first-wins для singular | p08t02/p08t10 |
| M7 | `fixed64` читается как varint | p08t07 |
| M8 | пустой BoolValue трактуется как отсутствие | p08t15 |
| M9 | `timezone_offset` без sign-extension | p08t08 |
| M10 | `Debug` печатает length-delimited/строки | p08t16 |

## Команды

```sh
cd rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core --test settings_proto
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core
flock /tmp/opencode/discord-cargo.lock cargo test --offline --workspace
flock /tmp/opencode/discord-cargo.lock cargo fmt --all -- --check
flock /tmp/opencode/discord-cargo.lock cargo clippy --offline --workspace --all-targets -- -D warnings
cd ..
npm run gate:worker
```
