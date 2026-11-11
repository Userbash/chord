# Спецификация волн RW-47, RW-48, RW-49 и Инструкции по написанию кода

## 1. Общие стандарты кодирования и правила TDD

Все задачи следующих волн строго подчиняются правилам `AGENTS.md`, `PROCESS.md`, `TEST-POLICY.md` и `.agents/rules/*`:

1. **Strict TDD (Красный ➔ Зелёный ➔ Рефакторинг):**
   - Никакой продуктовый код не пишется до появления падающего теста (`node --test tests/unit/<test>.test.mjs` или `cargo test -p <crate> --test <contract>`).
   - Тест обязан проверять независимый оракул (дамп `/home/sanya/re-deep/`, REA-артефакты, RFC, эталонные хеши xxh64/SHA-256).
2. **Мутационный гейт 5/5 (100% Mutation Kill Rate):**
   - Каждый юнит/контрактный тест обязан включать проверку 5 целенаправленных мутаций логики (инверсия условий, обход проверок прав, зануление ключей, искажение таймстемпов, пропуск валидации).
   - Тест обязан падать на всех 5 мутациях и восстанавливать исходное состояние (сохранение/сверка MD5).
3. **Лимиты архитектуры и чистота кода:**
   - Функции строго `< 50 строк` логики.
   - Исходные файлы строго `< 800 строк` (Soft Maintainability Ceiling).
   - Запрет ключевого слова `unsafe` во всех Rust-модулях (`#![forbid(unsafe_code)]`).
   - Изоляция скоупов: файлы воркеров не пересекаются; общие файлы правит только координатор.
4. **Контракты и сверка с оракулом `/home/sanya/re-deep/`:**
   - Опкоды REST v9, структуры JSON payload, битовые маски и форматы медиа сверяются с реальным дампами `re-deep`:
     - `re-deep/gateway-schema.json`, `gateway-protocol.md` (Gateway events)
     - `re-deep/rest-d02.json`, `d02-snapshot/` (REST v9 routes)
     - `re-deep/main-arch.json`, `backend-ipc.json` (IPC, RPC)
     - `re-deep/voice-deep.md`, `voice/` (WebRTC, DAVE, RTP)

---

## 2. Волна RW-47: Community, Threads & Interactive Chat

### TASK-1001: Форумные каналы `GUILD_FORUM` и карточки постов
- **Отдел:** `G2 Frontend` + `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/channels/forum_view.rs`
  - `scripts/verify-forum-channels.mjs`
  - `tests/unit/forum-channels-1001.test.mjs`
- **Инструкция по коду:**
  1. Реализовать парсер типа канала `type: 15` (`GUILD_FORUM`) из Gateway `CHANNEL_CREATE`/`GUILD_CREATE`.
  2. Поддержать теги форума `available_tags` (id, name, emoji_id, emoji_name, moderated).
  3. Отрисовать сетку постов: заголовок, автор, дата создания, теги, превью первого вложения, счётчик сообщений и реакций.
  4. Проверить пагинацию постов через REST `GET /channels/{id}/threads/active`.

### TASK-1002: Вложенные треды обсуждений `PUBLIC_THREAD` и автоархивация
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/channels/threads.rs`
  - `tests/unit/threads-lifecycle-1002.test.mjs`
- **Инструкция по коду:**
  1. Обработка опкода создания треда из сообщения: `POST /channels/{id}/messages/{msg_id}/threads`.
  2. Боковая панель треда шириной 360px справа от основного чата с независимым виртуальным скроллом.
  3. Поддержка таймеров автоархивации (`auto_archive_duration`: 60, 1440, 4320, 10080 минут).
  4. Контракт на корректное закрытие/открытие треда (`thread_metadata.archived: true/false`).

### TASK-1003: Голосовые сообщения с Waveform в чате (OGG Opus)
- **Отдел:** `G5 Voice` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-voice/src/voice_message.rs`
  - `rust/discord-gpui/src/chat/voice_message_player.rs`
  - `tests/unit/voice-messages-waveform-1003.test.mjs`
