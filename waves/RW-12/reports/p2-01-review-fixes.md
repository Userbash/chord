# P2-01 — закрытие находок независимого ревью

**Дата:** 2026-10-20
**Вход:** `waves/reviews/review-p2-01.md` — вердикт «не принято» (P0=0, P1=1, P2=3, P3=3).
**Статус:** исправления green, ожидается повторное независимое ревью. Приёмки в этом отчёте нет.

## 1. Что закрыто

| Finding | Уровень | Решение | Доказательство |
|---|---|---|---|
| Атом-строки (`data_mode`, `session_type`, `sessions[].status`, `client_info.client`, `client_info.os`) терялись | P1-1 | `value::text` принимает `Str` и `Atom`, кроме зарезервированных `nil`/`true`/`false` | `r1_atom_strings_are_text` |
| `from_dispatch` при не-map `d` возвращал `WrongType` | P2-1 | не-map `d` → `ReadyError::NotReady`; отсутствие `d` → `Missing` | `r2_non_map_d_is_not_ready` |
| Неверные типы вложенных необязательных полей молча получали дефолт | P2-2 | `&mut ReadyDiagnostics` протянут в `parse_user`, `parse_private_channel`, `parse_relationship`, `parse_client_info`, `parse_session`, `parse_member`; пути `ignored` — с `d.` | `r3_nested_wrong_types_are_ignored` |
| `merged_members[i]` сдвигался после `malformed`-гильдии | P2-3 | `parse_guilds` возвращает карту «сырой индекс → итоговый индекс»; группа привязывается по сырому индексу, группа пропущенной гильдии не приклеивается к соседней | `r4_merged_members_keep_raw_guild_index` |
| Явный `merged_members: []` не давал `malformed` | P3-1 | пустой явный список при непустых `guilds` — расхождение длин | `r5_explicit_empty_merged_members_is_malformed` |
| Пути `ignored` в `guild.rs` были без `d.` | P3-2 | все `ignored`-пути вложенных полей начинаются с `d.` | `r6_ignored_paths_use_d_prefix`, обновлён A8 |
| `value` остаётся публичным модулем | P3-3 | **отложено координатором**: модуль используется интеграционными тестами как оракул формы `EtfValue`; изоляция — отдельная задача backlog, публичный API `ready` она не меняет | backlog |
| `Ready::parse` при не-map `d` и обновлённый §3.5/TZ расходились | P2-4 | контракт разделён: `from_dispatch` не-map → `NotReady`, `Ready::parse` payload не-map → `WrongType`; §2.3/§3.5/TZ синхронизированы, `r2` проверяет оба входа | `r2_non_map_d_is_not_ready` |
| Приватные сигнатуры `parse_guilds`/`parse_user` не были синхронизированы с §3.4/TZ | P2-5 | §3.4, псевдокод §3.5 и TZ P-2 обновлены: `ParsedGuilds`, `parse_user(path, diags)` | DESIGN/TZ |
| Для дубликата гильдии не был зафиксирован победитель группы | P3-4 | в §2.3/§3.5 записано: побеждает группа последнего сырого вхождения, предыдущие — `malformed`; добавлен `r7` | `r7_duplicate_guild_keeps_last_member_group` |
| Устаревшие md5 red-копии и отсутствие exit-кодов в логах | P3-5 | md5-файлы пересозданы; в red/green/core/workspace/clippy-логи добавлены явные `# EXIT` | `p2-01-review-*.md5`, логи |

## 2. Red-first

- Первый red на новой спеке: 6 тестов, 6 failed, `# EXIT=101`
  (`p2-01-review-red-review.log`).
- После исправления ошибки тестового helper (дубликат ключа `guilds` в `minimal_d`) red
  переснят на **финальном** тексте тестов; финальный red — mutation-red с временным откатом
  продуктовых правок: `0 passed; 7 failed`, `# EXIT=101`
  (`p2-01-review-red-final.log`).
- Красная копия тестов: `p2-01-review-test.red.rs`;
  md5 `7ab589278a949a5c3e80c8cfeeb5eea4` совпадает с финальным
  `tests/ready_review.rs`.
- Green: `7 passed; 0 failed`, `# EXIT=0` (`p2-01-review-green-final.log`).
- После red менялся только rustfmt (форматирование), семантика тестов не менялась.

## 3. Полные прогоны

- `cargo test --offline -p discord-core`: **242 passed, 0 failed**, `# EXIT=0`
  (`p2-01-review-core-green.log`).
- `cargo test --offline --workspace`: **284 passed, 0 failed**, `# EXIT=0`
  (`p2-01-review-workspace-green-final.log`).
- `cargo fmt --all -- --check`: `FMT_OK`.
- `cargo clippy --offline --workspace --all-targets -- -D warnings`: `# CLIPPY_EXIT=0`
  (`p2-01-review-clippy.log`).
- Сеть не использовалась, живой Discord не трогался, новые зависимости не добавлялись.

## 4. Изменённые файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/ready/value.rs` | `090bf9441acd8459d310ee93d2242dd6` |
| `rust/crates/discord-core/src/ready/snapshot.rs` | `d88df7bc6ace6c44ca3fd81dae8b3c59` |
| `rust/crates/discord-core/src/ready/social.rs` | `33386b7304367ff98d06e37c8d1f4e64` |
| `rust/crates/discord-core/src/ready/guild.rs` | `2bfbea7ae9cfee092696768339f28c88` |
| `rust/crates/discord-core/tests/ready_review.rs` | `7ab589278a949a5c3e80c8cfeeb5eea4` |
| `rust/crates/discord-core/tests/ready_core.rs` | `04a5aa28d971041c45a2fd85ee422d5f` |
| `waves/RW-12/reports/p2-01-review-test.red.rs` | `7ab589278a949a5c3e80c8cfeeb5eea4` |

Контрактные уточнения внесены в `waves/RW-12/DESIGN-P2-01.md` §2.3/§3.4/§3.5 и
`waves/RW-12/TZ-P2-01.md` §2/P-2: атом-текст, разделение `from_dispatch`/`Ready::parse` для
не-map `d`, сырой индекс `merged_members` (включая дубликаты), `d.`-префикс `ignored`,
явный пустой `merged_members`, новые приватные сигнатуры.

## 5. После приёмки (2026-10-20)

- Повторное ревью round 3 — **«принято»**, P0/P1/P2 = 0
  (`waves/reviews/review-p2-01-round3.md`).
- Legacy `ready_data` удалён из `src/` и `tests/` (P-4): `discord-core` **242 → 233**,
  workspace **284 → 275** (`p2-04-ready-data-removal-green.log`,
  `p2-04-ready-data-removal-workspace-green.log`).
- Финальный гейт: **passed**, `.gate/worker-2026-10-20T09-56-10-288Z.json`
  (`final-gate-2026-10-20.log`: typecheck, meta 25/25, unit 169/169, smoke 20/20).
- P3-3 (`value` public) — backlog, приёмку не блокирует.
- События `accepted` для RW-10/RW-11/RW-12 и `gate` — в `waves/db.jsonl`.
