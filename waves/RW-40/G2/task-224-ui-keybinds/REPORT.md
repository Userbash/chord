# T-224 (FE-P2-23) — Keybind Shortcut Parser & Russian Display Formatter

## Что сделано

- `rust/gui-spike/ui/features/keybinds.js` — новый модуль с `// @ts-check` и JSDoc:
  - `DEFAULT_SHORTCUTS` — замороженный набор из 6 комбинаций;
  - `normalizeKeyCombo(event)` — сборка комбо (Ctrl/Ctrl-from-Meta → Alt → Shift → клавиша),
    `null` на пустой клавише и на голых `Control`/`Shift`/`Alt`/`Meta`, `" "` → `Space`,
    одиночный символ в верхний регистр;
  - `matchesShortcut(event, comboString)` — строгое сравнение нормализованного комбо;
  - `formatShortcutAriaLabelRu(comboString)` — `"Горячая клавиша: "` + комбо или `"не назначена"`.
- `tests/unit/keybinds.test.mjs` — 14 тестов (4 сьюта): точный набор и заморозка дефолтов,
  нормализация (Ctrl/Meta, порядок модификаторов, одиночная клавиша, пробел, отказы),
  совпадения/несовпадения, русская подпись.

## Red → Green

- Red: `node --test tests/unit/keybinds.test.mjs` → exit 1, `pass 0 / fail 1`
  (модуль не существовал), лог `red.log`.
- Green: тот же прогон → exit 0, `tests 14 / pass 14 / fail 0`, лог `green.log`.

## md5

```
5101ef99b194f32f2e1b7264154eeedd  rust/gui-spike/ui/features/keybinds.js
ced869f7d8343cd5fd268d02c46692cf  tests/unit/keybinds.test.mjs
```

## Границы и риски

Правки только в двух разрешённых файлах, `git commit` не делался. Полный `gate:worker`
и `cargo`-прогоны по режиму token-lean не запускались; `package-lock.json` уже был изменён
до начала работы и не трогался.
