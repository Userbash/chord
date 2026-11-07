# REVIEW-181A — PR-152 / task-268 (FE-INT-11)

Technical Review (этап 181A), роли: Technical Reviewer (DeepSeek), token-lean режим.
Дата: 2026-10-31. Ветка: `feature/task-268-ui-chat-helpers-bridge`, база `b196aea`.
Объём проверки: только три артефакта карточки. Тестовый файл не правился, `gate:worker` не запускался,
`git commit` не делался.

## Вердикт

**APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты.

## 1. Targeted-тест и сверка артефактов

```
node --test tests/unit/chat-helpers-bridge.test.mjs
ℹ tests 11  ℹ pass 11  ℹ fail 0      exit=0
```

| Файл | md5 (факт) | md5 в REPORT.md | Совпадение |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/chat-helpers-bridge.js` | `972ee85338329e2cb98ff3d909bd67cc` | `972ee85338329e2cb98ff3d909bd67cc` | да |
| `tests/unit/chat-helpers-bridge.test.mjs` | `e3daee06110c901214ff91d6978fe814` | `e3daee06110c901214ff91d6978fe814` | да |

Эталон для мутаций: побайтовая копия `chat-helpers-bridge.js` в
`/tmp/opencode/tr181a/chat-helpers-bridge.js.orig`
(`md5 = 972ee85338329e2cb98ff3d909bd67cc`, `sha256 = 49b90c05e95382f009f80bf452a38b504becd22ec9273c7a22da5734b91c807d`,
`cmp` против рабочего файла — идентичны).

## 2. Мутации (по одной, restore из бэкапа после каждой)

Методика: точная замена одного анкера в продуктовом файле, `node --test tests/unit/chat-helpers-bridge.test.mjs`,
фиксация `exit`/`pass`/`fail`, затем восстановление файла копированием из бэкапа и сверка `md5` + `cmp`.

| # | Мутация | md5 мутанта | exit | pass/fail | Убита | Тест-убийца |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | `Math.round((count / nextThreshold) * 100)` → `Math.floor(...)` | `45a7709adab8376e1f26da92c2804d79` | 1 | 10/1 | да | `boostProgressPercent — прогресс до следующего уровня` (6/7 = 85.71 → ожидается 86, floor даёт 85) |
| M2 | на максимальном уровне `return 100` → `return 99` | `aac471185edcc84c9f3eb070d826982b` | 1 | 10/1 | да | `boostProgressPercent — ... 100 на максимуме` (14 и 100 бустов) |
| M3 | обрезка темы `limit - 1` → `limit` (off-by-one) | `60a96a2af7b351c65469e125d9583dd4` | 1 | 9/2 | да | `formatChannelTopicTooltip: длинная тема режется до maxChars с многоточием` + тест `maxChars=1/2` |
| M4 | убран `Object.freeze` у модели | `3f6409a8fabc335b64b3d5ddc689db2d` | 1 | 10/1 | да | `возвращает замороженный объект с ровно шестью полями` (`Object.isFrozen`) |
| M5 | `isThreadLocked: Boolean(...)` → сырое значение | `19c306fa320726cc813bfb21668d087b` | 1 | 9/2 | да | `пустые options дают безопасные значения по умолчанию` (`undefined !== false`) + `locked: 1` |

Итог: **5/5 убиты, exit != 0 у всех**. После каждой мутации файл восстановлен байт-в-байт
(`cmp` + `md5` совпали с эталоном 5 раз из 5), финальный контрольный прогон — `11/11 pass`, `exit=0`.

## 3. Финальное состояние

```
md5(rust/gui-spike/ui/features/chat-helpers-bridge.js) = 972ee85338329e2cb98ff3d909bd67cc
md5(tests/unit/chat-helpers-bridge.test.mjs)           = e3daee06110c901214ff91d6978fe814   (не изменялся)
node --test tests/unit/chat-helpers-bridge.test.mjs    → 11/11 pass, exit=0
```

`git status`: только `node_modules` (D), `package-lock.json` (M) — предсуществующие изменения окружения,
и два untracked файла карточки. Продуктовый файл после мутационной сессии побайтово равен сдаче воркера.

## 4. Замечания по коду

Блокирующих нет (`P0 = 0`, `P1 = 0`). Контракт соблюдён: дублирования логики нет — бейдж непрочитанных,
справочник сроков и уровни бустов берутся из `thread-badge.js` / `guild-boost.js`; модель заморожена и
содержит ровно шесть полей; тесты проверяют публичное поведение, включая нормализацию неизвестного срока
и границы `maxChars=1/2`.

Неблокирующие наблюдения (не требуют правки в этой карточке):

- `P3` — `formatChannelTopicTooltip(topic, 0)` (или отрицательный `maxChars`) вернёт `"…"` длиной 1 при лимите 0:
  контракт такого входа не описывает, тесты не покрывают.
- `P3` — `const threadMetadata = options?.threadMetadata ?? undefined;` — `?? undefined` избыточен
  (поведение то же), читаемость не страдает.
- `P3` — модуль ещё не подключён к рендеру заголовка и не входит в `tsc`-проект UI: зафиксировано в
  `REPORT.md` как задача другого слайса, вне write-scope.
