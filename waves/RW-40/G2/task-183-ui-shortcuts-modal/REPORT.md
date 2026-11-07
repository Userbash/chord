# task-183 (FE-P2-05): горячие клавиши Discord и модалка «Горячие клавиши»

- Ветка: `feature/task-183-ui-shortcuts-modal` (от `unstable`)
- HEAD на момент работы: `f331110 feat(core): merge PR-63 (task-178 route catalog [S] markers & rest-d02.json oracle sync)`
- Оракулы: `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3 строка FE-P2-05;
  `re-deep/frontend-d03.md` §5 (Modal: `role="dialog"`, `aria-modal`, возврат фокуса);
  справка сочетаний клавиш клиента Discord (`re-deep/decoded/`)

## 1. Что сделано

Создан модуль `rust/gui-spike/ui/features/shortcuts.js` (`// @ts-check`, ESM, JSDoc):

- `DISCORD_SHORTCUTS` — замороженный массив (массив и каждая запись через `Object.freeze`)
  из 8 сочетаний в порядке эталонной справки:

  | id | combo | labelRu | category |
  |---|---|---|---|
  | `quick_switcher` | `Ctrl+K` | Быстрый переход | Навигация |
  | `toggle_mute` | `Ctrl+Shift+M` | Вкл./выкл. микрофон | Голос |
  | `toggle_deafen` | `Ctrl+Shift+D` | Вкл./выкл. звук | Голос |
  | `shortcuts_help` | `Ctrl+/` | Горячие клавиши | Справка |
  | `search_channel` | `Ctrl+F` | Поиск в канале | Навигация |
  | `pins_popout` | `Ctrl+P` | Закреплённые сообщения | Сообщения |
  | `mark_channel_read` | `Escape` | Отметить канал прочитанным / Закрыть модалку | Навигация |
  | `navigate_unread_prev` | `Alt+Shift+ArrowUp` | Предыдущий непрочитанный канал | Навигация |

  Поля записи: `id`, `combo`, `key`, `ctrlKey`, `shiftKey`, `altKey`, `labelRu`, `category`.

- `matchShortcut(event)` — точное совпадение `key` + трёх модификаторов; буквы сравниваются
  без учёта регистра (`K` = `k` после Shift), служебные клавиши — точно. В `input`/`textarea`/
  `contentEditable` (атрибут `contenteditable` или свойство `isContentEditable`) остаются
  рабочими только `Ctrl+K`, `Ctrl+Shift+M`, `Ctrl+Shift+D`, `Ctrl+/` и `Escape`;
  `Ctrl+F`, `Ctrl+P`, `Alt+Shift+ArrowUp` не перехватываются.

- `createShortcutsController(doc, handlers = {})`:
  - создаёт в `doc.body` модалку `div#shortcuts-help-modal` с `role="dialog"`,
    `aria-modal="true"`, `aria-label="Горячие клавиши"`, `hidden`, `aria-hidden="true"`;
  - в модалке заголовок, кнопка `[data-action="close-shortcuts-help"]` (`aria-label`,
    `type="button"`) и список из 8 строк `[data-shortcut-id]` с `<kbd>`-бейджами и подписями;
  - `Ctrl+/` открывает/закрывает справку и всегда вызывает `preventDefault()`;
  - `Escape` при открытой справке закрывает её, возвращает фокус ранее активному элементу
    и не вызывает `handlers.mark_channel_read`;
  - остальные сопоставленные шорткаты вызывают `handlers[shortcut.id](event)` и
    `event.preventDefault()`; без обработчика дефолт браузера всё равно отменяется;
  - публичный API: `openHelpModal()`, `closeHelpModal()`, `isHelpModalOpen()`, `destroy()`;
    `destroy()` снимает слушатель `keydown` с `body`, `click` с кнопки закрытия и удаляет модалку.

Тесты: `tests/unit/shortcuts.test.mjs` — 24 теста в 3 блоках (таблица, `matchShortcut`,
контроллер), включая input-guard, ARIA-контракт, toggle, возврат фокуса, приоритет Escape,
вызов обработчиков, идемпотентность и очистку в `destroy`.

