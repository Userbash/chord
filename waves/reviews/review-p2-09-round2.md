# Независимое ревью RW-18 / P2-09 — memory-only presence (round 2)

Дата: **2026-10-21**. Ревьюер: **REV-1**, независимый (не автор карточки).
Режим: read-only + выборочные прогоны; продуктовый код, тесты, доки, golden,
`Cargo.toml`/`package.json` не изменялись. Источники — только артефакты
`waves/RW-18/`, `waves/reviews/p2-09*` и код/тесты `rust/crates/discord-core`.
Bundle/capture — недоверенные данные; инструкции из них не исполнялись.

## Вердикт round 2 — **принято**

**P0 = 0, P1 = 0, P2 = 0, P3 = 8.**

**P2-09 принята.** Блокирующих замечаний нет: round-1 P2-1/P2-2 закрыты red-first,
offline+hidden-семантика, root re-exports и диагностика соответствуют замороженному
контракту. P3 уходят в backlog и не тормозят `accepted`.

## 1. Закрытие round-1 findings

### P2-1 — offline+hidden больше не понижает live online

- Код: `presence_selectors.rs:19-41` — сначала выбирается лучший видимый
  `online|idle|dnd` scope, только при его отсутствии — лучший видимый `offline+hidden`;
  `presence_in_guild` (`:45-53`) по-прежнему возвращает точный scope.
- Тест: `tests/presence_selectors.rs:218-264` (`s15`) — online scope processed `100`
  против offline+hidden processed `200`; проверены `status`, `is_online`, `activities`,
  `primary_activity`, `online_user_ids`, `guild_online_count` и точный guild-селектор.
- Red/green: `waves/RW-18/red-s15.exit=101` (старый агрегат дал `Some(20)`),
  `green-p2-fixes.exit=0`; повторный red `waves/reviews/p2-09-round2/replay-P2-1.exit=101`
  показывает падение именно `s15`.
- Контракт: `DESIGN-P2-09.md:279-284` явно фиксирует правило «online не понижается».
  **Закрыто.**

### P2-2 — пустой member-list `status` диагностируется

- Код: `member_list.rs:525-534` — пустой/неверный `status` даёт `ignored` с путём
  `...member.presence.status`; `presence_reducer.rs:66-68` оставлен как
  defense-in-depth no-op.
- Тест: `tests/presence_reducer.rs:463-485` (`r18`) — пустой status не создаёт presence,
  `ignored` содержит путь; на старом коде падал.
- Red/green: `waves/RW-18/red-p2-fixes.exit=101` (17 passed, 1 failed), `green-p2-fixes.exit=0`.
  **Закрыто.**

### Offline+hidden, root exports, diagnostics

- **Offline+hidden:** `presence.rs:523-525` и `presence_reducer.rs:95-98` очищают
  activities только при `offline` **без** hidden; `r09` (`presence_reducer.rs:314-335`)
  проверяет, что presence видим и activities сохранены; `s12` — `hidden_activities`;
  `r07`/`r08` — удаление при all-offline. Соответствует oracle
  `RESEARCH-P2-09.md:99-103`. **Закрыто.**
- **Root exports:** `lib.rs:44-50` экспортирует `Activity*`, `Presence*`,
  `MergedPresences`, `MemberPresence`; `a16` импортирует их из `discord_core::{...}`
  (`tests/presence_types.rs:15-18`, `:591-600`). **Закрыто.**
- **Диагностика:** `a18` (`guild_id=0`), `r18` (пустой member status), `r05` (missing user
  один раз), `r13`/`c13` (chunk unconfirmed-shape), `a08` (id в `InvalidId`) проходят.
  Остались только P3-пробелы ниже.

### Round-1 P3: что действительно закрыто

