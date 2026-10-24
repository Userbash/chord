# NET-R2 — REPORT (RE login UI)

**Роль:** NET-R2 (RE login UI), волна RW-39. PM: DeepSeek. Режим: read-only.
**Дата:** 2026-10-23. **Цель:** Discord desktop 1.0.162 (`client_build_number` 641004).

## Сделано

- Прочитан login-чанк `ae9016b5ac149349.js`, MFA-чанк `9efedacca22a4bc6.js`, QR-чанк
  `0c2bfaf5557c575e.js`, основной бандл `web.4d4b2f75fdea5470.js`, `/login` HTML и 7 CSS-чанков.
- Восстановлены: компоновка (784/480, две колонки, media 830/485), поля/тексты, точные стили
  и токены (в т.ч. живые computed-значения из `ui/tokens.json`), состояния ошибок, регистрация
  (2 шага), QR-вход (UI + протокол), captcha (триггер/поля/заголовки/sitekey/ошибки), 2FA
  (5 методов, слайды, SMS-send, таймеры, переходы).
- Собраны 109 фактов: `D` — 90, `D+M` — 11, `D+A` — 6, `M` — 1, `A` — 1.

## Выходы

| Файл | md5 |
|---|---|
| `waves/RW-39/NET/LOGIN-UI-RE.md` | `f01f291688430744a016e05e4c1c0404` |
| `waves/RW-39/NET/login-ui-re.json` | `86b5e0e39692081d4cf532e4d2c46618` |

Логи: `00-md5-inputs.log`, `01-anchors.log`, `02-login-keys.log`, `03-mfa-extract.log`,
`04-captcha-grep.log`, `05-qr-extract.log`, `06-css-extract.log`, `07-tokens.log`,
`08-login-ui-extract.log`, `09-window.log`.

## Ключевые выводы

- **Captcha:** триггер — HTTP 400 + `body.captcha_key`; сервисы `hcaptcha/recaptcha/
  recaptcha_enterprise/turnstile`; свои поля `captcha_sitekey/session_id/rqdata/rqtoken/
  should_serve_invisible/user_flow`; повтор с `X-Captcha-Key` (+`-Rqtoken`, `-Session-Id`).
  В бандле есть только рендерер **reCAPTCHA Enterprise** (sitekey прод `6LeYqFcq…`,
  dev `6LdtfVMq…`); hCaptcha — только значение сервиса (рендерер в недоступном чанке).
  Модалка (`536037`) в дампе отсутствует. Только интерактив, обходов в бандле нет.
- **2FA:** методы в порядке `webauthn → totp → backup → sms` (password-слайд есть, но в логине
  недостижим); TOTP/SMS — 6 цифр, backup 8–11 без дефисов, SMS-send `POST /auth/mfa/sms/send
  {ticket} → {phone}`; `POST /auth/mfa/{type}` с `{code,ticket,login_source,gift_code_sku_id,
  login_instance_id}`, `retries:2`; коды 60008/60003; предупреждение backup — 10 с.
- **QR:** это не вкладка, а вторая колонка (240px) в expanded-коробке; протокол remote-auth
  восстановлен (ws `?v=2`, RSA-OAEP-2048, `pending_login` → `/users/@me/remote-auth/login` →
  расшифровка токена) — частично закрывает AUTH-RE N-02, но реализовывать не будем.
- **Чего нет:** DOM/скриншот окна входа; чанк модалки captcha; рендерер hCaptcha/turnstile;
  точный RU-текст `N9U/9X`; живой ответ `/auth/login` (AUTH-RE N-01).

## Запреты

Сеть, окно, запуск Discord, `cargo`/`npm`, копирование ассетов и коммиты не выполнялись.
Рекомендаций по обходу captcha нет и быть не может.