## 2. Изменённые файлы и md5

| Файл | Строк | md5 |
|---|---|---|
| `rust/gui-spike/ui/features/shortcuts.js` (new) | 358 | `9598a7650cd513552207f438ed3fb10c` |
| `tests/unit/shortcuts.test.mjs` (new) | 423 | `90737538489f937f94508932785c21d4` |

Других правок нет: `git status` показывает только эти два untracked-файла и ранее
существовавшую модификацию `package-lock.json` (добавлены `bin`/`engines`; не моя правка,
не трогал). Git-коммитов не делал.

## 3. Red → Green

Red (`red.log`, до реализации):

```
Error [ERR_MODULE_NOT_FOUND]: Cannot find module '.../rust/gui-spike/ui/features/shortcuts.js'
ℹ tests 1  ℹ pass 0  ℹ fail 1
exit=1
```

Green (`green.log`):

```
node --test tests/unit/shortcuts.test.mjs
ℹ tests 24  ℹ pass 24  ℹ fail 0  exit=0

npx tsc -p tsconfig.json
exit=0
```

## 4. Мутации (`mutation.log`, 5/5 KILLED)

| # | Мутация | Результат |
|---|---|---|
| M1 | `toggle_mute.key`: `"m"` → `"n"` | KILLED (exit=1) |
| M2 | отключён guard полей ввода (`editable = false`) | KILLED (exit=1) |
| M3 | убран `event.preventDefault()` перед вызовом `handlers[id]` | KILLED (exit=1) |
| M4 | `destroy()` не снимает слушатель `keydown` | KILLED (exit=1) |
| M5 | `Escape` игнорирует открытую модалку | KILLED (exit=1) |

После каждой мутации файл восстанавливался из копии; sha256 до и после совпадает
(`399cfe7d…e3fe0d`).

## 5. Targeted-прогоны (`targeted.log`)

- `npm run typecheck` (tsconfig.json + tsconfig.agent.json) — exit=0.
- frontend-контракт: `gui-spike-dom`, `gui-spike-model`, `gui-spike-bench`, `gui-spike-features`,
  `gui-spike-auth`, `gui-spike-regressions`, `guild-rail`, `voice-ui`, `channel-switch` — exit=0.
- `node waves/RW-39/PM/w39c/geometry-check.mjs` — exit=0.
- `node --test tests/meta/plan-sync.test.mjs` — exit=1: §8 `TDD-PLAN.md` не описывает
  9 тестовых файлов, включая `tests/unit/shortcuts.test.mjs` (в списке также
  `attachments-ui`, `guild-modals`, `members-subscriptions`, `notifications`, `pins-threads`,
  `reactions-ui`, `user-popout`, `voice_ui`). Дефект существовал до этой задачи; строка §8 —
  общий файл, правит Team Lead после приёмки, воркер его не трогает.

`node scripts/test-impact.mjs` из-за ранее изменённого `package-lock.json` поднимает tier T2
и требует полный `npm test`/cargo; полный workspace-прогон оставлен integration-лейну,
targeted-набор и typecheck выше зелёные.

## 6. Риски и что осталось

1. Модуль пока не смонтирован: `ui/app.js` и `ui/index.html` вне write-scope этой карточки.
   Для включения нужен монтаж `createShortcutsController(document, { quick_switcher: …,
   toggle_mute: …, toggle_deafen: …, search_channel: …, pins_popout: …, mark_channel_read: …,
   navigate_unread_prev: … })` в `app.js` и стили `.shortcuts-help-*` (G3/следующая карточка).
2. `plan-sync` красный из-за §8: Team Lead добавляет строку для `tests/unit/shortcuts.test.mjs`
   с токенами `DISCORD_SHORTCUTS`, `matchShortcut`, `createShortcutsController`.
3. Meta-клавиша (Cmd на macOS) намеренно не приравнена к `ctrlKey`: контракт карточки
   фиксирует `ctrlKey`, расширение — отдельным решением PM.
4. `package-lock.json` был изменён до старта задачи; воркер его не редактировал.
