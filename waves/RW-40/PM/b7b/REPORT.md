# B7b — UI сетевого поиска (search-overlay → search_remote): отчёт G2

Дата: 2026-10-24. Воркер: G2 Frontend. PM: DeepSeek.
Карточка: B7b (`waves/RW-40/TASKS.md:65`). Контракт: `waves/RW-40/ATTACH-RE.md` §1,
backend B7a (`waves/RW-40/PM/b7a/REPORT.md`), `gui-spike/src/search.rs`, `main.rs:142`.

## 1. Статус

**done (UI, offline).** Сетевой путь включён, локальное окно — fallback; live/окно/сеть
не запускались (запрещены). Существующие тесты не удалялись и не ослаблялись: в двух
search-кейсах добавлено ожидание `search.whenIdle()` (запрос стал async), все проверки
и пороги сохранены.

## 2. Фича → тест → результат

| # | Требование | Реализация | Тест (features) | Результат |
|---|---|---|---|---|
| 1 | `invoke("search_remote")` + guildId → сетевой поиск; args `guildId`, `content`, `channelId?`, `sortBy?`, `sortOrder?`, `offset?` | `fetchRemote()`: `{ guildId, params: { content, channelId?, sortBy, sortOrder, offset } }` (контракт B7a), guildId — из `.guild-item.is-selected[data-guild]` | `зовёт search_remote с guildId и подтверждёнными params; рисует total_results` | pass |
| 1 | ответ `{messages,total_results,…}` | `flattenServerMessages()` (массив и массив групп), `normalizeRemoteMessage()`, статус `на сервере найдено N` | там же + `локальные фильтры…` | pass |
| 2 | fallback только без/при недоступной IPC-команде | `isSearchError()`: `SearchError.kind` → видимая ошибка; иное исключение (нет команды) → `remoteState="unavailable"` + локальный скан с пометкой «локально» | `IPC-команды нет (демо) — локальный fallback…` | pass |
| 2 | 401/429/сеть — видимая ошибка, не тихий fallback | `renderRemoteError()` + `reportError` + `onError` (surface `.content`) | `401 — видимая ошибка…`, `429 и сеть…` | pass |
| 3 | loading/empty/error | `renderLoading()` (`data-state="loading"`), `emptyNode()`, `renderRemoteError()` (`data-state="error"`) | `loading: …`, `пустой ответ сервера…`, 401/429 | pass |
| 3 | total_results | `remoteTotal` из ответа, статус `на сервере найдено N · показано M` | контрактный кейс | pass |
| 3 | offset-пагинация, без дублей | `remoteOffset += raw.length`, дедуп по `id` (fallback `timestamp|author|text`), стоп при `total_results`/пустой странице/странице из дублей | `offset-пагинация…` (offsets `[0,2]`, ids 9101/9102/9103) | pass |
| 4 | 7 фильтров: content/channel/sort — сервер | content — поле запроса; «В» → `channelId`; сортировка → `sort_by=timestamp`, `sort_order=desc/asc`; `data-scope="server"` | контрактный кейс + `7 фильтров…` | pass |
| 4 | остальные — честно «локально» | 6 секций `data-scope="local"` + видимая подпись `.search-section-note` «локально»; фильтры реально применяются к серверной выдаче | `7 фильтров…`, `локальные фильтры применяются…` | pass |
| 5 | не ослаблять | см. §4 (было 189 / auth 43 → стало 205 / auth 46; чужие +3 auth/+4 features — параллельный UI-FIX-1) | — | pass |

## 3. Прогоны

| Проверка | Результат | Лог |
|---|---|---|
| red: 8 новых B7b-тестов на копии с отключённым сетевым путём (`canRemote=false`) | **59 tests, 51 pass, 8 fail** (exit 1) | `red.log` |
| green: 5 файлов (`dom`, `model`, `bench`, `features`, `auth`) | **205 tests, 205 pass, 0 fail, 0 skip** (exit 0) | `green.log` |
| `features` отдельно (3 повторных прогона) | 59/59 стабильно | — |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **0** | `tsc.log` |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | **31/31** | `geometry.log` |

Разбивка green: dom 29, model 26, bench 45, features 59 (было 46: +8 моих B7b, +4 от
параллельного UI-FIX-1), auth 46 (было 43: +3 от UI-FIX-1).

