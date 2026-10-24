# Отчёт W39i (Backend) — P3-02: поля сообщений/участников + `search_page` + R5

Дата: 2026-10-23. PM: DeepSeek. Файлы (только эти):
`rust/gui-spike/src/data.rs`, `rust/gui-spike/tests/data_contract.rs`.
Новых зависимостей нет (`Cargo.toml` не менялся), `ui/**`, `fixtures/**`,
`ipc_contract.rs`, `launch.rs`, `main.rs` не менялись.

## Итог

- `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` → **50 passed, 0 failed**
  (data_contract **29** = 22 W39e2 + 7 новых; ipc_contract 20; launch 1).
- `cargo clippy ... --all-targets -- -D warnings` → 0.
- `cargo fmt ... -- --check` → 0.
- R1–R4 W39e2 сохранены: 64 MiB по `metadata` до чтения, `MAX_INPUT_LEN=256` +
  `preview()`, строгий кадр `t`/`d`, `collect_members → Result` (`Parse` с
  `ops[i].items[j]`); их тесты не менялись.

```text
data.rs                bcd0bc7baa0c5a973a3d81808db13219
tests/data_contract.rs ffa9e6cb2a0d9bf82f4f868cd2bee78f
```

Было (W39e2): `data.rs 58051842…`, тест `c55681f2…`.

Red-лог: `red-data_contract.log` (57 ошибок компиляции теста: полей/`SearchFilters`
ещё нет — тест писан до кода). Green: `green-test.log`, `green-clippy.log`,
`green-fmt.log`.

## 1. `messages_page`: поле → источник в фикстуре → тест

Кадры `fixtures/gateway/by-type/*MESSAGE_{CREATE,UPDATE}.json` (13 штук, 4 канала).

| Поле строки | Источник в фикстуре | Тест |
|---|---|---|
| `embeds[]` (`id`,`kind`,`title`,`description`,`url`,`color`,`providerName`,`authorName`,`footerText`,`thumbnailUrl`,`fieldCount`) | `d.embeds[]`: в CREATE `null`, реальный объект — в `MESSAGE_UPDATE` (`026/027/028/029/030/031`): `type:"article"`, `provider.name:"Хабр"`, `url`, `title`, `thumbnail.url`; `063-MESSAGE_CREATE` — `type:"rich"` с `fields[]` | `message_rows_expose_embeds_day_and_group_from_fixtures` (перенос embed из UPDATE при дедупликации), негатив-оракул `fixture_messages_have_no_attachments_reactions_replies_or_edit_timestamps` |
| `attachments[]` (`id`,`filename`,`size`,`url`,`contentType`,`width`,`height`) | `d.attachments` = `null` **во всех** кадрах → список всегда пуст | оба теста выше (оракул проверяет `null` по всем кадрам) |
| `reactions[]` (`emoji`,`count`,`me`) | ключа `reactions` **нет ни в одном** кадре → пусто | там же (оракул: `get("reactions").is_none()`) |
| `replyTo` | `message_reference`/`referenced_message` **отсутствуют** → `None` | там же (оракул: обоих ключей нет) |
| `editedAt` | `edited_timestamp` = `"nil"` во всех кадрах → `None`; `edited` = была ли строка в `MESSAGE_UPDATE` (реальная правка: добавился embed) | там же |
| `day` | префикс `timestamp` (`2026-10-19T…`) → `"2026-10-19"` | `message_rows_…` (`Some("2026-10-19")` у всех 3 строк) |
| `group` (`start`/`continuation`) | вычисляется по полному списку канала: первый/смена `author.id`/смена `day`; у `#новости` 3 строки одного автора и дня → `start, continuation, continuation` | `message_rows_…` |
| `pinned`, `messageType`, `mentions[]`, `mentionEveryone` | `pinned:"false"`, `type:0`, `mentions` (непустой только в `062`, 1 user), `mention_everyone:"false"` | `message_rows_…`, `search_page_matches_fixture_fields_and_paginates` (упоминание) |

`attachmentCount`/`embedCount` заменены на массивы (длины — `len()`).

## 2. `members_page`: роли, группы, стабильный порядок, счётчики

Источник: `GUILD_MEMBER_LIST_UPDATE` (`ops[].items[]`, `groups[]`, `member_count`)
+ READY (`guilds[].roles[].{id,name,position}`, `properties.name`).

| Поле | Источник | Тест |
|---|---|---|
| `roles[]` | `member.roles[]`, сортировка по `position` из READY (старшая первой) | `members_page_roles_and_groups_from_fixtures` |
| `roleName` | имя старшей роли из READY (`Гуру вуду ПК`) | там же |
| `group` | маркер `{"group":{"id":…}}` перед строками; без маркера — fallback по `presence.status` (`online/idle/dnd → online`, иначе `offline`) | там же |
| `groups[]` (`kind`,`title`,`position`,`announcedCount`,`loadedCount`) | `kind`: роль, если id из `groups[]` не статус; `title`/`position` из READY; `announcedCount` — `groups[].count` **последнего** снимка (для `294064825540280335` это `089`: админы 1, Гуру 1, СВОИ 7, online 6, offline 97); `loadedCount` — реально снятые строки (0/1/7/6/82) | там же |
| порядок | роли по `position` desc, затем `online/idle/dnd/offline`, затем незнакомые id; внутри группы `display_name` (lowercase) + `id` | там же (блоки групп + детерминизм повтора) |
| `total` | 96 уникальных участников (1+7+6+82) | там же |

Счётчики честные: `announcedCount` — по всей гильдии Discord, `loadedCount` —
сколько строк есть в фикстурах (у роли админов в `089` счётчик 1, строк 0: снимок
её участников не содержит).

