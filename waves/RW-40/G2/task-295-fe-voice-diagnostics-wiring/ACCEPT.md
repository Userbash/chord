# ACCEPT — PR-175 (`task-295` / `FE-INT-16`: Подключение `voice-diagnostics.js` в `rust/gui-spike/ui/features/voice.js`)

- **Статус PM-приёмки:** `ACCEPTED` (`Ready for Technical Review`)
- **Ветка:** `feature/task-295-fe-voice-diagnostics-wiring`
- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Модель исполнения:** `DeepSeek` (`deepseek/deepseek-v4.1-flash`, `L1 / P1`)

## Сверка с DoD
1. Из `rust/gui-spike/ui/features/voice.js` реэкспортированы `evaluatePingQuality` и `createVoiceDiagnosticsController` из `./voice-diagnostics.js`, сохранена 100% обратная совместимость `pingQuality(ping)` (`VG-06`), добавлен `mountVoicePanel(root, store, service, options)` и обработка клика по `#voice-ping` (`togglePopout`).
2. `node --test tests/unit/voice-ui.test.mjs` (`47/47 PASS`), `node --test tests/unit/voice-diagnostics.test.mjs` (`67/67 PASS`), `npm run typecheck` (`0 errors`).
3. Передано на техническое ревью `TR-201A` (5/5 мутаций).
