# REVIEW — TR-192A / PR-163 / task-285 (`ui/composer-popovers-bridge`: клонирование `options`, пустое имя команды)

- **Ревьюер:** Технический ревьюер (DeepSeek, TR-192A)
- **Ветка:** `bugfix/task-285-ui-composer-popovers-pass2` (HEAD `886ec1e`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED**
- **Дефекты:** `P0=0`, `P1=0`, `P2=0`, `P3=3`
- **Мутации:** **5/5 убиты**, файл восстановлен побайтно, MD5 совпал с эталоном ТЗ

## 1. Проверяемые файлы

| Файл | MD5 (эталон ТЗ) | MD5 (факт: до, после мутаций, после red-симуляции) | Статус |
|---|---|---|---|
| `rust/gui-spike/ui/features/composer-popovers-bridge.js` | `7f0ab60ad52f504e1f6ec8194e304a8a` | `7f0ab60ad52f504e1f6ec8194e304a8a` (три замера) | совпал |
| `tests/unit/composer-popovers-bridge.test.mjs` | `5c28324d2dd832960b186f6f667cb449` | `5c28324d2dd832960b186f6f667cb449` | совпал |

Диф — `+30/−3` в модуле и `+70/−0` в тестах. В тестах только добавления: два новых `it`, ни одна существующая проверка не удалена и не ослаблена. Это важно — оракулом остались 11 прежних тестов плюс два новых.

## 2. Scope и `git status`

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/features/composer-popovers-bridge.js
 M tests/unit/composer-popovers-bridge.test.mjs
```

Правки ровно в двух файлах ТЗ. `D node_modules` и `M package-lock.json` — шум окружения, воспроизводится в несвязанных деревьях (см. `REVIEW-187A.md` §2); в коммит PR-163 не включать.

## 3. Red-first: воспроизвёл сам, а не по чужому логу

Взял HEAD-версию модуля (`git show HEAD:<path>`), прогнал тесты, вернул рабочую версию:

| Состояние | `tests` | `pass` | `fail` | Падающие тесты |
|---|---|---|---|---|
| HEAD-модуль + новый тест-файл | 13 | 11 | 2 | «getFilteredSlashCommands отдаёт глубокую копию options…», «selectSlashCommand с пустым именем команды…» |
| Рабочая версия (PR-163) | 13 | 13 | 0 | — |

Красный ровно на двух новых контрактах, оба падения — из нужных тестов, а не падение импорта. Совпадает с `red.log` (`pass 11, fail 2`) и `green.log` (`pass 13, fail 0`).

## 4. Гейты

| Проверка | Команда | Результат |
|---|---|---|
| Unit | `node --test tests/unit/composer-popovers-bridge.test.mjs` | `tests 13, pass 13, fail 0`, exit `0` |
| Typecheck по файлу ТЗ | `npx tsc -p tsconfig.json 2>&1 \| grep composer-popovers-bridge` | `0` строк, `grep` exit `1` |
| Typecheck по репозиторию | `npx tsc -p tsconfig.json` | 49 строк ошибок, **все** в чужих `tests/unit/*` (attachment-validator, audit-log, embed-formatter, forum-filter и др.) — предсуществующие, карточку не касаются |

## 5. Мутации: 5/5

Метод: точечная замена одного выражения, прогон профильного теста, восстановление файла, сверка MD5. Мутации задевают оба новых контракта (M1/M3 — клонирование `options`, M2 — пустое имя) и три существующих.

| ID | Мутация | Итог | Убивший тест | Счёт |
|---|---|---|---|---|
| M1 | `cloneSlashCommandItem`: элементы `options` копируются по ссылке (`map((option) => option)`) | **убита** | «getFilteredSlashCommands отдаёт глубокую копию options и не мутирует состояние моста» | `pass 12, fail 1` |
| M2 | `selectSlashCommand`: удалён guard `selectedName.length === 0` | **убита** | «selectSlashCommand с пустым именем команды возвращает null и не вызывает колбэк выбора» | `pass 12, fail 1` |
| M3 | `getFilteredSlashCommands`: отдаёт внутренний `filteredSlashCommands` без клона | **убита** | «getFilteredSlashCommands отдаёт глубокую копию options и не мутирует состояние моста» | `pass 12, fail 1` |
| M4 | `openPopover`: снята проверка принадлежности `COMPOSER_POPOVER_LAYERS` | **убита** | «openPopover бросает Error при неизвестном или невалидном идентификаторе…» | `pass 12, fail 1` |
| M5 | `updateDraft`: фильтр команд берётся с `""` вместо `commandName` | **убита** | «updateDraft активирует slash-command-picker при слэш-префиксе и фильтрует команды по commandName» | `pass 12, fail 1` |

После каждого восстановления и после всего раунда MD5 — `7f0ab60ad52f504e1f6ec8194e304a8a`, контрольный прогон `13/13 pass`. Прогоны — `mutations/` вне репозитория (`/tmp/opencode/mutate-192a-v2.mjs`), рабочее дерево чистое по модулю.

Проверка на «бумажную» мутацию M3: клонирование в мосте **не избыточно**. `filterSlashCommands` из `slash-command-picker.js` уже клонирует элементы (`cloneCommandItem`), но мост хранит **один** массив `filteredSlashCommands` между вызовами, поэтому без клона на выдаче мутация потребителя попадает в следующую выдачу — M3 это и ловит.

## 6. Замечания (все P3, merge не блокируют)

### P3-1. Форма элемента изменилась: `options: []` появился там, где ключа не было

`cloneSlashCommandItem` всегда пишет `options` (ветка `: []`), поэтому команда без `options` теперь отдаётся как `{ id, name, options: [] }`, а раньше — `{ id, name }`. Проверил на живом модуле: `createComposerPopoversBridge({commands:[{id:"c2",name:"ping"}]}).getFilteredSlashCommands()` → `[{"id":"c2","name":"ping","options":[]}]`. Потребителей вне модуля и тестов нет (`grep` по `getFilteredSlashCommands`/`createComposerPopoversBridge` в `rust/gui-spike/ui`, `tests`, `src` пуст), `deepEqual` по элементам нигде не сравнивается целиком, так что риск нулевой. Но если появится код, различающий «нет options» и «пустые options», это выстрелит. Дешевле было `options: Array.isArray(cmd.options) ? ... : cmd.options`.

### P3-2. `cloneSlashCommandItem` дублирует приватный `cloneCommandItem` из `slash-command-picker.js`

Логика один в один (кроме фолбэка на `[]`). Дублирование вынужденное — `cloneCommandItem` не экспортируется, — но при правке формата `options` в пикере копия в мосте отстанет молча. Тестом это не ловится: обе копии сейчас эквивалентны.

### P3-3. `selectSlashCommand` валидирует `trim()`, а отдаёт имя как есть

`"  ping  "` проходит проверку (`selectedName.trim()`), но в колбэк и в возврат уходит исходный `name` с пробелами. Поведение не изменилось этим PR (было и раньше), тест новый его не фиксирует. Либо нормализовать `name` при выдаче, либо явно записать в JSDoc, что имя отдаётся как передано.

## 7. Что осталось за пределами ревью

- Полный `npm run gate:worker` не запускался: ТЗ TR-192A ограничивает проверки targeted-прогоном одного теста, мутациями и `tsc`. Полный гейт — на integration-лейне batch.
- `git commit`/`push` не выполнялись (запрещено ТЗ).
- 49 строк ошибок `tsc` в чужих `tests/unit/*` — отдельные карточки, к PR-163 отношения не имеют.

## 8. Итог

PR-163 закрывает ровно два заявленных контракта: глубокая копия `options` в `getFilteredSlashCommands` и отказ `selectSlashCommand` на пустом/отсутствующем имени без вызова колбэка и без изменения состояния. Красный воспроизведён независимо (11/13), зелёный — 13/13, оба новых контракта защищены тестами, что доказано убийством M1–M3, включая проверку неизбыточности клонирования. Пять точечных мутаций убиты, файл восстановлен побайтно.

**Вердикт: APPROVED** (`P0=0`, `P1=0`, мутации `5/5`).
