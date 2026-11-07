# REVIEW-157A — task-240 (`forum-view.js`, FE-P0-10)

- **Ревьюер**: Technical Reviewer (DeepSeek, slot 157A)
- **Карточка / ветка**: `task-240` (`feature/task-240-ui-forum-view`)
- **Объект ревью**: `rust/gui-spike/ui/features/forum-view.js`, `tests/unit/forum-view.test.mjs`
- **Отчёт воркера**: `waves/RW-40/G2/task-240-ui-forum-view/REPORT.md`
- **Дата**: 2026-10-31

## Verdict: APPROVED

## Проверка соответствия ТЗ и контракту

| Пункт | Результат |
| --- | --- |
| `FORUM_SORT_ORDERS = {LATEST_ACTIVITY:0, CREATION_DATE:1}`, `FORUM_LAYOUT_VIEWS = {LIST:1, GALLERY:2}`, `MAX_FORUM_POST_TAGS = 5`, все `Object.freeze` | OK |
| `sortAndFilterForumPosts` — фильтр по **всем** `selectedTagIds`, регистронезависимый `trim`+`toLowerCase` поиск по `title` | OK |
| Сортировка: `pinned === true` первыми, затем `lastActivityTimestamp` / `createdAtTimestamp` по убыванию, tie-break `String(id)` `localeCompare` desc | OK |
| Отсутствие мутации входа: `cloneForumPost` копирует `appliedTags` | OK |
| Контроллер: `[data-forum-view]` с `role="region"` / `aria-label="Форум"`, `[data-forum-tag-filter]` + `aria-pressed`, `[data-forum-post-id]` + `[data-pinned]` + `onSelectPost` | OK |
| Форма `[data-forum-new-post-form]`: `[data-forum-title-input]`, `[data-forum-body-input]`, чекбоксы тегов, лимит `MAX_FORUM_POST_TAGS = 5` (6-й disabled и игнорируется), `[data-forum-submit-btn]`, `onCreatePost` только при непустых trim-полях | OK |
| `setState` / `getState` / `destroy`, отсутствие глобальной зависимости от `window.document` | OK |

## Тесты

```
node --test tests/unit/forum-view.test.mjs  →  exit=0 (7/7 pass, 2 suites)
```

Тесты сверяют контракт (DOM-шим + внешние ожидания по `data-*`/ARIA), а не копируют реализацию. Чужие файлы не затронуты, `gate:worker` не запускался по режиму batch.

## Mutation testing (5/5 убиты)

Каждая мутация вносилась в `rust/gui-spike/ui/features/forum-view.js`, прогонялся targeted-тест, затем файл восстанавливался из исходного текста.

| # | Мутация | Результат |
| --- | --- | --- |
| M1 | `cloneForumPost`: `[...post.appliedTags]` → `post.appliedTags` (общая ссылка) | killed (exit=1) |
| M2 | Компаратор `pinned`: `pinnedB - pinnedA` → `pinnedA - pinnedB` | killed (exit=1) |
| M3 | Фильтр тегов: `requiredTags.every(...)` → `requiredTags.some(...)` | killed (exit=1) |
| M4 | Лимит тегов формы: `>= MAX_FORUM_POST_TAGS` → `> MAX_FORUM_POST_TAGS` | killed (exit=1) |
| M5 | Tie-break: `String(b.id).localeCompare(String(a.id))` → `String(a.id).localeCompare(String(b.id))` | killed (exit=1) |

**Итог: `mutations = 5/5 (100%)`.**

## Восстановление файла (byte-for-byte)

```
9c076101ba72900431651e4620af3e66  rust/gui-spike/ui/features/forum-view.js
0dc4cf045ad4dbb6fd31d42d1a17c26b  tests/unit/forum-view.test.mjs
```

md5 продовой реализации после всех мутаций совпадает с md5 в `REPORT.md` — правок не осталось, рабочее дерево в исходном состоянии.

## Замечания

Блокирующих нет. `P0 = 0`, `P1 = 0`. Рекомендация на будущее (не блокирует): добавить тест на `destroy()` без `removeChild`-ветки и на `setState` сброс `selectedTagIds` в пустой массив — сейчас ветки покрыты косвенно.
