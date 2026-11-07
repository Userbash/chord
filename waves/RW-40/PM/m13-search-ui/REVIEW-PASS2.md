# Независимое ревью Pass 2 — M-13-SEARCH-202-NSFW-UI

- **Ревьюер:** DeepSeek Review 37B (независимая проверка, автору код не принадлежит).
- **Дата:** 2026-10-30.
- **Ветка / HEAD:** `rw40/m13-search-202-ui` / `4e240481e59ff4d49732ac8bd5ba08a006363748` (правки Pass 2 — незакоммиченное рабочее дерево).
- **Объект:** `rust/gui-spike/ui/features/search.js`, `rust/gui-spike/ui/features/search-overlay.js`,
  `rust/gui-spike/ui/components/chat.css`, `tests/unit/search-remote-real-index.test.mjs`.
- **Основание:** `waves/RW-40/PM/m13-search-ui/REVIEW.md` (RETURN, P0=0, P1=3, P2=8).
- **Вердикт:** **PASS** (P0=0, P1=0, P2=2 остаточных вне четырёх файлов Pass 2, P3=1).

Все три P1 закрыты по существу, каждый подтверждён новым тестом. Шесть из восьми
P2 закрыты в коде и покрыты проверками; оставшиеся два (P2-7 — живая проводка
NSFW-контекста, P2-8 — команды запуска мутаций) лежат за пределами файлов Pass 2
(`app.js`, `waves/`) и не менялись. Это не регрессия ревьюируемого кода, но до merge
их нужно либо закрыть, либо явно пометить «live unverified» — см. §5.

## 1. Прогоны

Логи ревьюера: `review-pass2-tests.log` (md5 `0da632b96d9dd142bc4398e702d0c44d`),
`review-pass2-typecheck.log` (md5 `8fed99d5fda8abc0099955968cf039d6`).

| Команда | Результат |
|---|---|
| `node --test tests/unit/search-remote-real-index.test.mjs tests/meta/test-policy.test.mjs` | `tests 50`, `pass 50`, `fail 0`, `skipped 0`, exit `0` |
| — из них `search-remote-real-index` | `tests 33`, `pass 33`, `fail 0` (было 18 в Pass 1) |
| — из них `test-policy` | `tests 17`, `pass 17`, `fail 0` |
| `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | exit `0` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` (дополнительно) | exit `0` |
| `node --test tests/unit/gui-spike-features.test.mjs` (регрессии оверлея) | `tests 100`, `pass 100`, `fail 0` |

`ExperimentalWarning` про `localStorage` в выводе тестов отсутствует.

## 2. MD5 проверенных файлов

```
9e20fdcdc6fe3df692b19e18f73c746b  rust/gui-spike/ui/features/search.js
d2087ea41196331715689fdf7a572ec7  rust/gui-spike/ui/features/search-overlay.js
96aca93da1e6bc03694b21b2cd7d654c  rust/gui-spike/ui/components/chat.css
b485a9fe25f669a8f77a9f23599670e4  tests/unit/search-remote-real-index.test.mjs
```

`tokens.css` не менялся (`21d570a6bf9c4b5d77b00d3d9b3d1d08`) — и не должен был:
исправление P1-1 сделано заменой токенов, а не добавлением общих объявлений.

## 3. Закрытие P1

### P1-1. Несуществующие CSS-переменные — закрыт

`chat.css` больше не использует `--danger-bg`, `--danger-hover`, `--btn-pad-y`,
`--motion-slow`. Независимая проверка всех `var(--…)` файла (111 уникальных имён)
против объявлений `tokens.css` дала пустое множество ненайденных:

```
comm -23 used.txt defined.txt   # пусто
```

Замены: чип с отрицанием — `--state-selected` + `--danger`/`--danger-text`
(`chat.css:1511-1524`), спиннер — `animation … var(--motion-base)`
(`chat.css:1603-1611`), NSFW-баннер — `background: var(--surface-3)` и
`border-left: … var(--danger)` (`chat.css:1708-1719`), кнопки NSFW — паддинги
`--space-0`/`--space-6` (`chat.css:1766-1779`). Сырых `#hex`/`rgb()`/`hsl()` — `0`,
`!important` — `0`. Требование §0.2.1 выполнено.

### P1-2. Контраст 18+ и «Продолжить» — закрыт

Бейдж и destructive-кнопка переведены на `var(--danger-fill)` +
`var(--text-on-danger)` (`chat.css:1728-1741`, `1780-1784`), hover —
`var(--danger-fill-hover)` (`chat.css:1786-1789`). `tokens.css:90,103` фиксирует
4.85:1 для этой пары. Тест `styles M-13 with the normative chip classes and §6
banner colors` отдельно запрещает `background: var(--danger)` и проверяет пару
цветов у `.search-nsfw-btn-continue`.

### P1-3. Персистентность `GuildNSFWAgreeStore` — закрыт

`getBrowserLocalStorage()` (`search-overlay.js:756-782`) сначала пробует
`globalThis.window.localStorage` в `try/catch`, затем data-property дескриптора,
затем accessor через `desc.get.call(globalThis)`. Встроенный Node-getter без
`window` распознаётся `isNodeBuiltinStorageGetter()` и не вызывается — предупреждение
не появляется. `recordNsfwAgreement()` (`:815-826`) пишет карту
`{ [guildId]: true }` под ключом `GuildNSFWAgreeStore`.

Добавлены два теста: accessor-`localStorage` с пересозданием оверлея (согласие
видно во втором экземпляре) и обычный data-property. Оба зелёные.

## 4. Закрытие P2

