# A6-P2 (часть 1) — регресс-тест logout-reject

- **Файл:** `tests/unit/gui-spike-auth.test.mjs` (только новый `it`, существующие не менялись).
- **Тест:** «logout отклонён: поля и captcha/MFA-контекст сброшены, гейт открыт, без console.error».
  Сценарий: captcha → MFA → `ready`, затем `auth_logout` реджектится `new Error("offline")`.
  Проверки: login/password/token/code пусты, гейт в `no-session`/`error`, logout скрыт,
  приложение живо (4 экрана, чат), `console.error` пуст, повторный вход даёт
  `auth_login_password` args ровно `{login, password}` — без старых полей
  `captcha_key`/`captcha_rqtoken`/`captcha_session_id`/`captcha_rqdata` и MFA-ticket.
- **Прогон:** `node --test tests/unit/gui-spike-auth.test.mjs` → tests 43, pass 43, fail 0.
- **tsc:** `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0.
- **md5:** `320116736e036e9f78c980b8bb0f16bf`.
- Product-код, harness и прочие тесты не трогались; окно/cargo/gate/сеть/commit не запускались.
