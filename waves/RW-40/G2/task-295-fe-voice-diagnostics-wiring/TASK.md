# TASK-295 (FE-INT-16) — Интеграция `voice-diagnostics.js` (`RTCConnectionPopout` / `evaluatePingQuality`) с `rust/gui-spike/ui/features/voice.js` (`L1 / P1`)

- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Уровень сложности / Модель:** `L1 (Микро-задача 1–2 файла)` → `DeepSeek` (`--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Приоритет:** `P1`
- **Ветка:** `feature/task-295-fe-voice-diagnostics-wiring` (от `unstable`)
- **Источник:** `waves/RW-40/PM/BACKLOG-RE-22.md` (`RE-22-04`)

---

## 1. Строгий Write-Scope (ровно 2 файла)

1. `rust/gui-spike/ui/features/voice.js`
2. `tests/unit/voice-ui.test.mjs`

Никакие другие файлы менять нельзя.

---

## 2. Точный контракт изменений в `rust/gui-spike/ui/features/voice.js`

> **КРИТИЧЕСКИ ВАЖНО:** Не ломай обратную совместимость существующей функции `pingQuality(ping)` при вызове с одним аргументом `pingQuality(ping)` (`<= 120` → `"fine"`, `<= 250` → `"average"`, `> 250` → `"bad"`), так как контракт `waves/RW-40/DESIGN/voice-spec.json` (`VG-06`) и существующие тесты в `tests/unit/voice-ui.test.mjs` проверяют именно эти пороги для `#voice-ping[data-quality]`!

1. Импортируй и реэкспортируй из `./voice-diagnostics.js`:
   ```js
   import {
     evaluatePingQuality,
     createVoiceDiagnosticsController,
   } from "./voice-diagnostics.js";
   export { evaluatePingQuality, createVoiceDiagnosticsController };
   ```
2. В `mountVoicePanel(root, store, service = null, options = {})`:
   - Поддержи опциональное поле `options?.diagnosticsController` (или создание контроллера диагностики при передаче `options?.enableDiagnosticsPopout === true` и наличии `root?.ownerDocument`).
   - Если передан `options?.diagnosticsController` (объект с методом `togglePopout(anchorEl)` или `openPopout()`), при клике на `#voice-ping` в состоянии `connected` вызывай `options.diagnosticsController.togglePopout(pingEl)`.
   - Сохрани все существующие проверки `mountVoicePanel` без изменений.

---

## 3. Тестирование (`tests/unit/voice-ui.test.mjs`)

1. В `tests/unit/voice-ui.test.mjs` добавь 2–3 теста в конце файла:
   - проверка реэкспорта `evaluatePingQuality` и `createVoiceDiagnosticsController` из `voice.js` (`evaluatePingQuality(250, 0) === "fine"`, `evaluatePingQuality(251, 0) === "average"`, `evaluatePingQuality(501, 0) === "bad"`, `evaluatePingQuality(100, 10.1) === "bad"`);
   - проверка клика по `#voice-ping` в `mountVoicePanel` при переданном `options.diagnosticsController` (вызывается `togglePopout`).
2. Прогони:
   ```bash
   node --test tests/unit/voice-ui.test.mjs
   node --test tests/unit/voice-diagnostics.test.mjs
   npm run typecheck
   ```
3. Запиши краткий отчёт в `waves/RW-40/G2/task-295-fe-voice-diagnostics-wiring/REPORT.md`.