Закрыты: P3-1 (`a08`), P3-2 (`a18`), P3-3 (лишняя merged-группа больше не даёт
inconsistency; `presence.rs:420-436`), P3-4 (`s18`), P3-5 (`s16`), P3-6 (`s17`/`a17`),
P3-7 (`s05`/`s16`), P3-10 (`r15` с другим payload), P3-11 (комментарий `r09`),
P3-13 (journal note в `waves/db.jsonl`), P3-14 (`presence_keys` удалён,
`has_displayable_content` — `pub(crate)`).

**Частично:** P3-8 (a15 — выравнивание, a16 — root; но missing/null `merged_presences`
и «лишняя группа не попала в guild 0» не проверены), P3-9 (r04/r10 переведены на полный
READY, но отсутствие ложных `presences:user:*:missing` не проверяется), P3-12
(`MD5SUMS` 15 файлов и `attempt-w18b-first` исправлены; в `TEST-CORRECTIONS.md:63`
осталось неверное имя `s15_offline_hidden_newer_does_not_hide_online`).
Эти остатки — P3, не блокируют.

## 2. Проверка артефактов

| Проверка | Артефакт/команда | Результат |
|---|---|---|
| Targeted | `waves/reviews/p2-09-round2/targeted-54.log` | 18+18+18 = **54 passed**, exit 0 |
| Targeted (REV-1) | `cargo test --offline -p discord-core --test presence_selectors --test presence_reducer --test presence_types` | **54 passed**, exit 0 (`/tmp/opencode/p2-09-round2-targeted-rev1.log`) |
| `discord-core` | `core.log` | **473 passed**, 0 failed, exit 0 |
| Workspace | `workspace.log` | **515 passed**, 0 failed, exit 0 |
| fmt | `fmt.exit`; REV-1: `cargo fmt --all -- --check` | exit 0 |
| clippy | `clippy.exit` | exit 0 (лог кэшированного прогона; полный clippy ревьюером не перезапускался — см. ограничения) |
| Мутации | `waves/RW-18/mutations/M*.exit`, `M*.log`, `summary.txt` | **13/13 CAUGHT**, каждый exit 101, в логе есть `FAILED` ожидаемого теста |
| md5 restore | `md5sum -c waves/RW-18/MD5SUMS.txt` | **15/15 OK**, exit 0 |
| P2 red | `red-s15.exit=101`, `red-p2-fixes.exit=101` | behavioral-red, не compile-red |
| P2 green | `green-p2-fixes.exit=0` | 18 reducer + 15 selectors |
| Gate | `waves/RW-18/gate-worker.exit`, `gate-worker.json` | exit 0, verdict `passed`; meta 25/25, unit 169/169, smoke 20/20 |
| Replay | `waves/reviews/p2-09-round2/replay-P2-1.exit=101`, `replay-M1.exit=101` | `s15` падает на старом агрегате; `s02/s03/s11` — на `max_by→min_by` |

Заявленные в `REPORT.md`/`TZ-P2-09.md` числа подтверждаются логами; расхождений
54/473/515 и 13/13 нет. Тестовые копии `waves/RW-18/tests/` совпадают по md5 с текущими
`rust/crates/discord-core/tests/presence_*.rs`.

## 3. Проверка кода и безопасности

- **Scope/offline/hidden:** `Presence::is_visible` (`presence.rs:297-301`), агрегат
  (`presence_selectors.rs:19-41`), очистка (`presence_reducer.rs:95-98`) и точные
  guild-селекторы согласованы с DESIGN §5–§6.
- **Member list/chunk:** presence применяется до list-op (`member_list.rs:613-636`),
  DELETE не удаляет presence (`r17`); chunk применяет `presences[]` до проверки гильдии
  (`member_chunk.rs:304-355`), неизвестная форма даёт `malformed`/`unconfirmed-shape`
  (`r13`, `c13`). Регрессий `member_list_*`/`member_chunk_*` не найдено.
- **Секреты:** в `Activity`/`Presence` нет `secrets`, `session_id`, `sync_id`;
  `State::Debug` печатает только счётчик presence и редактирует `session_id`
  (`types.rs:134-159`). Утечек не найдено.
