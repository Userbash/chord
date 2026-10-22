# RW-17 / P2-07 — отчёт: read-state и бейджи

Статус: **принято** 2026-10-20 после двух раундов независимого ревью
(P0 = 0, P1 = 0, P2 = 0; P3 = 5 backlog). Карточка: **P2-07**, T-DATA-09.
Scope: memory-only `MESSAGE_ACK`, read-state, mentions/бейджи. Без сети, live Discord,
SQLite, UI, REST-запроса ACK и новых зависимостей.

## 1. Сделано

- `MessageAck` + `MessageAck::parse` в `src/state/read_state.rs`: обязательные
  `channel_id`/`message_id`; optional `flags`, `last_viewed`, `mention_count`, `manual`,
  `version`; fatal `Missing`/`InvalidId`/`WrongType`, optional `ignored("d.*")`.
- `EventPayload::MessageAck`, маршрут `DispatchKind::MessageAck` в `Event::from_routed`,
  re-export `discord_core::MessageAck`; `MESSAGE_ACK` больше не `Unsupported`.
- `apply_ack` в `src/state/read_state_reducer.rs`: ленивое создание записи в конец,
  non-channel guard, stale guard по `last_message_id`, равенство message-id как норма,
  `flags`/`last_viewed`, manual/non-manual mentions, глобальная `version`, неизменяемые
  `badge_count`/`last_acked_id`/`last_pin_timestamp`/`read_state_type`; effects не создаются.
- Селекторы в `src/state/read_state_selectors.rs`: `mention_count`, `has_mentions`,
  `mention_channels`, `total_mentions`, `guild_mention_count` (каналы + треды),
  `private_mention_count`, `badge_count`, `badge_entries` (`badge_count > 0`),
  `total_badges`, `acknowledged_message_id`, `is_message_read`, `read_state_version`.
- Уточнён контракт: channel-запись использует только `last_message_id`; `last_acked_id`
  принадлежит non-channel. Non-channel ACK отклоняется целиком, включая version.
- `raw_display` вынесен в `types.rs` (`pub(crate)`), локальный дубль удалён; doc `mod.rs`
  синхронизирован.

## 2. Воркеры и файлы

| Воркер | Файлы | Тесты |
|---|---|---|
| W17a | `src/state/read_state.rs`, `src/state/types.rs`, `src/state/reducer.rs`, `src/state/mod.rs`, `src/lib.rs` | `tests/read_state_types.rs` — 12 |
| W17b | `src/state/read_state_reducer.rs`, `src/state/reducer.rs` | `tests/read_state_reducer.rs` — 17 |
| W17c | `src/state/read_state_selectors.rs` | `tests/read_state_selectors.rs` — 11 |

Итого **40** тестов волны. W17a/W17c начинали с compile-red; W17b — с behavior-red
(ветка ACK была no-op).

## 3. Red

| Этап | Команда | Результат |
|---|---|---|
| W17a | `cargo test -p discord-core --test read_state_types` | compile-red, exit 101: нет `MessageAck`/`EventPayload::MessageAck` |
| W17b | `cargo test -p discord-core --test read_state_reducer` | **0 passed, 14 failed**, exit 101 (ACK no-op) |
| W17c | `cargo test -p discord-core --test read_state_selectors` | compile-red, 59 × `E0599`, exit 101 |
| P2-1 | `cargo test -p discord-core --test read_state_selectors s11_...` | **0 passed, 1 failed**, exit 101: `Some(700) != None` |

Логи: `red-w17a.txt`, `red-w17b.txt`, `red-w17c.txt`, `red-p2-1.txt`; копии —
`tests-red/read_state_{types,reducer,selectors}.rs`, `tests-red/p2-1_read_state_selectors.rs`.
Расхождение строк в `red-w17b.txt` относится к ревизии до финального форматирования;
копия теста и причина падения сохранены; старый `mutations.txt` помечен как superseded.

## 4. Green / проверки

| Проверка | Результат |
|---|---|
| Targeted | **40 passed / 0 failed** (12 + 17 + 11) |
| `cargo test -p discord-core` | **418 passed / 0 failed** |
| `cargo test --workspace` | **460 passed / 0 failed** |
| `cargo fmt --all -- --check` | чисто |
| `cargo clippy -p discord-core --tests -- -D warnings` | чисто |
| `npm run gate:worker` | **passed** (typecheck, meta 25/25, unit 169/169, smoke 20/20) |