| Пункт | Статус | Доказательство |
|---|---|---|
| P2-1 тексты каталога | **закрыт** | `"Индексация"` (`search-overlay.js:591, 1711`), `"Архив"` (`:1712`), `AXPbZr` дословно в `indexing` и `exhausted` (`:1773, 1790`), `E4Cd5I` дословно (`:929-933`), `"Повторить запрос"` (`:615`). Тест `uses catalog strings…` сверяет точные строки, включая `exhausted`. |
| P2-2 селекторы чипов / §6 | **закрыт** | `span.search-chip-prefix`/`.search-chip-value` (`:1197-1199`), `data-chip-remove` (`:1204`), CSS-правила на эти классы (`chat.css:1516-1533`). Бейдж индексации — `--warning`/`--text-on-warning` (`chat.css:1632-1643`). Тест проверяет классы, отсутствие `.search-chip-key/.search-chip-val` и цвета §6. |
| P2-3 очистка выдачи и триггеры гейта | **закрыт** | Гейт в `refresh()` по `in:`/инлайн-`in`, `guild.nsfw_level ∈ {1,3}` и текущему каналу (`:1915-1930`) очищает `results` до согласия. NSFW-хиты без согласия отфильтровываются (`:1483-1501`), над общегильдийской выдачей показывается баннер (`:1722`). Тесты: сценарий B и очистка после «Вернуться». |
| P2-4 фокус и «Вернуться» | **закрыт** | `showNsfwGate` переводит фокус только при первом открытии и не из поля ввода (`:912-936`). «Вернуться» очищает `in`, таймер, счётчик, `results` и статус (`:961-990`). Тесты на повторный `refresh` и на сброс статуса/выдачи. |
| P2-5 синхронизация/клавиатура/`aria-busy` | **закрыт** | Инлайн-токены заполняют `.search-sections` (`:1022-1059`), удаление кавычного токена режется по токену, а не `split(/\s+/)` (`:1062-1080, 1097-1099, 1188-1219`), `.search-chips-clear` возвращает фокус (`:1243-1269`), Backspace в позиции 0 и ArrowLeft/Right (`:1102-1140, 1200-1258, 1271-1281`), `aria-busy` ставится/снимается в `beginRequest/endRequest` (`:1281-1290`). Все пять сценариев покрыты тестами. |
| P2-6 чистые функции | **закрыт** | Невалидный `Retry-After` больше не блокирует IPC-поле (`search.js:506-520`); `searchDateBounds` пропускает невалидный `date` и применяет валидный `before` (`search.js:144-181`); `isAgeGatedSpacesRestricted` без маски возвращает `false` (`search.js:426-432`). Тест на все три границы. |
| P2-7 живая проводка NSFW | **не закрыт (вне scope)** | См. §5.1. |
| P2-8 evidence/покрытие | **частично** | Тесты на токены CSS и персистентность добавлены. `mutations.log` по-прежнему без команд запуска — см. §5.2. |

## 5. Остаточные замечания

### 5.1. P2-7. `setNsfwContext` не вызывается из продукта

`grep -rn "setNsfwContext" rust/gui-spike/ui` находит только определение в
`search-overlay.js:2032`; в `app.js` вызова нет, `index.html` не содержит
`data-nsfw`, а `chat.js` выставляет каналу только `data-channel-id`
(`chat.js:3585-3587`). В живом окне `nsfwContext` пуст, `isChannelNsfw` всегда
`false`, и NSFW-гейт не открывается ни по `in:`, ни по текущему каналу, ни по
`guild.nsfw_level`. Фича работает только в тестах, где контекст подаётся вручную.

Это не дефект четырёх файлов Pass 2: `app.js` в задание не входил и не менялся
(mtime 18:19, до правок Pass 2). Но REVIEW.md прямо требовал до merge либо вызов
из `app.js`, либо отдельную карточку с явным «live unverified» в отчёте. Ни того,
ни другого сейчас нет. Нужно решение Team Lead: карточка на проводку контекста
(гильдия/пользователь/каналы) или строка «live unverified» в отчёте.

### 5.2. P2-8. `mutations.log` без команд запуска

Лог фиксирует мутантов, exit-коды и восстановление MD5, но не команду/раннер.
Для воспроизводимости нужна команда в логе или в отчёте (то же замечание, что в
AT-03). Это артефакт `waves/`, а не код Pass 2; тесты, которые должны ловить
P1-1/P1-3, добавлены и зелёные.

### 5.3. P3. Мелкие отличия §6 не задокументированы

Спиннер использует `var(--motion-base)` (180 мс) вместо нормативных `0.9s`
(`chat.css:1610`), бейдж выровнен через `flex: 0 0 auto` + `flex: 1 1 auto` у
заголовка вместо `margin-left: auto` (`chat.css:1624-1636`). Визуально
эквивалентно, token-only соблюдён, но отступление от нормативного блока §6 нигде
не зафиксировано. Не блокирует.

## 6. Тесты не ослаблены

`git diff --numstat HEAD -- tests/unit/search-remote-real-index.test.mjs` —
`1255` добавленных строк, `0` удалённых: Pass 2 только расширил набор
(18 → 33 теста в файле). `.only`, `skip`, `todo` в файле отсутствуют,
`tests/meta/test-policy.test.mjs` — 17/17. Существующие тесты Pass 1 не
переписаны под новый код.

## 7. Границы ревью

Ревью read-only по продукту: изменены только этот отчёт и два лога ревьюера в
`waves/RW-40/PM/m13-search-ui/`. Код, тесты, `tokens.css` и `app.js` не
редактировались. `cargo` не запускался — карточка UI-only.
