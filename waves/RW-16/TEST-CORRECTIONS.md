# RW-16 / P2-05 — red-статус, поправки и закрытие ревью

## 1. Red

Первый прогон:

```text
cargo test --offline -p discord-core --test member_chunk_types \
  --test member_chunk_reducer --test member_chunk_selectors
```

`waves/RW-16/red-w16.log`, **exit 101**. Это compile-red: модулей `state::member_chunk`,
типов `MemberChunk*`, поля `State::member_chunks` и селекторов ещё нет (26 ошибок
`E0599`/`E0433`). Поведенческий red до появления API невозможен; ограничение зафиксировано
честно.

Поведенческие red после появления API:

- `red-w16-r12.log` / exit 101 — повтор `nonce=None/count=1` ошибочно давал
  `duplicate-index`; тест `r12` упал до исправления правила.
- `red-w16-r13.log` / exit 101 — P2-1 ревью: `nonce=None/count=2` не давал
  `duplicate-index`; тест `r13` упал до сужения условия.

## 2. Поправки тестов и контракта

| Кейс | Было | Стало | Почему |
|---|---|---|---|
| `s08_presence_effect_subrange_and_only_groups` | ожидал `[5313…, 3108…]` для range `2..4` | `[7025…, 5313…]` | позиции flat-list: 2 — member `7025…`, 3 — group `online`, 4 — member `5313…`; группа пропускается |
| `member_chunk_types::chunk_frame` | clippy `too_many_arguments` (8/7) | `#[allow(clippy::too_many_arguments)]` на тестовом конструкторе | конструктор покрывает все поля; продуктовая сигнатура не раздувается |
| `r04` / новый `r12` | `r04` проверял duplicate-index на `nonce=None`, а `r12` ожидал отсутствие диагностики | `r04` переведён на `Some("n")`; `r12` фиксирует повтор `0/1` без nonce как новый ответ | capture: s=78 и s=84 — два независимых `0/1`-ответа одной гильдии |
| P2-1 / новый `r13` | `duplicate-index` подавлялся для любого `nonce=None`, включая `count>1` | диагностика пишется для `nonce=Some` **или** `count>1`; исключение только `nonce=None/count=1` | без nonce при `count>1` корреляция неизвестна; молчаливое подавление скрывает дубль batch-а |

`s08` — test-only: ожидание исправлено по `jq`-оракулу, а не под код. `chunk_frame` — только
тестовая обвязка. `r04/r12/r13` изменили контракт и продукт; это зафиксировано в
`DESIGN-P2-05.md` §4.4/§6.4 и `TZ-P2-05.md` W16b. Ошибочный промежуточный лог
`green-w16-r12.log` (в нём был `FAILED`) переименован в
`attempt-w16-r12-failed.log`; фактическое зелёное доказательство — `green-w16-final.log`.

## 3. Решения по scope

- `presences[]` внутри chunk **не разбирается**: форма не подтверждена capture; при наличии
  поле даёт `ignored` (`d.presences`), чтобы не терять факт молча. Presence-запрос для
  видимого диапазона — `Effect::RequestPresence` (декларация, P2-09/transport).
- `nonce` — optional string; в обоих фикстурах отсутствует, ключ сборки `(guild_id, None)`.
  Wire-корреляция nonce не утверждается.
- `009` — legacy: исходный кадр `InvalidId` на `d.guild_id`; corrected copy с отрицательным
  member-id даёт `malformed` и пустой список member.
- `Guild.members`/`State.users` merge аддитивный: удаление по `not_found` не выполняется,
  `Guild.member_count` не меняется.
- `not_found` — метаданные ответа, не команда удаления; повтор `0/1` без nonce — новый
  ответ, а не duplicate.

## 4. Архив

- `tests-final/` — финальные тесты (types 19, reducer 13, selectors 11 = **43**), md5 в
  `tests-final/MD5SUMS.txt`; md5 продукта — `tests-final/SOURCE-MD5SUMS.txt`.
- Red: `red-w16.log`, `red-w16-r12.log`, `red-w16-r13.log`; green:
  `green-w16-final.log`, `green-core-w16.log`, `green-workspace-w16.log`.