**Честно про порядок red→green.** Файл `tests/unit/gui-spike-features.test.mjs` в момент
старта карточки правил параллельный воркер UI-FIX-1 (mtime менялся, в файле были DBG-логи),
поэтому мои тесты добавлены после кода, а red-фаза зафиксирована мутацией: на копии
`/tmp/opencode/b7b-red/ui` сетевой путь отключён (`canRemote=false`), 8 новых кейсов
падают, остальные 51 проходят. Мутация не затрагивает сдаваемые файлы.

## 4. Изменения по файлам

- `rust/gui-spike/ui/features/search-overlay.js` — сетевое состояние (`unknown/available/
  unavailable`), `fetchRemote()` с offset/дедупом, `whenIdle()`, loading/empty/error,
  честные `data-scope`/«локально», `REMOTE_SORTS`, `REMOTE_TOTAL`-статус. Локальный скан
  вынесен в `renderLocal()` без изменения контракта статуса (кроме честного суффикса).
- `rust/gui-spike/ui/app.js` — в `createSearchOverlay` переданы `invoke` и `guildId`
  выбранного сервера.
- `tests/unit/gui-spike-features.test.mjs` — новый describe `B7b: сетевой поиск
  (search_remote)` (8 кейсов) + `await search.whenIdle()` в двух существующих search-кейсах;
  ни одна проверка не удалена/не ослаблена.
- `tests/support/gui-dom-harness.mjs` — **не изменялся** (аддитивные API не понадобились:
  `invokeHandler`, `callsOf`, `setAttribute("data-*")` уже есть).

## 5. md5

```
6165808e54b0b446382425a05f6c5c2c  rust/gui-spike/ui/features/search-overlay.js
e1ab8bdee4edc7b60ef66f864e94370b  rust/gui-spike/ui/app.js
a44bdf0ee865d7206d193e9c28cf0eac  tests/unit/gui-spike-features.test.mjs
fdbcc1cde934b42328fe7a7ab67f59b2  tests/support/gui-dom-harness.mjs (без изменений)
b328e908eabfe379e9fc4f60dcedfbdc  waves/RW-40/PM/b7b/red.log
a6e7ad8dab1eb03a0e8cd5ea8394adf9  waves/RW-40/PM/b7b/green.log
```

## 6. Остаток / открытые вопросы

1. **Форма args.** В ТЗ `args {guildId, content, channelId?, …}`, но принятый backend B7a —
   `search_remote(backend, guild_id, params: Option<SearchParams>)`; UI отправляет
   `{ guildId, params: { content, channelId, sortBy, sortOrder, offset } }`. Плоские поля
   бэкенд бы проигнорировал (тихий пустой поиск). Если PM хочет плоский контракт — это
   правка `main.rs` (вне моих файлов).
2. **Имя канала ≠ id.** В UI/дампе нет каталога snowflake-id каналов; «В» уходит как
   `channel_id` «как есть»: числовой id работает, произвольное имя вернёт видимую ошибку
   (400 → network). Нужен каталог каналов или явное поле «id канала».
3. **Переход по серверному результату.** Сообщения может не быть в загруженном окне,
   поэтому клик не делает `jumpTo`, а честно пишет статус. Нужна синхронизация окна/переход
   по `channel_id + message_id` (отдельная карточка).
4. **Локальные фильтры + глубокая пагинация.** 6 локальных фильтров применяются к уже
   загруженным страницам: «0 после фильтров» не значит «0 на сервере» — в статусе показано
   `показано M` и `после локальных фильтров K` (осознанно; серверных параметров нет — gap).
5. **Нет debounce.** Каждый ввод — запрос (риск 429). Нужно решение по задержке UX.
6. `search_session_id`/`search_query_id` из дампа не отправляются (B7a §5.3).
7. Демо-рейл использует id `1/2/3` (не snowflake): с реальным токеном поиск по ним даст
   видимую серверную ошибку — это следствие требования «не делать тихий fallback». Нужна
   подстановка настоящего guildId.
8. 202 Accepted (индекс ещё строится) бэкендом не обрабатывается (B7a §5.1); UI покажет
   `data-state="error"`.
9. Live-проверка и скриншот не выполнялись: окно/сеть/cargo/gate запрещены ТЗ.
