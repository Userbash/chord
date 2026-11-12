# Независимое ревью RW-19 / P2-10 — memory-only voice states

Дата: **2026-10-21**. Ревьюер: **REV-P2-10**, независимый (не автор карточки).
Режим: **read-only** по продукту/тестам/докам/golden; запись только в
`waves/reviews/review-p2-10.md` и `waves/reviews/p2-10/` (логи прогонов). Cargo — только
последовательно под `flock /tmp/opencode/discord-cargo.lock`; полный workspace/gate повторно
не перезапускались. Контракт: `waves/RW-19/DESIGN-P2-10.md`, `TZ-P2-10.md`,
`TEST-MATRIX-P2-10.md`, research `RESEARCH-P2-10.md`.

## Вердикт — **не принято**

**P0 = 0, P1 = 0, P2 = 1, P3 = 8.**

Продуктовый код в основном соответствует замороженному контракту: типы, парсеры live-событий,
reducer, селекторы, интеграция I0/I1, `Debug`-редакция, отсутствие паник/сети/новых
зависимостей — в порядке. Targeted 15/15, core 503, workspace 545, fmt/clippy 0, gate
`passed` (meta 25/unit 169/smoke 20), 16/16 заявленных мутаций подтверждаются логами
(выборочно сверены V2/V3/V5/V8). Блокирует приёмку один P2: **аддитивность снимка и
last-wins дубликатов `(guild,user)` не проверены ни одним тестом**; единственный snapshot-тест
v09 подаёт `003` в пустой store, поэтому мутант «очистить карту перед циклом» проходит все
15 тестов. Это не стилевой нит: без теста целая замороженная семантика `READY_SUPPLEMENTAL`
остаётся незащищённой, а заявленное mutation-покрытие V8 её не ловит. P3 уходят в backlog и
сами по себе не блокируют.

## 1. Проверки

| # | Проверка | Команда / артефакт | Результат |
|---|---|---|---|
| 1 | md5 voice-продукта и тестов | `md5sum rust/crates/discord-core/src/state/voice/*.rs tests/voice_state_*.rs` | совпадает с `w19-report.md` §5 и `INTEGRATION.md` §6 |
| 2 | md5 shared-файлов | `md5sum src/state/{mod,types,reducer}.rs src/lib.rs tests/state_{types,reducer}.rs` | совпадает с `INTEGRATION.md` §6 |
| 3 | fixture `003` | `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` | md5 `55983830547abaf8d1b122798fdcf568`; 1 voice-запись, значения совпадают с v01 |
| 4 | targeted повторно | `flock … cargo test --offline -p discord-core --test voice_state_types --test voice_state_reducer --test voice_state_selectors` → `waves/reviews/p2-10/targeted-rerun.log` | **exit 0**, 6+5+4 = **15/15** |
| 5 | fmt | `flock … cargo fmt --all -- --check` → `waves/reviews/p2-10/fmt-check.log` | **exit 0**, пустой вывод |
| 6 | clippy | `flock … cargo clippy --offline -p discord-core --all-targets -- -D warnings` → `waves/reviews/p2-10/clippy-core.log` | **exit 0** |
| 7 | batch targeted | `waves/B01/logs/targeted-final.log` | voice 15 + typing 15 = **30/30**, 0 FAILED |
| 8 | core | `waves/B01/logs/core-503.log` | сумма `test result: ok` = **503**, 0 FAILED |
| 9 | workspace | `waves/B01/logs/workspace-545.log` | сумма = **545**, 0 FAILED |
| 10 | gate | `waves/B01/gate-worker.json`, `logs/gate-worker.log` | `verdict: passed`; meta 25/25, unit 169/169, smoke 20/20 |
| 11 | voice-мутации | `waves/B01/mutations/V1…V8.log`, `summary.txt`, `/tmp/opencode/b01-mutation-backup` | 8/8 exit 101; текущие md5 reducer/selectors совпадают с backup |
| 12 | паники/секреты/сеть | `grep` по `state/voice` | нет `unwrap/expect/panic!/todo!/unimplemented!/unsafe`, нет сети/SQLite/UI/keyring/новых зависимостей |
| 13 | golden | `git status --porcelain -- logs functions evidence` | пусто; golden не изменялись |
| 14 | live-значения | `reference-oracle/gw-proto/msgs-run2-95974_10029.jsonl:57` (s=51), run1 s=111, run2 s=112/52/53 | значения v02/v04 совпадают; в v03 расходится `self_mute` (см. P3-3) |

