# AUTH-CONTRACT — единый контракт входа и сессии (frontend ↔ backend ↔ core)

Статус: **заморожен** PM 2026-10-23. Изменения — только через `DECISIONS.md`.
Потребители: NET-DESIGN, NET-D2, NET-F1, NET-E2, QA-AUTH.

## 1. Состояния UI

| Состояние | Что видно | Переходы |
|---|---|---|
| `checking` | сплэш/скелетон | → `no-session` / `ready` / `error` |
| `no-session` | окно входа (token + login/password) | → `validating` |
| `validating` | спиннер, поля disabled | → `captcha` / `mfa` / `ready` / `error` |
| `captcha` | штатный hCaptcha-виджет | → `validating` (после решения) |
| `mfa` | экран кода + методы | → `validating` / `error` |
| `ready` | приложение, user-bar | → `no-session` (logout) |
| `error` | сообщение + повтор | → `no-session` |

`auth://state` payload: `{ state, user?, errorCode?, retryAfterMs? }` — без токена.

## 2. Команды Tauri

| Команда | Вход | Выход | Примечание |
|---|---|---|---|
| `auth_status` | — | `{state, stored, user?}` | keyring, без сети |
| `auth_login_token` | `{token}` | `{state, user}` | токен в keyring |
| `auth_login_password` | `{login, password, captchaKey?, captchaRqtoken?}` | `LoginOutcome` | пароль не сохраняется |
| `auth_mfa` | `{method, code, ticket}` | `{state, user}` | code не сохраняется |
| `auth_sms` | `{ticket}` | `{phoneMasked}` | — |
| `auth_restore` | — | `{state, user?}` | keyring → `/users/@me` |
| `auth_logout` | — | `{state:"no-session"}` | stop gateway + keyring.clear |

`LoginOutcome` = `{kind:"token"}` | `{kind:"mfa", ticket, methods[]}` |
`{kind:"captcha", sitekey, service, rqtoken}` | `{kind:"error", code, message}`.

## 3. Ошибки (коды)

`InvalidCredentials` (401), `CaptchaRequired`, `MfaRequired`, `RateLimited` (+`retryAfterMs`),
`Network`, `Keyring`, `Parse`, `Internal`.
Текст **никогда** не содержит токен/пароль/код; `Debug` — `<redacted>`.

## 4. UI-контракт (id/роли)

| Элемент | id / роль | Обязанность |
|---|---|---|
| Оверлей | `#auth-gate` `role=dialog` `aria-modal=true` | скрыт при `ready` |
| Форма | `#auth-form` `role=form` | submit |
| Логин | `#auth-login` `autocomplete=username` | email/phone |
| Пароль | `#auth-password` `type=password` | show/hide, очистка после submit |
| Токен | `#auth-token` `type=password` | альтернативный вход |
| Captcha | `#auth-captcha` | контейнер штатного виджета |
| MFA | `#auth-mfa` + `#auth-mfa-code` | код, методы |
| Ошибка | `#auth-error` `role=alert` `aria-live=assertive` | текст без секретов |
| Выход | `[data-action=auth-logout]` | logout |

## 5. Инварианты безопасности

1. Пароль/код: только в памяти на один запрос, затем overwrite; не в keyring/логах/DOM.
2. Токен: только keyring, не в `localStorage`, не в URL, не в ошибках.
3. Captcha: только интерактивная; без solver/обхода.
4. Сеть: TLS, официальные хосты; 429 → backoff, без циклов.
5. Live: только тестовый аккаунт и согласованное окно.

## 6. Приёмка

- `auth_status` без сети; `login_token` 401/ok; `login_password` → mfa/captcha/token;
- `auth_mfa` ok/неверный код; `auth_sms` маскированный номер; `logout` идемпотентен;
- после ready — `/users/@me`, gateway READY; после restart — restore;
- тесты: red-first, 401/429/сеть/битый JSON, утечки (grep по логам/DOM).
