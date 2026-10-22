# RW-18 / TEST-CORRECTIONS — P2-09

Дата: 2026-10-20. Исправления существующих тестов вызваны сменой контракта P2-09, а не
подгонкой под реализацию. Это три контрактных изменения в четырёх файлах тестов; каждое
зафиксировано в журнале ревью/отчёте, а red/green соответствующего набора приложены.

## 1. `tests/state_types.rs` — `t08_unknown_and_unsupported_events`

**Было:** `PRESENCE_UPDATE` с пустым `d` ожидался как `EventPayload::Unsupported`.

**Стало:** для проверки `Unsupported` используется `TYPING_START` (известный, но не
поддержанный тип). `PRESENCE_UPDATE` теперь типизирован и на пустом `d` даёт
`Missing("presence.user")`.

**Почему:** P2-09 добавляет `EventPayload::PresenceUpdate` и маршрут
`DispatchKind::PresenceUpdate`; старый тест проверял прежний контракт P2-02.

## 2. `tests/state_reducer.rs` — `r10_unknown_and_unsupported_are_non_fatal`

**Было:** `PRESENCE_UPDATE` с пустым `d` ожидался в `unsupported_events`.

**Стало:** используется `TYPING_START`.

**Почему:** то же изменение контракта; stale/unsupported-политика не менялась.

## 3. `tests/state_reducer.rs` — `r17_supplemental_invalid_raw_index_is_inconsistency`

**Было:** literal `ReadySupplemental { guild_ids, merged_members }`.

**Стало:** добавлено `merged_presences: Default::default()`.

**Почему:** P2-09 добавляет обязательное поле `ReadySupplemental.merged_presences`;
структурный literal должен его заполнить. Проверяемое поведение (invalid raw index →
inconsistency, не malformed) не изменилось.

## 4. `tests/member_chunk_types.rs` — `c13_presences_present_is_ignored_not_invented`

**Было:** `presences[]` с элементом `{user_id,status}` ожидался как `ignored "d.presences"`.

**Стало:** `c13_presences_unconfirmed_shape_is_diagnosed`:
- `chunk.presences` пуст (форма элемента `user_id`-only не принимается);
- есть inconsistency `...:presences:unconfirmed-shape`;
- есть malformed `d.presences[0]` с причиной.

**Почему:** P2-09 реализует отложенный P2-05 пункт: `presences[]` — best-effort только
для элементов формы `PRESENCE_UPDATE`. Форма `user_id`-only в capture не подтверждена и
не изобретается; неизвестность теперь явно диагностируется, а не просто `ignored`.
Новый тест `presence_reducer::r13` проверяет принятие `PRESENCE_UPDATE`-подобного элемента.

## 5. Черновые тесты, исправленные до green (не frozen-контракт)

- `presence_reducer::r09`: официальный store сохраняет activities при offline + hidden
  activities (очищает только offline без hidden). Ожидание приведено к oracle; строка TZ
  r09, требовавшая пустые activities, исправлена (это была ошибка TZ, не кода).
- `presence_selectors::s08`: после custom официальный приоритет даёт playing (1), затем
  listening (0), поэтому `primary_activity` — `play`, не `listen`.

## 6. Правки после независимого ревью (P2-1, P2-2)

Оба P2 закрыты red-first; это исправления продукта, а не ослабление тестов:

- **P2-1, агрегат не должен понижать live-scope.** Red: `red-s15.exit=101`
  (`s15_offline_hidden_newer_does_not_hide_online`), green: `green-p2-fixes.exit=0`.
  `presence()` теперь сначала выбирает лучший `online|idle|dnd` scope и только при его
  отсутствии — лучший `offline+hidden`. Точные guild-селекторы по-прежнему возвращают
  выбранный scope, включая offline+hidden.
- **P2-2, пустой `status` в member-list.** Red: `red-p2-fixes.exit=101` (`r18 ... FAILED`),
  green: `green-p2-fixes.exit=0`. `member_list::parse_presence` теперь диагностирует
  пустой `status` как `ignored` с путём, оканчивающимся на `member.presence.status`;
  reducer сохраняет защитный фильтр.

**TZ r09:** строка 9 TZ требовала очищать activities у offline+hidden. Ревью подтвердило,
что это ошибка TZ: официальный store сохраняет activities, если есть hidden activities.
Исправление внесено в TZ и зафиксировано здесь; Team Lead подтвердил его при подготовке
round 2 (journal note в `waves/db.jsonl`); это не означает `accepted` — карточка ещё на ревью.

Итог после правок: 54 целевых теста (18+18+18), core 473, workspace 515; мутации 13/13
пойманы (`MUTATIONS.md`), `gate:worker` — passed.