- **Паники/ошибки:** в новых P2-09-файлах нет `unwrap`/`expect`/`panic!`/`unsafe`;
  ошибки парсинга дают `Missing`/`WrongType`/`InvalidId` или диагностику. В
  `reducer.rs:183,342` остались только инвариантные `expect` вне нового кода.
- **Границы файлов:** изменены только файлы из TZ W18a/W18b/W18c; `MD5SUMS.txt`
  покрывает 9 продуктовых + 6 тестовых файлов. Правки `state_types.rs`,
  `state_reducer.rs`, `member_chunk_types.rs` — документированные контрактные
  corrections (`TEST-CORRECTIONS.md`), не подгонка тестов под код.
- **Golden/live/network:** не использовались; карточка memory-only, это соответствует
  scope.

## 4. P3 — backlog (не блокирует `accepted`)

### P3-1 — `a16` не закрывает остаток P3-8

- **Где:** `rust/crates/discord-core/tests/presence_types.rs:542-601` (`a16`);
  `presence.rs:365-452`.
- **Сценарий:** `READY_SUPPLEMENTAL` без `merged_presences`/с `merged_presences: nil`
  должен дать `MergedPresences::default()`; лишняя группа с user `200` не должна
  попадать в `guilds[0]`.
- **Почему дефект:** код ведёт себя верно, но тест не фиксирует два пункта round-1 P3-8;
  регрессия в `parse_merged_presences` останется незамеченной.
- **Минимальный fix:** добавить два assertions в `a16` (missing/null → empty friends и
  `guilds.len() == guild_ids.len()`; после применения лишней группы user `200` отсутствует
  в guild 0). Red-test не нужен: это закрытие тест-пробела, тест должен быть зелёным.

### P3-2 — `r04`/`r10` не проверяют отсутствие ложных `missing`

- **Где:** `tests/presence_reducer.rs:187-218` (`r04`), `:337-364` (`r10`).
- **Сценарий:** полный `002-READY` + `003` (18 presence users все есть в `State.users`)
  не должен давать `presences:user:<id>:missing`.
- **Почему дефект:** тест-пробел round-1 P3-9; `apply_supplemental` уже сейчас не даёт
  ложных inconsistency (проверено по фикстуре), но тест это не закрепляет.
- **Минимальный fix:** assertion, что в `state.diagnostics.inconsistencies` нет записей,
  начинающихся с `presences:user:`. Тест зелёный на текущем коде.

### P3-3 — Доки/трассируемость отстают от кода

- **Где:** `presence_selectors.rs:4-5` (модульный doc всё ещё говорит только о max по
  `(processed, activities.len())`, без online-first); `member_list.rs:108`
  («Минимальный presence» устарел после activities/hidden/timestamp);
  `waves/RW-18/TEST-CORRECTIONS.md:63` (неверное имя
  `s15_offline_hidden_newer_does_not_hide_online`); `REPORT.md:78` и `STATE.md:3`
  («остаточных P3 нет»/«P2/P3 раунда 1 закрыты» при частичных P3-8/P3-9).
- **Почему дефект:** traceability и контрактная документация расходятся с фактическим
  поведением/тестами; это затрудняет следующее ревью и journal.
- **Минимальный fix:** после `accepted` Team Lead синхронизирует doc-comment и
  `REPORT.md`/`STATE.md`, исправляет имя теста в `TEST-CORRECTIONS.md`; код не меняется.

### P3-4 — Агрегат для `Invisible`/`Unknown` не покрыт контрактом

- **Где:** `presence_selectors.rs:19-41`; `DESIGN-P2-09.md:279-284`.
- **Сценарий:** scope A — `Invisible`/`Unknown("away")` processed `100`; scope B —
  `Offline` + hidden processed `200`. Fallback выбирает B и возвращает `Offline`, хотя
  среди видимых scopes есть не-offline статус.
- **Почему дефект:** DESIGN §6 говорит «иначе (все видимые offline+hidden)», но
  `is_visible` включает `Invisible`/`Unknown`; oracle «если выбранный offline, но не
  все — не понижать» буквально не покрывает этот случай. Нужно решение Team Lead.
