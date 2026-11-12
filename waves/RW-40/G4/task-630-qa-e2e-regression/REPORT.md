# Отчёт воркера — TASK-630 (`G4 QA`): Headless Browser E2E Regression Suite для Build 005–006

- **Карточка / Задача**: `TASK-630` (`task_0c9fe6d031a0` / `ctx_9856d3026583`)
- **Отдел**: `G4 QA (Test Automation & Verification)`
- **Ветка**: `feature/task-630-qa-e2e-regression`

## 1. Что сделано

Реализован автоматизированный набор headless-браузерных E2E регрессионных тестов в `tests/e2e/e2e-build006.test.mjs` поверх реального `rust/gui-spike/ui/index.html`, локального HTTP-сервера, headless Chromium (CDP) и детерминированного fake-транспорта `window.__TAURI__` (`openApp` из `tests/helpers/ui-fixture.mjs`). Добавлена запись в реестр тестов `TDD-PLAN.md` §8 (требование мета-инварианта `tests/meta/plan-sync.test.mjs`).

Покрыты 7 сквозных E2E-сценариев функций **BUILD-005** и **BUILD-006**:
1. **`TASK-610` (Interactive hCaptcha Solver Modal)**: перехват ответа `HTTP 400` (`code: 40003`, `captcha_sitekey`, `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`) при `accept_invite`, открытие `#captcha-modal` (`role="dialog"`, `aria-modal="true"`), локальная валидация пустого токена в `#captcha-modal-error` без лишнего IPC-вызова, повторный вызов `accept_invite` с `captcha_key`/`captcha_rqtoken`/`captcha_session_id` и добавление приглашённой гильдии в `#guild-rail`.
2. **`TASK-611 / TASK-612` (Server Creation Wizard UI)**: открытие `#create-guild-dialog`, выбор шаблона (`gaming` → `guild_template_code: "2TffvPucqHkN"`), загрузка base64-иконки с превью, локальная валидация длины имени (`< 2` символов) с `aria-invalid="true"`, успешный вызов `create_guild` и появление созданного сервера в `#guild-rail`.
3. **`TASK-616 / TASK-623` (Stream Viewer, PiP & DAVE MLS E2EE)**: монтирование плеера `[data-stream-viewer]` в `watching`, проверка бейджа `DAVE MLS E2EE` с 8-значным кодом верификации и индикатора `LIVE`, переключение звука (`[data-action="toggle-mute"]`), разворачивание на весь экран (`[data-action="toggle-fullscreen"]`), автоматическое сворачивание в Picture-in-Picture (`data-pip="true"`) при переключении канала и отключение от стрима (`[data-action="leave-stream"]`).
4. **`TASK-617 / TASK-618` (Search Popover UI)**: открытие поискового попапа `[data-search-popover]` при фокусе в `#chat-search`, проверка всех 7 подсказок токенов (`from:`, `mentions:`, `has:`, `before:`, `during:`, `after:`, `in:`), парсинг и подсветка токенов (`.search-token-highlight`), клик по фильтру `has:image`, дебаунс-вызов `search_messages`, навигация `ArrowDown` (`aria-selected="true"`) и переход к сообщению (`onJumpToMessage`).
5. **`TASK-620` (Context Menu Actions, ПКМ)**: открытие контекстного меню `[data-context-menu]` по `contextmenu` с ограничением координат границами viewport (`left + width <= innerWidth`, `top + height <= innerHeight`), проверка слайдера громкости участника `0..200%` (`role="menuitem"`, `<input type="range" min="0" max="200">`), клавиатурная навигация `ArrowDown`/`ArrowUp` и закрытие кликом вне меню.
6. **`TASK-626` (Nitro Gifts & Entitlements IPC)**: полный цикл IPC-команд `list_nitro_gift_plans`, `resolve_gift_code`, `create_gift_code` (`sku_id`, `subscription_plan_id`, `custom_message`), `redeem_gift_code` (`code`, `channel_id`) и проверка негативного сценария (просроченный/недействительный код подарка `code: 10038`).
7. **`TASK-628` (Advanced Settings UI — Keybinds, Soundboard & Clips)**: монтирование `[data-advanced-settings]`, переключение вкладок `role="tablist"` (`keybinds`, `soundboard`, `clips`), запись хоткея `Ctrl+Shift+M` в режиме `data-recording="true"`, детектор конфликтов `.keybind-conflict-warning` при дублировании комбинации, воспроизведение звука в Soundboard (`onPlaySound`) с регулировкой громкости и сохранение настроек клипов (`clipsEnabled`, `clipLengthSec: 60`, `clipFps: 60`, `hardwareEncoding: false`) в `storage`.

