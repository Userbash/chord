# Независимое ревью RW-18 / P2-09 — memory-only presence

Дата: 2026-10-20. Ревьюер — отдельный агент, не автор кода. Режим: read-only + прогоны;
продуктовые файлы и тесты не изменялись. Контракт: `waves/RW-18/DESIGN-P2-09.md`,
`waves/RW-18/RESEARCH-P2-09.md`, `waves/RW-18/TZ-P2-09.md`, `waves/RW-18/TEST-CORRECTIONS.md`.
Артефакты прогонов: `waves/reviews/p2-09/`.

## Вердикт раунда 1 — **не принято**

**P0 = 0, P1 = 0, P2 = 2, P3 = 14.**

Карточка реализована в основном корректно: маршрут и парсер `PRESENCE_UPDATE`,
`READY_SUPPLEMENTAL`, member-list/chunk-интеграция, offline/all-offline семантика,
нормализация activities, безопасные поля и `State::Debug` — в порядке. Блокируют приёмку
два P2: расхождение агрегата с официальным oracle (offline+hidden понижает online) и
молчаливое отбрасывание пустого `status` из member-list. Дополнительно нет обязательного
доказательства мутаций: M1–M7 в `waves/RW-18/MUTATIONS.md` пока «ожидает», а `gate:worker`
в `REPORT.md` — «ожидает».

## P2 — блокируют приёмку

### P2-1 — offline+hidden scope понижает online-агрегат

**Где:** `rust/crates/discord-core/src/state/presence_selectors.rs:17-30` (выбор агрегата),
`rust/crates/discord-core/src/state/presence.rs:297-301` (`is_visible`), тест
`rust/crates/discord-core/tests/presence_selectors.rs:129-148` (`s04`).

**Оракул:** `waves/RW-18/RESEARCH-P2-09.md:102-103`: «Агрегат: выбрать scope с максимальным
`processedAtTimestamp`; при равенстве — с большим числом activities. Если выбранный offline,
но не все — статус не понижается».

**Факт:** `presence()` фильтрует `is_visible()` (offline+hidden считается видимым), затем
берёт max по `(processed_at_timestamp, activities.len(), PresenceKey)` и возвращает эту
запись без проверки, есть ли другой видимый online/idle/dnd scope. Независимый пробник
(`aggregation-probe.log`) на synthetic-кадрах:

- guild `10`: `online`, processed `100`;
- guild `20`: `offline` + hidden activity, processed `200`;

даёт:

```text
status = Offline
is_online = false
selected_scope = Some(20)
guild10_present = true
guild20_present = true
hidden_len = 1
```

Официальный store в этой ситуации обязан сохранить online. Баг влияет не только на
`status()`, но и на `is_online()`, `activities()`, `client_status()`, `online_user_ids()` и
`primary_activity()`, потому что все они идут через `presence()`.

**Почему P2:** пользователь видит offline при наличии live online-сессии; это ровно тот
класс расхождений с oracle, который карточка должна исключать. `s04` проверяет только
offline **без** hidden, поэтому не ловит случай.

**Закрытие red-first:**

1. Добавить `s15_offline_hidden_does_not_downgrade_online`: online scope processed `100`,
   offline+hidden scope processed `200`; ожидать `Online`, `is_online == true`, activities
   от online-scope; отдельно проверить, что offline-scope остаётся доступен через
   `presence_in_guild`.
2. На старом коде зафиксировать падение (ожидаемо `Offline`).
3. Исправить выбор: если среди видимых scopes есть хотя бы один online/idle/dnd — выбирать
   лучший среди них; offline+hidden выбирать только когда все видимые scopes offline.
   Альтернатива — синтезировать статус поверх выбранной записи, но текущий возвращаемый тип
   `Option<&Presence>` этого не позволяет без изменения API.
4. Согласовать формулировку `DESIGN-P2-09.md` §6 с oracle: сейчас она говорит только
   «max по `(processed, activities.len())`» и буквально допускает текущее поведение.
5. Перепрогнать targeted и полный workspace.

### P2-2 — пустой `status` member-list молча отбрасывается

**Где:** `rust/crates/discord-core/src/state/member_list.rs:525-534` (парсер принимает
`Some("")`), `rust/crates/discord-core/src/state/presence_reducer.rs:66-68` (фильтр
`!status.is_empty()` и `return` без диагностики).

