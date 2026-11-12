# PM Backlog — Live User Telemetry Session #2 (`2026-11-06T08:55:36Z` – `08:57:53Z`)

- **Источник данных**: `/home/user/.local/share/discord-client/user-telemetry.jsonl` и `/tmp/discord-full-user-activity.log`
- **Процесс**: `/home/user/orca/project/rust/target/release/discord-gui-spike` (`PID 134127` под GDB `PID 134090`)
- **Ответственный**: Продуктовый менеджер (PM) & Технический координатор ECC

---

## 1. Хронология действий пользователя и зафиксированных функций

| Время (UTC) | Действие пользователя (`ui_interaction` / `mouse_move`) | Модуль и функция (`module` -> `handler`) | Вызванные IPC-команды (`ipc_invoke`) и REST | Результат / Ошибка |
|---|---|---|---|---|
| `08:55:36.240Z` – `08:55:36.257Z` | Переключение разделов бокового рельса: `💬` Чат, `😀` Эмодзи, `👥` Участники, `⚙️` Настройки | `ui/features/guild-rail.js` -> `selectGuild / switchScreen` | — | UI переключился, рельс отображает: `DL0 Нет серверов+ 💬 😀 👥 ⚙️` |
| `08:55:36.258Z` | Клик по тумблеру `Предлагать устройства` | `ui/features/settings.js` -> `createSettingsConsole` | `settings_load` (60ms, ok), `voice_audio_devices` (57ms, ok), `settings_save` (16ms, ok) | Успешно сохранены параметры устройств |
| `08:55:50.000Z` – `08:56:00.000Z` | Взаимодействие с формой входа `auth-card` | `ui/features/auth.js` -> `createAuthGate` | `net_connect` (9ms, ok), `ready` (1012ms, ok) | Сессия авторизована |
| `08:56:24.000Z` – `08:56:27.000Z` | Ввод инвайта в `#join-invite-input` | `ui/features/join-server.js` -> `createJoinServer` | `resolve_invite` (2422ms, ok) | Превью инвайта: **«Game Room»** (3 участников, 0 в сети) |
| `08:56:28.216Z` | Клик по кнопке `Открыть сервер` (`data-action="enter-guild"`) | `ui/features/join-server.js` -> `enterStep` | `join_guild(guildId=1014537851418980372)` (1184ms) | **ОШИБКА 400**: Сообщение «Не удалось открыть сервер. Повторите попытку.» |
| `08:56:38.033Z` – `08:56:38.486Z` | Многократные повторные клики `Открыть сервер` | `ui/features/join-server.js` -> `enterStep` | — | **DEAD CLICKS**: клики игнорируются, переход на сервер не происходит |
| `08:56:38.960Z` | Клик `Отмена` | `ui/features/join-server.js` -> `hide` | — | Диалог закрыт |
| `08:57:18.201Z` | Клик `+` (Добавить сервер) на рельсе | `ui/features/guild-rail.js` -> `guild-add` | — | Открыт `#join-server-dialog` |
| `08:57:19.380Z` | Клик `Присоединиться` (`#join-submit`) | `ui/features/join-server.js` -> `submitNow` | `accept_invite` (8044ms) -> `POST /invites/wVc2y3ET` | **ОШИБКА 400 (40003)**: `Discord Captcha challenge: Обновите приложение, чтобы присоединиться к серверу.` |
| `08:57:24.069Z` | Повторный клик `Открыть сервер` (`data-action="enter-guild"`) | `ui/features/join-server.js` -> `enterStep` | — | **DEAD CLICK**: вызов `enterStep` заблокирован из-за `current === null` после ошибки `accept` |
| `08:57:24.640Z` | Фоновый Gateway Dispatch | `rust/gui-spike/src/data.rs` -> `ingest_live_dispatch` | `GUILD_CREATE(id=1014537851418980372)` | Сервер добавлен на бэкенд, но в UI рельс не перерисован |
| `08:57:25.000Z` – `08:57:53.000Z` | Клики по пустой подложке диалога `.dialog-backdrop` | `ui/features/join-server.js` | — | Интерфейс заблокирован модальным окном, пользователь пытается выйти |

---

## 2. Карточки дефектов и задач для передачи в отделы разработки

