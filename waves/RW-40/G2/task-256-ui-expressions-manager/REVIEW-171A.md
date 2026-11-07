# Technical Review 171A — PR-142 (`task-256` / `FE-P0-15`)

- **Ревьюер:** Технический ревьюер (DeepSeek), режим token-lean
- **Ветка:** `feature/task-256-ui-expressions-manager`
- **Проверяемые файлы:** `rust/gui-spike/ui/features/expressions-manager.js`, `tests/unit/expressions-manager.test.mjs`
- **Дата:** 2026-10-31

## Вердикт: **APPROVED**

`P0 = 0`, `P1 = 0`, мутации `5/5` убиты. Тестовый файл не изменялся, полный `gate:worker` не запускался, `git commit` не выполнялся.

## 1. Целевой прогон и сверка MD5

```
$ node --test tests/unit/expressions-manager.test.mjs
ℹ tests 9   ℹ suites 4   ℹ pass 9   ℹ fail 0   exit=0
```

| Файл | md5 (факт) | md5 в `REPORT.md` | Совпадение |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/expressions-manager.js` | `386ffc7915824c9ea2286aea2281b3e0` | `386ffc7915824c9ea2286aea2281b3e0` | да |
| `tests/unit/expressions-manager.test.mjs` | `b5841f715ff257616a2ce19055d74fe4` | `b5841f715ff257616a2ce19055d74fe4` | да |

## 2. Ручные мутации (5/5 убиты)

Методика: одна мутация за шаг, точная замена единственного вхождения, `node --test tests/unit/expressions-manager.test.mjs`, затем побайтовое восстановление из бэкапа `/tmp/opencode/expressions-manager.orig.js` и сверка `md5sum` с базовым `386ffc7915824c9ea2286aea2281b3e0`.

| ID | Мутация | Что ломает | exit | Статус | md5 после восстановления |
| --- | --- | --- | --- | --- | --- |
| M1 | `MIN_EMOJI_NAME_LENGTH = 2` → `1` | нижняя граница длины имени эмодзи | 1 | убита | `386ffc7915824c9ea2286aea2281b3e0` |
| M2 | `MAX_EMOJI_NAME_LENGTH = 32` → `33` | верхняя граница длины имени эмодзи | 1 | убита | `386ffc7915824c9ea2286aea2281b3e0` |
| M3 | `EMOJI_NAME_PATTERN = /^[A-Za-z0-9_]+$/` → `/^[A-Za-z0-9_-]+$/` | алфавит допустимых символов (дефис) | 1 | убита | `386ffc7915824c9ea2286aea2281b3e0` |
| M4 | tie-break по `id` в `compareCustomExpressionsAsc` → `return 0` | детерминированный порядок при равных `name` (`e-10`/`e-20`) | 1 | убита | `386ffc7915824c9ea2286aea2281b3e0` |
| M5 | `DATA_IMAGE_URI_PREFIX = "data:image/"` → `"data:"` | фильтр `data:text/plain` при валидном имени | 1 | убита | `386ffc7915824c9ea2286aea2281b3e0` |

Итог: `ALL_KILLED_AND_RESTORED = True`. Финальная проверка — файл побайтово совпадает с базовым (`cmp` чистый), повторный целевой прогон после восстановления: `9/9 pass, exit=0`.

## 3. Ревью кода (контракт и безопасность)

Проверено соответствие контракту из шапки модуля и `REPORT.md`:

- `EXPRESSION_MANAGER_TABS_RU` — глубоко заморожен (`Object.freeze` на массиве и элементах), порядок `emojis` → `stickers`, подписи «Эмодзи»/«Стикеры».
- `isValidCustomEmojiName` — `trim()`, границы `2..=32` включительно, шаблон `/^[A-Za-z0-9_]+$/`, отказ на нестроках; граница `32` и `33` покрыта тестом.
- `filterCustomExpressions` — `trim().toLowerCase()` для запроса, регистронезависимая подстрока, `localeCompare("ru")` по возрастанию, tie-break по `String(id)` по возрастанию, возврат клонов `{...item}` без мутации входа и без `Array.prototype.sort` по входному массиву.
- `createExpressionsManagerController` — регион `[data-expressions-manager]` с `role="region"` и `aria-label="Управление эмодзи и стикерами"`; вкладки `role="tab"` + `aria-selected`; счётчик `"Слоты: N / M"`; строки `[data-expr-item-id]` с `[data-expr-delete-btn]` и `aria-label`; форма `[data-expr-name-input]`, `[data-expr-datauri-input]`, `[data-expr-upload-btn]`.
- Загрузка вызывает `onUploadEmoji({ name: trimmedName, imageDataUri: trimmedUri })` только при валидном имени и префиксе `data:image/`; `preventDefault` вызывается и на `click`, и на `submit`.
- `setState`/`getState`/`destroy` — снимок состояния через клонирование массивов; мутация возвращённого снимка не влияет на внутреннее состояние (покрыто тестом); `destroy()` очищает контейнер.
- Безопасность: нет `innerHTML`, `eval`, `Function`, сетевых вызовов и динамических селекторов; весь текст идёт через `textContent`/`setAttribute`.

## 4. Замечания (не блокирующие)

- **P3.** `maxSlots` только отображается в счётчике; загрузка не блокируется и кнопка не дизейблится при достижении лимита. Контрактом карточки не требовалось.
- **P3.** Вкладки не реализуют клавиатурную навигацию (стрелки/roving `tabindex`) и не связаны с панелью через `aria-controls`/`role="tabpanel"`. В ТЗ зафиксированы только `role="tab"` и `aria-selected`.
- **P3.** Проверка `data:image/` пропускает `data:image/svg+xml`; контроллер не рендерит Data URI сам и передаёт его наружу, поэтому валидацию фактического содержимого обязан делать backend/загрузчик.
- **P3.** Повторный `destroy()` на уже размонтированном контроллере вызовет `replaceElementChildren(container, [])`, что очистит контейнер целиком. Идемпотентность не заявлена в контракте.

## 5. Финальные md5

```
386ffc7915824c9ea2286aea2281b3e0  rust/gui-spike/ui/features/expressions-manager.js
b5841f715ff257616a2ce19055d74fe4  tests/unit/expressions-manager.test.mjs
```

Файл продукта после всех мутаций побайтово идентичен исходному состоянию PR.
