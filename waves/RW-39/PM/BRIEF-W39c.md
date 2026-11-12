# BRIEF W39c — верстка shell + 4 экрана (Phase 2a)

Владелец: W39c (Markup/CSS). PM: DeepSeek. Запуск: после приёмки E1 и E2.
Контракт заморожен: `waves/RW-39/DESIGN/DOM-CONTRACT.md` (полный `index.html` в §3).
Числа — только из `waves/RW-39/DESIGN/tokens.css` (SPEC §1: без кода статуса число не берём).

## Файлы (только эти; `ui/tokens.css` — PM-approved дополнение к матрице TZ)

- `rust/gui-spike/ui/index.html` — каноническое дерево из DOM-CONTRACT §3, байт-в-байт по id/class/ARIA
- `rust/gui-spike/ui/tokens.css` — вендоринг `DESIGN/tokens.css` (те же имена/значения; шапка «source: waves/RW-39/DESIGN/tokens.css, md5 …»)
- `rust/gui-spike/ui/style.css` — layout/shell
- `rust/gui-spike/ui/components/*.css` — rail, tree, chat, members, composer, header, emoji, settings
- логи: `waves/RW-39/PM/w39c/`

`ui/app.js`, `ui/model.js`, `ui/features/**` — **не трогать** (W39d). `src/**` — W39e.

## Требования

1. Геометрия: rail 64–72, tree ~240, header ~48, composer ~56, members ~240, строки 22–26/48,
   участник 42 — из `tokens.css`/`layout.json`, не «на глаз».
2. Сохранить инварианты RW-38: id/роли из DOM-CONTRACT §2 (`#screen-*`, `#chat-list`,
   `#emoji-grid` как `listbox` + `option`, `#member-list`, `#settings`, `#bench`, `.rail-btn`,
   `.screen-title`, ARIA) — DOM-тест должен остаться зелёным.
3. Никаких `url()`, `@font-face`, спрайтов, логотипа Discord; бренд `DL`; emoji — системные.
4. CSS без JS-зависимостей для раскладки; скрытые экраны — `[hidden]`/`.screen[aria-hidden]`,
   не `display:contents` на контейнерах с ролями.

## Проверка

```bash
node --test tests/unit/gui-spike-dom.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
python3 scripts/gui-window-probe.py --json /tmp/opencode/gui-probe-w39c.json --timeout 30
# визуальная сверка: /tmp/opencode/gui-window.png vs reference-oracle/ui/00-app.png (read-only)
```

## Отчёт

`waves/RW-39/PM/w39c/REPORT.md`: таблица «блок → токен → значение → источник», png-сверка,
DOM/AT-SPI результаты, файлы+md5. Красное — честно, без skip.
