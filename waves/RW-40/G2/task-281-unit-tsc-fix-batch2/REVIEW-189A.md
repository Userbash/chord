# REVIEW-189A — TR (`task-281` / PR-160, unit tsc fix batch 2)

**Вердикт: APPROVED** — `P0=0`, `P1=0`, мутации `5/5`.
Ветка: `bugfix/task-281-unit-tsc-fix-batch2`, HEAD `17dd852`.
Объём ревью: только `tests/unit/svg-icons.test.mjs` и `tests/unit/channel-tree.test.mjs` (по ТЗ).

## 1. Контракт файлов

| Файл | md5 (факт) | md5 (ТЗ) | Совпадение |
|---|---|---|---|
| `tests/unit/svg-icons.test.mjs` | `fee2fa69b0ed19c9a572f18361c6d30f` | `fee2fa69b0ed19c9a572f18361c6d30f` | да |
| `tests/unit/channel-tree.test.mjs` | `7aa8a39f23f9dd2dcf73d5a707450404` | `7aa8a39f23f9dd2dcf73d5a707450404` | да |

Продуктовые модули не изменялись: `svg-icons.js` `7fb502e19f6495d77cc53542c023abb5`,
`channel-tree.js` `04c1e353683249095d2c3c80e26326d0`. `git diff --stat -- rust/gui-spike/ui/features` — пусто.

## 2. Baseline

```
npx tsc -p tsconfig.json --noEmit | grep -E "svg-icons\.test\.mjs|channel-tree\.test\.mjs"
→ 0 строк
```

Остаточные ошибки `tsc` в дереве относятся к другим файлам (`profile-modal.test.mjs`,
`voice-input-mode.test.mjs`) и в область карточки не входят.

```
node --test tests/unit/svg-icons.test.mjs tests/unit/channel-tree.test.mjs
→ tests 23, pass 23, fail 0, skipped 0
```

## 3. Суть правки

Диф — только типы: JSDoc-`@typedef` на `ChannelCategoryNode`, локализация `tree.categories[0]`
в переменную с кастом, `/** @type {any} */` на результат `createIconSvg` (возвращает
`SVGElement | null` при fake-DOM), `@returns {any}` у `makeDoc`, приведение `SVG_ICONS`
к `Record<string, string>`. Ни одно утверждение не удалено и не ослаблено: количество и текст
`assert` не менялись, ветки проверок те же.

## 4. Мутации (`5/5` убиты)

Каждая мутация применялась к продуктовому модулю по одному вхождению, прогонялся целевой
тест-файл, затем файл восстанавливался из сохранённой копии с проверкой md5 до/после
(`git checkout` не понадобился — ни одного остаточного изменения).

| # | Файл | Мутация | Результат | Убивший тест |
|---|---|---|---|---|
| M1 | `svg-icons.js` | `aria-hidden="true"` → `"false"` | KILLED | «создаёт svg с каноническими атрибутами по умолчанию» |
| M2 | `svg-icons.js` | `fill="currentColor"` → `"none"` | KILLED | «создаёт svg с каноническими атрибутами по умолчанию» |
| M3 | `svg-icons.js` | `viewBox` → `0 0 24 24` | KILLED | «канонические атрибуты» + «принимает размер и className» |
| M4 | `channel-tree.js` | снят порядок voice после text (`return 0`) | KILLED | «non-voice channels sort before voice-like ones…» |
| M5 | `channel-tree.js` | фильтр `unread === true` → `!== true` | KILLED | «collapsed category hides read channels…» + «…empty visible list» |

Итог: `TOTAL_KILLED=5/5`. После восстановления повторный прогон — `23/23 pass`,
`tsc`-grep по двум файлам — 0 строк.

## 5. Замечания

- `P0`: нет.
- `P1`: нет.
- `P2` (не блокирует): приведение через `/** @type {any} */` в `svg-icons.test.mjs` —
  приемлемый компромисс для fake-DOM, но при следующем касании файла дешевле расширить
  `FakeNode`/`makeDoc` типами (`@returns {Document}`) вместо каста результата.
- `git commit` не выполнялся.

## 6. Артефакты

- Лог tsc-grep: `/tmp/opencode/tsc-189a.txt` (0 строк).
- Скрипт мутаций с восстановлением и md5-проверкой: `/tmp/opencode/mutate-189a.py`.
