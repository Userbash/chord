# BRIEF W39c2 — добить верстку: канонический `index.html` + обвязка CSS

Владелец: W39c2 (Markup/CSS). PM: DeepSeek. Итерация: 2 (продолжение W39c).
Основание: W39c сделал `tokens.css` + shell-CSS, но `index.html` остался прежним (53 строки),
каноническое дерево из DOM-CONTRACT §3 не внедрено. Это остаток E3a.

## Файлы (только эти)

- `rust/gui-spike/ui/index.html` — заменить на каноническое дерево из `DOM-CONTRACT.md` §3
  (строки 61–272), байт-в-байт по id/class/ARIA; `#bench` сохранить.
- `rust/gui-spike/ui/tokens.css` — **read-only в этой итерации**: PM перевендорит из
  `DESIGN/tokens.css` после приёмки E1b (W39b2 идёт параллельно). Стили ссылаются на имена
  токенов, не на значения, поэтому маркапу это не мешает.
- `rust/gui-spike/ui/style.css`, `ui/components/*.css` — только если каноническая разметка
  требует правки селекторов; визуальные числа — из `tokens.css`/`layout.json`.
- логи: `waves/RW-39/PM/w39c2/`

`ui/app.js`, `ui/model.js`, `ui/features/**` — read-only (W39d). `src/**` — W39e.
`tests/**` — read-only: если тест падает, чинится разметка, а не тест.

## Инварианты RW-38 (ломать нельзя)

`#screen-chat|emoji|members|settings`, `#chat-list`, `#member-list`, `#emoji-grid`
(`role=listbox`) + ячейки `role=option` с `aria-label`, `#emoji-tabs`, `#emoji-search`,
`#settings`, `#bench`, `.rail-btn[data-screen]`, `.screen`, `.screen-title`;
ARIA: `role=list/listitem/tab/gridcell/region`, `aria-selected`, `aria-current`.
Скрытие экранов — `[hidden]`/`aria-hidden`, не `display:contents` на контейнерах с ролями.

## Проверки (без живого окна — WINDOW-BUDGET)

```bash
node --test tests/unit/gui-spike-dom.test.mjs
node --test tests/unit/gui-spike-model.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```

Окно/`gate:worker`/`gui-window-probe.py` **не запускать**: приёмка — один прогон PM.

## Запрещено

- `url()`, `@font-face`, спрайты, логотип/ассеты Discord; бренд `DL`; emoji системные.
- Менять `app.js`/`model.js`, ослаблять тесты, сеть, commit/push, cargo.

## Отчёт

`waves/RW-39/PM/w39c2/REPORT.md`: что заменено в `index.html`, какие CSS-селекторы пришлось
поправить и почему, вывод 4 проверок, md5 файлов, честный список остатка. Красное — красным.
