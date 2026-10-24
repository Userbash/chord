# NET-D — экран входа и auth-состояния (frontend)

**Волна:** RW-39, отдел NET, роль NET-D (Frontend). PM: DeepSeek. Дата: 2026-10-23.
**Основание:** `BRIEF-NETD.md`, `ADR-2-auth.md` §A/§5/§6, `NET/PM/netc2/REPORT.md` (API команд).
**Статус:** выполнено, red-first. Окно/probe/cargo/gate/сеть/commit не использовались.

## 1. Что сделано

| Файл | Что |
|---|---|
| `rust/gui-spike/ui/features/auth.js` (новый) | `createAuthGate`: состояния `idle → validating → ready → error`, вызовы `auth_status`/`auth_login_token`/`auth_restore`/`auth_logout` через `window.__TAURI__.core.invoke`, маппинг `AuthError.kind` → строки, очистка поля до ответа ядра, фокус, show/hide токена, logout в user-bar |
| `rust/gui-spike/ui/index.html` | оверлей `#auth-gate` (не `.screen`, `screenCount` = 4): `role=dialog`, `aria-modal`, `aria-label`, скрыт `hidden`/`aria-hidden` до входа |
| `rust/gui-spike/ui/app.js` | `createAuthGate` в `boot()`, `onReady` → `router.switchScreen("chat")` + фокус в `#chat-list`, `auth` в `App`/`currentApp()` |
| `rust/gui-spike/ui/components/auth.css` (новый) | слой `--z-modal`, карточка/поле/кнопки на токенах, `[hidden]`/`[aria-hidden]` → `display:none` |
| `rust/gui-spike/ui/style.css` | `@import url("components/auth.css")` (подключение, как в брифе) |
| `tests/unit/gui-spike-auth.test.mjs` (новый) | 18 поведенческих тестов (ниже) |
| `tests/unit/gui-spike-dom.test.mjs` | +1 тест: новый id `#auth-gate` и его id-контракт в шиме; существующие 129 не менялись |
| `waves/RW-39/NET/PM/netd/*.log` | baseline, red, green, tsc, geometry, мутации, md5 |

### Поведение

| Состояние | Видимый результат | Проверка |
|---|---|---|
| `idle` (+`stored`) | гейт открыт, `#auth-restore` виден, фокус в `#auth-token`, `auth_status` вызван 1 раз без аргументов | `idle + stored…` |
| `idle` (`stored=false`) | restore скрыт (`hidden`), submit активен | `idle без ключа в keyring…` |
| `validating` | `aria-busy=true`, submit/restore `aria-disabled=true` + `disabled` + `title`, статус «Проверяем токен…»; токен уже вне DOM | `submit: validating…`, `restore: …` |
| `ready` | гейт `hidden` + `aria-hidden=true`, `.screen.active = #screen-chat`, имя из `/users/@me` в `.user-name`, logout в `.user-actions` | `ready на старте…`, `submit: …` |
| `error` | гейт открыт, `#auth-error` (`role=alert`, `aria-live=assertive`) с понятной строкой, submit снова активен, фокус в поле | 401/429/сеть/IPC/Keyring — 5 тестов |

Тексты (без токена, URL и тела ответа): `InvalidToken` → «Токен недействителен…»,
`RateLimited` → «Слишком часто: подождите и повторите», `Network`/сорванный IPC →
«Нет связи…», `Keyring` → «Хранилище токена недоступно…», `Parse`/`Internal`/`EmptyToken`
— свои строки; неизвестный `kind` не показывает сырой текст.

Безопасность: токен захватывается в локальную переменную, поле очищается **до** `invoke`;
`auth.js` не содержит `console.*`; токен не пишется в атрибуты/`title`/`data-*`/снапшот.
Проверка — тест `токен не появляется ни в гейте, ни в user-bar, ни в снапшоте UI`
(полный обход DOM: теги/id/классы/title/value/атрибуты + `consoleErrors`/`consoleWarns` +
все IPC, кроме `auth_login_token`).

### Разметка: html-контейнер + сборка формы в JS (осознанное решение)

`tests/support/gui-dom-harness.mjs` — не файл NET-D и не правился; но существующий тест
`шим-разметка совпадает с ui/index.html` требует, чтобы **каждый** id/класс из html
существовал в шиме. Поэтому в html лежит только носитель `#auth-gate` (требование брифа),
а содержимое (форма/кнопки/ошибка) собирает `features/auth.js` — так же, как
`#emoji-grid`/`#settings` наполняются из JS. Один код-путь даёт одинаковый DOM в браузере
(переиспользует статический контейнер) и в шиме (создаёт его), дублирования разметки нет.
Контракт id закреплён тестом в `gui-spike-dom.test.mjs` (только новые id, как в брифе).