### CARD-TEL-005 (`P0` / Отдел `G2 Frontend`): Устранение dead-кликов «Открыть сервер» и разблокировка модального окна инвайтов
- **Файлы**: `rust/gui-spike/ui/features/join-server.js`, `rust/gui-spike/ui/app.js`.
- **Проблема**:
  1. При любой ошибке в `accept_invite` блок `catch` выполняет:
     ```javascript
     current = null;
     hidePreview();
     showFailure(failure, "accept");
     ```
     После этого кнопка `Открыть сервер` (`enter-guild`) остаётся видимой, но при нажатии функция `enterStep()` делает:
     ```javascript
     const data = current;
     if (data === null || data.guildId === "") return;
     ```
     и молча завершается без отклика для пользователя.
  2. Модальное окно не закрывается при клике на внешнюю подложку `.dialog-backdrop`, блокируя экран при возникновении ошибки REST.
- **Требуемое решение**:
  - Сохранять валидный `current` превью для возможности повторной попытки или переключения в режим lurker.
  - При ошибке выводить понятный статус пользователю и не блокировать повторные действия.
  - Реализовать закрытие модалки по клику на `.dialog-backdrop` и по нажатию клавиши `Escape`.

---

### CARD-TEL-006 (`P0` / Отдел `G1 Backend` & `G7 RE`): Обработка Captcha Challenge (код `40003`) и актуализация заголовков клиента при `accept_invite`
- **Файлы**: `rust/gui-spike/src/data.rs` (`accept_invite_via_live_rest`), `rust/crates/discord-core/src/rest/routes.rs`.
- **Фактический результат из телеметрии**:
  ```
  POST /invites/wVc2y3ET -> HTTP 400
  {"code": 40003, "message": "Discord Captcha challenge: Обновите приложение, чтобы присоединиться к серверу."}
  ```
- **Эталон RE (`reference-oracle/rest-d02.md`, `X-Super-Properties`)**:
  В эталонном Discord запрос на принятие инвайта передаёт:
  1. Актуальный заголовок `X-Super-Properties` с версией клиента Electron и сборки (`client_build_number`).
  2. При получении ответа `400` с капчей ответ содержит `captcha_key`, `captcha_sitekey`, `captcha_service` ("hcaptcha" / "turnstile") или требование обновления клиента.
- **Требуемое решение**:
  - Обновить `X-Super-Properties` и `User-Agent` в `discord-core` в соответствии с эталонным дампом `reference-oracle/`.
  - В случае возврата ошибки капчи `40003` возвращать структурированный ответ в UI с типом ошибки `captcha_required` и внятным текстом для пользователя вместо неопределённого `[object Object]`.

---

### CARD-TEL-007 (`P1` / Отдел `G2 Frontend` & `G1 Backend`): Реактивное обновление списка серверов и друзей на боковом рельсе (`guild-rail`)
- **Файлы**: `rust/gui-spike/ui/features/guild-rail.js`, `rust/gui-spike/ui/app.js`, `rust/gui-spike/src/data.rs`.
- **Проблема**:
  - Запросы `GET /users/@me/guilds`, `GET /users/@me/relationships`, `GET /users/@me/channels` завершились со статусом `200 OK`, и шлюз прислал `GUILD_CREATE(1014537851418980372)`.
  - Однако компонент `guild-rail.js` продолжал отображать `DL0 Нет серверов`.
- **Требуемое решение**:
  - Подписать компонент `guild-rail` на события обновления сессии (`live-guilds-updated`, `dm_friends_sync`).
  - При получении новых серверов от REST или Gateway немедленно перерисовывать список серверов (`#guild-list`) без необходимости перезапуска приложения.

---

### CARD-TEL-008 (`P2` / Отдел `G4 QA`): Регрессионный тест сценариев вступления в сервер и синхронизации гильдий
- **Файлы**: `tests/unit/join-server.test.mjs`, `rust/gui-spike/tests/guild_contract.rs`.
- **Требуемое решение**:
  - Добавить юнит-тесты на обработку ошибок `accept_invite` (проверка отсутствия dead clicks).
  - Добавить тест на закрытие модального окна инвайтов по клику на подложку и по `Escape`.
