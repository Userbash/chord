# REVIEW-175A — task-261 / FE-P0-18 Slash Command Picker (PR-146)

- **Ревьюер**: Технический ревьюер (DeepSeek), Technical Review 175A
- **Ветка**: `feature/task-261-ui-slash-command-picker`
- **HEAD при ревью**: `94b91b1` (`feat(bff): merge PR-141 (task-258 onboarding)`)
- **Дата**: 2026-10-31
- **Вердикт**: **APPROVED** — P0 = 0, P1 = 0, мутации 5/5 убиты

## 1. Объём проверки

Только целевые файлы:
- `rust/gui-spike/ui/features/slash-command-picker.js` (475 строк, ESM, `// @ts-check` + JSDoc)
- `tests/unit/slash-command-picker.test.mjs` (511 строк, `node:test` + `node:assert/strict`)
- `waves/RW-40/G2/task-261-ui-slash-command-picker/REPORT.md`

Полный `gate:worker` не запускался (режим batch), `git commit` не выполнялся, тестовый файл не изменялся.

## 2. Targeted-прогон и сверка MD5

```
$ node --test tests/unit/slash-command-picker.test.mjs
ℹ tests 10   ℹ suites 4   ℹ pass 10   ℹ fail 0   exit=0
```

| Файл | MD5 из REPORT.md | Фактический MD5 | Совпадение |
|---|---|---|---|
| `rust/gui-spike/ui/features/slash-command-picker.js` | `88da4074bc8fc73d8f6eea8a87fda62c` | `88da4074bc8fc73d8f6eea8a87fda62c` | ✅ |
| `tests/unit/slash-command-picker.test.mjs` | `b618ce0a27babd187469a34e5abdcb75` | `b618ce0a27babd187469a34e5abdcb75` | ✅ |

Заявленные в REPORT.md цифры (green: 10/10) подтверждены.

## 3. Ручные мутации (5/5 убиты)

Метод: одна правка в `slash-command-picker.js` → `node --test tests/unit/slash-command-picker.test.mjs` → фиксация `exit` → восстановление файла из побайтового бэкапа (`cp`) → `md5sum` = `88da4074bc8fc73d8f6eea8a87fda62c`.

| # | Мутация | Патч | Exit | Убита | Восстановление MD5 | Тест-детектор |
|---|---|---|---|---|---|---|
| M1 | Разрешены заглавные в имени команды | `SLASH_COMMAND_NAME_PATTERN`: `/^[a-z0-9_-]+$/` → `/^[A-Za-z0-9_-]+$/` | 1 | ✅ | `88da4074…` ✅ | «отклоняет пустые строки, длину > 32, заглавные буквы…» |
| M2 | Сдвиг границы длины имени | `trimmed.length > MAX_SLASH_COMMAND_NAME_CHARS` → `>=` | 1 | ✅ | `88da4074…` ✅ | «принимает имена длиной 1..=32…» (кейс `"a".repeat(32)`) |
| M3 | Потеря нормализации регистра | `commandName: match[1].toLowerCase()` → `commandName: match[1]` | 1 | ✅ | `88da4074…` ✅ | «разбирает строку с '/' на commandName в нижнем регистре…» |
| M4 | Инверсия приоритета префикса | `return bPrefix - aPrefix;` → `return aPrefix - bPrefix;` | 1 | ✅ | `88da4074…` ✅ | «фильтрует по префиксу и подстроке name/description с приоритетом префикса name…» |
| M5 | Снята цикличность навигации | `activeIndex = (((activeIndex + step) % count) + count) % count;` → `activeIndex = clampActiveIndex(activeIndex + step, count);` | 1 | ✅ | `88da4074…` ✅ | «поддерживает moveSelection(delta) с циклическим переходом…» |

Мутанты M1–M5 покрывают все четыре смысловых блока модуля: валидацию имени, разбор ввода, ранжирование и навигацию контроллера. Ни одна мутация не выжила; ни одна не потребовала правки тестов.

## 4. Финальное состояние после мутаций

```
$ md5sum rust/gui-spike/ui/features/slash-command-picker.js tests/unit/slash-command-picker.test.mjs
88da4074bc8fc73d8f6eea8a87fda62c  rust/gui-spike/ui/features/slash-command-picker.js
b618ce0a27babd187469a34e5abdcb75  tests/unit/slash-command-picker.test.mjs

$ node --test tests/unit/slash-command-picker.test.mjs
ℹ tests 10   ℹ pass 10   ℹ fail 0   exit=0
```

Файл возвращён к исходным байтам, `git status` не содержит изменений целевого файла относительно состояния до ревью (оба файла — untracked, как и были).

## 5. Замечания

### P0 — блокирующие
Нет.

### P1 — блокирующие
Нет.

### P2 — не блокируют merge, кандидаты в follow-up
1. `MAX_SLASH_COMMAND_OPTIONS = 25` экспортируется, но нигде не применяется: контроллер не валидирует и не усекает `options` длиннее 25. Оракул `rust/gui-spike/src/slash_commands.rs` задаёт лимит 25 — сейчас он только декларирован. Нужна либо проверка в `createSlashCommandPickerController`/`filterSlashCommands`, либо явная фиксация в спеке, что лимит валидируется на стороне BFF.
2. `parseSlashDraftInput` не сверяет `commandName` с `isValidSlashCommandName`/`MAX_SLASH_COMMAND_NAME_CHARS`: черновик `/` + 200 символов вернёт валидный на вид `commandName`. Для живого автодополнения это смягчается тем, что кандидат всё равно матчится с реальным списком команд, но контракт «разбор = валидация» сейчас не замкнут.
3. Лишний `trim()` в ветке `rawQuery.startsWith("/") ? rawQuery.slice(1).trim() : rawQuery` (`filterSlashCommands`): для запроса `"/mod "` хвостовой пробел обрезается, тогда как `"/mod x"` даст `needle = "mod x"` и не найдёт ничего. Поведение не противоречит тестам, но несимметрично.

### P3 — наблюдения
1. У `role="listbox"` нет `aria-activedescendant`/`tabindex`: активная опция помечается только `aria-selected`. Скринридер список прочитает, но связь «фокус → активная опция» не выражена; клавиатура обрабатывается снаружи (композером), что для текущего контракта приемлемо.
2. При отсутствии `id` у команды рендерится `[data-slash-cmd-id=""]` — селектор неоднозначен при нескольких таких элементах. На практике `id` обязателен по типу `SlashCommandItem`.
3. Безопасность: `innerHTML` не используется, весь текст идёт через `textContent`, вход клонируется (`cloneCommandItem`/`cloneCommandList`) — XSS-поверхности и мутации входа нет. Хорошо.

## 6. Итог

Реализация соответствует заявленному в REPORT.md контракту, тесты содержательные и независимые от внутренней структуры (проверяют наблюдаемое поведение и DOM-атрибуты), targeted-прогон зелёный, 5/5 ручных мутаций убиты, MD5 совпадает с отчётом воркера. **APPROVED** для перевода в `Ready for Merge`; P2-пункты 1–2 предлагаю оформить отдельной карточкой, они не блокируют PR-146.
