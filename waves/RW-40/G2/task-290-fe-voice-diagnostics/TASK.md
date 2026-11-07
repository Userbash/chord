# TASK — task-290 (`G2 Frontend L2 / P1`, `RE-22-04`: Модуль `voice-diagnostics.js` + юнит-тесты `voice-diagnostics.test.mjs`)

- **Отдел:** G2 Frontend (WebView UI & DOM/A11y) (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L2 / P1` (по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/feature-task-290-fe-voice-diagnostics` (`branch:feature/task-290-fe-voice-diagnostics`)
- **Разрешённые к созданию/изменению файлы (строго 3 файла):**
  1. `rust/gui-spike/ui/features/voice-diagnostics.js` (новый файл, с `// @ts-check` и полными JSDoc-типами)
  2. `tests/unit/voice-diagnostics.test.mjs` (новый файл, с `// @ts-check`)
  3. `TDD-PLAN.md` (добавить ровно 1 строку в конец таблицы §8 для `tests/unit/voice-diagnostics.test.mjs`, чтобы `node --test tests/meta/plan-sync.test.mjs` проходил `5/5`)
- **Артефакты отчёта:**
  - `/home/sanya/orca/project/waves/RW-40/G2/task-290-fe-voice-diagnostics/REPORT.md`
  - `/home/sanya/orca/project/waves/RW-40/G2/task-290-fe-voice-diagnostics/red.log`
  - `/home/sanya/orca/project/waves/RW-40/G2/task-290-fe-voice-diagnostics/green.log`

## Источники контракта (прочитай сразу, не сканируя весь репозиторий)
- `/home/sanya/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`
- `/home/sanya/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md` (§ `RE-22-04`)

## Требования к реализации (Strict TDD: сначала `tests/unit/voice-diagnostics.test.mjs` -> `red.log`, затем `voice-diagnostics.js` -> `green.log`)

1. **Константы и пороги по RE-оракулу (`decoded/3eefa7dae1e2c234_0`, модули `613681`, `978171`, `917592`, `412780`):**
   - `PING_FINE_MAX_MS = 250`, `PING_AVERAGE_MAX_MS = 500`
   - `LOSS_FINE_MAX_PCT = 5`, `LOSS_AVERAGE_MAX_PCT = 10`
   - `PING_HISTORY_MAX_POINTS = 199`, `PING_AVERAGE_WINDOW = 20`, `DEFAULT_PING_TIMEOUT_MS = 4000`
   - `SPARKLINE_WIDTH = 258`, `SPARKLINE_HEIGHT = 80`, `POPOUT_WIDTH_PX = 300`, `DEBUG_MODAL_WIDTH_PX = 854`, `DEBUG_MODAL_HEIGHT_PX = 480`
   - `RTC_DEBUG_SECTIONS = Object.freeze(["TRANSPORT", "OUTBOUND", "INBOUND", "SCREENSHARE", "CAMERA", "CLIPS", "STREAMS_OUTBOUND", "STREAMS_INBOUND"])`
2. **Чистые функции и контроллер в `rust/gui-spike/ui/features/voice-diagnostics.js`:**
   - `evaluatePingQuality(pingMs, outboundLossRatePct)`:
     - если `pingMs` не конечное число или `< 0` -> возвращает `"unknown"`;
     - если `outboundLossRatePct != null` и (!Number.isFinite(Number(outboundLossRatePct)) или `< 0` или `> 100`) -> бросает/возвращает `"unknown"` (или нормализует по контракту; документируй явно и протестируй: для невалидного `pingMs` (`null`, `undefined`, `NaN`, `-1`, `""`) возвращай `"unknown"`);
     - если `pingMs > 500` ИЛИ (` loss != null && loss > 10 `) -> `"bad"`;
     - если `pingMs > 250` ИЛИ (` loss != null && loss > 5 `) -> `"average"`;
     - иначе (`pingMs <= 250` и (`loss == null || loss <= 5`)) -> `"fine"`.
   - `extractShortHostname(endpoint)`:
     - из `"finland10192.discord.media:443"` возвращает `"finland10192"` (часть до первой точки `.` или двоеточия `:`; для пустой/нестроковой входа возвращает `""`).
   - `formatVoicePrivacyCodeChunks(code)`:
     - принимает строку из 30 десятичных цифр (допускаются пробелы между группами), возвращает замороженный массив из 6 строк по 5 цифр `["12345", "67890", ...]` (при неверной длине или нецифровых символах возвращает `null`).
   - `computeSparklinePoints(pings, width = 258, height = 80)`:
     - принимает массив точек пинга (чисел или `{ value }`, берёт последние до `199` точек) и возвращает массив координат `{ x, y, value }` в пределах `[0..width] × [0..height]`.
   - `formatPopoutMetricsRu({ shortHostname, averagePingMs, lastPingMs, outboundLossRatePct, isEndToEndEncrypted, secureFramesProtocolVersion })`:
     - форматирует русские строки попапа:
       - `headline`: `shortHostname ? \`Голосовая связь подключена — ${shortHostname}\` : "Голосовая связь подключена"`
       - `averagePingLabel`: `\`Средняя задержка: ${Math.round(Number(averagePingMs) || 0)} мс\``
       - `lastPingLabel`: `\`Последняя задержка: ${Math.round(Number(lastPingMs) || 0)} мс\``
       - `lossLabel`: `outboundLossRatePct == null ? "Потеря исходящих пакетов: —" : \`Потеря исходящих пакетов: ${Number(outboundLossRatePct).toFixed(1)}%\``
       - `encryptionLabel`: `isEndToEndEncrypted ? "Защищено сквозным шифрованием" : "Защищённое соединение"`
       - `hasWarning`: `Number(lastPingMs) > 250 || (outboundLossRatePct != null && Number(outboundLossRatePct) > 10)`
   - `createVoiceDiagnosticsController({ onFetchDiagnostics, onOpenDebugModal, onCopyPrivacyCode })`:
     - управляет состоянием попапа (`isOpen`, `activeTab`: `"integration"` | `"privacy"`, `isDebugModalOpen`, `activeDebugSection`: по умолчанию `"TRANSPORT"`, `copiedPrivacyCode`: `false`), методами `open()`, `close()`, `toggle()`, `selectTab(tab)`, `openDebugModal(section)`, `closeDebugModal()`, `copyPrivacyCode()`, `handleKeyDown(event)` (`Escape` закрывает попап/модалку), `updateSnapshot(snapshot)`, `getState()`.
3. **Строгая типизация и тесты (9 измерений):**
   - Обязательно прогони:
     - `node --test tests/unit/voice-diagnostics.test.mjs`
     - `npx tsc -p tsconfig.json --noEmit` (`0 errors`)
     - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`)
     - `node --test tests/meta/plan-sync.test.mjs` (`5/5 PASS`)
4. Запиши `red.log`, `green.log` и `REPORT.md` (с `md5sum` всех 3 файлов) и заверши через `orca orchestration complete`. Не делай `git commit/push`.
