# BRIEF W39d — фронтенд-интеракции (Phase 2b)

Владелец: W39d (Frontend). PM: DeepSeek. Запуск: после green W39c (файлы не пересекаются).
Вход: принятая верстка W39c + `DOM-CONTRACT.md` (id/роли менять нельзя) + `SPEC.md` §interactions.

## Файлы (только эти)

- `rust/gui-spike/ui/app.js`
- `rust/gui-spike/ui/features/*.js` (по одному модулю на фичу, ESM + `// @ts-check` + JSDoc)
- `rust/gui-spike/ui/model.js` — **только в рамках D-04** (переменная высота строк: 26/48/+22,
  участник 42)
- `tests/unit/gui-spike-dom.test.mjs` — **только ожидания высот** по D-04, парами
  «индекс → высота» из `waves/RW-37/RE/layout.json`; обязательна мутация, доказывающая, что
  тест ловит константу 44; удалять/ослаблять другие assert запрещено
- `tests/support/gui-dom-harness.mjs` — **только аддитивно** (новые узлы/свойства шима под
  генерацию; assert-ы и `boxHeight` не трогать; диф показывает PM)
- `waves/RW-39/DESIGN/DOM-CONTRACT.md` — **только §9.2** (числа 44/40 → 26/48/+22, участник 42)
- логи: `waves/RW-39/PM/w39d/`

`ui/index.html`, `ui/style.css`, `ui/components/**`, `ui/tokens.css` — read-only (W39c2);
`src/**` — W39e.

## Требования

1. Работать на новой разметке: rail-навигация 4 экранов (и `body[data-active-screen]` вместо
   временного CSS-моста `:has()`), категории эмодзи — **плоский listbox:** `#emoji-grid`
   `role=listbox`, ячейки `role=option` + `aria-label`, **без `.emoji-row[role=row]`** (D-07),
   поиск, настройки (38 тумблеров + `#settings-nav`), панель участников `#member-panel-list`
   и экран участников из `model.js` (виртуализация, `loadedRows ≤ 500`, `inFlight`).
2. Клавиатура и a11y: `aria-selected`/`aria-current`, focus-trap панелей, Escape, roving
   tabindex для `option` (OBS-01), `aria-live` только для счётчиков (не для FPS).
3. 60 fps p95 ≤ 20 мс на 100k/24 229/10/38 (метрика RW-37) — не регрессировать.
4. D-04: переменная высота строк 26/48/+22, участник 42 — в `model.js`, ожиданиях DOM-теста
   и `DOM-CONTRACT §9.2`; запись old→new в отчёт + мутация «константа 44 ловится».
5. Ошибки не глотать: типизированный `UIError` + видимое состояние, либо комментарий best-effort.

## Проверка

```bash
node --test tests/unit/gui-spike-dom.test.mjs
node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```

`cargo`, GUI smoke, `gate:worker`, окно — **не запускать** (параллельно идёт W39e с cargo;
WINDOW-BUDGET — один прогон на приёмку).

## Отчёт

`waves/RW-39/PM/w39d/REPORT.md`: фичи → проверка → результат, метрики p95/PSS, файлы+md5,
что осталось. Без commit/push.