- **Инструкция по коду:**
  1. Захват аудио с микрофона через `cpal` в буфер моно 48 кГц, кодирование в Opus 32 kbps в контейнер OGG.
  2. Генерация байтового массива пиковых амплитуд волны (256 байт base64 waveform) по алгоритму RMS на кадры по 20 мс.
  3. Отрисовка интерактивных столбиков волны в плеере GPUI со скраббингом по клику и кнопкой `1x/1.5x/2x`.
  4. Отправка через `POST /channels/{id}/messages` с флагом `flags: 8192` (`IS_VOICE_MESSAGE`).

### TASK-1004: Интерактивные опросы в сообщениях (Polls & Voting)
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/chat/polls.rs`
  - `tests/unit/polls-voting-1004.test.mjs`
- **Инструкция по коду:**
  1. Модель опроса `Poll`: вопрос (`question.text`), массив вариантов ответа (`answers[].poll_media`), `allow_multiselect`, `expiry`.
  2. Отправка голоса через `PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`.
  3. Отображение процентных шкал и количества проголосовавших с анимацией перехода.

### TASK-1005: Серверные эмодзи и анимированные стикеры Lottie
- **Отдел:** `G2 Frontend` + `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/chat/custom_emojis.rs`
  - `tests/unit/custom-emojis-stickers-1005.test.mjs`
- **Инструкция по коду:**
  1. Парсинг кастомных эмодзи `<:name:id>` и анимированных `<a:name:id>` в `MessageParser`.
  2. Двухуровневый кэш (память LRU 256 МБ + диск `~/.cache/discord-gpui/emojis/`).
  3. Рендеринг векторных стикеров Lottie через встроенный парсер кадров или кэшированные растры.

### TASK-1006: Нативная проверка орфографии в поле ввода (hunspell / libenchant)
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/chat/spellchecker.rs`
  - `tests/unit/spellchecker-native-1006.test.mjs`
- **Инструкция по коду:**
  1. FFI биндинг к системной `libenchant-2.so` или `libhunspell.so` с fallback на словарь опечаток.
  2. Асинхронная проверка слов при вводе текста с дебаунсом 150 мс.
  3. Отрисовка красной волнистой линии под словом и вывод контекстного меню с вариантами исправления при ПКМ.

---

## 3. Волна RW-48: Server Administration, Roles & Security

### TASK-1007: Иерархия ролей сервера и редактор битовой маски прав
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/guild/roles.rs`
  - `tests/unit/guild-roles-permissions-1007.test.mjs`
- **Инструкция по коду:**
  1. Модель роли: `id`, `name`, `color` (RGB u32), `hoist`, `position`, `permissions` (64-битное поле `u64`).
  2. Drag-and-drop упорядочивание позиций ролей с отправкой `PATCH /guilds/{id}/roles`.
  3. Матрица разрешений с 35+ тумблерами и проверкой невозможности выдать права выше наивысшей роли пользователя.

### TASK-1008: Журнал аудита сервера (Server Audit Log & REST API v9)
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/guild/audit_log.rs`
  - `tests/unit/audit-log-viewer-1008.test.mjs`
- **Инструкция по коду:**
  1. Запрос `GET /guilds/{id}/audit-logs` с фильтрацией по типу действия (Action Types 1..145) и пользователю.
  2. Декодирование `changes` (old_value, new_value) для ролей, каналов, прав, банов и интеграций.
  3. Форматированный список с иконками типов событий и временными метками.

### TASK-1009: Двухфакторная аутентификация TOTP 2FA и управление сессиями
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/auth/totp_dialog.rs`
  - `tests/unit/totp-sessions-1009.test.mjs`
- **Инструкция по коду:**
  1. Модальное окно `TotpDialog` с 6 раздельными полями ввода цифр и авто-переходом фокуса.
  2. Запрос списка активных сессий `GET /users/@me/auth/sessions` (браузер, ОС, IP, последнее время активности).
  3. Кнопка принудительного разлогина `DELETE /users/@me/auth/sessions/{id}` с инвалидацией сессионных токенов.

### TASK-1010: Автоматический перевод в AFK-канал при простое
- **Отдел:** `G1 Backend` + `G5 Voice` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/voice/afk_monitor.rs`
  - `tests/unit/voice-afk-monitor-1010.test.mjs`