**Контракт:** `DESIGN-P2-09.md` §7: «Ошибки не глотаются: каждая ветка либо меняет данные,
либо пишет диагностику, либо задокументирована как no-op (offline без записи)». §5
разрешает no-op только для `status: None`; пустая строка — не `None`, а битый текст.
Для `PRESENCE_UPDATE` парсер уже отвергает пустой status с `ignored` (`presence.rs:543-563`),
для member-list — нет.

**Факт:** member-list с `member.presence.status = ""` даёт `MemberPresence { status:
Some("") }`, затем `apply_member_presence` молча возвращается. Ни presence, ни `ignored`,
ни `malformed`, ни `inconsistencies` не появляются. Это противоречит и §7, и поведению
`client_status`, где пустые key/value уже диагностируются (`member_list.rs:552-563`).

**Почему P2:** это не косметика, а явное нарушение политики диагностики; при разборе
реального member-list такой кадр невозможно отличить от «presence отсутствует».

**Закрытие red-first:**

1. Добавить `r18_member_list_empty_status_is_diagnosed`: synthetic member-list с
   `status: ""`; ожидать отсутствие presence и `ignored`, содержащий `member.presence.status`
   (точный путь — по фактическому парсеру).
2. На старом коде зафиксировать падение (диагностики нет).
3. В `member_list.rs` заменить `Some(status) => Some(status.to_string())` на
   `Some(status) if !status.is_empty() => Some(...)`, иначе `ignored` + `None`. Фильтр в
   reducer можно оставить как defense-in-depth, но он не должен быть единственной защитой.
4. Перепрогнать targeted.

## P3 — backlog, не блокируют сами по себе

1. **`InvalidId.value` печатает весь user-объект.** `presence.rs:343-346`:
   `value: format!("{user_raw:?}")` вместо `raw_display` (`state/types.rs:710-718`).
   Это отличается от остальных парсеров и может вывалить большой/непредсказуемый объект в
   ошибку. Исправить на `raw_display` значения `id` (например,
   `raw_display(user_raw.get("id").unwrap_or(user_raw))`) и проверить в `a08`, что value —
   id, а не whole map.

2. **`optional_guild_id` принимает `0`.** `presence.rs:925-941`: `value::unsigned`
   возвращает `Some(0)`, а `DESIGN-P2-09.md` §3 требует положительную снежинку. Добавить
   `Some(id) if id > 0`, иначе `ignored "presence.guild_id"`; тест на `guild_id: "0"` →
   `None` + диагностика.

3. **Лишняя merged-группа даёт лишнюю inconsistency.** `presence.rs:417-428`: для
   `index >= guild_ids.len()` непустая группа получает и `malformed` о несовпадении длины,
   и `inconsistencies "merged_presences.guilds[i]"`. `DESIGN-P2-09.md` §3 для лишних групп
   говорит «malformed/игнор». Либо убрать inconsistency, либо зафиксировать её в DESIGN;
   `a16` сейчас не проверяет, что лишняя группа не слилась в последнюю валидную.

4. **`is_offline` и Unknown/Invisible.** `presence_selectors.rs:172-176`:
   `is_offline = !is_online`, поэтому `Unknown("away")` и `Invisible` считаются offline,
   хотя `PresenceStatus::is_offline()` (`presence.rs:69-73`) — только `Offline`. Решить,
   что является контрактом (отсутствие + offline или «не online»), и добавить тест; сейчас
   поведение не зафиксировано.

5. **`s09` не проверяет положительный VR и конфликты клиентов.**
   `tests/presence_selectors.rs:282-293`: мобильный online + vr idle, поэтому
   `is_vr_online` всегда false. Мутации «`is_vr_online` всегда false» и «убрать проверку
   desktop/vr в `is_mobile_online`» выживут. Добавить synthetic: vr online → true;
   mobile online + desktop online → `is_mobile_online == false`; desktop only → false.