Логи: `green-w17a.txt`, `green-w17b.txt`, `green-w17c.txt`, `green-p2-1.txt`,
`final-core.txt`, `final-workspace.txt`, `final-fmt.txt`, `final-clippy.txt`,
`gate-final.log`, `gate-post-journal.log`. Финальный gate:
`.gate/worker-2026-10-20T15-10-13-521Z.json`, копия — `waves/RW-17/gate-accepted.json`
(предыдущие `gate-final.json`, `gate-post-journal.json` и
`gate-final-post-journal.json` сохранены отдельно).

## 5. Независимое ревью

- **Раунд 1:** P0 = 0, P1 = 0, **P2 = 1**, P3 = 7. P2-1: channel-запись ошибочно делала
  fallback в `last_acked_id`, что давало ложный `is_message_read`; тест s06 закреплял
  неверное поведение.
- **Исправление red-first:** s11 упал (`Some(700) != None`); селектор для channel теперь
  возвращает только `last_message_id`; s06 исправлен на `None`; Correction 2 в
  `TEST-CORRECTIONS.md`.
- **Раунд 2:** **принято**, P0 = 0, P1 = 0, P2 = 0, P3 = 5. Добавлены r15 (forward ACK),
  r16 (version без max-guard), r17 (unknown-channel version), финальный green-лог,
  исправлен устаревший комментарий s06.
- Отчёт ревью: `waves/reviews/review-p2-07.md`.

## 6. Мутации

`MUTATIONS.md` + `mutation-logs/M1…M7.txt`: **7/7 caught**, исходники восстановлены,
md5 до/после совпадают.

| ID | Мутация | Поймана |
|---|---|---|
| M1 | `manual` игнорируется | reducer |
| M2 | stale guard инвертирован | reducer |
| M3 | `flags`/`last_viewed` не обновляются | reducer |
| M4 | guild не учитывает threads | selectors |
| M5 | forward `last_message_id` не двигается | reducer |
| M6 | `version` с max-guard (обе ветки) | reducer |
| M7 | max-guard только unknown-channel | reducer |

## 7. Файлы и md5

Артефакт: `MD5SUMS.txt`; копии финальных тестов — `tests-final/`.
Ключевые md5:

| Файл | md5 |
|---|---|
| `src/state/read_state.rs` | `90a7fe48213eae0d625644575498b676` |
| `src/state/read_state_reducer.rs` | `a18804708a45dccde3f5d2b105840924` |
| `src/state/read_state_selectors.rs` | `c050b2ce9ab3683b1f353bd4fd39c8b8` |
| `src/state/types.rs` | `fb78f4ced58fdee9ba32054461f6bb94` |
| `src/state/reducer.rs` | `f692d51844baed9aadbda300f160330d` |
| `src/state/mod.rs` | `01243fc0cb8b9c56861820c3f6e8f22c` |
| `src/lib.rs` | `411af7c9a9509947f4e3d3623565534d` |
| `tests/read_state_types.rs` | `44ac5f418d882f43a94370d8753a962a` |
| `tests/read_state_reducer.rs` | `4401564d3ddfb81a882e7bcc31877d3e` |
| `tests/read_state_selectors.rs` | `f078178e595d14c6882ed1cc00ff90aa` |

## 8. Риски / P3 / границы

- Read-state — memory-only; `unread_count` не выдуман (нужны сообщения и политика P2-11).
- Входящий ACK не порождает `Effect::AckMessage`; REST ACK остаётся P1-12.
- `version` присваивается без max-guard; stale guard по message-id — локальная защита,
  не заявленная семантика протокола.
- Non-channel ACK отклоняется целиком (entry и version), контракт зафиксирован в DESIGN
  §4.2/§4.7.
- P3: старый `mutations.txt` 4/4 оставлен как исторический и помечен superseded; red-лог
  W17b снят до финального форматирования; live-smoke отсутствует; cap/prune read-state не
  вводился (READY 157 записей, ACK только upsert). SQLite/UI/сеть — вне scope.
- Блокеры владельца не менялись: GUI, keyring, SQLite, live Discord.
