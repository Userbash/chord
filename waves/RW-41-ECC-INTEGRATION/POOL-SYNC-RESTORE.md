# POOL-SYNC-RESTORE — Пул задач восстановления и динамической синхронизации

Дата обновления: 2026-11-06T14:44:00+05:00. PM / Tech Lead ECC. Волна: `RW-41-ECC-INTEGRATION`.
Оркестрация Orca: `run_e652142f50b1` (ECC 2.0 Control Plane).

## 1. Политика моделей и маршрутизация агентов (Строго по AGENTS.md и указанию пользователя)

> [!IMPORTANT]
> **Прямое указание пользователя:** Полный запрет **DeepSeek** (`--agent opencode --model deepseek/*`) и полный запрет **Codex** (`--agent codex`).
> Для всех отделов (Backend, Frontend, Voice, QA, Design) назначен агент **Antigravity** с моделью **`gemini-4-argon-high`** и наивысшим уровнем рассуждения **`--effort high`**.
> Лимит ОЗУ: `WORKER_RAM_LIMIT <= 25 GB` (не более 5 активных слотов воркеров).

| Отдел | Область ответственности (Write-Scope) | Назначенный агент | Модель | Уровень усилий (Effort) |
|---|---|---|---|:---:|
| **G1 Backend** | `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs` | `antigravity` | `gemini-4-argon-high` | `high` |
| **G2 Frontend** | `rust/gui-spike/ui/**/*.{js,css,html}`, `tests/unit/*.test.mjs` | `antigravity` | `gemini-4-argon-high` | `high` |
| **G3 Design** | `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`, CSS-токены | `antigravity` | `gemini-4-argon-high` | `high` |
| **G4 QA** | `tests/{meta,unit,smoke,e2e}/`, `scripts/test-impact.mjs` | `antigravity` | `gemini-4-argon-high` | `high` |
| **G5 Voice / Platform** | `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, `voice_devices.rs`, WebRTC | `antigravity` | `gemini-4-argon-high` | `high` |
| **PM & Tech Review** | `ACCEPT.md`, `REVIEW.md` (5/5 мутаций, DoD верификация) | `antigravity` | `gemini-4-argon-high` | `high` |

---

## 2. Сводный реестр задач пула разработки

| ID задачи | Orca Task ID | Отдел | Приоритет | Название задачи | Статус | DoD (Критерий приёмки) |
|---|---|---|:---:|---|:---:|---|
| **`CARD-DYN-001`** | `task_6a09de0b8b43` | **G1 Backend** | `P0` | **Полное устранение хардкода ID и динамическая загрузка серверов/друзей/каналов** | 🟢 **DONE** | Вычищены все вхождения `384299970632876032`. Список серверов, друзей и каналов загружается строго для текущего вошедшего аккаунта через REST `/users/@me/*`. Сброс кеша `state_cache.json` при смене пользователя. |
| **`CARD-DYN-002`** | `task_95336995c1e2` | **G2 Frontend** | `P0` | **Реактивная сквозная синхронизация входящих сообщений (WebSocket MESSAGE_CREATE)** | 🟢 **DONE** | При приходе `MESSAGE_CREATE` по шлюзу входящее сообщение мгновенно монтируется в активный виртуальный список DOM без ручного переключения каналов (`syncLiveMessage` в `chat.js` и `applyMessageCreate` в `dispatch-view.js`). |
| **`CARD-DYN-003`** | `task_bdb820c9e6af` | **G2 Frontend / G5 Voice** | `P0` | **Восстановление звонков, камеры, показа экрана («демки») и групповых чатов в ЛС** | 🟢 **DONE** | В шапку ЛС выведены кнопки: 📞 Аудиозвонок, 📹 Видеозвонок, 🖥 Показ экрана («демка»), 👥+ Добавить друзей в беседу, 🔗 Приглашение. Кнопка создания группы открывает `#group-dm-dialog`. |
| **`CARD-DYN-004`** | `task_37af109669bd` | **G4 QA** | `P1` | **Контрактные и unit-тесты динамической изоляции аккаунтов и отсутствия хардкода** | 🟢 **DONE** | Написан и пройден сьют `tests/unit/account-dynamic-sync.test.mjs` (3/3 pass: 0 хардкодных ID, правила удаления, кнопки ЛС). |
| **`CARD-DYN-005`** | `task_7b5fee2f557e` | **G2 Frontend / G1 Backend** | `P0` | **Защита от сбоя при удалении сообщений (Permissions Guard on `delete_message`)** | 🟢 **DONE** | В ЛС кнопка удаления доступна **только для своих сообщений**. На сервере удаление чужих сообщений разрешено **только при наличии `MANAGE_MESSAGES` / Admin / Owner**. `canDeleteLiveMessage` блокирует клик и предотвращает 403-краш. |
| **`CARD-RESTORE-006`** | — | **G2 Frontend / G3 Design** | `P1` | **Монтирование реакций, пикера выражений, спойлеров Markdown и контекстного меню** | 🟢 **DONE** | Встроены модули реакций, быстрых реакций на ховере (`👍 ❤️ 😂`), контекстное меню сообщений и спойлеры. |

---

## 3. Критерии приёмки Продуктовым Менеджером (DoD)

1. **Динамический профиль и серверы:**
   - Все списки серверов, каналов и контактов привязаны строго к текущему активному токену и аккаунту (`Rustery`).
   - 0 вхождений ID `384299970632876032` в кодовой базе `data.rs`.
2. **Синхронизация сообщений:**
   - Входящие сообщения WebSocket отображаются реактивно в DOM ленты чата.
3. **Личные сообщения (Direct Messages):**
   - В шапке открытого ЛС отображаются кнопки звонка (📞), камеры (📹), демки (🖥) и добавления в групповой чат (👥+).
   - Модальное окно `#group-dm-dialog` создаёт групповой чат до 10 участников.
4. **Безопасное удаление сообщений:**
   - Удаление чужих сообщений в ЛС невозможно в интерфейсе; на сервере требуется `MANAGE_MESSAGES`.
   - Исключены ошибки `403 Forbidden` и краши приложения.
