# Анализ индекса Understand, кода и причин десинхронизации функций

- **Дата:** 2026-11-06
- **Статус графа Understand:** `analyzed: 2026-11-02T06:22:27.113Z @ 4e240481e59f` (зафиксирован на коммите начала Wave 40).
- **Актуальный коммит production (`origin/main` / `origin/beta`):** `2a42ba7` (**866 коммитов впереди**).

---

## 1. Главная причина: Почему «расползся проект» и куда делись функции

### Причина 1: Рассинхронизация воркспейсов и веток Git
Разработка велась через параллельные изолированные рабочие деревья Git (Worktrees):
1. `/home/user/orca/project` — **корневой каталог**, где запущен текущий терминал. Ветка `main` застряла на базовом коммите `4e24048` (2 ноября) и накопила **594 незакоммиченных файла**.
2. `/home/user/orca/workspaces/project/unstable` — ветка `unstable` (`016e8c7`).
3. `/home/user/orca/workspaces/project/testing` — ветка `testing` (`4191968`).
4. `/home/user/orca/workspaces/project/beta` — ветка `beta` (`2a42ba7`), которая соответствует релизной ветке `origin/main`.

> [!CAUTION]
> **Что пошло не так:**
> Релизный бинарник `discord-gui-spike` компилировался из **локального корневого каталога** `/home/user/orca/project/rust/gui-spike/` на старой базе коммита `4e24048`, а не из финальной ветки `origin/main` (`beta` / `2a42ba7`), в которую уже влиты волны 41–49!

---

## 2. Куда делись конкретные функции и где их реализация

### 1. Демонстрация экрана («Демки» / Go Live) и звонки (Voice Streams, Opcodes 18..22)
- **Где реализованы в коде:**
  - В эталонных проектах графа Understand: [`docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs`](file:///home/user/orca/project/docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs) и [`discord-gateway/src/voice.rs`](file:///home/user/orca/project/docs/reference-projects/sources/Serein/crates/discord-gateway/src/voice.rs) — выделенный цикл Go Live сессии (`run_stream_inner`), опкоды `18 (StreamCreate)`, `19 (StreamDelete)`, `20 (StreamWatch)`, `21 (StreamPing)`, `22 (StreamSetPaused)`.
  - В ветках `feature/task-601-go-live-dual-session`, `bugfix/task-605-stream-dave-mls-and-capture` и `beta`:
    - `rust/crates/discord-voice/src/lib.rs`, `media/rtp.rs`, `srtp.rs` (DAVE MLS, WebRTC H.264 декодер, RTP Assembler).
    - `rust/gui-spike/ui/features/voice.js` (82 КБ в `beta` против 51 КБ в старом коде) — методы `voice_stream_create`, `voice_stream_watch`, `onStartStream`, `onWatchStream`.
- **Почему не работали:** В скомпилированном бинарнике запускался устаревший `voice.js` без зарегистрированных UI-мостов захвата экрана и WebRTC-пайплайна.

### 2. Общие чаты в личных сообщениях (Group DMs)
- **Где реализованы в коде:**
  - Ветка `unstable` / `beta`: [`rust/gui-spike/ui/features/dm-friends.js`](file:///home/user/orca/workspaces/project/beta/rust/gui-spike/ui/features/dm-friends.js) (141 КБ).
  - Константы `GROUP_DM: 3`, лимиты `MAX_GROUP_DM_RECIPIENTS = 9`, модальное окно создания `#group-dm-dialog`, маршрут `/users/@me/channels/group-dm-shell`.
  - Кнопка вызова группового чата смонтирована в шапке ЛС: `headerGroupDmBtn = make("button", "icon-btn", "💬")` с атрибутом `data-action="open-create-group-dm"`.
- **Почему не работали:** На старой сборке при запуске сессии кнопка `#home-rail-btn` («DL») не открывала экран списка каналов ЛС, а модалка `#group-dm-dialog` не рендерилась из-за отсутствия вызова инициализации в устаревшем `app.js`.

### 3. Отправка и прием сообщений (`send_message`, `chat.js`, `composer.js`)
- **Где реализованы в коде:**
  - Backend: [`rust/gui-spike/src/main.rs:279`](file:///home/user/orca/workspaces/project/beta/rust/gui-spike/src/main.rs#L279) — IPC-команда `send_message`, делегирующая в `discord_gui_spike::messages::send_message`.
  - Frontend: `rust/gui-spike/ui/features/chat.js` (241 КБ) и `composer.js` (76 КБ).
- **Почему не работали:**
  - В приложении отображалось «Нет серверов», а в ЛС не был выбран активный `activeChannelId`.
  - В `composer.js` поле ввода и кнопка отправки активны **только при наличии валидного `currentChannelId`**. Без выбранного канала события клавиатуры и клики отправки не привязаны к маршруту `POST /channels/{id}/messages`.

---

## 3. Состояние индекса `understand` (`.ua/knowledge-graph.json`)

1. Граф `understand` содержит 281 узел и 497 связей, проиндексированных на коммите `4e24048`.
2. В графе зафиксированы архитектурные слои:
   - `Layer 1: Gateway Protocol & ETF Decoding`
   - `Layer 2: REST Transport & Rate Limiting`
   - `Layer 3: RTC Signaling & Voice Transport`
   - `Layer 4: Storage & Keyring Security`
   - `Layer 5: Tauri BFF & Desktop Shell`
3. Для актуализации требуется инкрементальный запуск `understand` поверх объединённой ветки после синхронизации коммитов `origin/main`.