- **Инструкция по коду:**
  1. Опрос системного времени бездействия пользователя через D-Bus `org.freedesktop.ScreenSaver` / `ext-idle-notifier-v1`.
  2. Сверка с настройкой гильдии `afk_timeout` (60, 300, 900, 1800, 3600 секунд).
  3. При превышении тайм-аута отправка Gateway Opcode 4 на перевод в `afk_channel_id` с выставлением флага self_mute.

### TASK-1011: Продвинутая WebRTC диагностика и RTC Internals (`RE-GAP-22`)
- **Отдел:** `G5 Voice` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/voice/rtc_diagnostics.rs`
  - `tests/unit/rtc-diagnostics-internals-1011.test.mjs`
- **Инструкция по коду:**
  1. Сбор RTCP receiver/sender reports (пакеты, потери, jitter, RTT, bitrate, packetsLost).
  2. Окно диагностики с динамическим графиком задержки и пропускной способности (60 кадров/с).
  3. Индикация статуса сквозного шифрования DAVE MLS v1 (текущая эпоха, ID группы, статус ратчета).

---

## 4. Волна RW-49: Ecosystem, Soundboard & Cross-Platform

### TASK-1012: Серверная звуковая панель (Soundboard SFX)
- **Отдел:** `G5 Voice` + `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-voice/src/soundboard.rs`
  - `tests/unit/soundboard-sfx-1012.test.mjs`
- **Инструкция по коду:**
  1. Воспроизведение звуков с отправкой Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`).
  2. Микширование звука саундборда в локальный RTP Opus поток с регулируемым уровнем громкости.
  3. Кэширование аудио-сэмплов на диске в формате Opus 48 кГц stereo.

### TASK-1013: Интеграция со Spotify по D-Bus MPRIS (Listen Along)
- **Отдел:** `G1 Backend` + `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/integrations/spotify_mpris.rs`
  - `tests/unit/spotify-mpris-presence-1013.test.mjs`
- **Инструкция по коду:**
  1. Подписка на сигналы D-Bus `org.mpris.MediaPlayer2.spotify` (`PropertiesChanged`).
  2. Извлечение метаданных: `xesam:title`, `xesam:artist`, `xesam:album`, `mpris:trackid`, `mpris:length`.
  3. Публикация активности типа 2 (`LISTENING`) в Gateway Opcode 3 (`STATUS_UPDATE`).
  4. Кнопка «Слушать вместе» (синхронизация воспроизведения через Spotify Connect Web API).

### TASK-1014: Режим Steam Deck & Gamepad UI
- **Отдел:** `G2 Frontend` + `G3 Design` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/input/gamepad.rs`
  - `tests/unit/steam-deck-gamepad-1014.test.mjs`
- **Инструкция по коду:**
  1. Опрос геймпада через `gilrs` (D-pad, стики, бамперы L1/R1, кнопки A/B/X/Y).
  2. Фокусная сетка: навигация по серверам через L1/R1, по каналам через D-pad вверх/вниз, вход в чат по кнопке A.
  3. Вызов экранной клавиатуры SteamOS по хоткею `Steam + X` или через D-Bus `org.kde.kwin.VirtualKeyboard`.

### TASK-1015: Автозапуск с системой (XDG Autostart & Start Minimized)
- **Отдел:** `DevOps` + `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/platform/autostart.rs`
  - `tests/unit/autostart-xdg-1015.test.mjs`
- **Инструкция по коду:**
  1. Запись/удаление `~/.config/autostart/discord-gpui.desktop`.
  2. Флаг `--start-minimized`: запуск в фоновом режиме без маппинга WGPU окна, отображение только иконки в трее.
  3. Поддержка сохранения состояния окна между перезагрузками системы.

### TASK-1016: Сборки и пакеты для Windows (MSI/Portable) и macOS (DMG/Vibrancy)
- **Отдел:** `DevOps` | **Приоритет:** `P2`
- **Файлы:**
  - `packaging/windows/wix.xml`
  - `packaging/macos/build-dmg.sh`
  - `tests/unit/cross-platform-packaging-1016.test.mjs`
- **Инструкция по коду:**
  1. Спецификация WiX Toolset для сборки нативного `.msi` установщика и `.exe` portable.
  2. Скрипт упаковки macOS `.app` бандла в `.dmg` с фоновой картинкой и симлинком `/Applications`.
  3. Поддержка macOS Window Vibrancy (NSVisualEffectView Material HUD Window).
