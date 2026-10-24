# ADR-2 — метод входа и сетевой сессии

Статус: **принято** (PM, 2026-10-23). Основание: `AUTH-RE.md` (87 фактов) + `CORE-AUDIT.md`.

## Контекст

- Живых `/auth/*` в дампе нет; флоу восстановлен из бандла: пароль → MFA → токен.
- `Authorization` — сырой токен (без `Bearer`); `X-Super-Properties` обязателен.
- QR/remote-auth объявлен, но протокола в дампе нет.
- Пароль/captcha/MFA-автоматизация = риск ToS и утечек; Not acceptable.

## Решение

**A. Импорт собственного токена владельца** (ручная вставка), хранение только в keyring.

- Никакой автоматизации пароля, captcha, MFA и QR.
- Токен вводится вручную в локальном UI, сразу в keyring; в логи/DOM/ошибки не попадает.
- Тестовый аккаунт; риск блокировки фиксируется письменно.
- OAuth2/bot-токен — отдельным сценарием позже (для разрешённых интеграций).

## Реализация

1. `ApiAuth::from_token(raw_token, SuperProperties, locale, tz)` — 6 заголовков, Debug redacted.
2. `GET /users/@me` → `Me` (JSON-парсер) → проверка токена; 401 → «токен недействителен».
3. keyring: `store/load/clear`; keyring `!Send` — выделенный поток-владелец.
4. Gateway: URL `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`;
   IDENTIFY по RE (capabilities 1734653, compress:false, properties+4 поля, presence,
   client_state, qos_token); heartbeat — **op40 `{seq,qos}` с fallback на op1**;
   RESUME `{token,session_id,seq}`; close 4004 → logout; 4014 → прекратить reconnect.
5. События UI: `auth://state` (idle/connecting/ready/error), `gateway://status`.
6. Logout: stop supervisor → keyring.clear → wipe session state → экран входа.

## Последствия и риск

- **ToS**: сторонний клиент с пользовательским токеном может привести к блокировке; принимается
  владельцем, только тестовый аккаунт.
- Токен = полный доступ; утечка критична. Обязательны: redaction, keyring, запрет логов,
  отсутствие токена в ошибках/URL.
- Heartbeat/IDENTIFY могут дрейфовать от версии клиента; фиксируем версию и тесты-снапшоты.
- Без сети/окна: все проверки на стабах; live READY — одно согласованное окно.

## Acceptance

- вставка токена → `/users/@me` 200 → READY; неверный токен → 401 без утечек;
- перезапуск → сессия из keyring; logout → токен удалён, gateway остановлен;
- 429/сеть → понятное состояние + backoff; 4004 → logout;
- ни одного токена в логах/ошибках (тест на redaction).
