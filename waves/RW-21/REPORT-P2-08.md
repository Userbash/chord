# RW-21 / REPORT-P2-08 — settings-proto

Дата: 2026-10-21. Карточка: **P2-08** (M). Batch: **B02**. Контракт:
`DESIGN-P2-08.md`, матрица: `TEST-MATRIX-P2-08.md`. Статус: **accepted (review round 2:
P0/P1/P2=0, P3 закрыт тестом t18; gate B02 — в конце batch)**.

## 1. Сделано

- `settings/wire.rs`: строгий protobuf wire-декодер (varint/fixed64/length/fixed32),
  лимиты `MAX_PROTO_BYTES=1 048 576`, `MAX_FIELDS=262 144`, точные ошибки; unknown-поля
  сохраняются как `ProtoField`; singular last-wins; без групп 3/4.
- `settings/preloaded.rs`: type 1 `PreloadedUserSettings`, секции `versions`, `inbox`,
  `appearance`, `voice_and_video`, `text_and_images`, `notifications`, `status`,
  `localization`; raw top-level сохраняет неизвестные `guilds`/folders/audio/privacy и др.
- `settings/frecency.rs`: type 2 `FrecencyUserSettings` — structural raw + `versions`;
  map-семантика favorite/frecency намеренно не выдумана (вне P2-08).
- `settings/mod.rs`: `SettingsProto::{Preloaded(Box<…>), Frecency, Unsupported}`,
  `parse`, `decode_settings`, convenience `decode_*`, константы type 1/2.
- `lib.rs`: `pub mod settings;` и корневые re-exports (TL-2 integration).
- `Debug`: `ProtoMessage` — только число полей, `LengthDelimited` — только длина;
  `StatusSettings` — `status_len`; строки `invisible`, `ru`, fixed64 id не печатаются.
- base64 не дублируется: используется принятый `rest::base64::decode` (P1-08).

## 2. Red / green

