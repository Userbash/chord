# NET-FUNCTIONS — карта сетевых функций и передача сетевикам

Дата: 2026-10-23. PM: DeepSeek. Источники: `NET/CORE-AUDIT.md`, `NET/AUTH-RE.md`, код ядра.

## 1. Уже реализовано в ядре (не дублировать)

| Функция / тип | Файл | Назначение | Тесты | Статус |
|---|---|---|---|---|
| `ApiClient::{new,execute,get}` | `rest/client.rs` | HTTP/1.1 + TLS + auth + retry + 429 | `rest_client.rs` | готово; **нет `post`** |
| `ApiAuth::{from_token,from_values,pairs}` | `rest/client.rs` | 6 заголовков, raw token | `rest_client.rs` | готово |
| `headers::auth_headers`, `classify_status` | `rest/headers.rs` | заголовки/статусы | `rest_headers.rs` | готово |
| `identity::{build,IdentitySources}` | `rest/identity.rs` | SuperProperties (22 поля) | `rest_identity.rs` | готово |
| `SuperProperties::{to_base64,from_base64}` | `rest/super_properties.rs` | base64 JSON | 8 тестов | готово |
| `rate_limit::*`, `retry::*` | `rest/` | 429/5xx, backoff | 10+11 | готово, дубль политик |
| `telemetry::guard_target` | `rest/telemetry.rs` | блок телеметрии | 4 | готово |
| `routes::{find,render_path}` (47) | `rest/routes.rs` | каталог маршрутов | 8 | готово; **нет `/auth/*`** |
| `GatewayUrl` | `ws.rs` | `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` | `ws_transport` | готово |
| `GatewayUrl::resume` | `ws.rs` | нормализация `resume_gateway_url` | `ws_transport` | готово |
| `encode_qos_heartbeat` / op40 | `ws.rs` | heartbeat `{seq,qos}` + fallback op1 | `ws_transport` | готово |
| `gateway::supervisor` | `gateway/supervisor.rs` | reconnect/backoff/close 4004/4013/4014 | 10 | готово |
| `identify::{build_identify,IdentifyProperties}` | `identify.rs` | op2 по RE | `identify` | готово |
| `resume::{build_resume_frame,plan_reconnect}` | `resume.rs` | RESUME/backoff | `resume_reconnect` | готово |
| `Me` + `parse_me` | `rest/user.rs` | `/users/@me` | `rest_user` | готово |
| keyring + `AuthBackend` | `discord-platform`, `gui-spike/src/auth.rs` | token store, status/login/logout | 21 | готово |

## 2. Что нужно добавить для входа «как в приложении»

| ID | Функция | Endpoint / механизм | Зависит от | Владелец |
|---|---|---|---|---|
| NF-01 | `ApiClient::post` (JSON body) | общий HTTP | — | NET-F1 |
| NF-02 | `login_password(login,password,captcha?)` | `POST /auth/login` | AUTH-RE A-02 | NET-F1 |
| NF-03 | `submit_mfa(method,code,ticket)` | `POST /auth/mfa/{totp\|sms\|backup\|password}` | AUTH-RE A-05 | NET-F1 |
| NF-04 | `request_sms(ticket)` | `POST /auth/mfa/sms/send` | AUTH-RE A-07 | NET-F1 |
| NF-05 | captcha-контекст (`captcha_sitekey`, `captcha_service`, `captcha_rqtoken`) | ответ `/auth/login` | нужен RE login UI | NET-R2 |
| NF-06 | `LoginOutcome` (Token / Mfa / Captcha / Error) | state machine | NF-02…05 | NET-F1 |
| NF-07 | login-окно (email/phone, password, QR, MFA, captcha) | UI | дамп | NET-R2/NET-D2 |
| NF-08 | `auth://state` event | Tauri emit | NET-C2 | NET-F1 |
| NF-09 | logout → stop supervisor | связка | NET-E1 | NET-F1 |
| NF-10 | 429 `retry_after_ms` в UI | — | AUTH-RE | NET-D2 |

## 3. Границы (обязательны)

- Пароль/2FA-код **не сохраняются** и не логируются; только в памяти на один запрос.
- Captcha **решает пользователь** в штатном виджете; никаких solver-сервисов, обхода,
  подмены fingerprint/UA, прокси-ротации, массовых аккаунтов.
- Только тестовый аккаунт владельца; риск ToS/блокировки зафиксирован в ADR-3.
- TLS только к официальным хостам; 429/401 — backoff и понятная ошибка, без «долбёжки».
- Live-проверка — только по согласованному окну.

## 4. Передача

| Воркер | Задача | Выход |
|---|---|---|
| **NET-R2** | RE login-окна и captcha-контекста по дампу | `NET/LOGIN-UI-RE.md` |
| **NET-F1** | backend login/MFA/captcha-контекст + post + state machine | `gui-spike/src/auth*.rs`, тесты |
| **NET-D2** | login-окно как в оригинале (после RE) | `ui/features/auth.js`, `auth.css` |
| **NET-E2** | live READY (после D2/F1) | окно, запись |
