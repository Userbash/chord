# TASK-298 (FE-FIX-09) — Восстановление 38/38 UI IPC-команд (`toggle_reaction`) в `rust/gui-spike/ui/features/reactions.js` (`L1 / P0`)

- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Уровень сложности / Модель:** `L1 (Микро-задача, 1 файл, 3 строки)` → `Antigravity Flash Low` (`--agent antigravity --model gemini-3.8-flash-low --effort low`)
- **Приоритет:** `P0 (Gate Blocker: ipc_wiring_contract 37 != 38)`
- **Ветка:** `bugfix/task-298-fe-reactions-invoke-alias-fix` (от `unstable`)

---

## 1. Строгий Write-Scope (ровно 1 файл)

- `rust/gui-spike/ui/features/reactions.js`

Никакие другие файлы менять нельзя.

---

## 2. Точное описание проблемы и исправления (строки 295–311)

В `rust/gui-spike/tests/ipc_wiring_contract.rs:899-908` функция `extract_ui_invoke_commands` сканирует `rust/gui-spike/ui/**/*.js` по префиксам:
`"invoke(\""`, `"invoke('"`, `"invokeFn(\""`, `"invokeFn('"`, `"invokeReaction(\""`, `"invokeReaction('"`.
В `rust/gui-spike/ui/features/reactions.js` на строках `295–311` переменная была названа `invoker`, из-за чего вызов `await invoker("toggle_reaction", ...)` перестал попадать в `extract_ui_invoke_commands` (осталось 37 из 38 команд).

### Что нужно сделать в `rust/gui-spike/ui/features/reactions.js` (строки 295–311):
Замени локальное имя `invoker` на `invokeFn`:
```js
  const invokeFn = options.invoke !== undefined ? options.invoke : invokeReaction;
  pending.add(requestKey);
  element?.setAttribute("data-pending", "true");
  if (!invokeFn) {
    pending.delete(requestKey);
    element?.removeAttribute("data-pending");
    options.onSettled?.();
    return;
  }
  try {
    await invokeFn("toggle_reaction", {
      // Явный реальный адрес приоритетнее bench-значений по умолчанию.
      channelId,
      messageId,
      emoji,
      active,
    });
```

---

## 3. Команды проверки (без запуска cargo, так как cargo занят слотом G1)

```bash
node --test tests/unit/reactions.test.mjs tests/unit/reactions-picker.test.mjs
npm run typecheck
npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
```
Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G2/task-298-fe-reactions-invoke-alias-fix/REPORT.md`. Не делай git merge в `unstable`.