### Что проверено по коду

- `VoiceState`/`VoiceStateUpdate`/`VoiceChannelStartTime`/`VoiceChannelStatus` — поля и
  `Option`-ность по DESIGN §3; `session_id` отсутствует; `connected_at_ms` — checked
  `u64`-конверсия и умножение; `VoiceState` не хранит `User`, поэтому `Debug` не печатает
  member payload.
- `parse_voice_state_update`: обязательные `guild_id`/`user_id`/`channel_id` (nil допустим),
  optional `connected_at`/флаги/`discoverable`/`request_to_speak_timestamp`, `member` с
  проверкой `member.user.id == user_id`; пути диагностики `voice.*`; `d` не map — `WrongType`.
- `parse_voice_states`: nil/absent → пусто; не list → `malformed(path)`; битые элементы
  пропускаются; parent `guilds[i].id` приоритетен, fallback на `guild_id` элемента;
  расхождение → `inconsistencies voice:<parent>:<user>:guild:<element>`; nil `channel_id` →
  `inconsistencies …:channel:nil`; дубликаты не диагностируются, порядок сохраняется.
- `parse_voice_channel_start_time`/`parse_voice_channel_status`: nil/missing/пустая строка
  трактуются как очистка, отрицательный/нечисловой timer — `WrongType`, не очистка.
- `apply_update`: upsert `State.users` только при совпадении id, затем `Some` → insert,
  `None` → remove; `apply_snapshot` — последовательный upsert/remove без очистки карты;
  `apply_start_time` — `Some(v >= 0)` → insert, иначе remove; `apply_status` — непустая
  строка → insert, иначе remove.
- `READY` создаёт пустые карты; `READY_SUPPLEMENTAL` вызывает `apply_snapshot`;
  `MessageCreate` и прочие события voice не трогают.
- 13 селекторов: точный guild-фильтр, сортировка по `user_id` через BTreeMap, `guild_count`
  по уникальным guild, `self_voice_state` — первый по `(guild,user)`, `connected_at < 0` →
  `None`, checked/saturating арифметика, пустой статус → `None`.
- Интеграция: `EventPayload`-варианты, routes, arms в `apply_event`, `merge_supplemental`,
  re-exports `state/mod.rs` и `src/lib.rs`; shared-тесты `state_types` 15/15 и
  `state_reducer` 19/19 входят в общий прогон.

## 2. P0/P1/P2

P0 и P1 не найдены.

### P2-1 — аддитивность снимка и last-wins дубликатов не проверены; v09 не отличает merge от replace

**Severity:** P2 (блокирует приёмку).

**Где:**
`rust/crates/discord-core/src/state/voice/reducer.rs:26-31` (`apply_snapshot`),
`rust/crates/discord-core/tests/voice_state_reducer.rs:140-173` (v09),
`waves/B01/MUTATIONS.md:37` (V8), `/tmp/opencode/b01-mutations.sh:56`.

**Контракт:** DESIGN §1 — «`READY_SUPPLEMENTAL` добавляет снимок аддитивно»; §4.2 —
«дубликаты `(guild,user)` в одном снимке — last wins, `duplicates` не писать»; §5 —
«`apply_snapshot`: для каждой записи с `channel_id Some` — upsert; `None` — remove. Снимок
не очищает карту целиком; `READY` уже её сбросил».

