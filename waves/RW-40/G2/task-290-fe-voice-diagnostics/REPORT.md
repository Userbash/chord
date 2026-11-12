# REPORT — task-290 (`RE-22-04`, G2 Frontend): `voice-diagnostics.js` + unit-тесты

- **Ветка:** `feature/task-290-fe-voice-diagnostics` (рабочее дерево `/home/user/orca/workspaces/project/feature-task-290-fe-voice-diagnostics`)
- **Волна / отдел:** `RW-40` / `G2 Frontend (WebView UI & DOM/A11y)`, уровень `L2`, приоритет `P1`
- **Модель:** `opencode --model deepseek/deepseek-v4.1-flash`
- **Дата:** 2026-10-31
- **Коммитов нет:** `git commit` / `git push` не выполнялись (запрет процесса).

## 1. Сделано

| Файл | Статус |
|---|---|
| `tests/unit/voice-diagnostics.test.mjs` | новый, 67 тестов / 14 сьютов, `// @ts-check` |
| `rust/gui-spike/ui/features/voice-diagnostics.js` | новый, `// @ts-check`, полные JSDoc-типы, без DOM |
| `TDD-PLAN.md` | +1 строка в §8 (`tests/unit/voice-diagnostics.test.mjs`), позиция по алфавиту между `voice-devices-settings` и `voice-grid` |

## 2. md5sum

```
4faed2e5814fee87d04df0bd9ad294cc  tests/unit/voice-diagnostics.test.mjs
75a2a965b69d65471377f8c0506895ac  rust/gui-spike/ui/features/voice-diagnostics.js
66f4670fb4dbb6bd9c13bd21af61a35d  TDD-PLAN.md
```

## 3. Strict TDD (red → green)

**Red** — `red.log`: тест написан и запущен до реализации, модуль ещё отсутствует:

```
Error [ERR_MODULE_NOT_FOUND]: Cannot find module '.../rust/gui-spike/ui/features/voice-diagnostics.js'
✖ tests/unit/voice-diagnostics.test.mjs
ℹ pass 0  ℹ fail 1   EXIT=1
```

По ходу зелёной фазы один тест-ожидание было исправлено (это ошибка теста, не кода): `(5.05).toFixed(1) === "5.0"` из-за двоичного представления; ожидание приведено к семантике оракула (`toFixed(1)`) на значении `5.06 → "5.1"`.

**Green** — `green.log`:

| Проверка | Команда | Результат |
|---|---|---|
| targeted unit | `node --test tests/unit/voice-diagnostics.test.mjs` | `67/67 pass`, exit 0 |
| tsc (корень) | `npx tsc -p tsconfig.json --noEmit` | 0 errors, exit 0 |
| tsc (UI) | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | 0 errors, exit 0 |
| plan-sync | `node --test tests/meta/plan-sync.test.mjs` | `5/5 pass`, exit 0 |
| весь unit-набор (доп.) | `npm run test:unit` | `2005/2005 pass`, exit 0 |

## 4. Соответствие RE-оракулу

Оракул: `reference-oracle/decoded/3eefa7dae1e2c234_0` (read-only), документы `RE-GAP-22-VOICE-DIAGNOSTICS.md` и `BACKLOG-RE-22.md` §`RE-22-04`.

| Элемент контракта | Реализация | Оракул |
|---|---|---|
| `PING_FINE_MAX_MS=250`, `PING_AVERAGE_MAX_MS=500`, `LOSS_FINE_MAX_PCT=5`, `LOSS_AVERAGE_MAX_PCT=10` | `evaluatePingQuality` | мод. `613681` @ `2469479` (`get quality`) |
| `PING_HISTORY_MAX_POINTS=199`, `PING_AVERAGE_WINDOW=20`, `DEFAULT_PING_TIMEOUT_MS=4000` | константы + окно спарклайна (`slice(-199)`) | мод. `613681` @ `2474810`, `2504200`, `2504320` |
| `SPARKLINE_WIDTH=258`, `SPARKLINE_HEIGHT=80` | `computeSparklinePoints` | `<canvas width="258" height="80">` (мод. `229659`/`546738`) |
| `POPOUT_WIDTH_PX=300`, `DEBUG_MODAL_WIDTH_PX=854`, `DEBUG_MODAL_HEIGHT_PX=480` | константы | `.container__50387`, `PopoutWindowRTCDebug` (`767675`) |
| `RTC_DEBUG_SECTIONS` (8 секций, `Object.freeze`) | константа, порядок `zWA` | мод. `412780` @ `9264635` |
| `evaluatePingQuality` | `bad > 500 \|\| > 10%`, `average > 250 \|\| > 5%`, иначе `fine`; невалидный вход → `unknown` | мод. `613681` @ `2469479` |
| `extractShortHostname` | `split(".")[0]`, плюс отсечение порта до `:`; нестроковый/пустой вход → `""` | мод. `978171` @ `7459580` |
| `formatVoicePrivacyCodeChunks` | 30 цифр (пробелы допускаются) → замороженные 6×5, иначе `null` | мод. `917592` @ `6025550` |
| `computeSparklinePoints` | `{x,y,value}`, `x∈[0..width]`, `y` инвертирован `[0..height]`, `{value}`-точки, отброс невалидных, окно 199 | мод. `229659`/`546738` |
| `formatPopoutMetricsRu` | `headline`, `averagePingLabel`, `lastPingLabel`, `lossLabel`, `encryptionLabel`, `hasWarning` (`250 мс` / `10%`) | мод. `978171` @ `7459850..7460350`, `7459160` |
| `createVoiceDiagnosticsController` | `isOpen`, `activeTab` (`integration`/`privacy`), `isDebugModalOpen`, `activeDebugSection` (`TRANSPORT`), `copiedPrivacyCode`; `open/close/toggle/selectTab/openDebugModal/closeDebugModal/copyPrivacyCode/handleKeyDown/updateSnapshot/getState` | мод. `978171`, `999751`, `917592`, `233545` |

