# FULL-REPORT — состояние приложения (2026-10-24)

PM: DeepSeek. Срез на момент отчёта; активная задача — **B3a (backend реакций)**.

## 1. Кратко

Приложение — **Tauri GUI-спайк** Discord-клиента с рабочим offline-контуром
«вход → keyring → `/users/@me` → gateway-драйвер → отправка сообщения».
Live-подключение к Discord и продуктовая часть (вложения, реакции, треды, голос, ОС)
ещё не завершены. Релизом это не является.

| Слой | Готовность | Доказательство |
|---|---:|---|
| Ядро Rust | ~85 % | core **573/0**, workspace **1111/0** |
| Backend BFF/auth | ~85 % | gui-spike **111/0** (до B3a), auth 39/0, send 7/0 |
| Frontend | ~85 % каркаса | **175/0**, tsc 0, geometry 31/31 |
| Live-сеть Discord | ~10 % | READY/send не проверены на живом аккаунте |
| Продуктовые функции | ~30 % | P3-02 частично, P3-03 done offline |
| Голос/ОС | ~0–20 % | headless-часть есть, UI нет |
| QA/релиз | ~50 % | gate passed_with_skips, e2e 4 skip |

## 2. Что работает

### Вход и сессия
- импорт токена, пароль + MFA (totp/sms/backup/password), SMS-send;
- captcha-контекст: `sitekey/service/session_id/rqdata/rqtoken/should_serve_invisible/user_flow`,
  заголовки `X-Captcha-*`; только интерактив, без обхода;
- keyring, restore, logout (stop gateway → clear), идемпотентность;
- `auth://state`; close 4004 → очистка токена, 4013/4014 → stop/error;
- `SupervisorEvent` callback, `CoreGatewayDriver`, `net_connect/status/disconnect`.

### Чат
- виртуализация 100k, высоты 26/48/+22, группировка, day/unread, reply, embeds, реакции (вид);
- composer: Enter/Shift+Enter, лимит 2000, optimistic-строка, 401/429/сеть, ручной retry;
- отправка сообщения: `POST /channels/{id}/messages`, `send_message`, nonce.

### Backend/Core
- BFF: `ready/messages_page/members_page/settings/search_page/send_message`;
- core: REST/TLS/headers/super-properties/rate-limit/retry, PUT/POST/DELETE (B3a в работе),
  gateway/supervisor/identify/resume, state/history/store, voice/media headless.

## 3. Сейчас в работе

| Задача | Статус | Что делается |
|---|---|---|
| **B3a backend реакций** | **in work** (~5 мин) | `ApiClient::put/delete`, `ReactionApi`, Tauri `toggle_reaction`, fake-тесты |

После B3a: B3b UI реакций → B4 меню сообщения → B2 вложения → B5 участники → B6/B7 поиск/QS.

## 4. Не реализовано

### Сообщения
- B3b: клик реакции, счётчик, снятие, ошибки;
- B2: вложения — upload, прогресс, превью, drag&drop;
- B4: меню сообщения — правка/удаление/копия/ответ;
- C1: треды, пины;
- B7: серверный поиск (7 фильтров на живых данных).

### Люди/навигация
- B5: группы участников по ролям/статусу, полный поиск;
- B6: QuickSwitcher `Ctrl+K`;
- C2: profile-popout, меню гильдии, DM-список.

### Настройки/система
- B8: сохранение настроек в store;
- B9: уведомления D-Bus в UI;
- C3: темы light/cozy;
- C4: i18n-рантайм en/ru;
- C6: трей, бейджи, автозапуск, deep links, updater UI.

### Голос/медиа
- C5: устройства, PTT, mute/deafen, статистика RTT/потерь (движок headless готов).

### Сеть/безопасность
- live READY и live send (нужен тестовый токен + окно);
- QR-вход (протокол RE, UI disabled);
- WebAuthn (нативная подпись);
- P2: регресс-тесты clear-error/logout-reject (A6-P2).