**Факт:** v09 — единственный voice-тест, проверяющий снимок в `State`. Он начинает с
`minimal_state()` (`State::from_ready`, пустые карты) и подаёт `003`, где ровно одна запись.
Shared-тесты (`state_reducer::r03`, `state_selectors::s04`, `presence_reducer::r04`) тоже
применяют `003` после `READY`, но voice-карту не проверяют. Ни один тест не подаёт снимок
в непустой voice-store, не содержит двух элементов с одинаковым `(guild,user)` и не
проверяет remove-ветку `channel_id: None` (парсер такую запись не создаёт, но §5 прямо
описывает её для прямого вызова). Следствия по инспекции:

- мутант «`state.voice_states.clear()` перед циклом» проходит все 15 тестов: v09 стартует
  с пустой карты, поэтому очистка ничего не меняет, а затем вставляется одна запись;
- мутант «first wins» (`if !contains_key { insert }`) также проходит: дубликатов в тестах
  нет;
- V8 из `MUTATIONS.md` — это **не** clear-before-loop: скрипт очищает карту на каждой
  итерации и выбрасывает запись (`let _ = voice; …clear();`), поэтому v09 падает на
  `voice_state_count() == 0`. Red-лог V8 получен на пустом store и не доказывает
  сохранение предсуществующих записей; clear-before-loop (потеря только старых записей)
  и first-wins дубликатов не проверяются вовсе.

```rust
// reducer.rs:27-30 — очистки нет, но это проверяется только кодом
for voice in voice_states { apply_voice_state(state, voice); }

// tests/voice_state_reducer.rs:143 — store заведомо пуст
let (mut state, _) = apply_frame(minimal_state(), &load("003-READY_SUPPLEMENTAL.json"));
```

**Почему P2:** это не косметика и не единичная ветка парсера. Незащищённой остаётся
семантика, ради которой снимок вообще отделён от `READY`: при регрессе в `clear()`/replace
молча теряются voice-записи гильдий, которых нет в supplemental, а при first-wins —
побеждает устаревший дубликат. Все 15 тестов при этом зелёные, то есть карточка не
удерживает замороженный контракт. Mutation-отчёт создаёт ложное впечатление, что V8 эту
ветку покрывает.

**Закрытие red-first:**

1. Добавить `v09b_snapshot_merges_into_existing_store`: в `State` сначала применить live
   join для второго пользователя/гильдии, затем `003`; ожидать **2** записи, обе с
   исходными каналами, `voice_state_count() == 2`.
2. Добавить `v09c_snapshot_duplicate_last_wins`: synthetic `READY_SUPPLEMENTAL` с двумя
   элементами одного `(guild,user)` и разными `channel_id`; ожидать последний канал и
   отсутствие `duplicates`.
3. Добавить прямой вызов `apply_snapshot` с `channel_id: None` для существующей записи;
   ожидать remove (ветка §5, недостижимая через парсер).
4. На текущем коде новые тесты должны быть зелёными (поведение корректно); red-first
   доказывается мутантами **V8b** (`clear-before-loop`) и **V8c** (`first-wins`) — оба
   обязаны дать exit 101 на новых тестах. Логи — в `waves/B01/mutations/` или
   `waves/reviews/p2-10/`.
5. Зафиксировать расширение в `TEST-MATRIX-P2-10.md` через `TEST-CORRECTIONS.md`/повторный
   freeze Team Lead, а не молчаливой правкой матрицы.
6. Перепрогнать targeted (15 + новые) и подтвердить md5 восстановления после мутантов.

## 3. P3 backlog (не блокирует `accepted`)

### P3-1 — ветки ошибок `parse_voice_states` не покрыты тестами

- **Где:** `rust/crates/discord-core/src/state/voice/parser.rs:50-134`; тест
  `tests/voice_state_types.rs:89-126` (v01) — только happy path `003`.
