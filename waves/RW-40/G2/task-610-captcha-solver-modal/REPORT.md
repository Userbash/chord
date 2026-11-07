# REPORT — TASK-610 (G2 Frontend + G1 Backend): hCaptcha Solver Modal (`#captcha-modal` / `ui/features/captcha-modal.js` + `invites.rs`)

- **Ветка:** `feature/task-610-captcha-solver-modal`
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/task-610-captcha-solver-modal`
- **Отделы:** `G2 Frontend (WebView UI & DOM/A11y)` + `G1 Backend (Rust Core & Tauri BFF)`
- **Затронутые карточки:** `TASK-610` (`CARD-TEL-006` / hCaptcha Solver Modal при `POST /invites/{code}`)
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (правила пайплайна `AGENTS.md` §6).

---

## 1. Сделано

1. **Изучены RE-оракулы и спецификации:**
   - `AGENTS.md`, `PROCESS.md` §4.2, `docs/re/rest-d02.md`, `/home/sanya/re-deep/rest-d02.md`.
   - `waves/RW-39/NET/LOGIN-UI-RE.md` §8 (`CAP-01`..`CAP-15`: формат ответа HTTP 400 с `code: 40003`, `captcha_service: "hcaptcha"`, `captcha_sitekey`, `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id` и повтор с `captcha_key` / `X-Captcha-Key`).
   - `waves/RW-40/NETWORK-CONTRACTS.md` §1.2 (`POST /invites/{code}`).
2. **G2 Frontend (`rust/gui-spike/ui/features/captcha-modal.js`, `join-server.js`, `app.js`):**
   - Создан модуль `rust/gui-spike/ui/features/captcha-modal.js` (`// @ts-check` + полные JSDoc-аннотации):
     - Классификатор и парсер `isCaptchaChallengeError` / `extractCaptchaChallenge` для HTTP 400 (`code: 40003`, `captcha_service: "hcaptcha"`, `captcha_sitekey`, опциональные `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`).
     - Генератор URL встроенного веб-виджета `buildHcaptchaWidgetUrl` (`https://newassets.hcaptcha.com/captcha/v1/embed.html?sitekey=...&host=discord.com&hl=ru`).
     - Контроллер `createCaptchaModal`: динамически монтирует диалог `#captcha-modal` (`role="dialog"`, `aria-modal="true"`, `aria-labelledby="captcha-modal-title"`), отображает контейнер виджета hCaptcha (`#captcha-widget-container`, `#captcha-widget-frame`) и форму ввода токена решения (`#captcha-token-input`, `#captcha-submit-btn`, `#captcha-cancel-btn`), поддерживает как ручной ввод, так и callback виджета (`solveWithToken` / `window.hcaptcha.render`), валидирует пустой токен, защищает от двойной отправки (`inFlight` / `aria-busy="true"`) и повторно вызывает `accept_invite` с параметром `captcha_key` (плюс `captcha_rqtoken` / `captcha_session_id` при наличии).
   - Интегрирован в `rust/gui-spike/ui/features/join-server.js`: при ошибке `accept_invite` с валидным hCaptcha-челленджем автоматически открывает `#captcha-modal`, а после успешного повтора с `captcha_key` закрывает оба модальных окна и завершает вступление на сервер (`onAccepted(guild)`).
   - В `rust/gui-spike/ui/app.js` добавлен маппинг `captcha_key` → `captchaKey` для живого Tauri IPC и регистрация `#captcha-modal` в карте UI-телеметрии.
3. **G1 Backend (`rust/gui-spike/src/invites.rs`, `rust/gui-spike/src/main.rs`):**
   - В `InviteError` добавлен вариант `CaptchaRequired { code, captcha_service, captcha_sitekey, captcha_rqdata, captcha_rqtoken, captcha_session_id }`.
   - В `decode_accept` добавлен разбор HTTP 400 с hCaptcha-челленджем (`parse_captcha_challenge`), сохраняющий обратную совместимость с пустыми/не-капча 400 ответами (`InviteError::Client { status: 400 }`).
   - Добавлены структура `CaptchaSolution`, метод трейта `InvitesApi::accept_with_captcha`, функция `accept_invite_with_captcha` и проброс заголовков `X-Captcha-Key` / `X-Captcha-Rqtoken` / `X-Captcha-Session-Id` и поля `captcha_key` в `ApiInviteTransport::post`.
   - В Tauri-команду `accept_invite` (`rust/gui-spike/src/main.rs`) добавлены опциональные параметры `captcha_key`, `captcha_rqtoken`, `captcha_session_id` с делегированием в `invites::accept_invite_with_captcha`.

---

## 2. Строгий TDD (Red → Green)

### Red-прогон (до реализации):

