# REVIEW-97A — task-183 (FE-P2-05): shortcuts.js

Вердикт: **APPROVED** (P0=0, P1=0, мутации 5/5 killed)

Проверено: `rust/gui-spike/ui/features/shortcuts.js` (358 строк, md5 `9598a7650cd513552207f438ed3fb10c`) против ТЗ карточки и `REPORT.md`.

## Контракт
- Таблица из 8 сочетаний совпадает с отчётом и эталонной справкой; массив и записи заморожены.
- `matchShortcut`: точное совпадение `key` + `ctrlKey`/`shiftKey`/`altKey`; буквы — case-insensitive, служебные клавиши — точно.
- Input-guard: в `input`/`textarea`/`contenteditable`/`isContentEditable` остаются `quick_switcher`, `toggle_mute`, `toggle_deafen`, `shortcuts_help` и `Escape`; `Ctrl+F`, `Ctrl+P`, `Alt+Shift+ArrowUp` не перехватываются.
- Модалка: `role="dialog"`, `aria-modal="true"`, `aria-label="Горячие клавиши"`, `hidden` + `aria-hidden` синхронны; 8 строк `[data-shortcut-id]`, кнопка `[data-action="close-shortcuts-help"]` с `type="button"` и `aria-label`.
- `Ctrl+/` — toggle + `preventDefault`; `Escape` при открытой модалке закрывает её, возвращает фокус и не дергает `mark_channel_read`; прочие шорткаты зовут `handlers[id]`.
- `destroy()` снимает `keydown` с `body`, `click` с кнопки, удаляет модалку; повторный вызов идемпотентен.

## Прогоны
- `node --test tests/unit/shortcuts.test.mjs` — 24/24 pass, exit=0.
- Мутации (каждая — красный прогон, затем восстановление из копии): M1 `toggle_mute.key: m→n` fail 3; M2 guard `editable=false` fail 3; M3 снят `preventDefault` перед handler fail 4; M4 `destroy()` без снятия слушателя fail 1; M5 `Escape` игнорирует открытую модалку fail 2. Итог 5/5 KILLED.
- После восстановления md5 файла = `9598a7650cd513552207f438ed3fb10c`, совпадает с отчётом.

## Замечания (не блокирующие)
1. `Meta`/Cmd на macOS не приравнен к `ctrlKey` — зафиксировано в отчёте как отдельное решение PM.
2. Модуль не смонтирован в `ui/app.js` и без стилей `.shortcuts-help-*` — вне write-scope карточки, вынесено в следующую.
3. `plan-sync` красный из-за §8 `TDD-PLAN.md` — общий файл, правит Team Lead после приёмки.

Git-коммитов не делал.
