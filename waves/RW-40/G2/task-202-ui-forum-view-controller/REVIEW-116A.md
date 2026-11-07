# REVIEW-116A — task-202 UI forum view controller

Вердикт: **APPROVED** (P0=0, P1=0, мутации 5/5 killed).
Файлы: `rust/gui-spike/ui/features/forum-view.js`, `tests/unit/forum-view.test.mjs`.
md5 до/после мутаций: `a7f98f2cdb7ea34f1adc20d51677e5c4` — совпадает с REPORT.md, рабочее дерево чистое.

## Проверка
- `node --test tests/unit/forum-view.test.mjs` — exit 0, повторяет green из REPORT.md.
- Контракт шапки (поиск, сортировка `latest_activity`/`creation_date`, чипы `aria-pressed`, счётчики
  `countThreadsByTag`), карточки с `data-pinned`/бейджем `Закреплено`/`[data-message-count]`,
  `onSelectThread(thread.id)`, пустое состояние и `submitNewThread` соответствуют REPORT.md.
- Ошибки не глотаются: `resolveDocument` бросает `TypeError`, `submitNewThread` возвращает `errorRu`.

## Мутации (каждая — правка forum-view.js + прогон теста + восстановление)
1. `aria-pressed` всегда `"false"` — KILLED (exit 1).
2. `data-pinned` всегда `"false"` — KILLED (exit 1).
3. Текст пустого состояния подменён — KILLED (exit 1).
4. Убрана проверка `trimmedContent` — KILLED (exit 1).
5. Подпись опции `Недавняя активность` → `Недавние` — KILLED (`ERR_ASSERTION`, actual/expected).

## Замечания (не блокируют)
- P3: `searchInput.setAttribute("placeholder", ...)` дублируется свойством `placeholder = ...` (строки 192/194);
  мутация атрибута выживает, потому что поведение задаёт свойство. Удалить дубль — отдельной правкой.
- P2 (наблюдение): `resolveTags` и `matchMode` покрыты только через публичный рендер; прямых кейсов на
  `tags` с `label`/`name` и режим `all` в отчёте не видно — стоит добавить при доработке фильтра.