1. **Unit-тест фронтенда:**
   - Команда: `node --test tests/unit/captcha-modal.test.mjs`
   - Вывод:
     ```text
     Error [ERR_MODULE_NOT_FOUND]: Cannot find module '.../rust/gui-spike/ui/features/captcha-modal.js' imported from .../tests/unit/captcha-modal.test.mjs
     ✖ tests/unit/captcha-modal.test.mjs (21.30719ms)
     ℹ tests 1 | pass 0 | fail 1 (exit 1)
     ```
2. **Rust-контракт:**
   - Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test invites_contract`
   - Вывод:
     ```text
     error[E0432]: unresolved imports `discord_gui_spike::invites::accept_invite_with_captcha`, `discord_gui_spike::invites::CaptchaSolution`
     error[E0599]: no variant named `CaptchaRequired` found for enum `InviteError`
     error: could not compile `discord-gui-spike` (test "invites_contract") due to 2 previous errors
     ```

### Green-прогон (после реализации):

1. **Targeted unit-тесты (`captcha-modal` + `join-server`):**
   - Команда: `node --test tests/unit/captcha-modal.test.mjs tests/unit/join-server.test.mjs`
   - Результат: `38/38 pass`, `0 fail` (`1085.99ms`, exit 0).
2. **Targeted Rust-контракты (`invites_contract`, `ipc_contract`, `ipc_wiring_contract`, `server_join_and_subscriptions_contract`):**
   - Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test invites_contract --test ipc_wiring_contract --test ipc_contract --test server_join_and_subscriptions_contract`
   - Результат: `28 passed` (`invites_contract`), `20 passed` (`ipc_contract`), `9 passed` (`ipc_wiring_contract`), `6 passed` (`server_join_and_subscriptions_contract`), `0 failed` (exit 0).
3. **Проверка типов и мета-тестов:**
   - `npm run typecheck` → `0 errors` (exit 0).
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `0 errors` (exit 0).
   - `node --test tests/meta/plan-sync.test.mjs tests/meta/ipc-path.test.mjs` → `17/17 pass` (exit 0).
4. **Полный прогон `npm run test:unit`:**
   - Команда: `npm run test:unit`
   - Результат: `1842/1842 pass`, `0 fail`, `0 skipped` (`6051.53ms`, exit 0).

---

## 3. Мутационная проверка (5/5 убито)

Проверены 5 мутаций на `rust/gui-spike/ui/features/captcha-modal.js` против `tests/unit/captcha-modal.test.mjs`:
- `M1_status_check` (`status !== 400` → `status === 400`): **KILLED** (`exit=1`)
- `M2_service_check` (`rawService !== HCAPTCHA_SERVICE` → `rawService === HCAPTCHA_SERVICE`): **KILLED** (`exit=1`)
- `M3_empty_token_guard` (`if (captchaKey === "")` → `if (false)`): **KILLED** (`exit=1`)
- `M4_omit_captcha_key_arg` (`captcha_key: captchaKey` → `captcha_key: ""`): **KILLED** (`exit=1`)
- `M5_double_submit_guard` (снятие проверки `inFlight`): **KILLED** (`exit=1`)
- После проверки файл восстановлен бит-в-бит (`md5 = f8fe341400bc2e081d1885c074981e15`).

---

## 4. Затронутые файлы и `md5sum`

```text
f8fe341400bc2e081d1885c074981e15  rust/gui-spike/ui/features/captcha-modal.js
6d6debb547bec98f0ceebe67500a050d  rust/gui-spike/ui/features/join-server.js
a6f60bc0dfd7b9f08c4bfef4a551b257  rust/gui-spike/ui/app.js
450fabca2193848d823b71805542f4e9  rust/gui-spike/src/invites.rs
765c9460f08b7e0baa2b488cf6a39564  rust/gui-spike/src/main.rs
0e32803321836e2d8074789113a6483a  rust/gui-spike/tests/invites_contract.rs
cd12b988d85ce7a421c6de29584f4111  tests/unit/captcha-modal.test.mjs
b31ca170c580b5cdd9ee2442aa767e10  TDD-PLAN.md
```

---

## 5. Остаточные риски и замечания

- В офлайн/тестовой среде внешний скрипт `https://js.hcaptcha.com/1/api.js` не загружается по сети (согласно запрету сетевых вызовов в тестах); виджет рендерит детерминированный контейнер с `data-sitekey` и `data-widget-url`, поддерживает `window.hcaptcha.render` при наличии и позволяет передать/подтвердить полученный `captcha_key` через форму или `solveWithToken`.
- Существующие тесты `CARD-TEL-006` в `tests/unit/join-server.test.mjs` (ошибки `40003` без `captcha_sitekey`) полностью сохранены и зелёные.
