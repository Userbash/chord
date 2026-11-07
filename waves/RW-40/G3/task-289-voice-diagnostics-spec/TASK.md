# TASK — task-289 (`G3 Design L1 / P2`, `RE-22-03`: Дизайн-спецификация и CSS-токены `RTCConnectionPopout` и `RTCDebugModal`)

- **Отдел:** G3 Design (UX/UI & Specs) (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L1 / P2` (по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/design-task-289-voice-diagnostics-spec` (`branch:design/task-289-voice-diagnostics-spec`)
- **Разрешённые к созданию файлы (строго 2 файла):**
  1. `docs/specs/VOICE-DIAGNOSTICS-SPEC.md`
  2. `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md`
- **Артефакты отчёта:**
  - `/home/sanya/orca/project/waves/RW-40/G3/task-289-voice-diagnostics-spec/REPORT.md`

## Источники (прочитай сразу, не сканируя весь репозиторий)
- `/home/sanya/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`
- `/home/sanya/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md` (§ `RE-22-03` и `RE-22-04`)

## Требования к спецификации
1. Зафиксируй точные размеры и геометрии по RE-оракулу (`RE-GAP-22`):
   - Индикатор качества в голосовой панели (`#voice-ping`): иконка `18×18px`, `padding: 7px`, `border-radius: 8px`, классы `.rtcConnectionQualityFine`, `.rtcConnectionQualityAverage`, `.rtcConnectionQualityBad`.
   - Пороги качества (`pingQuality`, модуль `613681`):
     - `fine`: `ping <= 250` мс И (`outboundLossRate == null` ИЛИ `outboundLossRate <= 5.0%`);
     - `average`: (`ping <= 500` мс И `outboundLossRate <= 10.0%`), если не `fine`;
     - `bad`: `ping > 500` мс ИЛИ `outboundLossRate > 10.0%`.
   - Попап `RTCConnectionPopout`: `width: 300px`, Canvas-спарклайн `258×80px` (история до `199` точек пинга), `shortHostname`, `Средняя задержка: {avg} мс` (`toFixed(0)` по 20 точкам), `Последняя задержка: {last} мс`, `Потеря исходящих пакетов: {loss}%` (`toFixed(1)`), баннер предупреждения (`badPing=250`, `badLossRate=10`).
   - Вкладки попапа: `Интеграция` / `Конфиденциальность` (E2EE DAVE, сетка 3 колонки `--secure-frames-columns: 3` для 30-значного `Voice Privacy Code` 6 групп × 5 цифр, кнопка `Копировать` -> `Скопировано`).
   - Модальное окно `RTCDebugModal`: `854×480px`, 8 секций (`TRANSPORT`, `OUTBOUND`, `INBOUND`, `SCREENSHARE`, `CAMERA`, `CLIPS`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`), скользящее окно графиков `600` точек `{ value, time }`.
2. Опиши CSS-токены (`--text-feedback-positive`, `--text-feedback-warning`, `--text-feedback-critical`, `--background-feedback-positive/warning/critical`, `--brand-500`), ARIA-атрибуты (`role="dialog"`, `aria-labelledby`, `role="tablist"`, `role="tab"`, `role="tabpanel"`) и клавиатурную навигацию (`Escape`, `Tab`, `ArrowLeft/ArrowRight`).
3. Запиши спецификацию в оба файла (`docs/specs/VOICE-DIAGNOSTICS-SPEC.md` и `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md`), оформи `/home/sanya/orca/project/waves/RW-40/G3/task-289-voice-diagnostics-spec/REPORT.md` с `md5sum` и заверши через `orca orchestration complete`. Не делай `git commit/push`.