6. **Не покрыт displayable tie-break и list-форма `party.size`.**
   `presence.rs:973-984`: сортировка имеет третий ключ `has_displayable_content`, но `s06`
   и `s07` используют разные `created_at`, поэтому третий ключ не срабатывает. Добавить две
   activity одного приоритета и `created_at`, одну с `details`/`party`, другую без, и
   проверить порядок. `parse_party_size` (`presence.rs:791-821`) поддерживает list
   `[current, max]`, но `a14` проверяет только байтовую строку `"\x01\x03"`; добавить
   list-форму.

7. **Не покрыты селекторы `client_status_in_guild` и `presence_scope(Private)`.**
   `presence_selectors.rs:111-118`, `:38-42`; в тестах `s05` вызывает `presence_scope`
   только с `Guild(10)` (Private есть лишь в `presence_scopes`), `client_status_in_guild`
   не вызывается вовсе. Добавить тесты (про `presence_keys` — P3-14).

8. **`a15`/`a16` не проверяют выравнивание и root re-export по-настоящему.**
   `tests/presence_types.rs:435-466`: `a15` проверяет длины групп `[4,4,3,0,1,14,0]` и
   общее число, но не то, что у каждого presence `guild_id` соответствует индексу группы.
   `a16` (`:467-526`) импортирует типы из `discord_core::state` (`:14-17`), а не из
   `discord_core::{...}`, поэтому root re-export не тестируется; также нет проверки, что
   user `200` из лишней группы не попал в `guilds[0]`. Исправить импорт и добавить точные
   assertions. Отдельно не проверен missing/null `merged_presences` →
   `MergedPresences::default()`.

9. **`r04`/`r10` называют себя «READY+003», но используют `minimal_state()`.**
   `tests/presence_reducer.rs:187-190` и `:336-339`. Из-за этого не проверяется интеграция
   с полным `002-READY` и не проверяется отсутствие ложных
   `presences:user:<id>:missing` для известных пользователей. Перевести на
   `apply_named` от `002-READY` и добавить проверку диагностики.

10. **`r15` применяет тот же `010`-кадр.** `tests/presence_reducer.rs:425-438`: мутант
    «записать stale, но всё равно применить событие» выживет — payload идентичен, поэтому
    значения presence совпадут, а `stale_sequences` останется заполненным. Нужен второй кадр
    с тем же seq и **другим** status/activity, затем assert, что presence не изменился.

11. **Doc-comment `r09` противоречит телу.** `tests/presence_reducer.rs:313`:
    «activities очищаются», а `:328-333` проверяют сохранение. Исправить комментарий на
    «сохраняются при hidden» (это уже соответствует oracle и TZ).

12. **Артефакты/документация.** `waves/RW-18/MD5SUMS.txt` покрывает 10 файлов и не включает
    `state/mod.rs`, `src/lib.rs`, `tests/state_types.rs`, `tests/state_reducer.rs`,
    `tests/member_chunk_types.rs`, хотя они менялись в P2-09. `green-w18b-first.log` назван
    green, но содержит `r09 ... FAILED` и `green-w18b-first.exit=101`; это attempt-лог,
    его стоит переименовать. `DESIGN-P2-09.md` §8 говорит «три места» (контрактные области),
    а `TEST-CORRECTIONS.md`/`REPORT.md` — четыре контрактных места (в трёх файлах);
    формулировки стоит унифицировать.
    Это не влияет на код, но мешает воспроизводимости.

13. **TZ менялся в ходе ревью.** `waves/RW-18/TZ-P2-09.md` имеет mtime `2026-10-20
    21:36:40`; формулировка `r09` приведена к «activities сохраняются». Текущий текст
    соответствует коду, тесту и oracle, но правка контракта во время ревью должна быть
    подтверждена координатором и отражена в журнале волн; иначе review шёл по движущейся
    цели. Кодовых изменений не требует.

14. **Лишний публичный API.** `presence_selectors.rs:217-219` (`presence_keys`) и
    `presence.rs:240-251` (`pub fn has_displayable_content`) не перечислены в
    `DESIGN-P2-09.md` (`presence_keys` — вне §6; `has_displayable_content` — helper §2/§4).
    `has_displayable_content` используется только внутри `compare_activity`, его можно
    сделать `pub(crate)`; `presence_keys` либо покрыть тестом, либо убрать/описать как
    extension.

## Соответствие контракту