- **Факт:** не проверены: не-list → `malformed(path)`; не-map элемент → `malformed(path[i])`;
  битый/отсутствующий `user_id`; fallback на `guild_id` элемента при `parent == None`;
  расхождение parent/element → inconsistency и приоритет parent; nil `channel_id` →
  `inconsistencies …:channel:nil`; отсутствующий/нечисловой `channel_id` → malformed.
  Отдельно: невалидный `guild_id` элемента (`0`/строка) фильтруется в `None` и молча
  заменяется parent — диагностики нет, хотя §4.2 говорит про «есть и не равен parent».
  Пути диагностики — часть контракта §4.2, но ни один из них не сверяется тестом.
- **Почему P3:** паник нет, основные ветки §4.2 реализованы; невалидный element
  `guild_id` — диагностическая мелочь на поле, которого нет в fixture/live, а не
  наблюдаемый дефект данных.
- **Закрытие:** table-driven тест (напрямую `parse_voice_states` или synthetic
  `READY_SUPPLEMENTAL` с двумя гильдиями) на точные пути и отсутствие паник; red-first не
  обязателен, если тест фиксирует уже корректное поведение, но мутация одной ветки полезна.

### P3-2 — V2 не проверяет удаление guard’а `member_user.id == user_id`

- **Где:** `waves/B01/MUTATIONS.md:31` («upsert member без проверки `user_id`»),
  `/tmp/opencode/b01-mutations.sh:50` (sed `==` → `!=`),
  `rust/crates/discord-core/src/state/voice/reducer.rs:18-22`,
  `tests/voice_state_reducer.rs:81-114` (v07), `tests/voice_state_types.rs:365-435` (v06).
- **Факт:** V2 инвертирует условие, а не удаляет проверку. Падение `V2.log` — это v07:
  при совпадающем member upsert не происходит. Настоящий мутант «`if let Some(user) =
  member_user { upsert_user(...) }`» (без сравнения id) выживает: парсер v06 при
  несовпадении возвращает `member_user = None`, поэтому до reducer’а такой случай не
  доходит, а v07 использует совпадающий id.
- **Почему P3:** в текущем потоке событий парсер уже отфильтровывает mismatch, поэтому
  пользовательского дефекта нет; guard — defense-in-depth, но он заявлен в §5 и остаётся
  непроверенным.
- **Закрытие:** прямой unit-тест reducer’а: собрать `VoiceStateUpdate` с
  `member_user.id != state.user_id`, вызвать `apply_update`, проверить, что `State.users`
  не изменился, а voice-запись применилась; затем мутант с удалённой проверкой → exit 101,
  восстановить md5.

### P3-3 — v03 расходится с capture по `self_mute`

- **Где:** `tests/voice_state_types.rs:175-211` (v03, `self_mute` false в кадре и
  ассерте), `waves/RW-19/TEST-MATRIX-P2-10.md:18` («флаги как в capture»), capture
  `reference-oracle/gw-proto/msgs-run2-95974_10029.jsonl:57`.
- **Факт:** в live s=51 `self_mute: "true"`, `self_deaf/self_video/suppress/mute/deaf` —
  false; тест ставит `self_mute=false` и утверждает false. Guild/user/channel/connected_at
  совпадают, поэтому содержательная проверка leave не страдает, но заявление матрицы
  «флаги как в capture» неверно.
- **Почему P3:** это точность оракула/тестового вектора, а не дефект продукта; `self_mute`
  отдельно покрыт v02 (`true`).
- **Закрытие:** либо поставить в v03 `self_mute=true` и ассерт true (точное повторение
  capture), либо переформулировать матрицу: «флаги синтетические false, capture-значения
  проверяются в v02». Второй вариант дешевле и не меняет смысл теста.

### P3-4 — набор мутаций расходится с DESIGN §9 (M3/M4/M5/M8)

- **Где:** `waves/RW-19/DESIGN-P2-10.md:301-306` (§9, M1–M8),
  `waves/B01/MUTATIONS.md:28-38` (V1–V8).
