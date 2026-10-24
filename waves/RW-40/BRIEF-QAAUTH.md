# BRIEF QA-AUTH — приёмка входа и сети

Владелец: G4 QA lead. PM: DeepSeek. Запуск после A2/A3.
Вход: `AUTH-CONTRACT`, `LOGIN-SPEC`, `QA/PLAN-E9` §1/§8, `NET/PM/*`.

## Матрица
1. token: ok/401/битый; restore; logout идемпотентен.
2. password: ok/401/429/captcha/mfa; пароль не в DOM/логах; лимит 2000.
3. MFA: totp/sms/backup/password/webauthn; неверный код 60008; истёкший ticket 60006.
4. captcha: adapter, поля session/rqdata, headers, отмена.
5. gateway: connect/READY/close 4004/reconnect/stop.
6. секреты: grep по логам/DOM/ошибкам — 0 совпадений.
7. a11y: labels, aria-live, focus, disabled.
8. live: одно согласованное окно, тестовый аккаунт.

## Выход
`QA/AUTH-REPORT.md`: P0/P1/P2, repro, evidence, скриншот/AT-SPI.
Правило: без repro дефект не принимается; тесты не ослаблять.