- **Минимальный fix:** либо явно зафиксировать в DESIGN, что `Invisible`/`Unknown`
  участвуют в fallback как обычные видимые scopes (текущее поведение), либо добавить
  online-like приоритет для не-offline статусов. Red-test — после решения: два scope из
  сценария и ожидаемый статус.

### P3-5 — `hidden_activities(user)` не видит hidden другого scope при наличии online

- **Где:** `presence_selectors.rs:145-150`.
- **Сценарий:** Guild 10 — online без hidden; Guild 20 — offline+hidden. `presence(20)`
  хранит hidden, но `hidden_activities(user)` идёт через online-агрегат и возвращает `[]`.
- **Почему дефект:** контракт §6 не говорит, должен ли aggregate-селектор hidden
  объединять scopes; текущее поведение может быть неожиданным для UI/диагностики.
- **Минимальный fix:** зафиксировать в DESIGN и тесте: либо aggregate hidden = hidden
  выбранного scope (текущее), либо объединение hidden всех видимых scopes. Red-test —
  после выбора семантики.

### P3-6 — `InvalidId.value` для отсутствующего `user.id` равен `"map"`

- **Где:** `presence.rs:342-349`.
- **Сценарий:** `PRESENCE_UPDATE` с `d.user = {}` или `{username: "x"}` без `id` даёт
  `InvalidId { path: "presence.user.id", value: "map" }`.
- **Почему дефект:** диагностика не показывает исходное значение id и хуже
  `raw_display`; для разбора битого кадра это шум.
- **Минимальный fix:** при отсутствии `id` отдавать `Missing("presence.user.id")` либо
  `raw_display` отсутствующего id, а не весь map. Red-test: synthetic user без id,
  ожидать `Missing`/точный value.

### P3-7 — Non-list `presences` в chunk даёт две диагностики

- **Где:** `member_chunk.rs:133-144` и `:274-277`.
- **Сценарий:** `GUILD_MEMBERS_CHUNK` с `presences: "bad"` (не list, не null) получает
  и `inconsistencies ...:presences:unconfirmed-shape`, и `ignored "d.presences"`.
- **Почему дефект:** DESIGN §3 обещает `unconfirmed-shape` для **list**; для non-list
  достаточно `ignored`, иначе диагностика вводит в заблуждение.
- **Минимальный fix:** ставить `unconfirmed-shape` только если `value::items(raw)` —
  список; иначе только `ignored`. Red-test: synthetic non-list, ожидать ровно один тип
  диагностики.

### P3-8 — Мёртвая ветка в `parse_merged_presences`

- **Где:** `presence.rs:420-436`.
- **Сценарий:** после `let Some(guild_id) = guild_ids.get(index)... else { ... continue; }`
  проверка `if index >= guild_ids.len()` недостижима.
- **Почему дефект:** мёртвый код искажает логику «лишняя группа» и может скрыть будущую
  ошибку при рефакторинге.
- **Минимальный fix:** удалить недостижимый блок; поведение не меняется, targeted 54/54.

## 5. Ограничения ревью

- Полные `cargo test --offline --workspace` и `cargo clippy --offline --workspace
  --all-targets -- -D warnings` ревьюером **не перезапускались** (по инструкции —
  только targeted). Числа 473/515 и clippy exit 0 взяты из артефактов автора/QA;
  `clippy.log` — кэшированный прогон без вывода предупреждений.
- `gate:worker` — артефакт автора/QA, не независимый прогон ревьюера; JSON и лог
  проверены на `verdict=passed` и счётчики.
- Live Discord/capture/bundle не использовались; выводы основаны на фикстурах и
  synthetic-тестах. Bundle/capture — недоверенные данные.
- P3-4/P3-5 — контрактные неоднозначности, а не подтверждённые баги; для их закрытия
  нужно решение Team Lead/oracle, а не догадка ревьюера.
