# Детальная декомпозиция задач, микро-планы реализации и аннотации кодирования

Данный документ представляет собой пошаговую детализацию всех недостающих функциональных возможностей оригинального Discord, выявленных при сверке с дампом `/home/sanya/re-deep/` и бэклогом переноса.
Каждая крупная возможность декомпозирована на **атомарные задачи**, а каждая задача разбита на **микро-шаги по методологии Strict TDD** с точными путями к файлам, ссылками на оракулы и правилами мутационного тестирования.

---

## 1. Блок А: Чат и интерактивное сообщество (Community & Interactive Chat)

### Эпик A1: Форумные каналы (`GUILD_FORUM` type 15)

#### Задача TASK-1001A: Сетевой протокол, парсер Gateway и REST API форумов
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/forum.rs`
  - `tests/unit/forum-protocol-1001a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/gateway-schema.json` (событие `CHANNEL_CREATE`, флаг `type: 15`), `/home/sanya/re-deep/rest-d02.json` (`GET /channels/{id}/threads/active`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Написать юнит-тест `tests/unit/forum-protocol-1001a.test.mjs`, проверяющий десериализацию полей `available_tags`, `default_reaction_emoji`, `default_sort_order` и `default_forum_layout`.
  2. *Micro-Step 2 (Green):* Определить структуры `ForumTag`, `ForumLayout`, `ForumSortOrder` в `forum.rs` с сериализацией `serde`.
  3. *Micro-Step 3 (Green):* Реализовать метод клиента `get_active_forum_threads(channel_id)` с поддержкой пагинации по Snowflake.
  4. *Micro-Step 4 (Mutation 5/5):* Проверить убийство 5 мутаций (ошибочный type, пропуск флага moderated в тегах, инверсия сортировки, пустой emoji_id, зацикливание пагинации).

#### Задача TASK-1001B: UI-сетка постов форума `ForumGridView`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/channels/forum_view.rs`
  - `tests/unit/forum-view-1001b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/frontend-d03.json` (разметка карточек постов и фильтрация).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест проверяет компоновку сетки `ForumGridView` (высота плашки 96px, отступы 12px, адаптивная 2-колоночная раскладка).
  2. *Micro-Step 2 (Green):* Отрисовка плашек тегов с поддержкой фильтрации (мультивыбор тегов, подсветка активных токеном `#5865f2`).
  3. *Micro-Step 3 (Green):* Карточка поста: заголовок (жирный 15px), превью автора (аватар 20px + ник), бейджи тегов, счётчик ответов и реакций.
  4. *Micro-Step 4 (A11y & Mutation):* Навигация стрелками клавиатуры по карточкам и тест 5 мутаций.

---

### Эпик A2: Вложенные треды обсуждений (Threads type 11/12)

#### Задача TASK-1002A: Жизненный цикл тредов и автоархивация
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/threads.rs`
  - `tests/unit/threads-lifecycle-1002a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`POST /channels/{id}/messages/{msg_id}/threads`, `PATCH /channels/{id}`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест создания треда из сообщения с параметрами `name`, `auto_archive_duration` (60, 1440, 4320, 10080 минут).
  2. *Micro-Step 2 (Green):* Реализация `create_thread_from_message()` и парсера метаданных `thread_metadata` (`archived`, `locked`).
  3. *Micro-Step 3 (Green):* Синхронизация списка участников треда `THREAD_MEMBERS_UPDATE`.
  4. *Micro-Step 4 (Mutation 5/5):* 5 контрольных мутаций (пропуск locked при архивации, сбой u32 таймера, дублирование member_id).

#### Задача TASK-1002B: Боковая панель треда `ThreadSidebar`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/chat/thread_sidebar.rs`
  - `tests/unit/thread-sidebar-1002b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/frontend-d03.json` (сплит-панель чата 360px).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Проверка макета split-view: правая панель фиксированной ширины 360px, кнопка закрытия, заголовок треда.
  2. *Micro-Step 2 (Green):* Независимый экземпляр `virtual_list` для сообщений треда (отдельный буфер скролла).
  3. *Micro-Step 3 (Green):* Индикатор в основном чате: плашка под родительским сообщением «N ответов • Последний 5 мин назад».
  4. *Micro-Step 4 (Mutation 5/5):* Мутации (разрушение ширины при ресайзе окна, потеря фокуса ввода при переключении треда).