- **Факт:** DESIGN перечисляет M3 `discoverable default false`, M4 `connected_at` без
  умножения на 1000, M5 `voice_start_time nil` не очищает, M8 stale-политика; в batch
  этих мутантов нет. Поведенчески ветки покрыты (v01/v10/v11/v14), но mutation evidence
  не подтверждает их силу. Особенно важен default `discoverable`: цель §9 — «100 % для
  критических ветвей: выход, member-id, таймер, default», а default-ветка не мутировалась.
- **Почему P3:** 8/8 заявленных мутантов пойманы, бюджет 5–8 соблюдён; это неполнота
  mutation-плана, а не дефект продукта.
- **Закрытие:** в следующем batch добавить M3/M4 (и, при желании, M5/M8) или явно
  зафиксировать в матрице, почему они заменены V-набором; логи — по одному на мутант.

### P3-5 — nil optional bool молча даёт default, хотя §4.1 говорит «иначе default + ignored»

- **Где:** `rust/crates/discord-core/src/state/voice/parser.rs:372-397`
  (`optional_bool`, nil-ветка 383-385); `tests/voice_state_types.rs:298,311,333-343`
  (v05).
- **Факт:** `value::is_null(raw)` возвращает default без `diagnostics.ignored`; v05
  специально утверждает, что `voice.self_mute` **не** попадает в `ignored` при nil.
  DESIGN §4.1 для флагов: «bool/атом `true|false` → значение; иначе default +
  `ignored "voice.<field>"`». Nil — это «иначе», если читать буквально.
- **Почему P3:** live-провод присылает атомы `true/false`, а не nil (RESEARCH §2.2), так
  что практического расхождения нет; но контракт и тест фиксируют разные трактовки, и
  диагностика теряет сигнал о неожиданном nil.
- **Закрытие:** либо добавить `ignored` для nil (и поправить v05), либо уточнить §4.1:
  «nil/missing — default без диагностики; ignored только для не-bool значения». Решение —
  за Team Lead, чтобы не менять замороженный контракт молча.

### P3-6 — неточности в `w19-report.md`

- **Где:** `waves/RW-19/w19-report.md:27-29`, `:59`.
- **Факт:**
  1. «37 ошибок» — это счётчик последнего target’а (`voice_state_selectors`, строка cargo
     `due to 37 previous errors`); всего в логе три target’а: 6 + 21 + 37 = **64**
     диагностики `error[E…]`. Формулировка читается как общее число.
  2. `apply_snapshot` не может «merge `member_user` в `State.users`»: он принимает
     `Vec<VoiceState>`, где `User` отсутствует; `parse_voice_states` разбирает `member`
     только для диагностики и отбрасывает валидного пользователя (`parser.rs:129-130`).
  3. `apply_update` не содержит stale-логики; stale-проверка — в shared
     `state/reducer.rs` (`apply_frame_at`) до применения события.
- **Почему P3:** отчётные формулировки не влияют на продукт/тесты, но вводят в
  заблуждение при приёмке и следующем ревью.
- **Закрытие:** поправить три строки отчёта (64/6+21+37; snapshot не трогает `State.users`;
  stale — shared reducer).

### P3-7 — матрица v01 обещает пустые diagnostics, тест проверяет только voice-диагностику

- **Где:** `waves/RW-19/TEST-MATRIX-P2-10.md:16` («diagnostics пусты»),
  `tests/voice_state_types.rs:92-105` (v01).
- **Факт:** `003` содержит дубликат `merged_members[0].members:289091818442719242`
  (P2-01, не voice), поэтому `diagnostics.duplicates` не пуст; тест корректно проверяет
  пустоту `malformed`/`ignored`/`inconsistencies` и отсутствие voice-префиксов в
  `duplicates`, но матрица этого не оговаривает.
- **Почему P3:** тест сильнее и точнее матрицы; расхождение только в документе.
- **Закрытие:** заменить в матрице «diagnostics пусты» на «voice-diagnostics пусты;
  `duplicates` содержит только `merged_members…` от P2-01».

