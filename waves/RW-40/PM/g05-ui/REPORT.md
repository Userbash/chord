# G-05/G-04 UI — поповер сервера и подтверждение выхода: отчёт G2

Дата: 2026-10-24. Воркер: G2 Frontend. PM: DeepSeek.
Карточки: G-05 (поповер меню сервера, P1), G-04 UI (confirm выхода, P1).
Контракт: `waves/RW-40/DESIGN/SERVER-MENU.md` (D-74) §4.2, §4.4, §4.6, §4.7, §6, §7.2, §7.4;
`waves/RW-40/DESIGN/server-menu.json` (`menu_items`, `copy`).
Backend (только чтение): `src/guilds.rs` / `main.rs` `leave_guild` (D-89) — не менялся.

## 1. Статус

**done (frontend, offline).** Меню открывает поповер из 10 пунктов с ARIA/клавиатурой;
«Покинуть сервер» открывает `alertdialog`, подтверждение делает ровно один
`invoke("leave_guild", {guildId, lurking})`; успех — только resolved 2xx, плитка удаляется
после ответа, выбирается предыдущая; 401/403/429/сеть/неизвестный исход — видимый текст,
рейл не мутируется. Окно/live/cargo/gate/сеть/commit не запускались; `src/**`,
`search-overlay.js` и остальной UI не менялись.

## 2. Сделано

| Пункт ТЗ | Где | Проверка (тест) |
|---|---|---|
| 1. Поповер: 10 пунктов D1, `aria-haspopup=menu`, `role=menu/menuitem`, Escape/клик вне, возврат фокуса, disabled по правам/неизвестному | `actions.js`: `SERVER_MENU_SPEC`, `ensureServerMenu`, `openMenu/closeMenu`, `refreshMenuState`, keydown | `G-05: open: … 10 пунктов`, `keyboard: ↑/↓/Home/End…`, `owner: leave disabled…` |
| 2. Leave confirm → `invoke("leave_guild", {guildId, lurking})`; 2xx → удалить плитку/выбрать предыдущую; 401/403/429/сеть — видимая ошибка; `lurking` из явного состояния | `actions.js`: `ensureLeaveDialog`, `confirmLeave`, `completeLeave`, `removeGuildTile`, `leaveFailureText`; `app.js` передаёт `invoke` | `2xx: один invoke…`, `lurking берётся из явного состояния…`, `401`, `403/429`, `двойной клик…` |
| 3. Invite не выдумывает endpoint/preview: пункт disabled по capability (неизвестна → `false`) | `actions.js`: `data-capability-invite`, `refreshMenuState`, handler `guild-invite` | `open: … активны только leave/copy-id` |
| 4. Один путь IPC (`initActions({invoke})`), токен не логируется, backend не тронут | `app.js`, `actions.js` | payload-проверка `JSON` без `token`; `src/**` — только чтение |
| 5. Тесты red-first, 218 прежних не ослаблены | `tests/unit/gui-spike-features.test.mjs` (+10) | red 10 fail → green 228 pass |
| 6. `tsc` 0, geometry 31/31 | — | `tsc.log`, `geometry.log` |

Состав меню (порядок D1): 1 «Пометить как прочитанное», 2 «Пригласить на сервер»,
3 «Заглушить сервер», 4 «Параметры уведомлений» + «Только @упоминания», 5 «Скрыть
заглушённые каналы» (`menuitemcheckbox`, `aria-checked=false`), 6 «Настройки
конфиденциальности», 7 «Редактировать личный профиль сервера», 8 «Переместить...»,
9 «Покинуть сервер» (`data-variant=danger`), 10 «Копировать ID сервера». Активны 9 и 10;
2 disabled, пока нет capability инвайтов (C2), остальные — статичные заглушки.

`lurking`: явный `data-lurking="true"` на плитке гильдии; без атрибута — `false`
(lurker/guest-семантику решает backend-контракт, UI её не вычисляет). Владелец —
явный `data-owner="true"` на плитке: leave disabled, диалог не открывается.

## 3. Red → green

RED (`red.log`, exit 1): **218 passed / 10 failed** — падают ровно новые тесты
(поповер/confirm отсутствовали в DOM).
GREEN (`green.log`, exit 0): **228 passed / 0 failed** (218 база + 10 новых; ни один
прежний тест не менялся).

## 4. Проверки

| Проверка | Результат | Лог |
|---|---|---|
| `node --test gui-spike-{dom,model,bench,features,auth,regressions}.test.mjs` | exit 0 · 228 passed / 0 failed | `green.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0 (0 ошибок) | `tsc.log` |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | exit 0 · 31/31 в допуске | `geometry.log` |

Cargo/gate/window/сеть не запускались; backend `leave_guild` не менялся.

## 5. Файлы и md5

```
2e783fedf7d71db5756048105c46ccfd  rust/gui-spike/ui/features/actions.js
b535a176b88680372c5008dd3148426a  rust/gui-spike/ui/app.js
6dff5c0a6d355a938bd6f4dd48c0ac25  rust/gui-spike/ui/index.html
7b0a6595c0a2e2572e898b1dd5e73e60  rust/gui-spike/ui/components/server-menu.css
e7201e22e7b26fcc1b4e5c94bd6b7825  tests/unit/gui-spike-features.test.mjs
```

`index.html` — только пустые контейнеры `#server-menu`, `#leave-server-dialog`,
`#server-status` и подключение `components/server-menu.css`; содержимое собирает
`actions.js` (один код-путь с DOM-шимом, как `#auth-gate`). `tests/support/gui-dom-harness.mjs`
не менялся. `search-overlay.js`, `src/**`, остальной UI — read-only.

## 6. Остаток / открытые вопросы

1. **Invite (C2)**: пункт 2 disabled до появления share-потока; endpoint/превью не
   выдумывались. При явной `data-capability-invite="true"` обработчик даёт видимый статус,
   а не молчит.
2. **429**: текст с `retry_after` показывается, кнопки снова активны (ручной retry);
   обратный отсчёт с disabled до 0 — следующая карточка (нужны фейковые часы/таймер).
3. **401**: видимая ошибка есть; авто-открытие `#auth-gate` не добавлялось (гейт живёт в
   auth-фиче) — при интеграции стоит связать.
4. **Owner-flow**: leave disabled по `data-owner`; «Удалить сервер»/передача прав — C2.
5. **Empty-состояние рейла**: после выхода последней гильдии ставится
   `#guild-rail[data-state="empty"]`; текстовый empty-state «Нет серверов» — отдельная
   карточка списка серверов.
6. **Геометрия диалогов** — `unmeasured` (SERVER-MENU §4.4): значения из токенов; после
   первого живого окна сверить числа.
7. **`guild-copy-id`**: clipboard best-effort (в шиме API нет), ID не логируется.