**Явно задокументированные решения нормализации (в JSDoc модуля):**
- `pingMs` принимается только числом: `null`, `undefined`, `NaN`, `-1`, `""`, строки, объекты → `"unknown"` (как в `RE-GAP-22` §2.2).
- `outboundLossRatePct` опционален; переданное значение обязано лежать в `[0..100]`, иначе `"unknown"` — защита от битого снимка статистики.
- `formatPopoutMetricsRu`: нечисловая потеря печатается прочерком `—` (как `null`); `secureFramesProtocolVersion` принимается, но в строки не попадает — оракул показывает только баннер E2EE/SRP.
- `copyPrivacyCode()` возвращает `false` без валидного кода (снимок пуст/код короче 30 цифр) и не вызывает колбэк.

## 5. 9 измерений тестового покрытия

1. **Позитивные** — `fine/average/bad` по пингу и потерям, `getShortHostname`, полный набор русских строк попапа, полный жизненный цикл контроллера, копирование кода.
2. **Негативные** — невалидные пинг/потери, нестроковый хост, нецифровой/короткий/длинный код, неизвестная вкладка, `Escape` без открытого UI.
3. **Граничные** — ровно `250/251`, `500/501` мс, `5/5.0001`, `10/10.0001` %, `0 мс`, 199 из 250 точек, координаты в `[0..258]×[0..80]`, ровно 30 цифр.
4. **Ошибки** — `Infinity`, `-Infinity`, `NaN`, `-1`, `101%`, `"abc"`, `MAX_SAFE_INTEGER`, отсутствие колбэков.
5. **Пустые** — `[]` → `[]`, `""` → `""`, `""`/`null`/`undefined` код → `null`, `updateSnapshot(null|"bad"|7)` → `null`, пустой объект попапа.
6. **Загрузочные** — `open()` вызывает `onFetchDiagnostics` ровно один раз, повторный `open()` не дублирует загрузку, снимок до загрузки `null`.
7. **Права доступа / безопасность** — `Object.freeze` на `RTC_DEBUG_SECTIONS` и на чанках кода (мутация бросает `TypeError`), баннер E2EE выбирается только по `isEndToEndEncrypted === true`.
8. **Восстановление** — `Escape` закрывает сначала модалку, затем попап; `close()` чистит флаги; после `close()` → `open()` состояние пригодно; `updateSnapshot` сбрасывает `copiedPrivacyCode`.
9. **Совместимость / наблюдаемость** — `getState()` отдаёт копию (мутация результата не влияет), `updateSnapshot` копирует снимок, `getState().snapshot` напрямую скармливается в `formatPopoutMetricsRu`.

## 6. Риски и замечания

- `voice.js` **не трогался** (вне write-scope этой карточки): пороги `120/250` и отсутствие обработчика клика по `#voice-ping` остаются задачей подключения модуля — нужен отдельный write-scope на `voice.js` (см. `BACKLOG-RE-22` §`RE-22-04`, где `voice.js` указан, но в `TASK.md` разрешены только 3 файла).
- Контроллер намеренно без DOM: рендер попапа/модалки (Canvas, `role="dialog"`, `aria-labelledby`) в этот срез не входит, тесты проверяют переносимое состояние и чистые форматтеры.
- `package-lock.json` (M) и `node_modules` (D) были изменены/удалены до старта задачи и мной не трогались.
- Полный `gate:worker` (fmt/clippy/workspace) не запускался: задача в batch-лейне, уровень гейта `T0` (targeted unit + tsc + meta); дополнительно прогнан весь `npm run test:unit` — 2005/2005.

## 7. Проверка приёмки (команды для ревьюера)

```bash
cd /home/user/orca/workspaces/project/feature-task-290-fe-voice-diagnostics
node --test tests/unit/voice-diagnostics.test.mjs          # 67/67
npx tsc -p tsconfig.json --noEmit                          # 0 errors
npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit        # 0 errors
node --test tests/meta/plan-sync.test.mjs                  # 5/5
```
