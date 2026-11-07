# REVIEW-96A — FE-P2-01 (task-182) tooltip.js

**Вердикт: APPROVED.** P0=0, P1=0, мутации 5/5 KILLED. Не коммитил.

## Проверено

- `rust/gui-spike/ui/features/tooltip.js` (381 строка): один общий `#discord-tooltip`
  (`role="tooltip"`, `aria-hidden`, `data-open`, `data-placement`), миграция `title → data-tooltip`,
  `aria-describedby` на активной цели, `attach/detach/showFor/hide/destroy`, инъецированные таймеры.
- `node --test tests/unit/tooltip.test.mjs` → **20/20 pass**, exit 0.
- md5 до и после прогона: `3cae3eca51640289a37424c03b7da2bb` — совпал.

## Мутации (каждая ломает свой контракт)

1. `TOOLTIP_SHOW_DELAY_MS 400 → 300` → exit 1, pass 18 / fail 2.
2. Удалён `target.removeAttribute("title")` → exit 1, pass 17 / fail 3.
3. `TOOLTIP_CLOSE_KEYS.includes(key) → false` → exit 1, pass 19 / fail 1.
4. Удалён `setAttribute("data-open", "true")` → exit 1, pass 18 / fail 2.
5. Удалён `setAttribute("aria-describedby", …)` → exit 1, pass 18 / fail 2.
После каждой мутации файл восстановлен из бэкапа, финальный md5 сверен.

## Остаточные риски (не блокирующие)
1. Подпись `attach` «Прикрепить файл» против «Прикрепить вложения» в `ru-catalog.json` (G3/G7).
2. `tests/unit/tooltip.test.mjs` нет в `PLAN.md` §8 — зона Team Lead, `meta`/`plan-sync` красный.
3. Контроллер не смонтирован в `app.js`/`index.html`; позиционирование — вне модуля.