---

### Эпик A3: Голосовые сообщения в чате (Voice Messages & Waveform)

#### Задача TASK-1003A: Аудио-рекордер и вычисление Waveform
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-voice/src/voice_message_recorder.rs`
  - `tests/unit/voice-recorder-1003a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`flags: 8192`, заголовок `audio/ogg; codecs=opus`, параметр `waveform`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест кодирования аудиопотока 48 кГц mono в OGG Opus (32 kbps, VBR) и формирования 256-байтного массива пиков.
  2. *Micro-Step 2 (Green):* Реализация буфера захвата через `cpal` с расчетом среднеквадратичной амплитуды (RMS) на 20 мс кадр.
  3. *Micro-Step 3 (Green):* Нормализация значений амплитуд в диапазон `0..255` и экспорт в base64 строку для REST.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ошибочный sample_rate, переполнение u8 при нормализации, рассинхронизация длительности).

#### Задача TASK-1003B: Интерактивный плеер `VoiceMessagePlayer`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/chat/voice_player.rs`
  - `tests/unit/voice-player-1003b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/ui7/` (плеер голосовых сообщений).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Отрисовка 64 или 128 вертикальных полосок волны (ширина 2px, закругление 1px, зазор 1px).
  2. *Micro-Step 2 (Green):* Прогресс воспроизведения: окрашивание проигранной части волны в акцентный цвет `#23a55a`.
  3. *Micro-Step 3 (Green):* Интерактивный скраббинг: клик в любую точку волны мгновенно перематывает позицию воспроизведения.
  4. *Micro-Step 4 (Speed Toggle & Mutation):* Кнопка скорости воспроизведения (`1x -> 1.5x -> 2x -> 1x`) и тест 5 мутаций.

---

### Эпик A4: Интерактивные опросы в чате (Polls & Voting)

#### Задача TASK-1004A: Модель данных опроса и REST-голосование
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/polls.rs`
  - `tests/unit/polls-core-1004a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`, `DELETE ...`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Десериализация объекта `poll`: `question`, `answers[]`, `expiry`, `allow_multiselect`, `results`.
  2. *Micro-Step 2 (Green):* Методы `cast_poll_vote()` и `remove_poll_vote()` с валидацией тайм-аута истечения опроса.
  3. *Micro-Step 3 (Green):* Расчет процентов по формуле $\frac{\text{votes}_i}{\sum \text{votes}} \times 100\%$ с корректным округлением.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (деление на ноль при 0 голосов, прием голоса в истекший опрос, обход allow_multiselect).

#### Задача TASK-1004B: UI-карточка опроса `PollCardView`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/chat/poll_view.rs`
  - `tests/unit/poll-view-1004b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/frontend-d03.json` (интерактивный блок опроса в сообщении).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест проверяет отрисовку рамки опроса, чекбоксов (для мультивыбора) или радио-кнопок.
  2. *Micro-Step 2 (Green):* Анимированная заливка шкалы прогресса ответа полупрозрачным фоном `#5865f233`.
  3. *Micro-Step 3 (Green):* Отображение индикатора победившего ответа (иконка короны / галочки) после закрытия голосования.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (отсутствие дебаунса кликов, искажение ширины прогресс-бара > 100%).

---

### Эпик A5: Серверные эмодзи и анимированные стикеры

#### Задача TASK-1005A: Парсер эмодзи и дисковый LRU-кэш
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-media/src/emoji_cache.rs`
  - `tests/unit/emoji-cache-1005a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/frontend-d03.json` (регулярные выражения и CDN URL).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Парсинг токенов `<:name:snowflake>` (статичный) и `<a:name:snowflake>` (анимированный GIF).
  2. *Micro-Step 2 (Green):* Формирование CDN URL `https://cdn.discordapp.com/emojis/{id}.webp?size=48&quality=lossless`.
  3. *Micro-Step 3 (Green):* Двухуровневый кэш: память (256 МБ LRU) + диск `~/.cache/discord-gpui/emojis/` с валидацией ETag.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (path traversal через имя файла, утечка памяти кэша, искажение snowflake id).