| Область | Итог | Основание |
|---|---|---|
| `PRESENCE_UPDATE` route/parser | ✅ | `a01`–`a14`, `r01`–`r03`; фикстуры `010/012/018/043/062` |
| `READY_SUPPLEMENTAL` merged presences | ✅ (с оговоркой) | `a15`, `r04`; длины/null-группы верны, но `a15` не проверяет привязку `guild_id` к индексу (P3-8) |
| member-list presence | ⚠️ | `r10`, `r11`, `r12`, `r17`, `s14`; пустой status молча теряется (P2-2) |
| chunk `presences[]` best-effort | ✅ | `r13`, `r14`, `c13`; форма `PRESENCE_UPDATE`-like, unconfirmed-shape диагностируется |
| offline/all-offline семантика | ✅ | `r06`–`r09`, `s04`, `s12`; offline без hidden удаляет, hidden сохраняет видимость |
| агрегат/селекторы | ❌ | offline+hidden понижает online (P2-1); остальные проверенные селекторы корректны, семантика `is_offline` для Unknown/Invisible не зафиксирована (P3-4) |
| нормализация activities | ✅ (с пробелами) | `s06`–`s08`, `a14`; displayable tie-break и list `party.size` не покрыты (P3-6) |
| диагностика | ⚠️ | в основном есть; P2-2, P3-1/2/3 |
| API/root re-exports | ✅ | `src/lib.rs:44-49`, `state/mod.rs:32-35`; тест `a16` проверяет не root (P3-8) |
| `State`/`ReadySupplemental` | ✅ | `State::from_ready` даёт пустой `presences`; `merged_presences` в payload; `Debug` — счётчик |
| P2-04 regression | ✅ | `apply_member_presence` не вызывает `upsert_user`; `r12` чувствителен: в `008` пять user id, отсутствующих в `002` (проверено jq) |
| безопасность | ✅ | нет `session_id`/`secrets`/`sync_id`; `Debug` редактирует `session_id`; нет `unwrap`/`expect`/`unsafe` в P2-09 product-файлах |
| untrusted fixtures | ✅ | битые merged/member/chunk элементы дают диагностику, без паник |

## Качество 47 тестов и мутационные пробелы

- 47 тестов: 16 `presence_types`, 17 `presence_reducer`, 14 `presence_selectors`.
  Оракул в основном независимый: реальные фикстуры `010/012/018/043/062/002/003/008/022`
  плюс synthetic для границ; проверяются поля, счётчики, scope, диагностика.
- Red-first соблюдён: `red-w18a/b/c.exit=101` (compile-red), green `16/16`, `17/17`,
  `14/14`; `green-w18b-first.exit=101` — отдельная неудачная попытка `r09`, не green.
- Сильные места: `a15` сверяет реальные числа `003`; `r12` реально ловит upsert в
  `State.users`; `c13` проверяет, что неизвестная форма не изобретается; `r13` проверяет
  default guild для chunk.
- Пробелы и слабые API (P3-4…P3-10, P3-14): положительный `is_vr_online`, desktop/vr-конфликт,
  displayable tie-break, list `party.size`, `client_status_in_guild`,
  `presence_scope(Private)`, `presence_keys`, точное выравнивание `a15`, root re-export
  `a16`, полный READY в `r04/r10`, реально изменившийся stale-кадр `r15`, пустой status
  `r18`.
- Планируемые мутации M1–M7 **не выполнены** (`MUTATIONS.md` — «ожидает»), поэтому
  «caught» не подтверждён. Ожидаемо: M2 → `r07/r08`, M3 → `s06/s07`, M4 → `r10`,
  M5 → `r13`, M6 → `r09/s12`, M7 → `s07`. M1 покрыта `s02/s04` лишь частично: случай
  offline+hidden не ловится (P2-1). Дополнительно выживут мутанты: `is_vr_online => false`,
  удаление desktop-guard в `is_mobile_online`, удаление displayable tie-break, удаление
  stale-return при одинаковом payload, принятие `guild_id=0`, отсутствие диагностики
  пустого member-status.

## Существующие тесты: правки оправданы

- `state_types::t08` и `state_reducer::r10`: `PRESENCE_UPDATE` больше не `Unsupported`;
  замена на `TYPING_START` сохраняет проверку ветки `Unsupported`, а не ослабляет её.