### P3-8 — behavior-red лог отсутствует, хотя DoD ТЗ требует compile и behavior

- **Где:** `waves/RW-19/TZ-P2-10.md:70` («red-лог с exit 101 (compile и behavior)»),
  `waves/RW-19/TEST-MATRIX-P2-10.md:45-47` (допускает compile-red **или** behavior-red),
  `waves/RW-19/w19-report.md:61-63` (behavior-red не снимался).
- **Факт:** сохранены два compile-red лога (до и после product-кода), behavior-red нет.
  Отчёт объясняет, что до I0/I1 поведенческий прогон недостижим; после I0 можно было
  снять behavior-red на заглушках, но этого не сделано. ТЗ и матрица противоречат друг
  другу.
- **Почему P3:** red-first по существу подтверждён compile-red’ом с exit 101 и
  последующим green; процессный артефакт не влияет на код.
- **Закрытие:** либо согласовать DoD с матрицей (compile-red достаточно для интеграционной
  карточки), либо снять behavior-red на следующем batch и сохранить его в `waves/RW-19/`.

## 4. Качество тестов и mutation evidence

- 15 тестов соответствуют матрице по ID и покрывают happy path `003`, live join/leave,
  timer/status, optional/broken-таблицу, member mismatch, reducer join/move/leave, READY
  reset, stale/malformed, селекторы и границы.
- Сильные стороны: реальный fixture с проверкой md5, точные live-значения s=111/51/112/52/53,
  проверка `State.users` и неизменности `Guild.members`, saturating/checked-границы,
  отсутствие паник на битом входе.
- Слабые места: snapshot-merge (P2-1) и error-ветки snapshot-парсера (P3-1); mutation-набор
  не полностью соответствует §9 (P3-4), V2-формулировка расходится с фактическим
  мутантом (P3-2), V8 не проверяет clear-before-loop (P2-1).
- 16/16 мутаций — по логам; выборочно проверены V2, V3, V5, V8: exit 101, после
  восстановления md5 reducer/selectors совпадает с backup. Остальные 12 логов проверены
  по `summary.txt` и наличию `EXIT=101`, без повторного запуска.

## 5. Ограничения ревью

- Полные `cargo test --workspace`, core-503 и gate не перезапускались (режим ревью и
  готовые gate-артефакты); проверены суммы `test result: ok`, отсутствие `FAILED` и
  `verdict: passed`.
- Мутанты V8b (clear-before-loop) и V8c (first-wins) **не запускались** из-за read-only;
  вывод об их выживании — по инспекции тестов (v09 — единственный voice-snapshot-тест,
  store пуст, дубликатов нет; shared-тесты voice-карту не проверяют). Это и есть
  требование к закрытию P2-1: QA должен получить реальные red-логи.
- Live Discord, SQLite/UI/keyring и запись в golden не запускались; capture читался
  только как источник значений.
- Секреты/токены не проверялись отдельным сканером; в voice-коде нет логирования
  `session_id`/member payload, `Debug` показывает только длины.

## 6. Условия повторного ревью

1. Закрыть P2-1: добавить v09b/v09c (+ remove-ветку) и mutation-red логи
   clear-before-loop/first-wins с exit 101; зафиксировать расширение матрицы через
   `TEST-CORRECTIONS.md`.
2. Targeted с новыми тестами — зелёный, exit 0; md5 продуктовых файлов не изменился или
   изменения объяснены.
3. P3-1…P3-8 могут остаться backlog’ом; для `accepted` достаточно отсутствия новых
   P0/P1/P2. P3-6 (отчёт) и P3-7 (матрица) желательно поправить одновременно с P2-1,
   чтобы журнал волн не расходился с фактами.
4. Если для P2-1 всё же меняется продуктовый код (например, `apply_snapshot`), нужен
   полный batch-gate и повторный targeted, а не только review-прогон.