#### Задача TASK-1005B: Интерактивный пикер эмодзи и стикеров `EmojiPickerModal`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/chat/emoji_picker.rs`
  - `tests/unit/emoji-picker-1005b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/ui8/` (попап выбора эмодзи и стикеров).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Модальное окно 380×420px, поле быстрого поиска, вкладки категорий (Часто используемые, Серверы, Стандартные).
  2. *Micro-Step 2 (Green):* Виртуализированная сетка 8 колонок по 32px с плавным скроллом и предзагрузкой видимых изображений.
  3. *Micro-Step 3 (Green):* Вкладка стикеров с поддержкой векторного рендеринга Lottie JSON кадров.
  4. *Micro-Step 4 (Keyboard Nav & Mutation):* Навигация стрелками клавиатуры, закрытие по `Escape` и 5 мутаций.

---

### Эпик A6: Нативная проверка орфографии (Spellchecker)

#### Задача TASK-1006A: FFI-биндинг `libenchant-2` / `hunspell`
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-platform/src/spellcheck.rs`
  - `tests/unit/spellcheck-ffi-1006a.test.mjs`
- **Оракул:** Системные словари `/usr/share/hunspell/*.dic`.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест проверяет инициализацию словаря для активной локали (ru_RU / en_US).
  2. *Micro-Step 2 (Green):* Метод `check_word(word) -> bool` с фильтрацией пунктуации, URL, эмодзи и меншнов `@user`.
  3. *Micro-Step 3 (Green):* Метод `suggest(word) -> Vec<String>` с ограничением до 5 лучших вариантов замены.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (пропуск слов с дефисом, утечка памяти C-строк, сбой при смене локали).

#### Задача TASK-1006B: Рендеринг опечаток и контекстное меню в `MessageBar`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/chat/input_spellcheck.rs`
  - `tests/unit/input-spellcheck-1006b.test.mjs`
- **Оракул:** Стандарт отрисовки текстовых полей Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест проверяет расчет координат ошибочных подстрок с дебаунсом 150 мс.
  2. *Micro-Step 2 (Green):* Отрисовка красной волнистой линии цвета `#f23f43` под словами с ошибками в `gpui::TextView`.
  3. *Micro-Step 3 (Green):* Открытие нативного контекстного меню при ПКМ с выводом вариантов исправления и пункта «Добавить в словарь».
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (смещение линии при скролле текста, замена не того слова в строке).

---

## 2. Блок Б: Голос и звук (Voice Extensions)

### Эпик Б1: Серверная звуковая панель (Soundboard)

#### Задача TASK-1012A: Протокол Soundboard и кэширование звуков
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-voice/src/soundboard_protocol.rs`
  - `tests/unit/soundboard-protocol-1012a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/voice-deep.md` (Voice Gateway Opcode 18 `SOUNDBOARD_SOUND`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест формирования payload Opcode 18: `sound_id`, `volume` (0.0..1.0), `source_guild_id`.
  2. *Micro-Step 2 (Green):* Парсинг входящего события Voice Gateway Opcode 19 (`SPEAKING` с флагом soundboard).
  3. *Micro-Step 3 (Green):* Предзагрузка и локальное кэширование сэмплов с CDN в формате Opus 48 кГц stereo.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (выход volume за пределы 0.0..1.0, сбой парсинга SSRC источника, утечка буфера сэмпла).

#### Задача TASK-1012B: Микшер звуковых эффектов в исходящий RTP
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-voice/src/soundboard_mixer.rs`
  - `tests/unit/soundboard-mixer-1012b.test.mjs`
- **Оракул:** RFC 3550 и спецификация RTP кадрирования Opus.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест линейного сложения сэмплов микрофона и SFX с софт-клиппингом через функцию `tanh()`.
  2. *Micro-Step 2 (Green):* Сохранение четкости речи без клиппинга и артефактов при одновременном воспроизведении звука.
  3. *Micro-Step 3 (Green):* Автоматическая остановка микширования по окончании сэмпла без лишних циклов CPU.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (рассинхронизация 20 мс квантования, сдвиг фазы стерео, зацикливание сэмпла).

#### Задача TASK-1012C: Сетка кнопок саундборда `SoundboardGridView`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/voice/soundboard_view.rs`
  - `tests/unit/soundboard-view-1012c.test.mjs`
- **Оракул:** `/home/sanya/re-deep/voice/ui7/` (панель выбора звуковых эффектов).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Модальное окно саундборда: сетка кнопок с эмодзи звука, названием и индикатором хоткея.
  2. *Micro-Step 2 (Green):* Анимация нажатия и звуковой индикатор активности воспроизведения вокруг кнопки.
  3. *Micro-Step 3 (Green):* Ползунок общей громкости саундборда в настройках звука клиента.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (спам звуками без дебаунса, наложение хоткеев, потеря громкости).

---

### Эпик Б2: 3D Spatial Audio & HRTF позиционирование

#### Задача TASK-1017A: Бинауральный HRTF фильтр пространственного звука
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-voice/src/spatial_audio.rs`
  - `tests/unit/spatial-audio-filter-1017a.test.mjs`
- **Оракул:** Стандарт ISO Head-Related Transfer Function (HRTF) KEMAR dataset.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест расчета межушной разности времени (ITD до 0.7 мс) и разности уровней (ILD до 20 дБ) по азимуту $(-180^\circ .. +180^\circ)$.
  2. *Micro-Step 2 (Green):* Быстрое сверточное преобразование (FFT Convolver) моно-потока участника в стерео-сигнал.
  3. *Micro-Step 3 (Green):* Затухание громкости в зависимости от виртуальной дистанции $1/r$.
  4. *Micro-Step 4 (Perf & Mutation):* Потребление CPU $< 1.5\%$ на 10 участников и 5 контрольных мутаций.

#### Задача TASK-1017B: UI радара участников в голосовом канале
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/voice/spatial_radar.rs`
  - `tests/unit/spatial-radar-1017b.test.mjs`
- **Оракул:** Визуализация пространственного звука Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Круглый радар $360^\circ$ с аватаром слушателя в центре и аватарами участников на окружности.
  2. *Micro-Step 2 (Green):* Интерактивное перетаскивание аватаров мышью для индивидуальной расстановки в пространстве.
  3. *Micro-Step 3 (Green):* Сохранение координат расстановки участников в локальной базе SQLite.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (выход аватара за радиус радара, сброс координат при переподключении).

---

### Эпик Б3: Продвинутая WebRTC диагностика и RTC Internals (`RE-GAP-22`)

#### Задача TASK-1011A: Сборщик низкоуровневых RTCP метрик
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-voice/src/rtc_metrics.rs`
  - `tests/unit/rtc-metrics-1011a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест агрегации отчетов RTCP: `fraction_lost`, `cumulative_lost`, `rtt_ms`, `jitter_ms`, `audio_bitrate_kbps`.
  2. *Micro-Step 2 (Green):* Циклический буфер последних 120 измерений (кольцевой буфер 60 секунд с шагом 500 мс).
  3. *Micro-Step 3 (Green):* Мониторинг DAVE MLS: номер текущей эпохи, количество участников группы, статус верификации SAS.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ошибочный расчет скользящего среднего RTT, переполнение буфера, утечка DAVE ключа).

#### Задача TASK-1011B: 60 FPS экран графиков `RtcDiagnosticsHud`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/voice/rtc_hud.rs`
  - `tests/unit/rtc-hud-1011b.test.mjs`
- **Оракул:** Окно WebRTC Internals официального Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Окно HUD 640×480px, темный фон `#1e1f22`, 3 графика (RTT, Jitter, Bitrate).
  2. *Micro-Step 2 (Green):* Векторная отрисовка графиков на WGPU с частотой 60 кадров/с без просадки основного UI.
  3. *Micro-Step 3 (Green):* Всплывающие тултипы со точными значениями миллисекунд при наведении курсора мыши.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (масштабирование оси Y при пиках, зависание графика при паузе передачи).

---

### Эпик Б4: Автоматический перевод в AFK-канал при простое

#### Задача TASK-1010A: Системный детектор бездействия (D-Bus / Wayland)
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-platform/src/idle_monitor.rs`
  - `tests/unit/idle-monitor-1010a.test.mjs`
- **Оракул:** D-Bus `org.freedesktop.ScreenSaver` и Wayland протокол `ext-idle-notifier-v1`.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест получения миллисекунд бездействия клавиатуры/мыши через нативные интерфейсы.
  2. *Micro-Step 2 (Green):* Fallback между Wayland (`ext-idle-notifier-v1`) и X11 (`XScreenSaverQueryInfo`).
  3. *Micro-Step 3 (Green):* Событие `IdleStateChanged(is_idle: bool)` при превышении настраиваемого порога.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ложное срабатывание при воспроизведении видео, утечка D-Bus коннекта).

#### Задача TASK-1010B: Логика автоматического перемещения в AFK-канал
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/voice/afk_handler.rs`
  - `tests/unit/afk-handler-1010b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/gateway-schema.json` (Voice State Update Opcode 4).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест сверки времени бездействия с настройкой сервера `guild.afk_timeout` (1–60 минут).
  2. *Micro-Step 2 (Green):* Автоматическая отправка Gateway Opcode 4 на смену `channel_id` на `guild.afk_channel_id`.
  3. *Micro-Step 3 (Green):* Принудительное выставление флага `self_mute: true` при нахождении в AFK-канале.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (перевод в AFK при отсутствии afk_channel_id, повторный спам опкодом 4).

---

## 3. Блок В: Администрирование сервера и безопасность (Server Management)

### Эпик В1: Иерархия ролей и 64-битные права

#### Задача TASK-1007A: Модель ролей и валидатор 64-битной маски прав
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/roles.rs`
  - `tests/unit/roles-permissions-1007a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`GET /guilds/{id}/roles`, `PATCH /guilds/{id}/roles`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест 64-битных битовых флагов `Permissions`: `ADMINISTRATOR` ($1 \ll 3$), `MANAGE_CHANNELS` ($1 \ll 4$), `BAN_MEMBERS` ($1 \ll 2$) и т.д.
  2. *Micro-Step 2 (Green):* Валидатор прав: проверка невозможности назначить роль с правами, превышающими права текущего пользователя.
  3. *Micro-Step 3 (Green):* REST-метод `reorder_roles(positions)` с обновлением порядка ролей на сервере.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (эскалация прав обычного пользователя до админа, искажение битовой маски `u64`).

#### Задача TASK-1007B: Drag-and-drop интерфейс иерархии ролей
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/guild/roles_hierarchy.rs`
  - `tests/unit/roles-hierarchy-1007b.test.mjs`
- **Оракул:** Диалог настройки ролей Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Список ролей с цветными плашками, бейджами участников и иконкой перетаскивания.
  2. *Micro-Step 2 (Green):* Интерактивный Drag-and-Drop с визуальной линией вставки новой позиции роли.
  3. *Micro-Step 3 (Green):* Палитра выбора цвета роли (пресеты Discord + произвольный HEX-пикер).
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (попытка перетащить роль выше роли владельца сервера, рассинхронизация индекса).

#### Задача TASK-1007C: Интерактивная матрица разрешений роли
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/guild/permissions_matrix.rs`
  - `tests/unit/permissions-matrix-1007c.test.mjs`
- **Оракул:** Экран разрешений роли в Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Список из 35+ переключателей прав, сгруппированных по категориям (Общие, Текст, Голос, Расширенные).
  2. *Micro-Step 2 (Green):* Предупреждающий баннер при включении опасных прав (`ADMINISTRATOR`, `MENTION_EVERYONE`).
  3. *Micro-Step 3 (Green):* Кнопка «Сохранить изменения» с индикатором несохраненных правок внизу экрана.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (сохранение без изменений, сброс переключателей при смене вкладки).

---

### Эпик В2: Журнал аудита сервера (Server Audit Log)

#### Задача TASK-1008A: Парсер журнала аудита и декодер изменений
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/audit_log.rs`
  - `tests/unit/audit-log-parser-1008a.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`GET /guilds/{id}/audit-logs`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Парсинг типов действий `AuditLogActionType` (1..145: Guild Update, Channel Create, Member Kick/Ban, Role Update).
  2. *Micro-Step 2 (Green):* Декодер диффов `AuditLogChange`: преобразование `old_value` и `new_value` в человекочитаемые строки.
  3. *Micro-Step 3 (Green):* Сопоставление `user_id` инициатора действия и `target_id` цели из включенного объекта `users[]`.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (неизвестный action_type, пропуск null-полей в diff, искажение ID модератора).

#### Задача TASK-1008B: Фильтруемый UI-список `AuditLogViewer`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/guild/audit_log_view.rs`
  - `tests/unit/audit-log-view-1008b.test.mjs`
- **Оракул:** Экран Audit Log в настройках сервера Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Список записей: иконка типа действия (зеленая/красная/желтая), имя модератора, описание события, дата.
  2. *Micro-Step 2 (Green):* Выпадающие фильтры: «Фильтр по действию» и «Фильтр по пользователю».
  3. *Micro-Step 3 (Green):* Раскрытие подробностей записи по клику (точный diff измененных полей).
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (сбой фильтрации, падение при пустом списке аудита).

---

### Эпик В3: Двухфакторная аутентификация TOTP 2FA и сессии

#### Задача TASK-1009A: Модальное окно TOTP 2FA `TotpDialog`
- **Отдел:** `G2 Frontend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/discord-gpui/src/auth/totp_view.rs`
  - `tests/unit/totp-view-1009a.test.mjs`
- **Оракул:** Экран ввода 2FA кода в Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Модальное окно с 6 отдельными квадратными полями ввода цифр цвета `#1e1f22`.
  2. *Micro-Step 2 (Green):* Автоматический переход фокуса на следующее поле при вводе цифры и возврат по `Backspace`.
  3. *Micro-Step 3 (Green):* Поддержка вставки 6-значного кода из буфера обмена целиком (`Ctrl+V`).
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ввод буквенных символов, отправка недозаполненного кода < 6 цифр).

#### Задача TASK-1009B: Менеджер сессий и принудительный разлогин
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-core/src/sessions_manager.rs`
  - `tests/unit/sessions-manager-1009b.test.mjs`
- **Оракул:** `/home/sanya/re-deep/rest-d02.json` (`GET /users/@me/auth/sessions`, `DELETE ...`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Запрос списка сессий: ID, ОС, браузер, IP-адрес, примерная локация, время последнего входа.
  2. *Micro-Step 2 (Green):* Выделение текущей сессии зеленым бейджем «Это устройство».
  3. *Micro-Step 3 (Green):* Кнопка «Завершить сессию» с отправкой `DELETE` и мгновенным удалением из списка.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (попытка удалить чужую сессию без прав, потеря статуса текущей сессии).

---

### Эпик В4: Шифрование локальной базы сообщений (SQLite Storage Encryption)

#### Задача TASK-1021A: Деривация ключа шифрования из Linux Keyring
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-store/src/keyring_crypto.rs`
  - `tests/unit/keyring-crypto-1021a.test.mjs`
- **Оракул:** Linux Secret Service API (`org.freedesktop.secrets`).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест генерации криптографически стойкого 256-битного мастер-ключа и сохранения в Keyring.
  2. *Micro-Step 2 (Green):* Деривация рабочего ключа БД через Argon2id с солью устройства.
  3. *Micro-Step 3 (Green):* Очистка памяти секретов через `zeroize` при завершении работы.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (логирование открытого ключа, слабый fallback-ключ, утечка дескриптора памяти).

#### Задача TASK-1021B: Прозрачное шифрование страниц SQLite (AES-256-GCM)
- **Отдел:** `G1 Backend` | **Приоритет:** `P1`
- **Файлы:**
  - `rust/crates/discord-store/src/encrypted_sqlite.rs`
  - `tests/unit/encrypted-sqlite-1021b.test.mjs`
- **Оракул:** Архитектура страниц SQLite VFS / SQLCipher.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест проверки зашифрованного файла SQLite на диске (отсутствие открытых строк сообщений в hex-дампе).
  2. *Micro-Step 2 (Green):* Страничное шифрование страниц 4096 байт через AES-256-GCM с уникальным IV на страницу.
  3. *Micro-Step 3 (Green):* Проверка целостности WAL-журнала и предотвращение повреждения базы при внезапном сбое питания.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (переиспользование IV, повреждение тега аутентификации GCM, утечка в WAL).

---

## 4. Блок Г: Платформа и внешние интеграции (Ecosystem & Cross-Platform)

### Эпик Г1: Интеграция со Spotify (Listen Along & Presence)

#### Задача TASK-1013A: D-Bus MPRIS клиент и Gateway статус
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-platform/src/spotify_mpris.rs`
  - `tests/unit/spotify-mpris-1013a.test.mjs`
- **Оракул:** Спецификация D-Bus `org.mpris.MediaPlayer2` и Gateway Opcode 3 `STATUS_UPDATE` (activity type 2).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Подписка на сигналы `PropertiesChanged` интерфейса Spotify MPRIS.
  2. *Micro-Step 2 (Green):* Извлечение метаданных: название трека, исполнители, альбом, `mpris:length`, позиция воспроизведения.
  3. *Micro-Step 3 (Green):* Отправка активности `type: 2` (`LISTENING`) в шлюз Discord с таймстемпами начала и конца.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (рассинхронизация таймстемпов, зависание статуса при закрытии Spotify).

#### Задача TASK-1013B: Виджет Spotify в профиле и синхронное прослушивание
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/profile/spotify_widget.rs`
  - `tests/unit/spotify-widget-1013b.test.mjs`
- **Оракул:** Виджет прослушивания Spotify в карточке профиля Discord.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Плашка в профиле: зеленая иконка Spotify `#1db954`, обложка альбома, название трека, бегущая строка.
  2. *Micro-Step 2 (Green):* Интерактивный таймлайн трека с плавным движением ползунка в реальном времени.
  3. *Micro-Step 3 (Green):* Кнопка «Слушать вместе» (Listen Along) с вызовом команды воспроизведения в локальный Spotify.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (зависание таймлайна при паузе, рассинхронизация времени более 2 сек).

---

### Эпик Г2: Режим Steam Deck & Gamepad UI

#### Задача TASK-1014A: Драйвер геймпада на базе `gilrs`
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-platform/src/gamepad_driver.rs`
  - `tests/unit/gamepad-driver-1014a.test.mjs`
- **Оракул:** `docs/feature-backlog.md` (карточка `R6-01` Steam Deck Game Mode).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Инициализация библиотеки `gilrs` и фильтрация событий осей и кнопок геймпада.
  2. *Micro-Step 2 (Green):* Преобразование нажатий D-pad (крестовины) в логические события навигации `NavUp/Down/Left/Right`.
  3. *Micro-Step 3 (Green):* Преобразование бамперов L1/R1 в переключение серверов, триггеров L2/R2 — в переключение каналов.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (дрифт стика без deadzone, дребезг кнопок, зависание события при отключении геймпада).

#### Задача TASK-1014B: Фокусная сетка интерфейса GPUI и экранная клавиатура
- **Отдел:** `G2 Frontend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/input/gamepad_focus.rs`
  - `tests/unit/gamepad-focus-1014b.test.mjs`
- **Оракул:** Стандарт интерфейса Steam Deck / Big Picture.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Подсветка текущего элемента в фокусе яркой неоновой рамкой цвета `#5865f2`.
  2. *Micro-Step 2 (Green):* Автоматический скролл списков чата и каналов при перемещении фокуса геймпада.
  3. *Micro-Step 3 (Green):* Вызов экранной клавиатуры SteamOS через D-Bus `org.kde.kwin.VirtualKeyboard` при фокусе поля ввода.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (потеря фокуса в пустоту, застревание в модальном окне).

---

### Эпик Г3: Автозапуск с системой (XDG Autostart & Start Minimized)

#### Задача TASK-1015A: Генератор XDG Autostart desktop-файла
- **Отдел:** `DevOps / Integration` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/crates/discord-platform/src/autostart_xdg.rs`
  - `tests/unit/autostart-xdg-1015a.test.mjs`
- **Оракул:** Спецификация FreeDesktop XDG Autostart.
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест создания файла `~/.config/autostart/discord-gpui.desktop` с правами 0644.
  2. *Micro-Step 2 (Green):* Валидация полей: `Type=Application`, `Exec=/usr/bin/discord-gpui --start-minimized`, `X-GNOME-Autostart-enabled=true`.
  3. *Micro-Step 3 (Green):* Удаление файла при выключении автозапуска в настройках клиента.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ошибочный путь бинарника, повреждение синтаксиса desktop-файла).

#### Задача TASK-1015B: Режим тихого старта свёрнутым в трей
- **Отдел:** `G1 Backend` | **Приоритет:** `P2`
- **Файлы:**
  - `rust/discord-gpui/src/platform/minimized_launch.rs`
  - `tests/unit/minimized-launch-1015b.test.mjs`
- **Оракул:** `docs/feature-backlog.md` (карточка `R4-02` Трей и скрытый старт).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Проверка флага `--start-minimized` в аргументах CLI (`std::env::args()`).
  2. *Micro-Step 2 (Green):* Пропуск вызова `window.open()` и инициализации WGPU surface до первого клика по трею.
  3. *Micro-Step 3 (Green):* Экономия памяти при старте свёрнутым: VmRSS $< 8$ МБ до отображения графического окна.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (случайное мигание окна при старте, зависание трея при вызове).

---

### Эпик Г4: Кросс-платформенные сборки (Windows & macOS)

#### Задача TASK-1016A: Спецификация WiX Toolset для Windows (MSI & Portable)
- **Отдел:** `DevOps / Integration` | **Приоритет:** `P2`
- **Файлы:**
  - `packaging/windows/wix.xml`
  - `scripts/build-windows-msi.sh`
  - `tests/unit/windows-packaging-1016a.test.mjs`
- **Оракул:** `docs/feature-backlog.md` (карточка `R6-03` Windows специфика).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Валидация XML-манифеста WiX: GUID продукта, пути к бинарнику, ярлыкам в меню Пуск.
  2. *Micro-Step 2 (Green):* Конфигурация переносимого Portable-режима (сохранение профиля в папку `data/` рядом с `exe`).
  3. *Micro-Step 3 (Green):* Интеграция оверлей-иконок бейджей непрочитанных сообщений в таскбаре Windows.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (ошибочный UpgradeCode, запись в реестр в portable-режиме).

#### Задача TASK-1016B: Сборка macOS `.dmg` и поддержка Vibrancy
- **Отдел:** `DevOps / Integration` | **Приоритет:** `P2`
- **Файлы:**
  - `packaging/macos/build-dmg.sh`
  - `rust/discord-gpui/src/platform/macos_vibrancy.rs`
  - `tests/unit/macos-packaging-1016b.test.mjs`
- **Оракул:** `docs/feature-backlog.md` (карточка `R6-04` macOS специфика).
- **Микро-план реализации (Micro-TDD):**
  1. *Micro-Step 1 (Red):* Тест структуры бандла `DiscordGPUI.app` (`Contents/MacOS/`, `Contents/Resources/`, `Info.plist`).
  2. *Micro-Step 2 (Green):* Скрипт сборки сжатого `.dmg` с фоновым изображением и символической ссылкой на `/Applications`.
  3. *Micro-Step 3 (Green):* Поддержка нативного эффекта размытия фона окна `NSVisualEffectView` Material Dark HUD.
  4. *Micro-Step 4 (Mutation 5/5):* 5 мутаций (битый Info.plist, отсутствие x86_64/arm64 универсального бинарника).
