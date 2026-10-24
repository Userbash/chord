# BRIEF W39h3 — закрыть P0/P1/P2 Codex во frontend

Владелец: W39h3 (Frontend fix). PM: DeepSeek. Запуск: после W39h2 (общий `actions.js`).
Основание: `waves/RW-39/PM/w39h/REVIEW-codex.md` (FAIL: P0=1, P1=1, P2=1).

## Файлы (только эти)
- `rust/gui-spike/ui/features/embeds.js`
- `rust/gui-spike/ui/features/actions.js`
- `rust/gui-spike/ui/index.html` (только атрибуты gift)
- `rust/gui-spike/ui/features/search-overlay.js`
- `tests/unit/gui-spike-features.test.mjs`, `tests/unit/gui-spike-dom.test.mjs`
- логи: `waves/RW-39/PM/w39h3/`

## Задачи (red → green)
1. **P0 URL.** В `linkEmbed` разрешить только `http:`/`https:`; иначе `href` не ставить
   (или `#` + `aria-disabled`), сохранить `rel="noreferrer noopener"`. Red-тест:
   `javascript:alert(1)` не попадает в `href`; `https://…` попадает.
2. **P1 gift.** Либо рабочий handler (открывает заглушку «Phase 3» с видимым состоянием),
   либо `aria-disabled="true"` + подпись. Мёртвого клика быть не должно. Red-тест: клик не
   даёт `ACTION_FAILED`, состояние меняется/disabled.
3. **P2 sort.** «По новизне» и «Самое актуальное» должны различаться: актуальное — закреплённые
   выше, затем свежие; новизна — чисто по времени. Red-тест на разный порядок.
4. Не ослаблять 120 тестов; добавить минимум +3.

## Проверки
```bash
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```
Окно/probe/cargo/gate/сеть/commit — запрещены.

## Отчёт
`waves/RW-39/PM/w39h3/REPORT.md`: P0/P1/P2 → red → green, md5, остаток.