## 3. `search_page(query, filters, cursor, limit)`

Локально по фикстурам, без сети; `scanned` — сколько сообщений просмотрено (13),
`total` — сколько совпало. `limit 1..=100`, курсор — id последнего результата,
пустой результат валиден.

| Фильтр R-10 | Поле фикстуры | Тест (ожидание) |
|---|---|---|
| автор | `author.id` | 3 (`268478587651358721`) |
| канал | `channel_id` | 8 (`-2684694197495146496`) |
| содержит | подстрока `content`, регистр не важен | 1 (`РАДЫ ВИДЕТЬ` → `062`) |
| упоминание | `mentions[].id` или `<@id>`/`<@!id>` в `content` | 1 (`-12103814603318778368`) |
| дата | `day` из `timestamp` | 13 (`2026-10-19`) |
| тип | `type` | 13 (`0`), 0 (`1`) |
| закреплено | `pinned` | 0 для `true` (закреплённых нет), 13 для `false` |

Тесты: `search_page_matches_fixture_fields_and_paginates` (пустой query = все 13,
пагинация limit 5 → 3 страницы без пересечений; имя канала `📃┃новости` из READY,
у не-READY канала `channelName: null`), `search_page_empty_result_and_filters_are_valid`
(несуществующий текст, тип 1, комбинация 5 фильтров, пустые строки фильтров =
«не задан» — иначе пустое поле ввода отдавало бы ноль результатов).

Негатив: `search_page_rejects_bad_limit_cursor_and_oversized_filters` — `limit 0/101`,
`query` 10 000 символов, каждый строковый фильтр 10 000 символов, дата `19.10.2026`,
длинный/чужой курсор → `InvalidArg`; в сообщении усечение `…`, длина < 256; границы
включительно: `limit=100`, фильтр ровно 256 символов — валидны.

## 4. R5: `settings(section?)`

`ensure_input_len(section, "section")` до чтения READY. Тест
`settings_rejects_oversized_section`: 10 000 символов → `InvalidArg` с `…`, строка
не копируется целиком, сообщение < 256 символов; ровно 256 символов → `NotFound`
(валидная длина). Существующие тесты `settings` не менялись.

## Негативные кейсы (сводка)

| Кейс | Тест | Статус |
|---|---|---|
| битый JSON READY / by-type | существующие `broken_*_json_is_parse_error` | ok |
| пустой каталог, нет READY, `gateway` не каталог | существующие `empty_catalog…`, `missing_ready_file…`, `unreadable_catalog…` | ok |
| отсутствующий id (канал/гильдия/секция) | существующие `*_unknown_*`, `settings_section_filter_and_not_found` + R5-граница | ok |
| слишком длинные id/курсор/query/фильтры/section | R2 + новые `search_page_rejects_…`, `settings_rejects_oversized_section` | ok |
| пустой результат поиска | `search_page_empty_result_and_filters_are_valid` | ok |
| пустая страница при известном ресурсе | существующие `messages_page_empty_for_known_channel…`, `members_page_empty_for_guild…` | ok |
| битый элемент участника / кадр без `t`/`d` | R3/R4 существующие | ok |
| недопустимый `limit` / чужой курсор | существующие + новый поиск | ok |

`unwrap()` на входных данных в `data.rs` нет; отсутствие обязательного поля — `Parse`.

## Чего нет в фикстурах (честно)

1. **Вложения** — `attachments: null` во всех 13 кадрах сообщений; API всегда отдаёт `[]`
   (структура полей готова, но реальных данных нет).
2. **Реакции** — ключа `reactions` нет ни в одном кадре → всегда `[]`.
3. **Ответы (reply)** — `message_reference`/`referenced_message` отсутствуют → `replyTo: null`.
4. **Правки** — `edited_timestamp` везде `"nil"` → `editedAt: null`; метка `edited`
   выставляется только фактом `MESSAGE_UPDATE` (у `#новости` это реальная правка:
   добавился embed).
5. **Embeds** — только link/rich-статьи (`article` ×3 после дедупликации + `rich` в `063`);
   нет image/video/multi-embed, нет `image`-полей; `color` в статье отсутствует.
6. **Участники** — снимки есть только у 3 гильдий (`1014537851418980372`,
   `294064825540280335`, `-1467272931325252608`); 6 групп из MEM-02 не воспроизводятся
   (максимум 5 в `089`), названия групп-статусов в дампе не сняты (BFF отдаёт id,
   подпись — фронтенд), у гильдии `-1467272931325252608` нет ролей в READY → `title: null`.
7. **Разрыв группы сообщений по времени (>7 мин)** не реализован: нужен парсер
   ISO-времени, новых зависимостей не добавляли. Группировка — автор + день.
8. **Имена каналов** есть только у каналов из READY: `#новости` — да, три канала с
   отрицательными id — `channelName: null` (они из другой сессии дампа).
9. `mention_everyone`, `pinned`, `type` во всех кадрах константны (`false`/`false`/`0`) —
   фильтры проверены, но «боевого» разнообразия значений нет.

## Остаток / риски

- `search_page` пока не зарегистрирован как Tauri-команда: `main.rs` вне моего
  файлового мандата (read-only). Нужен патч PM/frontend-интеграции:
  `invoke_handler!` + обёртка с `SearchFilters` (структура публичная, `Default`).
- Контракт изменился для фронтенда: `attachmentCount`/`embedCount` → `embeds`/`attachments`,
  `MemberRow.roles: usize` → `roles: Vec<String>` + `roleName`/`group`,
  `MembersPage.groups`. В `ui/**` потребителей этих полей нет (проверено grep).
- Счётчики групп: `announcedCount` берётся из последнего снимка; при живом обновлении
  (не в спайке) BFF статичен по фикстурам.
