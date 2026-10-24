# BRIEF W39h — frontend P3-02 «чат-вью» по спеке дизайна

Владелец: W39h (Frontend). PM: DeepSeek. Запуск: после приёмки W39g.
Вход: `DESIGN/SPEC-P3-02.md`, `HANDOFF.md` §3/§5, `DOM-CONTRACT.md` §8, текущие
`ui/features/*`, `ui/model.js`.

## Файлы (только эти)
- `rust/gui-spike/ui/app.js`
- `rust/gui-spike/ui/features/chat.js`, `members.js`, `search.js`, `actions.js`, новые
  `features/{rows,embeds,reactions,search-overlay}.js`
- `rust/gui-spike/ui/components/chat.css`, `members.css`, `composer.css` (только дополнения)
- `tests/unit/gui-spike-features.test.mjs`, `gui-spike-dom.test.mjs` (только новые проверки;
  существующие не ослаблять)
- логи: `waves/RW-39/PM/w39h/`
READ-ONLY: `ui/index.html`, `ui/tokens.css`, `ui/model.js` (менять только при доказанной
нехватке — через PM), `src/**`, `DESIGN/**`.

## Требования
1. Рендер сообщений: group start/continuation, day divider, unread divider, reply-quote,
   reactions-row, embeds (link/image), edited-метка.
2. Hover-панель сообщения (состав — R-16), клики меняют состояние, без «мёртвых» кнопок:
   либо действие, либо `aria-disabled=true` + подсказка.
3. Композер: состояния empty/typing/disabled, лимит 2000, attach/emoji affordance.
4. Участники: 6 групп (роль/статус), поиск по загруженному окну, счётчики.
5. Search overlay: 7 секций + сортировка, результаты по загруженному окну; честно показывать
   «найдено / загружено».
6. 60 fps p95 ≤ 20 мс, `loadedRows ≤ 500`, `inFlight` без регрессий; ошибки — `UIError`.

## Проверки
```bash
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```
Окно/probe/cargo/gate — запрещены (WINDOW-BUDGET).

## Отчёт
`waves/RW-39/PM/w39h/REPORT.md`: фича → проверка → результат, новые тесты, md5, остаток.
