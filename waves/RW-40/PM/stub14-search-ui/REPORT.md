# STUB-14-UI-SEARCH-FILTERS — отчёт

Дата: 2026-10-30. Ветка: `RW-40`. HEAD на момент работы:
`4e240481e59ff4d49732ac8bd5ba08a006363748` (`RW-40: integrate Discord GUI spike,
dispatch, paging and QA evidence`).

## Что сделано

Разбор и нормализация 7 фильтров оверлея поиска приведены к подтверждённой
таблице `queryKey` из `waves/RW-40/RE-DEEP-REA.md` §2.3–2.4.

`rust/gui-spike/ui/features/search.js` — чистый слой без DOM и сети:

- `SEARCH_PAGE_SIZE = 25` (константа `T_y`/`iG` из дампа);
- `SEARCH_HAS_VALUES` — ровно подтверждённый набор
  `link, embed, poll, snapshot, file, video, image, sound, sticker`;
- `SEARCH_HAS_ALIASES` — подписи `HAS_OPTIONS` и русские строки каталога
  клиента (`ссылку`, `вложение`, `изображение`, `аудио`, `стикер`, `переслать`)
  → серверные токены;
- `SEARCH_AUTHOR_TYPES` — «Человек/Бот/Вебхук» → `user/bot/webhook`;
- `snowflakeFromTimestamp` и `searchDateBounds` — `(ms − 1420070400000) << 22`,
  начало и конец выбранного дня;
- `splitQueryValues`, `normalizeHasFilter`;
- `buildSearchQuery(filters, options)` — REST query-string: `content`,
  `author_id`, `mentions`, `has`, `channel_id`, `pinned`, `author_type`,
  `min_id`, `max_id`, `limit`, `sort_by`, `sort_order`, `offset`. Пустые
  фильтры в результат не попадают.

`rust/gui-spike/ui/features/search-overlay.js` — проводка:

- `buildRemoteParams` кладёт полный набор в `params.query`, сохраняя принятый
  контракт backend B7a (`content`, `channelId`, `sortBy`, `sortOrder`,
  `offset`) без изменений;
- `remoteHasContent` фильтрует серверную выдачу по тем же токенам §2.3
  (`file` → `attachments`, `image/video/sound` → `content_type`, `link/embed` →
  `embeds`, `poll`, `snapshot`, `sticker`), а не по русским подписям;
- `applyFilter` пишет значения во все контролы, а не только в дату, так что
  программный фильтр проходит тот же разбор, что и ввод пользователя;
- комментарии модуля приведены в соответствие с §2 (старый текст называл
  точные REST-параметры gap).

Локальное применение фильтров и пометка «локально» сохранены: принятый backend
`search.rs` читает только 5 полей, поэтому убирать локальный слой сейчас нельзя.

## Red

`waves/RW-40/PM/stub14-search-ui/red.log` — до правок product-кода:

```
ℹ tests 5
ℹ pass 2
ℹ fail 3
✖ STUB-14: передаёт все подтверждённые фильтры поиска в query (§2)
✖ STUB-14: дата → min_id/max_id, has — массив токенов, сортировка asc
✖ STUB-14: пустые фильтры не уходят в query (только content/limit/sort/offset)
```

Два существующих ассерта `search-remote-real-index.test.mjs` проходили до и
после: ни один assert не удалён и не ослаблен (в тесте +140 строк, −0).

## Green

`waves/RW-40/PM/stub14-search-ui/green.log`:

```
node --test tests/unit/search-remote-real-index.test.mjs tests/unit/gui-spike-features.test.mjs
ℹ tests 105
ℹ suites 14
ℹ pass 105
ℹ fail 0
npx tsc -p tsconfig.json
tsc_exit=0
```

## Файлы и md5sum

```
b6f1226b7c4b9496256bbc18c5b79e6f  rust/gui-spike/ui/features/search-overlay.js
87379a7b6a315bb6fe114e99fa42d13d  rust/gui-spike/ui/features/search.js
9b099543e8bbb465288bc27ac05eb4e2  tests/unit/search-remote-real-index.test.mjs
```

Артефакты: `red.log`, `green.log`, `REPORT.md` в `waves/RW-40/PM/stub14-search-ui/`.

## Риски и границы

- `params.query` backend `search.rs` пока игнорирует (serde без
  `deny_unknown_fields`); фильтры дублируются локальным применением, поэтому
  выдача не врёт. Чтобы фильтры реально уходили в REST, нужна отдельная
  карточка на `search.rs` и его команду — вне файлового мандата STUB-14.
- Соответствие «приглашение → embed», «вложение → embed», «эмодзи → sticker»
  выведено из русского каталога клиента (`ru-catalog.json`) и набора §2.3;
  отдельного оракула «подпись UI → токен» в дампе нет.
- Имя автора/канала по-прежнему уходит строкой: каталога «имя → snowflake» в
  дампе нет, числовые id работают штатно.
- `limit=25` передаётся как параметр, но шаг `offset` оставлен прежним (по
  числу полученных записей): на него опирается принятый тест пагинации
  `gui-spike-features.test.mjs` (`offsets [0, 2]`).
- `cargo` не запускался, `app.js`, `chat.js`, `quick-switcher.js` не менялись.
