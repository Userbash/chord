# BRIEF NET-D2 — окно входа по LOGIN-SPEC (frontend)

Владелец: NET-D2 (Frontend). PM: DeepSeek. Red-first. Запуск: сейчас.
Вход: `NET/LOGIN-SPEC.md`, `NET/AUTH-CONTRACT.md`, `NET/PM/netf1/REPORT.md`,
текущий `ui/features/auth.js` (token-import уже работает) и `tests/unit/gui-spike-auth.test.mjs`.

## Файлы (только эти)
- `ui/features/auth.js` — расширить: состояния/форма/captcha/MFA/QR-панель
- `ui/components/auth.css` — layout 784/416/240, состояния, адаптив ≤830
- `ui/index.html` — только id-хосты, если нужны (`#auth-qr`, `#auth-mfa-methods`)
- `ui/app.js` — wiring
- `tests/unit/gui-spike-auth.test.mjs`, `tests/unit/gui-spike-dom.test.mjs` (только новые id)
- `tests/support/gui-dom-harness.mjs` — аддитивно, если нужен stub hCaptcha
- логи `NET/PM/netd2/`
READ-ONLY: `src/**`, `DESIGN/**`, остальной `ui/**`.

## Требования
1. Карточка 784: форма 416 + gap 64 + QR 240 + паддинги 32; ≤830 — QR скрыт.
2. Поля login/password (+ token alternate), show/hide, autocomplete, submit, ошибки.
3. `auth_login_password` → `LoginOutcome`: token → ready; mfa → экран кода; captcha → зона
   captcha. **Captcha только интерактивная**: официальный hCaptcha-виджет в live; в тестах —
   fake-adapter, возвращающий токен по клику пользователя. Никаких solver/обхода.
4. MFA: методы totp/sms/backup/password, `auth_mfa`, `auth_sms`, таймер/повтор, ошибки.
5. Ошибки 401/429/captcha/сеть — `#auth-error role=alert`, без секретов.
6. Пароль/код очищаются из DOM после submit; в логах нет; токен только в keyring (backend).
7. QR-панель: локальная inline-SVG заглушка (ассет не копировать), подпись «QR-вход (Phase 3)»
   либо disabled-affordance — без мёртвой кнопки.
8. Существующие 18 auth + 130 DOM/features не ослаблять; новые тесты на каждое состояние.

## Проверки (без окна/cargo/gate)
```bash
node --test tests/unit/gui-spike-auth.test.mjs
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
node waves/RW-39/PM/w39c/geometry-check.mjs
```
Окно/probe/cargo/gate/сеть/commit — запрещены.

## Отчёт
`NET/PM/netd2/REPORT.md`: состояние → тест → результат, md5, остаток (live hCaptcha, WebAuthn).
