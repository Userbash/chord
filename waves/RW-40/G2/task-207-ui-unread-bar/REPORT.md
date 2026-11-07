# T-207 UI: unread bar (баннер непрочитанных + «Перейти к последним»)

Карточка: task-207-ui-unread-bar, отдел G2 (Frontend), режим token-lean.

## Результат
- `rust/gui-spike/ui/features/unread-bar.js` — чистые функции состояния, `// @ts-check` + JSDoc:
  - `JUMP_TO_PRESENT_THRESHOLD_PX = 300`;
  - `computeUnreadBannerState(unreadCount, sinceLabelRu = "")` — скрытие при `<= 0` и нечисловых, `Math.trunc`, подпись «Новых сообщений: N[ с <label>]», действие «Отметить как прочитанное»;
  - `computeJumpToPresentState(distanceFromBottomPx, newMessagesCount = 0)` — видимость при `distance >= 300` или новых сообщениях, бейдж `99+`, подписи «Перейти к последним сообщениям» / «К настоящему моменту».
- `tests/unit/unread-bar.test.mjs` — 11 тестов, 3 сьюта.

## TDD
- Red: `node --test tests/unit/unread-bar.test.mjs` → падение (модуль отсутствует), лог `red.log`.
- Green: тот же прогон → 11/11 pass, 0 fail, лог `green.log`.

## md5sum
```
40dc98f1f3a829a6e150dae9eb9cb57f  rust/gui-spike/ui/features/unread-bar.js
b547e466c62b131cbb429b53445c64c4  tests/unit/unread-bar.test.mjs
```

## Границы и далее
- Правки только в двух файлах Write-Scope; чужие модули, доки и §8 не тронуты, `git commit` не делался.
- Полный `gate:worker` и остальные тесты не запускались по условию token-lean — нужны в integration-лейне.
- Интеграция (mount в chat-панель, a11y-атрибуты, CSS) остаётся за отдельной карточкой.