- `state_reducer::r17`: добавлено обязательное поле `merged_presences: Default::default()`;
  проверяемое поведение (invalid raw index → inconsistency) не изменилось.
- `member_chunk_types::c13`: P2-05-отложенная форма `presences[]` теперь best-effort;
  новый тест проверяет пустой `chunk.presences`, `malformed d.presences[0]` и
  `unconfirmed-shape`, то есть сильнее старого `ignored`.
- Черновые `r09`/`s08` исправлены до green по oracle и описаны в `TEST-CORRECTIONS.md`;
  `r09` — не подгонка под код, а исправление ошибочного ожидания TZ (official store
  сохраняет activities при offline+hidden). Это подтверждается `RESEARCH-P2-09.md:99-103`.
- Замечание: `DESIGN-P2-09.md` §8 говорит «три места», а `TEST-CORRECTIONS.md`/`REPORT.md`
  — четыре контрактных места (в трёх файлах); это docs-несогласованность (P3-12), не
  скрытая правка.

## Прогоны

| Команда | Результат |
|---|---|
| `cd rust && cargo test --offline -p discord-core --test presence_types --test presence_reducer --test presence_selectors` | **47 passed, 0 failed**, exit 0 (`waves/reviews/p2-09/presence-targeted.log`) |
| `cd rust && cargo test --offline -p discord-core --test member_list_types --test member_list_reducer --test member_chunk_types --test state_types --test state_reducer` | **84 passed, 0 failed**, exit 0 (`regression-targeted.log`) |
| `cd rust && cargo test --offline --workspace` | **508 passed, 0 failed**, exit 0 (`workspace-tests.log`) |
| `cd rust && cargo fmt --all -- --check` | exit 0 (`fmt-check.log`) |
| `cd rust && cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 (`clippy.log`) |
| `md5sum -c waves/RW-18/MD5SUMS.txt` | 10/10 OK (но список неполный, P3-12) |
| `red-w18a/b/c.exit` | 101, compile-red |
| `green-w18a-first` / `green-w18b` / `green-w18c` | 16/16, 17/17, 14/14, exit 0 |
| `green-w18b-first.exit` | **101**, `r09 ... FAILED` (attempt-лог, P3-12) |
| `npm run gate:worker` | ревьюером не запускался; в `REPORT.md` — «ожидает» |

Независимый пробник агрегата: `waves/reviews/p2-09/aggregation-probe.log` (P2-1).

## Отклонения scope / процесса

- `presence_keys` и публичный `has_displayable_content` — расширение API вне §6 (P3-14).
- `DESIGN-P2-09.md` §6 не фиксирует oracle-правило «не понижать online» (P2-1); это
  контрактная дыра, а не только баг реализации.
- `TZ-P2-09.md` менялся в ходе ревью (P3-13); текущая версия соответствует oracle, но
  требуется подтверждение координатора и запись в журнале.
- `MD5SUMS.txt` не покрывает часть изменённых файлов; `green-w18b-first` назван неверно
  (P3-12).
- Живой Discord/live capture не использовались — это соответствует scope карточки
  (memory-only), не является отклонением.

## Условия повторного ревью

1. P2-1 и P2-2 закрыты red-first: есть падающий тест на старом коде, затем green; targeted
   и полный workspace перепрогнаны.
2. `DESIGN-P2-09.md` §6 явно фиксирует правило агрегата (online не понижается), либо
   координатор письменно подтверждает иное решение; `TZ-P2-09.md` r09-правка подтверждена
   и отражена в журнале.
3. M1–M7 реально выполнены, каждая caught, исходники восстановлены, `md5sum -c` проходит;
   добавлены мутанты на `is_vr_online`/desktop-guard, displayable tie-break, stale-payload,
   `guild_id=0` и пустой member-status (или зафиксировано, почему они не нужны).
4. `npm run gate:worker` — зелёный; `REPORT.md` обновлён (сейчас «ожидает»).
5. Как минимум P3-12 (MD5SUMS/имя green-лога) и P3-11 (комментарий `r09`) исправлены;
   остальные P3 могут уйти в backlog, но должны быть перечислены в отчёте.
6. Ревьюер повторно проверяет, что продуктовые файлы и тесты не менялись под ревью, и
   прогоняет те же команды.
