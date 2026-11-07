# STUB-14-UI-SEARCH-FIX — отчёт

Дата: 2026-10-30. Batch: G2 Frontend Lane B (RW-40). HEAD:
`4e240481e59ff4d49732ac8bd5ba08a006363748`. Основание:
`waves/RW-40/PM/stub14-search-ui/REVIEW.md` (RETURN, P1=1, P2=1).

## Что сделано

### P1 — числовые `author_id`/`mentions` в локальном фильтре

`rust/gui-spike/ui/features/search-overlay.js`:

- `RemoteMessage` получил независимые `authorId` и `mentionIds`;
  `normalizeRemoteMessage` заполняет их из `author.id` (плюс fallback
  `author_id`/`authorId` на самом сообщении) и `mentions[].id` (строка-id тоже
  принимается).
- `applyRemoteFilters` больше не сравнивает числовой фильтр с отображаемым
  именем: `matchesFilterValue` берёт значения через `splitQueryValues` и
  считает совпадением либо точный id из `authorId`/`mentionIds`, либо прежнее
  вхождение подстроки в имя автора. Серверное сообщение, уже отфильтрованное по
  числовому snowflake, больше не отбрасывается ложно; несовпавшие id
  по-прежнему отсекаются (проверено вторым сообщением в тесте).

### P2 — невалидные календарные даты

- `rust/gui-spike/ui/features/search.js`: добавлен `isCalendarDate` — regex,
  `Date.parse` и обратная сверка `getUTCFullYear`/`getUTCMonth + 1`/
  `getUTCDate` с исходными `ГГГГ-ММ-ДД`. `searchDateBounds` использует его и
  возвращает `null` для `2026-02-31`, `2026-04-31`, `2023-02-29`.
- `rust/gui-spike/ui/features/search-overlay.js`: `isIsoDate` делегирует в
  `isCalendarDate`, поэтому `refresh()` и `applyFilter()` показывают ошибку и
  не шлют `min_id`/`max_id` для нормализованной даты.

Тесты (`tests/unit/search-remote-real-index.test.mjs`, +3 `it`, −0 строк):

- P1: сообщение с `author.id = 42` и `mentions = [{id: "77"}]` при фильтрах
  `from=42`, `mentions=77` остаётся в выдаче; сообщение с id `43`/`78`
  отсекается.
- P2: `searchDateBounds` отклоняет невалидные даты и считает `2024-02-29` по
  UTC (оракул — `snowflakeAt` = `(ms − Discord epoch) << 22`).
- P2: оверлей с датой `2026-02-31` не вызывает `search_remote`, показывает
  `data-state="error"`, `applyFilter` бросает `ACTION_FAILED`.

## Red

До правок product-кода:

```
node --test tests/unit/search-remote-real-index.test.mjs
exit 1
ℹ tests 8
ℹ pass 5
ℹ fail 3
✖ P1: числовые author_id/mentions не отбрасывают совпавшее серверное сообщение
✖ P2: невалидные календарные даты отклоняются, валидные считаются по UTC
✖ P2: оверлей не шлёт запрос с min_id/max_id для невалидной даты
```

Первые строки падений: `локальный фильтр по числовому id отбросил совпавшее
серверное сообщение`, `2026-02-31 нормализована в март`, `для даты 2026-02-31
ушёл серверный запрос`. Лог: `red-fix.log`.

## Green

```
node --test tests/unit/search-remote-real-index.test.mjs tests/unit/gui-spike-features.test.mjs
exit 0
ℹ tests 108
ℹ suites 15
ℹ pass 108
ℹ fail 0
```

`npx tsc -p tsconfig.json` → exit 0. Лог: `green-fix.log`.

## Файлы и md5sum

```
b5c5c3e6538fa0f6c24c45c3ac1c36a0  rust/gui-spike/ui/features/search.js
860e9237a00e4eacd66e343bbeb5ab45  rust/gui-spike/ui/features/search-overlay.js
b2692ff1bb6b0fb107e998ac2f5b2c2d  tests/unit/search-remote-real-index.test.mjs
```

Артефакты: `red-fix.log`, `green-fix.log`, `REPORT-FIX.md`.

## Mutation gate (REVIEW-PASS2)

Закрывает P2 из `REVIEW-PASS2.md`: обязательный mutation proof. Код и тесты не
менялись, поэтому их md5 совпадают с разделом выше. Прогон воспроизводим
скриптом с точными anchor-заменами; после каждой мутации файл восстанавливался
из копии, md5 сверялся.

```
node --test tests/unit/search-remote-real-index.test.mjs
```

| # | Файл | Мутант | Exit | Killed by | Вердикт |
|---|---|---|---|---|---|
| M1 | `features/search.js` | `isCalendarDate` без обратной сверки UTC-компонентов (regex + `Date.parse`, старое поведение) | 1 | P2: невалидные календарные даты; P2: оверлей не шлёт запрос для невалидной даты | KILLED |
| M2 | `features/search-overlay.js` | `isIsoDate` — только regex `ГГГГ-ММ-ДД`, без календарной проверки | 1 | P2: оверлей не шлёт запрос с `min_id`/`max_id` для невалидной даты | KILLED |
| M3 | `features/search-overlay.js` | `matchesFilterValue` сравнивает только с отображаемым именем, без `ids.includes` | 1 | P1: числовые `author_id`/`mentions` не отбрасывают совпавшее сообщение | KILLED |
| M4 | `features/search-overlay.js` | `normalizeRemoteMessage` не заполняет `authorId` из `author.id` | 1 | P1: числовые `author_id`/`mentions` | KILLED |
| M5 | `features/search.js` | `buildSearchQuery` не кладёт `mentions` в `query` | 1 | §2: все подтверждённые фильтры; P1: числовые id | KILLED |

Итого: **5/5 KILLED**. MD5 до мутации и после восстановления совпали для
каждого прогона:

```
search.js           b5c5c3e6538fa0f6c24c45c3ac1c36a0
search-overlay.js   860e9237a00e4eacd66e343bbeb5ab45
test file           b2692ff1bb6b0fb107e998ac2f5b2c2d
```

Артефакт: `mutations-fix.log` (md5 `1db70c70ba7ca3139f8e9f025ea8951b`).
После восстановления контрольный прогон обоих файлов: 20 passed / 0 failed,
exit 0.

## Ограничения и риски

- `mentions` в локальном фильтре по-прежнему допускает совпадение по имени
  автора (прежнее поведение), плюс новое совпадение по `mentions[].id`.
  Каталога «имя → snowflake» в дампе нет; для числовых id семантика теперь
  корректна.
- `author_type` и `pinned` не менялись: вне P1/P2.
- Backend `search.rs` всё ещё игнорирует `params.query` — ограничение
  зафиксировано в `REPORT.md`, отдельная карточка.
- Полный `npm run gate:worker` не запускался: по `AGENTS.md` §3 в batch его
  выполняет integration-лейн; здесь targeted-набор + `tsc`.
- `git commit` не делался; чужие файлы не тронуты.