### Тесты/качество
- e2e: 4 теста **skip** (live Discord/CDP);
- QA-матрица `PLAN-E9` не прогонялась вживую;
- coverage считает только `src/**`, не UI/Rust;
- COV-01…05: `srtp_key_derive`, settings-декодеры, voice-парсеры, `build_identify_frame`,
  `close_fds` — без прямых тестов.

## 5. Тесты и гейты

| Прогон | Результат |
|---|---|
| `cargo test --workspace` | **1111/0** |
| `cargo test -p discord-core` | **573/0** |
| gui-spike `cargo test` | **111/0** (до B3a) |
| frontend unit (dom/model/bench/features/auth) | **175/0** |
| auth contract / net / messages | **39/0, 6/0, 7/0** |
| `gate:release` | **passed_with_skips** (smoke 22/22) |
| e2e | 4 skip (opt-in) |
| tsc / geometry | 0 / 31/31 |

## 6. Команда и задачи

- **G1 Auth/Network** — B3a (backend реакций), затем B4a/B2a.
- **G2 Frontend** — B3b, B4b, B2b, B5b, B6.
- **G3 Design** — паритет login/тем; ждёт UI-сверку.
- **G4 QA** — A5 offline PASS; live/D-матрица впереди.
- **G4b Security** — PASS; P2 A6-P2.
- **G5 Platform** — B9/C5/C6 после P0/P1.
- Доска: `waves/RW-40/TASKS.md`; разбивка: `TASKS-DETAIL.md`; каналы: `INBOX/g1…g5`.

## 7. Риски

| Риск | Митигация |
|---|---|
| Live Discord/ToS/блокировка | тестовый аккаунт, ADR-3, одно окно |
| Утечка токена/пароля | keyring, redaction, security PASS, тесты |
| e2e/QA не покрывают UI | D1/D2, live-матрица |
| Расхождение контрактов | AUTH-CONTRACT/DOM-CONTRACT, PM-приёмка |
| Нет воркеров (баланс) | Codex на критическом пути, очередь задач |

## 8. Следующие шаги

1. B3a green → B3b UI реакций.
2. B4 меню сообщения → B2 вложения.
3. B5 участники → B6 QuickSwitcher → B7 серверный поиск.
4. B8/B9/C3/C4 — настройки/система/темы/i18n.
5. D1–D3 — e2e/QA/coverage; D4 live READY/send; D5 merge gate.

Критерий «полноценное приложение» (RW-40 §4) пока **не достигнут**: нет live-сети и
большинства продуктовых функций; offline-каркас и auth/send — рабочие.

---

## Обновление 2026-10-24 (B3/B4)

- **B3a принят**: backend реакций (PUT/DELETE), gui 117/0.
- **B4a принят**: edit/delete/reply backend, gui **121/0**, core **573/0**.
- **B3b (UI реакций) — в работе**, поэтому frontend-набор временно в red-фазе (35/6),
  пока не будет green; это не регресс, а TDD-цикл.
- **G3 Design-MSG принят** (D-54): реакции/вложения/меню 15/15/треды; вход для B3b/B4b/B2/C1.
- Готовность: offline-клиент ~80 %, релиз ~35 %; live READY/send по-прежнему не выполнены.

---

## Обновление 2026-10-24 (B7, UI-FIX)

- B7a backend guild-search принят (core 574, gui 132).
- B7b UI сетевого поиска принят (frontend 205/0, gate passed).
- UI-FIX-1: P1/P2 Codex-review закрыты (реакции IPC, delete tombstone, auth busy, MFA, draft/edit).
- workspace **1112/0**; gate **passed_with_skips** (unit 374, smoke 22/22).
- Codex — только ревью; кодовые задачи у субагентов.
- Остаток: live, вложения (blocked), B5b/B8/B9/C3–C6, D1–D5.