| Фаза | Команда | Результат |
|---|---|---|
| red (missing module) | `cargo test --offline -p discord-core --test settings_proto` | `E0583 file not found for module settings`, exit 101 — `logs/p08-red-compile.log` |
| green targeted | та же команда | **18 passed, 0 failed**, exit 0 — `logs/p08-targeted-final2.log` |
| targeted final | та же команда после fmt | **18 passed, 0 failed** — `logs/p08-targeted-final2.log` |
| core | `cargo test --offline -p discord-core` | **524 passed, 0 failed** — перегон после B02 |
| workspace | `cargo test --offline --workspace` | **580 passed, 0 failed** — перегон после B02 |
| fmt | `cargo fmt --all -- --check` | exit 0 |
| clippy | `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 — `logs/b02-clippy.log` |
| gate | `npm run gate:worker` | **passed**: typecheck, meta 25/25, unit 169/169, smoke 20/20 — `gate-worker-b02.json` |

## 3. Фикстуры и независимый оракул

Фикстуры скопированы из `/home/sanya/re-deep` (read-only) в
`rust/crates/discord-core/tests/data/`; тесты не читают RE-дамп.

| Файл | base64 | raw | raw sha256 | файл md5 |
|---|---:|---:|---|---|
| `settings-preloaded-ready-4400.b64` | 4400 | 3300 | `c79fca65e321cae5135401f22ee9041809958a34e52e959d0a1a63a04f7c5672` | `b50ff77239d73a16316fffba19954233` |
| `settings-frecency-update-8269.b64` | 17 436 | 13 077 | `86eaa7e3b6c83aa4f4705a2fcc3823e4bde10836ccca9f3907e6ef195c7d4c80` | `624bbfe8dbd190046f497c96551d8312` |
| `settings-frecency-update-8608.b64` | 17 436 | 13 077 | `4ac5dd6e143ffda4fec63400dcb4f8c06034a8590efe2828b916c297818f1079` | `b4c6f1f1ae0642ef68e7c549a0cdef13` |
| `settings-frecency-update-13072.b64` | 17 436 | 13 077 | `8cd02970195054bbf5bfc481486264df9027213101432548f4a1b26a8d1531e5` | `eaf1a3ffa00192de12ec23bbfefe3116` |

Оракул — `oracle.py` (stdlib, не импортирует Rust). READY: 18 top-level полей, theme=3,
density=1, hour=2, locale=`ru`, offset=-300, status=`invisible`, created=1766220108588,
acked=1041362756533751819. UPDATE: type=2, `partial=false`, top-level
`1,2,4,6,7,9,11,12,13`, versions client=10, data=7545/7546/7547 для трёх capture.
`docs/re/rest-d02.md` PATCH-тело обрезано (4001 символ) и **не** использовалось как оракул.

## 4. Мутации

`waves/RW-21/mutations/M1…M10.log`, восстановление проверено `diff -r` и md5.

| ID | Мутант | Пойман |
|---|---|---|
| M1 | нет лимита `MAX_PROTO_BYTES` | p08t13 |
| M2 | разрешён 11-байтовый varint | p08t13 |
| M3 | field number 0 принимается | p08t13 |
| M4 | wire 3 как length-delimited | p08t13 |
| M5 | нет проверки length vs remaining | p08t13 |
| M6 | first-wins вместо last-wins | p08t02 |
| M7 | fixed64 notification как varint | p08t07 + смежные |
| M8 | пустой BoolValue = отсутствие | p08t15 + смежные |
| M9 | отрицательный int64 не sign-extended | p08t08 |
| M10 | Debug печатает status string | p08t16 |
| M11 | Debug `NotificationSettings` печатает fixed64 id | p08t17 |
| M12 | Debug `LocalizationSettings` печатает locale | p08t17 |
| M13 | `MAX_FIELDS` off-by-one | p08t18 |

**Итого: 13/13 caught.** Product tree восстановлен; md5 после restore совпал с
исходным (`wire d0fad3…`, `preloaded 01539d…`, `mod 2a7ca2…`, `frecency ab24ec…`).

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/settings/wire.rs` | `d0fad3ec3275c898b3442c1174afc351` |
| `rust/crates/discord-core/src/settings/preloaded.rs` | `01539d24d6ba64d730502e2bbf041593` |
| `rust/crates/discord-core/src/settings/frecency.rs` | `ab24ecc23829cdf501c4ea5d6730d0f9` |
| `rust/crates/discord-core/src/settings/mod.rs` | `2a7ca224846834e8ba8a07ba326a4cb2` |
| `rust/crates/discord-core/src/lib.rs` | `65678407b42b3ec0183f2c27ee64cf45` |
| `rust/crates/discord-core/tests/settings_proto.rs` | `617010189be41bff41e244a211951e82` |
| `rust/crates/discord-core/tests/data/settings-preloaded-ready-4400.b64` | `b50ff77239d73a16316fffba19954233` |
| `rust/crates/discord-core/tests/data/settings-frecency-update-8269.b64` | `624bbfe8dbd190046f497c96551d8312` |
| `rust/crates/discord-core/tests/data/settings-frecency-update-8608.b64` | `b4c6f1f1ae0642ef68e7c549a0cdef13` |
| `rust/crates/discord-core/tests/data/settings-frecency-update-13072.b64` | `eaf1a3ffa00192de12ec23bbfefe3116` |

## 6. Риски / вне scope

- `StatusSettings.status_expires_at_ms` и `custom_status` не подтверждены capture и не
  типизированы; остаются в raw/структурно. Это не блокер P2-08.
- `FrecencyUserSettings` map/favorite-поля не интерпретируются — цель карточки structural
  decoder; полная semantics потребует отдельного capture/карточки.
- `MAX_FIELDS`/`TooManyFields` закрыт boundary-тестом `p08t18` (262 144 поля — ok,
  262 145 — ошибка) и mutation M13.
- Нет state/REST/UI/SQLite/keyring/live/network/new dependencies.
- P3-находки review (если будут) — backlog, не блокируют P2-08.

## 7. Независимый review

- Round 1 (Codex CLI, read-only): **P2** — `Debug` секций `NotificationSettings` и
  `LocalizationSettings` печатал fixed64 id/locale/timezone. Красный тест
  `p08t17_section_debug_redacts_sensitive_values` (`logs/p08-p2-red.log`), затем redacted
  `Debug`; green 17/17 (`logs/p08-p2-green.log`).
- Round 2 (Codex CLI, read-only): **accepted with P3**; P0=0, P1=0, P2=0, P3=1
  (`MAX_FIELDS` без boundary-теста). P3 закрыт тестом `p08t18_max_fields_boundary`
  (`logs/p08-p3-green.log`), mutation M13 caught.
- Итог: **P0/P1/P2/P3 = 0**; замечание ревьюера о «failing log» относилось к
  `mutations/M*.log` (намеренные мутанты), а не к текущему продукту.
