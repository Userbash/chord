# Отчёт воркера — `TASK-1002` (Вложенные треды обсуждений `PUBLIC_THREAD` / `PRIVATE_THREAD` и автоархивация)

- **Batch / Карточка:** `RW-47` / `TASK-1002` — `G1 Backend` + `G2 Frontend (GPUI)`
- **Воркер / Терминал:** `term_2971f3be-cd80-43bf-acee-f285e50600e4` (`task_9087c0ed3c34`, `ctx_060c2d0fa444`)
- **Ветка:** `feature/sprints-1-5-build-004`

---

## 1. Сделано

1. **Модуль жизненного цикла вложенных тредов (`rust/discord-gpui/src/channels/threads.rs`):**
   - Поддержка типов каналов `CHANNEL_TYPE_PUBLIC_THREAD = 11` (`PUBLIC_THREAD`) и `CHANNEL_TYPE_PRIVATE_THREAD = 12` (`PRIVATE_THREAD`) (`ThreadChannelKind`).
   - Формирование REST-запросов создания треда из сообщения `POST /channels/{id}/messages/{msg_id}/threads` (`REST_CREATE_MESSAGE_THREAD_TEMPLATE`) и приватного треда в канале `POST /channels/{id}/threads` (`REST_CREATE_CHANNEL_THREAD_TEMPLATE`) с JSON-конвертом `{"name","type","auto_archive_duration","location":"Message"}` по RE-оракулу `/home/sanya/re-deep/rest-static.json`.
   - Проверка битовых масок прав доступа (`PERM_VIEW_CHANNEL = 1 << 10`, `PERM_MANAGE_THREADS = 1 << 34`, `PERM_CREATE_PUBLIC_THREADS = 1 << 35`, `PERM_CREATE_PRIVATE_THREADS = 1 << 36`, `PERM_SEND_MESSAGES_IN_THREADS = 1 << 38`).
   - Правая боковая панель `Thread Split View` (`360px sidebar`, `padding: 8px`, `spacing: 8px`, дизайн-токены `#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`, состояния `thread-indicator`: `link -> underline -> open sidebar`) с независимым виртуальным скроллом (`scroll_thread_sidebar_viewport` vs `scroll_main_chat_viewport`) и сохранением позиции скролла каждого треда (`per_thread_scroll`).
   - Таймеры автоархивации неактивных тредов `ALLOWED_AUTO_ARCHIVE_DURATIONS = [60, 1440, 4320, 10080]` мин (`1 ч.`, `24 ч.`, `3 дн.`, `1 нед.`), автоматическое разархивирование при новом сообщении и защита `locked`-тредов (`LockedThreadRequiresManageThreads` без права `MANAGE_THREADS`).
   - Обработка событий синхронизации `THREAD_MEMBERS_UPDATE` (`apply_thread_members_update` с дедупликацией) и `THREAD_LIST_SYNC` (`apply_thread_list_sync`).
   - Полная валидация 9 обязательных измерений `TEST-POLICY.md`: граничные Snowflake (`0`, `u64::MAX`), имя треда (`1..=100` символов), сообщение (`2000` символов), вложения (`10` файлов / `25 МБ`), `offset <= 9975`, громкость `0..=200%`, бейдж `99+`, HTTP-статусы `200/202/204/400/401/403/404/413/429 (retry_after_ms)/5xx`, `AGE_GATED_SPACES` и маскирование секретов (`mfa.*`, `dQw4w9WgXcQ:`, `ghp_*`, JWT/Bearer/Bot) в JSON-телеметрии.

2. **Контрактный и мутационный тест (`tests/unit/threads-lifecycle-1002.test.mjs`):**
   - Сверка с независимыми оракулами: `waves/kanban-board.json` (`TASK-1002`), `docs/re/rest-d02.json` (`/api/v9/channels/{id}/threads/search`), `rust/crates/discord-core/src/permissions.rs`, `rust/crates/discord-ui/src/reply_thread.rs`, `rust/gui-spike/ui/features/threads-drawer.js`, `/home/sanya/re-deep/rest-static.json` и `/home/sanya/re-deep/modules-index.json` (`45494`, `152007`, `219065`, `456874`, `970278`).
   - Покрытие `SUB-1002-0` .. `SUB-1002-5`, включая `5/5` убитых мутаций (`MUT-threads-01` .. `MUT-threads-05`, `100%` mutation kill rate).

---

## 2. Red-прогон (до реализации модуля)

Команда:
```sh
node --test tests/unit/threads-lifecycle-1002.test.mjs
```
Результат (`waves/RW-47/BACKEND/task-1002-threads-lifecycle/red.log`):
- `tests 6 | pass 0 | fail 6` (`AssertionError: Модуль rust/discord-gpui/src/channels/threads.rs должен существовать`).

---

## 3. Green-прогон и Мутационный гейт (после реализации)

Команда:
```sh
node --test tests/unit/threads-lifecycle-1002.test.mjs
```
Результат (`waves/RW-47/BACKEND/task-1002-threads-lifecycle/green.log` и `mutation.log`):
- `tests 6 | suites 1 | pass 6 | fail 0` (`860.5 ms`).
- Мутации `threads.rs`: `5/5` убито (`MUT-threads-01` .. `MUT-threads-05`, kill rate `1.0`).

---

## 4. Проверки качества (`T0`)

- `#![forbid(unsafe_code)]` в `rust/discord-gpui/src/channels/threads.rs`.
- `0` вызовов `.unwrap()` / `.expect()` в продуктовом коде.
- `rustfmt --check --edition 2024` — чисто.
- `rustc --test --edition 2024 -D warnings` — `0` предупреждений.
- Лимиты размера (`checkRustSourceSizeLimits`): все функции `< 50` строк, файл `791` строка (`< 800`).
- Секреты (`scanTextForSecrets` из `scripts/secret-scan.mjs`): `0` находок.

---

## 5. Файлы и MD5

| Файл | Строк | MD5 |
|---|---:|---|
| `rust/discord-gpui/src/channels/threads.rs` | 791 | `12d699a5bcce91e345900de2d5f8b5ec` |
| `tests/unit/threads-lifecycle-1002.test.mjs` | 470 | `d48a5a6585a1a734a13b2a175b05fcf2` |
| `waves/RW-47/BACKEND/task-1002-threads-lifecycle/red.log` | 110 | `eabad5bd0fe836e4843601e0d1c35a37` |
| `waves/RW-47/BACKEND/task-1002-threads-lifecycle/green.log` | 16 | `e462f730d72c6bfe033aadcc16e1f813` |
| `waves/RW-47/BACKEND/task-1002-threads-lifecycle/mutation.log` | 16 | `e462f730d72c6bfe033aadcc16e1f813` |

---

## 6. Риски и примечания

- Общие индексные файлы (`rust/discord-gpui/src/main.rs`, `rust/discord-gpui/src/channels/mod.rs`) не модифицировались в соответствии с правилом `AGENTS.md` §1.2 (реэкспорт выполняется на этапе интеграции Team Lead).