## 2. Red-прогон (TDD)

Команда:
```sh
node --test --test-concurrency=1 tests/e2e/e2e-build006.test.mjs
```
Лог сохранён в `waves/RW-40/G4/task-630-qa-e2e-regression/red.log` (`Could not find '.../tests/e2e/e2e-build006.test.mjs'`, код выхода `1`).

## 3. Green-прогон (Targeted E2E)

Команда:
```sh
node --test --test-concurrency=1 tests/e2e/e2e-build006.test.mjs
```
Результат (`waves/RW-40/G4/task-630-qa-e2e-regression/green.log`):
- `tests: 7`, `suites: 1`, `pass: 7`, `fail: 0`, `skipped: 0` (`3084.6 ms`).

## 4. Результат `npm run gate:worker`

Команда:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target npm run gate:worker
```
Результат (`waves/RW-40/G4/task-630-qa-e2e-regression/gate-worker.log`, `.gate/worker-2026-11-07T14-57-13-931Z.json`):
- `typecheck` — `0.6 с` (PASS)
- `ui-tsc` — `0.4 с` (PASS)
- `meta` — `0.2 с` (`118/118 pass`, `0 fail`)
- `unit` — `6.9 с` (`1901/1901 pass`, `0 fail`)
- `spike-rust` — `38.1 с` (PASS)
- `workspace-rust` — `26.0 с` (`1390 pass`, `0 fail`, `12 ignored`)
- `smoke` — `1.1 с` (`19 pass`, `0 fail`, `3 skip`)

## 5. Изменённые и созданные файлы + MD5

| Файл | MD5 | Статус |
|---|---|---|
| `tests/e2e/e2e-build006.test.mjs` | `89eca1b06cec0f96f381fd48881bc3f3` | Создан (7 E2E тестов) |
| `TDD-PLAN.md` | `b612e5179572cc282e2dcbdc3ae421ac` | Обновлён (§8 реестр тестов) |
| `waves/RW-40/G4/task-630-qa-e2e-regression/red.log` | `f17eb8cc27959a96e9a5a771909f7c75` | Создан |
| `waves/RW-40/G4/task-630-qa-e2e-regression/green.log` | `ba2922441824dabd898fd7391f973f0a` | Создан |
| `waves/RW-40/G4/task-630-qa-e2e-regression/gate-worker.log` | `3c2a92856e6f51e05fde52e1c1df89b0` | Создан |

## 6. Риски и наблюдения для G2 Frontend

- **Наблюдение по взаимодействию `actions.js` и `#captcha-modal` (`TASK-610`)**: глобальный обработчик кликов `document.body.addEventListener("click", ...)` в `rust/gui-spike/ui/features/actions.js` (строки 1163–1170) закрывает `#join-server-dialog` (`join.cancel(true)`), если клик пришёл вне `join.card`. Так как `#captcha-modal` монтируется в `document.body` (вне `join.card`), клик мышью внутри `#captcha-modal` всплывает до `document.body` и может вызвать `join.cancel(true)` (`current = null`) до завершения `onSolved`. При отправке формы `#captcha-modal-form` (событие `submit` / клавиша `Enter`) сценарий отрабатывает штатно; в будущем для `actions.js` рекомендуется добавить проверку `!target.closest("#captcha-modal")` перед вызовом `join.cancel(true)`.