`screens.js` не менялся: гейт живёт вне `.screen`, а блокировка rail на время входа
сломала бы `UI_SNAPSHOT.actions.screenClicks=4` (автопрогон кликает rail). «Чат доступен»
обеспечено `onReady` (роутер → chat, фокус в `#chat-list`) и снятием `aria-hidden` с `.shell`.

## 2. Проверки

| # | Состояние → тест | Команда | Результат |
|---|---|---|---|
| 1 | baseline до правок | `node --test tests/unit/gui-spike-{dom,model,bench,features}.test.mjs` | **129 passed**, `00-baseline.log` |
| 2 | red: гейта нет | `node --test tests/unit/gui-spike-auth.test.mjs` | **18 fail** (0 pass), `01-red.log` |
| 3 | green | то же | **18 passed**, `02-green.log` |
| 4 | регресс существующих | 4 файла (dom/model/bench/features) | **130 passed** (129 + новый id-тест), `03-existing.log`/`05-dom.log` |
| 5 | типы | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **exit 0**, `04-tsc.log` |
| 6 | геометрия | `node waves/RW-39/PM/w39c/geometry-check.mjs` | **31/31 в допуске**, `07-geometry.log` |
| 7 | мутация «имя команды» (`auth_login_token`→`auth_login`) | auth-тесты | **7 fail** → открыто, `08-mutation-cmd.log` |
| 8 | мутация «нет очистки поля» (удалён `ui.token.value = ""`) | auth-тесты | **7 fail** → открыто, `09-mutation-clear.log` |
| 9 | green после восстановления | auth + 4 файла | **18 pass**, **130 pass**, `10-green-final.log`/`11-existing-final.log` |

Покрыто тестами: разметка/семантика оверлея и подключение CSS; idle+stored / idle без
ключа; ready на старте (гейт скрыт, чат активен, имя, logout); submit (trim, очистка поля,
`auth_login_token {token}`, двойной клик → один invoke, validating-аффордансы, ready);
пустой ввод (без IPC); 401/429/Network/IPC-reject/Keyring-в-состоянии; restore (успех,
отозванный токен); logout (гейт вернулся, имя сброшено, фокус в поле); show/hide;
«нет мёртвых кнопок» (только reveal/submit/restore, disabled → с подсказкой); среда без IPC
(приложение живо, чат отрисован, `console.error` пуст); отсутствие утечки токена.

## 3. md5

```
05af33f2d41cdb0b98e90b39714d2e41  rust/gui-spike/ui/index.html
f90dc8334c883e00d463cac8a4140200  rust/gui-spike/ui/features/auth.js
8e6ef5c52dfaeab96f16fb934c10a8b2  rust/gui-spike/ui/app.js
9a786825489ad6e1b84730b7945ef18c  rust/gui-spike/ui/style.css
8f10de67d5e01c7646e12037a2b715bb  rust/gui-spike/ui/components/auth.css
5ded1c36e8f94899c48224b6dcc2728d  tests/unit/gui-spike-auth.test.mjs
182a754d0d5e9bfbdc393c971decb1a1  tests/unit/gui-spike-dom.test.mjs
```

Baseline (до правок): `index.html 653282056da39a0c84eeae84c7d54eb1`,
`app.js 5e7c3999487cde76783982700c88fb95`, `screens.js b5cad5847b8ca4b99d70a8df9ababfc5`,
`style.css f660b65d28c27b7b64d3c881ed2a862b` (`00-baseline-md5.log`).

Границы соблюдены: `src/**`, `DESIGN/**`, `TDD-PLAN`, `tests/support/gui-dom-harness.mjs`,
остальной `ui/**`, `logs/`, `functions/`, `evidence/` не изменялись; коммитов нет.

## 4. Остаток (вне NET-D)

1. **Событие `auth://state`** (ADR-2 §5) по-прежнему не эмитится backend'ом (netc2 §4.2):
   UI работает на снимках команд. Для спиннера на внешние переходы (4004/logout из ядра)
   нужен `AppHandle::emit` + `window.__TAURI__.event.listen("auth://state", …)`; в
   DOM-шиме `__TAURI__.event` нет — тест на стрим появится вместе с эмиссией.
2. **Gateway**: logout в UI вызывает только `auth_logout`; остановки супервизора в
   gui-spike нет (появится с gateway-волной, порядок из ADR-2 §6).
3. **Live-проверка** (вставка токена → READY, 401, рестарт из keyring, logout) — только
   в согласованном окне; здесь запрещена.
4. **Retry-after 429**: строка «подождите» статична; обратный отсчёт по
   `retry_after_ms` — после появления стрима состояний.
5. **Keyring prompt**: при заблокированной коллекции первый `auth_status` может ждать
   ответа потока-владельца; UI показывает `validating` без таймаута — нужен лимит
   ожидания (кандидат в NET-C2/UI-доработку).
6. **ToS**: импорт user-токена = риск блокировки аккаунта (ADR-2, `AUTH-RE.md` §8);
   только тестовый аккаунт, письменная фиксация у владельца.
