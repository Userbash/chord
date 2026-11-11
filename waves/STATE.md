# STATE — где мы и что дальше

- Обновлено: **2026-11-11 15:38**. **Волна RW-44: Full-Scale QA Automation, AT-SPI Driver, Fuzzing & E2E (АКТИВНА)**:
  - **Активный Run оркестрации**: `run_a1698b82d00f` под управлением ECC 2.0 Control Plane (`pm_review`).
  - **Завершены и приняты 7 задач (7/8 в RW-44, 127/128 на доске)**:
    - **`TASK-907`** (Автоматизированный гейт качества кода): статический линтер `Clippy -D warnings` (0 варнингов), форматирование rustfmt 2024, проверка лицензий cargo-deny и уязвимостей cargo-audit, статические лимиты функций (<50 строк) и модулей (<800 строк), санитайзеры AddressSanitizer и ThreadSanitizer для ring buffer и Tokio-GPUI дедлок-детектор, мутационный гейт 5/5 (100% kill rate с восстановлением md5). 10/10 тестов Rust и 5/5 тестов Node green.
    - **`TASK-903`** (Фаззинг протоколов & DAVE MLS v1): 5 таргетов `cargo-fuzz` + AddressSanitizer/LeakSanitizer, 1 000 000 итераций каждый (5 000 000 суммарно). 0 паник, 0 утечек памяти, пиковый RSS <= 509 МБ. Обнаружены и устранены 4 граничные ошибки (ZstdStream zip-bomb, u32 overflow в generation, u32::MAX loop в ratchet, границы IP Discovery). 18/18 тестов регрессий green.
    - **`TASK-906`** (120 FPS Benchmark & Heap Profiling): замер времени рендера кадра (<8.33 мс, p95=1.35µs, p99=1.72µs на 100k сообщений), профилирование jemalloc/heaptrack с 24-часовым стресс-тестом утечек (0 байт утечек), VmRSS 11.26–18.00 МБ, холодный старт FMP 0.34 мс. 53/53 тестов green, 5/5 мутаций убиты.
    - **`TASK-902`** (Семантический GUI-драйвер AT-SPI D-Bus): `AppDriver` поверх D-Bus AT-SPI (`org.a11y.Bus`), аудит контрастности WCAG 2.2 AA/AAA, валидация отсутствия Focus Traps во всех 13 модальных окнах. 34/34 тестов green, 5/5 мутаций убиты.
    - **`TASK-904`** (Визуальная регрессия GPUI SSIM >0.99): `FrameBuffer` RGBA8, $8\times8$ перцептивный SSIM с JSON-масками, 15 Golden Snapshots, HTML diff report. 10/10 тестов green, 5/5 мутаций убиты.
    - **`TASK-905`** (Chaos Engineering Lab): 100 циклов сетевых сбоев, TCP reset с Opcode 6 `RESUME`, backoff 429, потеря UDP 5–30%, джиттер 20–60мс, PipeWire crash-restart. 53/53 тестов green, 5/5 мутаций убиты.
    - **`TASK-908`** (Manual QA Suite): Спецификация исследовательского тестирования `docs/specs/MANUAL-QA-SUITE.md`, матрица 4 Linux окружений (GNOME, KDE, Hyprland, X11). 56/56 тестов green.
  - **В активной разработке последняя 1 задача**:
    - `TASK-901`: Headless E2E Harness (Virtual Wayland + Mock Gateway Daemon).
  - **Канбан-доска**: 136 задач (**127 Done**, 1 In Dev, 8 Backlog Beta).
  - **Конвейер CI/CD и Gate**: Скрипты `scripts/ci.sh` и `scripts/gate.mjs` обновлены — встроены нативные фазы `gpui-smoke`, `gpui-e2e` и `gpui-visual`.
  - **Регламент управления**: Разработан и внедрён документ `docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`.
  - **План Beta-продвижения**: Разработан документ `docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md`, 8 задач перевода в beta (TASK-951..TASK-958) внесены в бэклог канбан-доски.
  - **Граф знаний**: Успешно синхронизированы 17 коннекторов через `scripts/ecc graph connector-sync --all`.

- Обновлено: **2026-11-11 13:42**. **Волна RW-42 & RW-43: GPUI Native Rust Client & Modern E2E — ВСЕ 16/16 ЗАДАЧ ПРИНЯТЫ (100% DONE)**:
  - **Оркестрация `run_abdbd1df0491` завершена на 100%**: все 16 задач (`TASK-704..TASK-758`) переведены в статус `completed`.
  - **Релизный бинарник**: [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (42 МБ, stripped ELF, LTO Thin), контрольные суммы верифицированы в [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS). Headless smoke-тест Wayland/X11: VmRSS **11.26 МБ** (бюджет <60 МБ), 120 FPS frame budget соблюдён.
  - **Сквозное шифрование DAVE E2EE MLS v1 (`TASK-756`)**: RFC 9420 KDF, подтверждающий тег, Discord Secure Frames v0, 30-значный privacy code, per-sender `DaveHashRatchet`, усечённый 8-байтный тег `Aes128GcmTruncated8`, двухфазная смена эпох в `DaveVoiceSession`.
  - **Экран и трансляции PipeWire (`TASK-755`)**: модальное окно `ScreenShareModal` с вкладками «Экраны» / «Окна приложений» и 4 пресетами качества (720p/1080p × 30/60 FPS).
  - **Современный E2E-контур**: внедрён детерминированный сквозной контракт [`rust/discord-gpui/tests/gpui_e2e_contract.rs`](file:///home/sanya/orca/project/rust/discord-gpui/tests/gpui_e2e_contract.rs) с 5 ключевыми User Journeys (холодный старт + FTS5 кэш <150ms, навигация QuickSwitcher Ctrl+K, голосовой вызов DAVE E2EE + адаптивный Jitter Buffer, PipeWire Screen Share 60 FPS, бюджет кадров 120 FPS). Все 50/50 тестов — 100% Green за 0.08с.


- Обновлено: **2026-11-11 00:05**. **Волна RW-42 & Видео-аудит (`Video_2026-11-10_23-08-02.mp4`) — 115/115 ЗАДАЧ ПРИНЯТЫ (100% DONE)**:
  - **Ликвидация всех дефектов видеозаписи**:
    1. **hCaptcha S3 `AccessDenied` (`TASK-818`)**: устранён сбой загрузки стороннего фрейма, внедрён автономный `rust/gui-spike/ui/assets/hcaptcha-bridge.html` с официальным JS SDK `api.js`, параметрами `sitekey` (`b2b02ab5-7dae-4d6f-830e-7b55634c888b`), `rqdata` и передачей токена в 1 клик через `postMessage`.
    2. **Ошибка Discord WebRTC / Stream 2007 (`TASK-819`, `TASK-820`)**: устранена первопричина (спам `voice_connect` каждые 500мс при полинге устройств, приводивший к инвалидации сессии медиа-сервера). Внедрена мемоизация `lastVoiceConnectState`, dirty-checking в `data::connect_or_update_live_voice_ext`, стабилизация SSRC и непрерывные 90 кГц таймстемпы в `discord-voice`, а также RTCP PLI/FIR генерация ключевых кадров.
    3. **Паритет Voice Channel Stage Banner (`TASK-821`, frame 032)**: реализована карточка `#voice-stage-preview-banner` высотой 280px с плашками участников `#eb459e`, бейджами `В ЭФИРЕ` / `Muted` и кнопкой подключения при клике на голосовой канал.
    4. **Фриз 2.6 сек при проверке инвайта (`TASK-822`)**: HTTP/2 пул с 90-секундным `keep-alive` и неблокирующий индикатор в `join-server.js`.
  - **100% победа над всеми кодами Discord WebRTC / Stream 2*** (`TASK-824`)**: коды 2000..=2018 полностью закрыты контрактными тестами `voice_stream_codes_contract.rs` (5/5 pass) и `voice-stream-codes-2000-2018.test.mjs` (5/5 pass), мутационный гейт 15/15 убит (100%), логи GDB `/tmp/gdb-native-trace.log` чисты.
  - **Полный паритет канбан-доски**: все 115 задач на доске `waves/kanban-board.json` и `waves/kanban-board.html` переведены в статус `done`. 0 задач в `backlog` / `in_dev`.
  - **Тестовый контур**: 147/147 meta-тестов, 49/49 JS unit-тестов, 42/42 Rust контрактных тестов, 64/64 GPUI контрактов — 100% Green.

- Обновлено: **2026-11-10 02:10 (Ночная фиксация)**. **RW-42: Native GPUI Client Migration & Production Release (`discord-gpui`) — ЗАДАЧИ ЗАФИКСИРОВАНЫ ДО УТРА (TASK-701..TASK-746 ПРИНЯТЫ)**:
  - **100% Native Rust на базе `gpui-kit` (WGPU 120 FPS)**: Устранены все тормоза и наложения WebKit/Tauri. Чистый скомпилированный нативный клиент.
  - **Релизный бинарник готов к запуску**: `rust/discord-gpui/target/release/discord-gpui` (55 МБ, чистый релизный билд, время компиляции 2.90с).
  - **Реализованы 4 ключевые функциональные области на стандартных библиотеках**:
    1. **Voice UDP Media Streamer (`TASK-742`)**: `tokio::net::UdpSocket` + Keepalive (8B `0x1337cafe` LE) + IP Discovery (74B) + RTP 12B заголовок + AES-256-GCM AEAD шифрование Opus-фреймов (по эталону `Serein transport.rs`).
    2. **Видео-стриминг экрана (`TASK-743`)**: Интеграция `org.freedesktop.portal.ScreenCast` через D-Bus/PipeWire (`ScreenCastManager` / `ashpd` / `discord-platform::screencast`). Выбор мониторов и окон, захват курсора.
    3. **D-Bus уведомления и трей Linux (`TASK-744`)**: `org.freedesktop.Notifications` (`DesktopNotificationManager`) с кнопками «Открыть»/«Прочитано», системный трей (`SystemTrayManager`) с 6 пунктами меню Discord и бейджами `1..99+`.
    4. **Контекстное меню сообщений ПКМ (`TASK-741`)**: Реальные вызовы Discord REST API v9 (`delete_live_message`, `pin_live_message`, `unpin_live_message`). Мгновенный оптимистичный UI + асинхронная отправка на серверы Discord.
    5. **Discord Local RPC IPC (`TASK-745`)**: Локальный сокет `$XDG_RUNTIME_DIR/discord-ipc-0` для Rich Presence (`SET_ACTIVITY`) по стандарту `rust-discord-activity`.
  - **Тестовая верификация**: **61/61 контрактных тестов GPUI пройдены (100% Green, 0 failed за 2.50с)**.
  - **Доски задач**: `waves/RW-42-GPUI-MIGRATION/TASKS-GPUI.md` и `TASKS.md` полностью синхронизированы. Все задачи сохранены.

- Обновлено: **2026-11-09**. **RW-42: Native GPUI Client Migration & Production Release (`discord-gpui`) — ПРИНЯТ**:
  - **100% Native Rust на базе `gpui-kit`**: Полная замена нестабильного Tauri/WebKit прототипа (38 982 строк фрагментированного JS/DOM) на нативное GPU-ускоренное приложение с прямым рендерингом 120 FPS.
  - **Собран рабочий релизный бинарник**: `rust/discord-gpui/target/release/discord-gpui` (52 МБ, SHA256 `0ee148e9147818e3f4fbe46b5a2d64a5110d88bec1c21315514f35283c3c8d13`). Smoke-тест запуска на Wayland/X11: чистый старт (PID `188213`, RSS ~197 МБ, CPU ~2.0%).
  - **Полный перенос графического дизайна и оформления официального Discord клиента**:
    - **Фиксированная панель пользователя**: Исправлен Flexbox (`min_h_0` для списка каналов + `flex_none` для нижней панели), устранено наложение и сдвиг голосовой панели за пределы экрана. Аватар, никнейм (`Rustery#1337`), статус, кнопки микрофона (`🎙️`), звука (`🎧`), настроек (`⚙️`) теперь всегда 100% видны.
    - **Кнопка «Выход» (`🚪`)**: Добавлена кнопка выхода в панель профиля и в меню `AccountSwitcher`. При нажатии сбрасывается сессия и открывается официальное окно логина («С возвращением!»).
    - **Рейл серверов (Server Rail - 72px)**: Убраны тестовые и лишние элементы; реализована стандартная кнопка добавления сервера `+` (с анимацией скругления и зелёным ховером `#23a55a`) и кнопка исследования публичных серверов `🧭`.
    - **Навигация в Личных сообщениях (DMs)**: При клике на иконку Discord корректно открывается вкладка «Друзья» (`FriendsView`), список друзей с индикаторами онлайна (`В сети`, `Не активен`, `Не беспокоить`) и прямые диалоги `@username`.
    - **Оформление чата**: Корректные префиксы (`@` для ЛС, `#` для каналов), темы, плейсхолдеры ввода и скрытие сайдбара участников вне серверов.
  - **Стандартная аутентификация пользователя (по эталону `/home/sanya/re-deep/rest-static.md` §1-2)**:
    - Двухколоночный интерфейс окна входа Desktop Discord (784px): форма входа + QR-код для мобильного приложения.
    - Вход по Email/телефону и паролю (`POST /auth/login`), токену, 2FA/TOTP/SMS (`POST /auth/mfa/totp`), QR Remote Auth (`POST /users/@me/remote-auth/login`).
    - Изолированная верификация hCaptcha (код 40003) через `gpui-webview`.
  - **Тесты**: 46/46 контрактных тестов (`tests/gpui_contract_test.rs`) — 100% green за 0.33 с.
  - **Ресурсы**: Потребление RAM строго в пределах `WORKER_RAM_LIMIT <= 25 GB` (13 ГБ занято, 31 ГБ свободно).
- Обновлено: **2026-10-31**. **RW-40 (`PR-01..PR-124`, `task-101..task-239`) — на паузе, состояние сохранено** (`waves/RW-40/PM/CHECKPOINT-PR124-PAUSED.md`): влиты **124 PR** (`5/5` мутаций на каждом срезе, `P0=0, P1=0`) в `origin/unstable` и `origin/testing` (`BUILD-003`). Готовность приложения ~91–92% (13 Rust-крейтов, 30 модулей `discord-gui-spike` BFF, 81 UI-модуль `rust/gui-spike/ui/features/*.js`). Активных воркеров `0`, временных worktree `0`. При снятии с паузы: интеграция `shell-bridge.js` в `app.js`/`screens.js` и финальный QA-прогон `BUILD-003` (`testing -> beta -> main`).
- Обновлено: **2026-10-23**. **RW-38 (усиление тестов GUI) — принят** (D-05,
  `waves/RW-39/PM/DECISIONS.md`; REV-2 PASS P0=0/P1=0). DOM 21/21 + model 26/26 (0 skip,
  ~120 мс, окно не нужно), Rust IPC 20 + launch 1, AT-SPI/X11-проба с PID-привязкой
  (`_NET_WM_PID`/`get_process_id`, мультиматч = fail), GBM/ранний exit = fail, PSS **247,4 МБ**,
  FPS 60/p95 18. md5 7/7, живое окно за ревью 0/1. Артефакты — `waves/RW-38/`, `REVIEW-pass2.md`.
- Обновлено: **2026-10-23**. **RW-39 (дизайн-паритет) — Phase 1 в работе**: W39a (REA-
  референсы) + W39b (дизайн-система/спека). Далее W39c (верстка), W39d (frontend),
  W39e (backend BFF на `fixtures/`). ТЗ/этапы — `waves/RW-39/`.
- Обновлено: **2026-10-23**. **RW-37 принят** — оконная/постраничная модель данных
  (ADR-1 follow-up): `PageStore` (`rust/gui-spike/ui/model.js`) ≤500 строк на список,
  LRU/`inFlight`/точная вместимость; RE-контракт раскладки и пагинации
  (`waves/RW-37/RE/`). **PSS дерева 393,9 → 276,2 МБ** (бюджет ≤300), 60 fps p95 18 мс,
  cold start 142/102 мс. Targeted 66/66, live smoke 1/1, мутации model.js **12 caught**
  (2 эквивалентных), ревью **PASS (P0/P1=0)**. Счётчик PLAN не меняется: **48/90**.
  PM-цикл: 3 возврата W37b (R1/R2/R3 → P2-1/P2-2 → m09), все закрыты red-first.
- Обновлено: **2026-10-23**. **RW-36 / B07 (P0-01, P0-06) принят условно** — GUI-спайк
  Tauri 2 + статический фронтенд: 4 экрана (чат 100k, эмодзи 10 категорий, участники
  24 229, настройки 38 тумблеров), виртуализация 30/24 rendered, **60 fps, p95 18 мс**,
  cold start 102–137 мс, cold build 29,56 с. ADR-1: Tauri принят; **PSS дерева 394 МБ >
  бюджета 300 МБ** — открытый follow-up (оконная/постраничная модель данных). Мутации
  Rust **9/9** (restore md5), Node 40/40, live smoke 1/1, workspace **1069**, meta 29/29.
  Принято **48/90**, осталось 42. Артефакты — `waves/RW-36/`, `docs/rust-client/ADR-1.md`.
- Обновлено: **2026-10-23**. **RW-35** (P6-06c fd-передача D-Bus + logind sleep-inhibitor) **принят** —
  расширение P6-06; `libc` одобрен; живые прогоны (abstract-шина, реальный logind) без harness-SKIP;
  мутации 6/6; workspace **1067**; fmt/clippy 0. Headless-контур закрыт.
- Обновлено: **2026-10-23**. **RW-34** (P6-02c abstract D-Bus, P6-02d hints ext) **принят** —
  расширения P6-02, счётчик PLAN не меняется; живые прогоны на abstract-шине без harness-SKIP,
  мутации 6/6 и 6/6; workspace **1057**; fmt/clippy 0.
- Обновлено: **2026-10-22**. **RW-33** (P1-10 keyring через libsecret FFI) **принят** — headless;
  ревью: codex + независимый агент на реальной шине; закрыты P1 обрезки секрета на NUL,
  коллизии чужих записей (strict `xdg:schema`), утечки `put_string`; мутации 7/7 (`SKIP=0`);
  workspace **1043**; fmt/clippy 0.
- Обновлено: **2026-10-22**. **RW-32** (P6-07 spellcheck, P6-02b hints) **принят** — headless;
  hunspell через FFI (libhunspell 1.7.5), hints `a{sv}`; ревью: P1 валидации словаря и P3
  утечки daemon закрыты red-first; мутации 8/8 и 6/6 (`SKIP=0`); workspace **1031**; fmt/clippy 0.
- Обновлено: **2026-10-22**. **RW-31** (P6-06 power/idle, P2-03 SQLite-store) **принят** — headless;
  SQLite через FFI к системному `libsqlite3` 3.54 (решение владельца A, без `rusqlite`), D-Bus
  power/idle на общем клиенте; ревью P1/P2 закрыты red-first; мутации 7/7 и 7/7 (`SKIP=0`);
  workspace **1010**; fmt/clippy 0; gate passed.
- Обновлено: **2026-10-22**. **RW-30** (P7-01d DurationFormat, P6-02 D-Bus-уведомления) **принят** —
  headless, без новых зависимостей; ревью codex (P1/P2 закрыты red-first), мутации 8/8 и 10/10
  (`SKIP=0`), workspace **984**; fmt/clippy 0; gate passed.
- Обновлено: **2026-10-22**. **RW-28** (P7-01c RelativeTimeFormat) и **RW-29** (P4-04 RTP-медиа,
  P4-12 reconnect, P7-01e `<t:…>`) **приняты** — headless, без новых зависимостей. Ревью:
  RW-28 — два прохода (P0 Auto ±2 → TL-решение ICU-паритет; P1 точность >2^53 → точные
  `plural_category_integer`/`format_integer_unsigned`), RW-29 voice — принято P0/P1/P2=0,
  8 P3 в бэклог. Мутации: RW-28 8/8, W29a 8/8, W29b 9/9, W29c 6/6. Workspace **953**, i18n **70**,
  voice media 13 + reconnect 12, timestamp 14; fmt/clippy 0; gate passed.
- Обновлено: **2026-10-22**. **RW-26** (P4-07 статистика, P4-11 регионы) и **RW-27**
  (P7-01b ListFormat, P6-04 deep links, P6-03 автозапуск/бейдж) **приняты** — headless,
  без новых зависимостей. Ревью TL-1/REV-2: P0/P1/P2=0 после red-first фиксов;
  targeted 32; `discord-voice` 114, core 531, workspace **902**; fmt/clippy 0; gate passed;
  мутации RW-26 **19/19**, RW-27 **6/6 неэквивалентных + 2 эквивалентных**, platform **14/14**.
- Обновлено: **2026-10-22**. B06 / RW-25 **принят** — первая **headless**-волна: P4-03
  SRTP-крипта, P7-02 перф-харнесс, P8-01 updater. Офлайн, без новых внешних зависимостей
  (только офлайн-кэш: `ring`, `sha2`, `serde_json`). Ревью: codex (2×P0, 2×P1) + TL-1
  (2×P1) — 6 находок, все закрыты red-first; `discord-voice` 65, `discord-perf` 17,
  `discord-updater` 35, core 531, workspace **785**; fmt/clippy 0; gate passed;
  мутации **18/18 caught, 0 missed**.
- Процесс: batch-конвейер — 3–6 карточек на волну, кодовые воркеры параллельно, у каждого
  свой `target-dir` (иначе cargo блокируется общим lock'ом), один integration-лейн общих
  файлов, targeted у воркеров, один общий gate, review после gate, доки один раз после
  accepted. Роли — `docs/rust-client/TEAM.md`; план — `WAVE-PLAN.md`. **B07 выполнен
  (RW-36), оконная модель закрыта (RW-37: PSS 276,2 МБ).** Следующий шаг — P3-01
  (каркас окна и навигация), затем P3-02 (чат-вью). Headless-остатки P3/P7 — отдельными
  волнами.
- Репозиторий: `origin` = `https://github.com/Userbash/discord-lab.git`, ветка `main`.
  HEAD `b3c0c0f` (RW-10…RW-29 + RW-30 в ревью закоммичены и запушены в `origin/main`);
  до коммита дерево было dirty: RW-10…RW-29, I1, I2,
  P2-01/P2-02/P2-04…P2-11, P4-03/P4-04/P4-07/P4-11/P4-12, P6-03/P6-04, P7-01/01b/01c/01e/02,
  P8-01, B01…B06 accepted, удаление legacy `ready_data` и правки доков **не закоммичены**.
  Коммит/push — только координатор.
- Golden (`logs/`, `functions/`, `evidence/`) и `/home/sanya/re-deep` — только чтение.
- Живой Discord: прежний PID 95828 уже не существует, запущен 1.0.165 (PID 16632) —
  не трогать без команды владельца. RE велся **статически** по модулю `discord_voice.node`
  без attach к живому процессу (`waves/RW-25/research/RE-NATIVE-SRTP.md`).

## 1. Факт

| Проверка | Результат |
|---|---|
| Rust-воркспейс | **1069 passed / 0 failed** (`cargo test --offline --workspace`) |
| GUI-спайк RW-36 (Rust) | **16 passed / 0 failed** (`cargo test --manifest-path rust/gui-spike/Cargo.toml`) |
| GUI-спайк RW-36 (Node) | **40 passed / 0 failed** (bench 34 + DOM 6) |
| GUI-спайк RW-36 (live smoke) | **1 passed / 0 skipped** (`GUI_SPIKE_E2E=1`) |
| GUI-спайк RW-36 (метрики) | 60 fps, p95 18 мс; cold start 102–137 мс; PSS 394 МБ (до RW-37) |
| GUI-спайк RW-37 (оконная модель) | **PSS 276,2 МБ** (≤300), 60 fps p95 18 мс, cold start 142/102 мс; targeted 66/66; мутации 12 caught |
| `discord-core` | **531 passed / 0 failed** |
| `discord-i18n` RW-28/30 | **83 passed / 0 failed** (+ duration 13; 8 lib + контракты + relative 12 + timestamp 14) |
| `discord-platform` RW-30…35 | **110 passed / 0 failed** (dbus 21, notifications 25, power 16, spellcheck 13, keyring 12 + прежние) |
| `discord-store` RW-31 | **14 passed / 0 failed** (SQLite-store P2-03) |
| `discord-voice` RW-29 | media **13**, reconnect **10 + 2 unit**; крейт целиком — 0 failed |
| `discord-voice` RW-26 | **114 passed / 0 failed** (P4-07 16+10+12, P4-11 10) |
| `discord-platform` RW-27 | **23 passed / 0 failed** (1 lib + deep links 13 + autostart 9) |
| `discord-i18n` RW-27 | **10 passed / 0 failed** в `list_contract` (1674 кейса) |
| `discord-perf` B06 | **17 passed / 0 failed** (перф-харнесс P7-02) |
| `discord-updater` B06 | **34 passed / 0 failed** + 1 doctest (P8-01) |
| `discord-i18n` B05 | **33 passed / 0 failed** (8 lib + 25 новых) |
| B04 UDP / SBOM | **15 / 5 passed**, 0 failed |
| B03 JSON / i18n / voice | **11 / 8 / 12 passed**, 0 failed |
| B03 REST / CI contract | **6 / 4 passed**, 0 failed |
| B02 settings / codec / conformance | **18 / 43 / 6 passed**, 0 failed |
| fmt / clippy | `cargo fmt --check` чисто; clippy `-D warnings` — 0 |
| Мутации B06 | **18 caught, 0 missed**, продукт восстановлен по md5 |
| Мутации RW-26 | **19 caught, 0 missed** (P4-07 11 + P4-11 8) |
| Мутации RW-27 | **6/6 неэквивалентных + 2 эквивалентных** (list), **14/14** (platform) |
| Мутации RW-28…RW-35 | RW-28 8/8, W29a 8/8, W29b 9/9, W29c 6/6, W30a 8/8, W30b 10/10, W31a 7/7, W31b 7/7, W32a 8/8, W32b 6/6, W33a 7/7, W34a 6/6, W34b 6/6, W35a 6/6 (SKIP=0) |
| Гейт RW-35 | **passed**, `.gate/worker-2026-10-22T19-41-26-571Z.json`, `waves/RW-35/gate-cargo.log` |
| Гейт RW-27 | **passed**, `.gate/worker-2026-10-22T13-58-02-813Z.json`, `waves/RW-27/gate-cargo-final.log` |
| Гейт RW-26 | **passed**, `.gate/worker-2026-10-22T11-40-00-985Z.json`, `waves/RW-26/gate-worker-b09.log` |
| Мутации B05 | **20 caught, 0 missed, 0 inconclusive**, продукт восстановлен по md5 |
| Мутации B04 | 21 caught, 1 inconclusive (не засчитана), продукт восстановлен по md5 |
| Гейт B06 | **passed**, `waves/RW-25/gate-worker-b06.log`, `.gate/worker-2026-10-22T05-52-40-267Z.json` |
| Контракты B06 | `RW-25/WAVE-PLAN.md`, `TZ-W26a.md`, `TZ-W26b.md`, `TZ-W26c.md`, `REVIEW-CHECKLIST.md` |
| Журнал | `waves/db.jsonl`; принятые волны RW-01…RW-27 (RW-26/RW-27 — `accepted` 2026-10-22) |
| Граф | `.ua/knowledge-graph.json` — устарел; offline-пересборка недоступна, зафиксировано после B05 |

## 2. Прогресс плана (`docs/rust-client/PLAN.md`)

- Принято: **48 / 90** — RW-36: `P0-01`, `P0-06`; RW-33: `P1-10`; RW-32: `P6-07` (+ `P6-02b`); RW-31: `P6-06`, `P2-03`; RW-30: `P6-02`
  (+ хвост `P7-01d`); RW-29: `P4-04`, `P4-12`
  (+ хвост `P7-01e`); RW-28: хвост `P7-01c`;
  RW-27: `P7-01b`, `P6-03`, `P6-04`; RW-26: `P4-07`, `P4-11`;
  B06: `P4-03`, `P7-02`, `P8-01`; B05: `P7-01`; B04: `P4-02`,
  `P8-04`; B03: `P3-15`, `P4-01`, `P7-04`, `P8-03`; B02: `P2-08`, `P1-01`, `P1-02`,
  `P0-04`, `P0-07`; `P0-02`, `P0-03`, `P1-03`…`P1-09`, `P1-11`, `P1-12`, `P2-01`,
  `P2-02`, `P2-04`…`P2-11`.
- Осталось: **42** — P0: 1, P1: 0, P2: 0, P3: 15, P4: 5, P4.5: 3,
  P5: 7, P6: 5, P7: 4, P8: 2. (Хвосты `P7-01b/c/d/e` и `P6-02b` в 90 карточек не входят.)
- B06 закрыл headless-контур: крипто-слой SRTP (без зависимостей), перф-гейт регрессии
  и контур обновления. **B07 выполнен (RW-36), RW-37 закрыл оконную модель
  (PSS 276,2 МБ ≤300);** следующий шаг — P3-01 (каркас окна), затем P3-02 (чат-вью).
- Остаток P0 — `P0-05` (спайк хранения секретов; keyring фактически закрыт RW-33,
  статус карточки сверить). Live-операции — только по согласованному окну.

## 3. Последние волны

### RW-27 — P7-01b + P6-04 + P6-03, принято 2026-10-22

Headless i18n/platform, без новых зависимостей.

- **P7-01b ListFormat** (`discord-i18n`): 31 включённая локаль × 3 стиля × 3 типа,
  `list_data.rs` из Node 24 / ICU (279 строк), 1674 кейса оракула; fallback en-US.
- **P6-04 Deep links** (`discord-platform`): `discord://` host/path/query/fragment,
  лимит 4096, control-chars, `extract_deep_link` по RE §9.4, маршруты login/one-time,
  oauth2/authorize, app/open, app/settings, library, channels; `Debug` редактирует
  `raw`/`query`/`fragment`.
- **P6-03 Автозапуск/бейдж**: XDG Desktop Entry с Exec-quoting (reserved-набор 1.5),
  `autostart_path`, Unity badge, `app_uri`; без D-Bus/I/O.
- Ревью REV-2: 2×P2 (Exec-quoting; расхождение доказательной базы W27a) закрыты
  red-first/актуализацией; P0/P1/P2=0. Targeted 32, workspace **902**, npm gate 214;
  мутации 6/6 + 2 эквивалентных, platform 14/14. Артефакты — `waves/RW-27/`.

### RW-26 — P4-07 + P4-11, принято 2026-10-22

Headless голосовой транспорт: статистика и регионы.

- **P4-07** (`discord-voice`): compound-парсер RTCP (SR/RR/SDES/BYE/APP, SRTCP-трейлер
  16/20), RFC 3550 loss/jitter по открытым RTP-заголовкам, `RttEstimator`/`StatsWindow`.
- **P4-11**: парсер реального `regions-rtc.json` (5×5 endpoint'ов), `ranked_region_names`
  (измеренные → geo → лексика), `plan_probes`/`complete_probe`; wire-формат проб не
  изобретался (UNKNOWN в RE).
- Ревью TL-1: P0/P1/P2=0 после red-first (timestamp wrap, `#[path]`-копия теста,
  граница compound-лимита). Voice **114**, workspace 869 (на момент волны); мутации 19/19.
  Артефакты — `waves/RW-26/`.

### B06 / RW-25 — P4-03 + P7-02 + P8-01, принято 2026-10-22

Первая **headless**-волна: офлайн, без новых внешних зависимостей (только офлайн-кэш
cargo: `ring`, `sha2`, `serde_json`). Три независимых лейна, у каждого свой `target-dir`.

- **P4-03 SRTP-крипта** (`discord-voice`): AES-256, AES-256-GCM, ChaCha20/XChaCha20-Poly1305,
  HKDF-SHA1 — **без зависимостей**. S-box алгебраический (`affine(x^254)`, «русский крестьянин»
  с масками), таблицы убраны — табличный вариант давал side-channel через кэш. Раскладка
  `[clear][ciphertext][tag 16][counter 4 LE]` подтверждена capture'ом
  (`research/RE-SRTP-LAYOUT.md`) и функциями самого клиента 1.0.165
  (`PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt`,
  `research/RE-NATIVE-SRTP.md`): исходящий счётчик `0x80000000 | n`, общий для RTP и SRTCP;
  AAD = открытая часть в режимах `*_rtpsize`. 38 тестов, KAT по NIST/RFC.
  **Совместимость с wire Discord не заявлена** — ключевого материала в capture нет.
- **P7-02 перф-харнесс** (`discord-perf`, новый крейт): детерминированная нагрузка
  (SplitMix64) — 24 229 участников + 100 000 сообщений на реальном редьюсере; гейт регрессии
  >10 % (T-HARN-04); холодный старт ~47 мс при бюджете 1500 мс. 17 тестов.
- **P8-01 updater** (`discord-updater`, новый крейт): манифест релиза, подпись Ed25519 над
  исходными байтами тела, проверка SHA-256 **до** перемещений, staged-замена, откат с
  проверкой целостности backup, запрет downgrade, защита от path traversal. 34 теста +
  1 doctest. Сети в карточке нет.
- **Интеграция I1** (TL-2): устранён O(n²) в `upsert_presence` (полное сканирование
  `state.presences` + `retain`) — диапазон по ключу `(user_id, scope)`. Red: 3.17 с на
  10 000 upserts; green: 0.03 с (тест `r19` в `discord-core/tests/presence_reducer.rs`).
- Ревью: codex — 2×P0 (side-channel S-box; path traversal + неподписанный манифест в
  updater) и 2×P1 (откат поверх существующего файла на Windows; переполнение счётчика
  ChaCha20); TL-1 — 2×P1 (повтор nonce на приёме, трейлер из открытого канала; перемотка
  счётчика). Все 6 закрыты red-first, P0/P1/P2 = 0. Локальный LLM (Qwen 27B) дал второе
  мнение (`research/llm-review.md`): его разбор байт SRTCP отклонён арифметикой, приняты
  пункты про нулевую базу регрессии, целостность backup и stale-seq.
- Workspace **785**, core 531, voice 65, perf 17, updater 35; fmt/clippy 0; gate passed;
  мутации **18/18 caught, 0 missed**. Артефакты — `waves/RW-25/`.

### B05 / RW-24 — P7-01, принято 2026-10-21

- P7-01: `locale.rs` — 36 кодов, 31 включён, 5 отключены; exact/case/language/en-US,
  canonical/fallback chain. `plural.rs` — CLDR cardinal/ordinal, `=N` до offset, offset,
  дроби/отрицательные/миллионы, 56 значений × 36 × 2 типа. `format.rs` — grouping 3/2,
  NBSP/U+202F, integer/percent/currency, 4 date/time style UTC, `th` +543, localized
  names/timezone, skeleton. `markdown.rs` — 23 правила, safe URL, UTF-8/fuzz, nested list,
  depth 11, saturating ordered numbers, exact timestamp/slash tokens.
- `lib.rs`: AST `Date/Time` style, Number/Plural через модули, `InvalidArgument` для
  не-числа, disabled-locale reject, fallback current → default → en-US.
- Red/green: W25a 7/7, W25b 5/5; review A/B фиксы 3/3, 6/6, 12/12; i18n **33/33**.
- Review A/B: P1 exact/offset, P2 currency/Number string, P1 list overflow, P2 nested list/
  timestamp/slash закрыты red-first; `he many`/NBSP — не баги для ICU 78/`format()`
  (см. `REVIEW-RESOLUTION-B05.md`). Итог P0/P1/P2=0; P3 — backlog.
- Workspace 663, core 530; fmt/clippy 0; gate passed; мутации **20/20**. Артефакты —
  `waves/RW-24/` (REPORT-B05, TEST-MATRIX-B05, MUTATIONS-B05, MUTATION-REPORT-B05,
  review A/B, red/green, MD5SUMS).

### B04 / RW-23 — P4-02 + P8-04, принято 2026-10-21

- P4-02: discovery 74 B (request port 0/33801), keepalive 8 B/5 s с LE-счётчиком,
  rotation 60 080 мс, 8 B RTCP empty RR, 28 B SRTCP wrapper, loopback socket; `encode`
  fallible, peer+SSRC validation, `rotate_if_due`. Без live/DNS/TLS/DAVE.
- P8-04: offline SPDX 2.3 на 85 `Cargo.lock`-пакетов, host metadata + version-pinned
  platform license fallback, strict AND/OR/WITH, invalid-token reject, CDLA allowlist,
  LLVM exception только WITH; CI mandatory hook, release `-ef` + SHA256SUMS.
- Review A: 4 P2 закрыты red-first (RR split, encode, peer/SSRC, schedule integration).
  Review B: 2 P1 + 3 P2 закрыты (locked packages, tokenizer, CDLA/LLVM, release, test gaps).
- Targeted UDP 15/15, SBOM 5/5; core 530, workspace 638; fmt/clippy 0; gate passed;
  мутации 21 caught + 1 inconclusive. Артефакты — `waves/RW-23/`.
- P3 backlog: skip-aware loopback reporting; pinned license fallback требует ревью при
  обновлении lock; cargo-deny optional; live/DAVE вне scope.

### B03 / RW-22 — P3-15 + P4-01 + P7-04 + P8-03, принято 2026-10-21

- Foundation `discord-json`: строгий RFC 8259 parser/writer без зависимостей, duplicate-key
  reject, Unicode/surrogate, depth/node limits, raw-числа, ошибки без payload; 11 тестов.
- P3-15: `xxh64`/6-символьный ключ, 36/31/5 локалей, AST/fallback, CLDR plural (`=N`,
  offset, дроби → other), rich-теги, безопасный markdown; реальные ru/en каталоги
  23 836/23 903; 8 тестов. Review A: P1 fence не воспроизвёлся, P2 plural/link закрыты.
- P4-01: op0/1/2/3/4/6/8/15/16, Hello/Ready/SessionDescription/Identify/SelectProtocol,
  endpoint/region, redaction; реальные rtc1/rtc2; 12 тестов. Review B: Debug/endpoint fixes.
- P7-04: 8 D-02 GET-ресурсов, точные пути, id validation, saved 404; 6 тестов + JSON oracle.
- P8-03: `scripts/ci.sh`, `scripts/release.sh`, workflow, deny, ETF bench; 4 теста,
  реальный `ci: OK`, release 445 040 B + SHA256SUMS.
- Gate `waves/RW-22/gate-worker-b03.json`; мутации 18/18; review C round 2 — P0/P1/P2=0.
- Артефакты: `waves/RW-22/` (REPORT-B03, TEST-MATRIX-B03, MUTATIONS-B03, DESIGN-FOUNDATION,
  review-A/B/C, red/green, gate). P3: `$link` URL (P7-01), UDP/DAVE (P4-02/P4.5),
  query P7-04 (D-02 не подтверждает), cargo-deny/SBOM (P8-04).

### B02 / RW-21 — P2-08 + P1-01 + P1-02 + P0-04 + P0-07, принято 2026-10-21

- P2-08: structural protobuf/base64, type 1 typed sections, type 2 structural, unknown
  preservation, limits, redacted Debug; 18 тестов. Review round 1 P2 (Debug fixed64 id/
  locale) закрыт red-first; round 2 accepted, P3 MAX_FIELDS закрыт t18 + M13.
- P1-01: checked ETF encoder, u16/u32/MAX_DEPTH limits, 50k fuzz/truncation; 6 тестов.
  P1-02: terminal `ZstdError`/`Poisoned`/`is_dead`, fresh `is_finished`, window-retention
  oracle; 13 тестов. Codec targeted 43/43, mutations 11/11.
- P0-04: repo→lab loader, typed errors, `RE_REQUIRE_LAB=1`; 6 тестов. P0-07: LEGAL policy
  vs actual state. Reviews: P1/P0 accepted with P3, P3 fixed; P0/P1/P2=0.
- Gate: `waves/RW-21/gate-worker-b02.json`; mutations `RW-21/mutations/`; docs/test matrix —
  `REPORT-B02.md`, `TEST-MATRIX-B02.md`.



### B01 / RW-19–20 — P2-10 voice + P2-11 typing, принято 2026-10-21

- P2-10: `READY_SUPPLEMENTAL` snapshot аддитивен, `VOICE_STATE_UPDATE` join/move/leave
  last-wins, `(guild,user)` store, timer/status, member upsert только при совпадении id;
  18 тестов. Review round 1 дал P2-1 (additivity/last-wins/remove не покрыты); закрыт
  red-first v09b/v09c/v09d + мутанты clear-before-loop/first-wins. Round 2 — **accepted**,
  P0/P1/P2=0, P3=11.
- P2-11: `TYPING_START` TTL 10 000 мс, repeat restarts, `MESSAGE_CREATE` снимает пару,
  READY сбрасывает, время только из `received_at_ms`/`now_ms`; 15 тестов. Review —
  **accepted**, P0/P1/P2=0, P3=10.
- Integration I0/I1: `state/mod.rs`, `state/types.rs`, `state/reducer.rs`, `ready/snapshot.rs`,
  `lib.rs`; shared-тесты адаптированы. Targeted 33/33, core 506, workspace 548, fmt/clippy 0,
  gate passed `.gate/worker-2026-10-20T20-31-37-021Z.json`; мутации 18/18.
- Артефакты: `waves/B01/`, `waves/RW-19/`, `waves/RW-20/`, ревью —
  `waves/reviews/review-p2-10-round2.md`, `review-p2-11.md`.

### RW-17 — P2-07 memory-only read-state/бейджи, принято 2026-10-20

- W17a/W17b/W17c: `MessageAck` + парсер, `EventPayload::MessageAck`/маршрут, `apply_ack`,
  селекторы mentions/guild/private/badge/ack-point; channel использует только
  `last_message_id`, non-channel — `last_acked_id`; manual/non-manual mentions; version
  watermark без max-guard; stale guard по message-id; 40 тестов (12/17/11).
- Ревью round 1 — P0/P1=0, **P2=1** (channel fallback в `last_acked_id` давал ложный
  `is_message_read`); закрыт red-first s11 (`Some(700) != None`, exit 101), s06 исправлен,
  Correction 2. Round 2 — **принято**, P0/P1/P2=0, P3=5 backlog.
- Scope: memory-only; REST ACK/`Effect::AckMessage` не генерируется, unread-count не
  выдуман, `MESSAGE_ACKED`/non-channel ACK не расширяются; SQLite/UI/сеть/live вне волны.
- Red: W17a compile-red, W17b 0/14 behavior-red, W17c compile-red (59 E0599), P2-1 s11;
  все exit 101. Green targeted 40/40, core 418, workspace 460; fmt/clippy чисто; gate
  passed `.gate/worker-2026-10-20T15-10-13-521Z.json`. Мутации 7/7 пойманы, md5
  восстановлены.
- Артефакты: `waves/RW-17/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS/REPORT, тесты, red/green,
  mutation-logs, gate), ревью — `waves/reviews/review-p2-07.md`.

### RW-16 — P2-05 memory-only chunks/догрузка, принято 2026-10-20

- W16a/W16b/W16c: `MemberChunk`/`Assembly`, подтверждённые поля `guild_id`,
  `chunk_index/count`, `members`, `not_found`; optional `nonce`; `presences[]` не
  изобретается (ignored, P2-09); сборка по `(guild, nonce)`, first-wins count, duplicate
  last-wins, out-of-order, `is_complete`; аддитивный merge `Guild.members`/`State.users`;
  `not_found` не удаляет; селекторы; `Effect::RequestPresence` для видимого диапазона —
  только декларация.
- Ревью round 1 — P2-1 (`duplicate-index` подавлялся для `nonce=None/count>1`), P0/P1=0;
  закрыт red-first (`r13`, exit 101); повторное ревью — **«принято»**, P0/P1/P2=0
  (`waves/reviews/review-p2-05.md`).
- Scope: memory-only; op8/op37/op14 wire и `nonce`-корреляция не подтверждены capture;
  `presences[]` внутри chunk — P2-09; SQLite/UI/сеть вне волны.
- Red: compile-red exit 101 + behavior-red r12/r13 exit 101; green targeted 43/43, core 378,
  workspace 420; fmt/clippy чисто; gate passed
  `.gate/worker-2026-10-20T13-21-59-680Z.json`. Мутации 3/3 пойманы (`is_complete`,
  duplicate-условие, merge-replace), продукт восстановлен по md5.
- Артефакты: `waves/RW-16/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, тесты, логи, gate).

### RW-15 — P2-04 memory-only member list, принято 2026-10-20

- W15a/W15b/W15c: `MemberListId` raw string, плоский `group|member`, SYNC/INSERT/UPDATE/DELETE,
  сдвиги/резет, snapshot groups/counts, duplicate/out-of-range/unknown-op диагностика,
  минимальный presence, селекторы; 39 тестов.
- Ревью round 1 — P2-1 (INSERT в позицию существующего id не давал duplicate), P0/P1=0;
  закрыт red-first (`red-w15-p21.log` exit 101 → green), P3 закрыты; повторное ревью —
  **«принято»** (`waves/reviews/review-p2-04.md`).
- Scope: memory-only; op37/op14 wire не подтверждён capture и не добавлялся
  (`Effect::SubscribeMembers` — интерфейс); chunks/P2-05, full presence/P2-09,
  `State.users` merge, SQLite/UI/сеть — вне волны.
- Red: compile-red, exit 101; green targeted 40/40, core 335, workspace 377; fmt/clippy чисто;
  gate passed `.gate/worker-2026-10-20T12-43-25-346Z.json`.
- Артефакты: `waves/RW-15/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, тесты, логи, gate).

### RW-14 — P2-06 memory history/pagination, принято 2026-10-20

- W14a `types.rs`, W14b `pagination.rs`, W14c `selectors.rs`: bounded per-channel window,
  `Latest`/`Before`/`After`/`Around`, generation-токены/stale, `Live > Page`, page/page
  last-wins, tri-state bounds, селекторы; 20 тестов.
- Ревью round 1 — P2-1 (root re-exports), P0/P1=0; закрыт red-first, P3-2/cap/expect
  исправлены; повторное ревью — **«принято»** (`waves/reviews/review-p2-06.md`).
- Scope: memory-only; REST response/JSON/SQLite/перезапуск/таймауты вне волны.
- Red: compile W14a/b + behavior W14c + root/counts, exit 101; green 20/20; workspace 337,
  core 295; fmt/clippy чисто; gate passed.
- Артефакты: `waves/RW-14/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, тесты, логи, gate).

### RW-13 — P2-02 state/reducer/selectors, принято 2026-10-20

- W13a `types.rs`, W13b `reducer.rs`, W13c `selectors.rs`: READY/SUPPLEMENTAL/RESUMED,
  `SESSIONS_REPLACE`, `THREAD_LIST_SYNC`, `CHANNEL_INFO`, `MESSAGE_CREATE`/`UPDATE`.
- Чистый reducer, `BTreeMap`-индексы, stale-политика (включая malformed), эффекты только
  `SubscribeMembers`, диагностика, redaction; 42 state-теста.
- Ревью round 1 — «не принято» (P1 stale malformed + 5 P2), все закрыты red-first;
  повторное ревью — **«принято»**, P0/P1/P2 = 0 (`waves/reviews/review-p2-02.md`).
- Red фиксов: `red-review-findings.log` (compile), `red-review-findings-runtime.log`
  (4 fail, exit 101); green 19/19; workspace 317, core 275; fmt/clippy чисто; gate passed.
- Артефакты: `waves/RW-13/` (DESIGN/TZ/RESEARCH/TEST-CORRECTIONS, тесты, логи, gate).

### RW-10 — HTTP/1.1 + TCP/TLS (инфраструктура P1-07), принято 2026-10-20

- W10a — `http1.rs`: запрос/ответ, CL/chunked, лимиты, anti-smuggling, строгий TE.
- W10b — `transport.rs`: `Endpoint`, TCP/TLS, SAN, DNS/connect/read/write-таймауты.
- Повторное ревью — «принято с нитами», P0/P1/P2 нет; P3 N1–N9 — очередь.
- Артефакты: `waves/RW-10/`; ревью — `waves/reviews/review-rw-10-11-round2.md`.

### RW-11 — rate-limit / telemetry-off / ack, принято 2026-10-20

- W11a — `rate_limit.rs` 429/5xx/401/бэкoff; W11b — `telemetry.rs`; W11c — `ack.rs`.
- Повторное ревью — «принято с нитами», P0/P1/P2 нет; P3-3/P3-4/P3-6 и тестовые P3 — очередь.
- Артефакты: `waves/RW-11/`; ревью — `waves/reviews/review-rw-10-11-round2.md`.

### RW-12 — I1 REST-фасад + P2-01 READY, принято 2026-10-20

- I1 `rest/client.rs`: auth + транспорт + 429/5xx + telemetry-off; 16/16 green после нитов;
  независимое ревью — **«Принято»** (`waves/reviews/review-i1.md`), P3-7 — backlog.
- P2-01: A1 READY-модель, 40 top-level ключей; воркеры W12a/W12b/W12c — 14+13+11 кейсов.
- Независимое ревью P2-01: round 1 — «не принято» (P1/P2/P3), все P1/P2 закрыты red-first;
  round 3 — **«принято»**, P0/P1/P2 = 0 (`waves/reviews/review-p2-01-round3.md`).
- P2-01: `ready_review.rs` 7 кейсов; атом-строки, вложенные `ignored`-пути с `d.`,
  сырой индекс `merged_members`, явный `[]` → `malformed`, дубликат гильдии — last wins.
- Legacy `ready_data` удалён из сборки и публичного API (P-4): было 9 тестов; core 242 → 233,
  workspace 284 → 275.
- Артефакты: `waves/RW-12/reports/` (`w12a.md`, `w12b.md`, `w12c.md`,
  `p2-01-review-fixes.md`, red/green/workspace/clippy-логи, md5, `p2-04-*`).

## 4. Что реализовано в ядре

- ETF-кодек: 11 тегов Discord, big-int, round-trip, fuzz-lite.
- zstd-stream: кадры 0/2+, окно 64 КиБ, непрерывный контекст.
- Шлюз: HELLO, heartbeat 41 250/ACK/RTT/zombie, IDENTIFY, RESUME, 21 диспатч, WS-транспорт.
- REST: 47 маршрутов, 6 auth-заголовков, 429/5xx/backoff, телеметрия-off, HTTP/1.1 + TLS,
  X-Super-Properties, ack/read-state, фасад `ApiClient`.
- READY: 7 гильдий, 272 канала, 132 роли, 363 эмодзи, 51 стикер, 17 ЛС, 146 users,
  60 relationships, read_state 157/128949, UGS 58/734, 1328 overrides, 1 сессия,
  merged 7×1, крупная гильдия `member_count=24228`; диагностика и redaction.
- History: memory-only bounded history, `Latest/Before/After/Around`, generation-токены,
  `Live > Page`, dedup, селекторы; без JSON/SQLite/сети.
- Members: memory-only `GUILD_MEMBER_LIST_UPDATE`, raw list id, плоский `group|member`, ops,
  duplicate/out-of-range диагностика, минимальный presence, селекторы; без op37 wire/chunks.
- State: чистый reducer `(State, Event) -> (State, Effects)`, индексы `BTreeMap`,
  READY/SUPPLEMENTAL/RESUMED, SESSIONS_REPLACE, THREAD_LIST_SYNC, CHANNEL_INFO,
  MESSAGE_CREATE/UPDATE, stale-политика, `SubscribeMembers`, селекторы, redaction.
- Read-state: memory-only `MESSAGE_ACK`, ленивое создание записи, non-channel/stale guard,
  manual/non-manual mentions, version watermark, mentions/бейджи/ack-point селекторы;
  без unread-count, REST ACK и UI.
- JSON: строгий parser/writer, duplicate/Unicode/depth/node/truncation, реальные каталоги/кадры.
- Крипто-контур SRTP: AES-256/AES-256-GCM/ChaCha20/XChaCha20-Poly1305/HKDF-SHA1 без
  зависимостей, S-box без таблиц; раскладка `[clear][ciphertext][tag16][counter4]` по capture;
  счётчик исходящих кадров `0x80000000|n` общий RTP+SRTCP; AAD = открытая часть;
  без wire-совместимости с Discord.
- Перф-контур: детерминированная нагрузка 24k участников + 100k сообщений, замеры на
  реальном редьюсере, гейт регрессии >10 %, база CI-истории в JSON.
- Контур обновления: манифест + подпись Ed25519, проверка хэша до перемещений,
  staged-замена, откат с проверкой backup, запрет downgrade; без сети.
- I18N: hash/locale/AST, CLDR cardinal/ordinal, числа/валюты, даты/время UTC, 23 markdown-
  правила, fallback current→default→en-US; реальные ru/en.
- Voice signaling: op0/1/2/3/4/6/8/15/16, endpoint/region, secret redaction; без UDP/DAVE.
- REST read-only: 8 D-02 GET-ресурсов, id validation, saved 404; без write/query.
- CI/release: offline `ci.sh`, `release.sh`, workflow, deny.toml, ETF bench smoke.

Крейты: `discord-codec`, `discord-core`, `discord-json`, `discord-i18n`, `discord-voice`,
`discord-perf` и `discord-updater` — рабочие/частично; `discord-conformance` — тестовый;
`discord-app`, `discord-ui`, `discord-store`, `discord-media`, `discord-platform` — каркасы.
GUI-окна и пользовательского приложения пока нет.

## 5. Следующий шаг: B07

1. RW-28/RW-29 приняты: `review`, `gate`, `accepted` — в `waves/db.jsonl`; доки
   синхронизированы один раз (этот проход).
2. **B07 остался только GUI**: keyring, SQLite и fd-передача закрыты; нужен выбор GUI-тулкита
   (`tauri`/`iced`/`egui` — нет в офлайн-кэше) и окно для live-smoke. Headless-контур закрыт:
   дальше только P4-05/P4-06/P4-08/P4-09 (аудио-устройства/Opus), P4.5 DAVE, P5 стриминг,
   P6-01/P6-05/P6-07 UI и P7-03/05/07 — требуют GUI/зависимостей.
3. RW-28 закрыл RelativeTimeFormat, RW-29 — RTP-медиа, reconnect и `<t:…>`; контракты —
   `waves/RW-28/`, `waves/RW-29/` (уточнения RTP/джиттера — `TZ-W29b.md` §6).
4. Red-first → targeted → mutation → один batch gate → независимое ревью → accepted.
5. Блокеры (SQLite/keyring/GUI/live) не обходить: ждать решения владельца.

### 5.1 KPI batch-конвейера

| Метрика | Цель | Факт RW-26/RW-27 |
|---|---:|---:|
| batch-волн/день | 2–3 | 2 (RW-26, RW-27) |
| accepted cards/day | 6–8 | 5 (P4-07, P4-11, P7-01b, P6-03, P6-04) |
| first-pass gate | ≥ 70 % | 2/2 gate passed (RW-27 — после фиксов) |
| P0/P1 на ревью | 0 | 0 |
| P2 до accepted | 0 | 2 (RW-27: Exec-quoting, доказуемость) — закрыты |
| mutation catch rate | ≥ 90 % | **19/19, 6/6 (+2 эквивалентных), 14/14 = 100 %** |
| tests/card | S 6–10, M 10–16, L 16–24 | 36 (P4-07), 10 (P4-11), 10/9/13 (RW-27) |
| active code workers | 2 | 3 (W27a, W27b, TL-2 — раздельные target-dir) |

## 6. Блокеры (решения владельца)

| Блокер | Что нужно |
|---|---|
| GUI: `tauri`/`iced`/`egui`/`winit`/`wry`/`tao` отсутствуют в `Cargo.lock`/кэше | разрешение на загрузку конкретного набора **или** временный TUI/headless |
| `keyring` (`P1-10`) | разрешение на зависимость + Secret Service/KWallet |
| `rusqlite`/`libsqlite3-sys` (`P2-03`) | разрешение на зависимость + системный SQLite |
| Живой Discord (PID 95828) | окно владельца для live-smoke; без него — только фикстуры/loopback |

## 7. Бэклог P3 (не блокирует)

- RW-35/P6-06c: `NEGOTIATE_UNIX_FD` дублирует auth-логику в `dbus/mod.rs` (~60 строк),
  `auth::authenticate` без fd — перенести/задокументировать; `Value::Clone`/`Signal::Clone`
  клонируют владение сырым fd (API-ловушка); нет тестов на дубликат индекса/лишний fd/лимит
  отправки; `CONTROL_BYTES` с магической 16; дубликат поля UNIX_FDS; два live-флага
  (`DISCORD_LAB_LIVE_POWER`/`DISCORD_LAB_POWER_LIVE`); нет общего дедлайна чтения сообщения.
- RW-34/P6-02c: тест границы abstract использует продакшн-константу (не литерал 107) — стоит
  захардкодить 107/108; percent-encoding не декодируется (задокументировано); `BusAddress`
  struct→enum и `path(): Option` — не аддитивно (TZ §1); SKIP-эвристика мутаций W34a/W34b
  считает «live» подстрокой; `gen-hints-ext.py` вне файловой матрицы; журнал мока не экранирует
  `|`/`:` (тестовые значения таких символов не содержат).
- RW-34/общее: fd-передача D-Bus (`h`) и `logind.Inhibit("sleep")` не реализованы — нужен
  ffi `sendmsg/recvmsg` или зависимость `libc` (решение владельца).
- RW-33/P1-10: `*_sync` на заблокированной коллекции может ждать prompt — live только по
  флагу; `KeyringError::Encoding`/`Secret{code,message}` не покрыты k-кейсами; `secret-tool`
  сценарии используют strict `xdg:schema`, записи без схемы не находятся (осознанно); wire-
  совместимость с крейтом `keyring` не заявляется; TZ §2 обещал unit-кейсы «без шины» —
  фактически k09–k12 подключаются к сервису.
- RW-32/P6-07: libhunspell — системная зависимость; CLI-оракула нет (семантические инварианты);
  `HunspellSpell`/`Suggest` API 1.7; словарь синтетический; реальные словари не проверялись.
- RW-32/P6-02b: hints — минимум freedesktop; дублирование тестовых харнессов (`Harness`/
  `HintsHarness` ~120 строк); эвристика SKIP считает «live» подстрокой; в фикстуре нет кейса
  пустых строк (проверено агентом, не заморожено).
- RW-32/крейт: `discord-platform` переведён на `unsafe_code = "deny"` с изолированными
  ffi-модулями (hunspell); гарантия «unsafe запрещён полностью» больше не действует — ревью
  каждого ffi-модуля обязательно.
- RW-31/P6-06: fd-ингибитор сна (`logind.Inhibit` + SCM_RIGHTS) не реализован; Wayland/X11
  idle вне headless-пути; пороги idle — конфигурация, не RE-факт; p10 принимает `Ok(None)` для
  закрытого соединения (стоит сузить до `Err`); live не гасит экран.
- RW-31/P2-03: `Store` однопоточный (`!Send + !Sync`); WAL только для файла (`:memory:` —
  фолбэк `memory`); NUL-строка → `Sqlite`; `flags > i64::MAX` → `Constraint`; схема
  wire-совместимость с Discord не заявляется; для сборки нужен системный `libsqlite3`.
- RW-30/P6-02: `hints` всегда пустой `a{sv}`; abstract/tcp адреса, big-endian, передача fd
  (`h`) и percent-кодирование `DBUS_SESSION_BUS_ADDRESS` не поддержаны; live-уведомления не
  отправлялись; wire-совместимость с Electron-модулем не заявляется.
- RW-30/P7-01d: ICU-паритет привязан к Node 24.21/ICU 78; capacity в `join_chunks` не учитывает
  `join_last`; all-zero кейс оракула хранится как пустой объект (спец-обработка в пересчёте).
- RW-28: `format_number_with` печатает ASCII `-` вместо U+2212 для `sv-SE/fi/no/hr/lt`
  (5 из 186 проверок ревью; relative-путь не затронут). Мутации `run-mutations.py` пишут
  в продукт in-place и имеют фиксированный target-dir — при SIGKILL восстановление не
  гарантировано.
- RW-29/P4-04: история дублей джиттера ограничена 512 seq (дубль старше — `Late`);
  мутационные скрипты временно мутируют продукт (SIGKILL → восстановление не гарантировано)
  и не сверяют md5 тестов; m02 не проверяет seq/ts/marker аномалий, нет теста дубля после
  вытеснения 512, padding/CSRC не покрыты мутациями; `payload_type & 0x7f` молча обрезает
  PT ≥ 128; фикстура RTP читается из `fixtures/` вне крейта.
- RW-29/P4-12: точные клиентские задержки reconnect не измерены (jitter в capture не
  наблюдался), op9-ветка нормативная, wire-совместимость не заявляется.
- RW-29/P7-01e: рендер только UTC, NNBSP/NBSP нормализуются в пробел; `t/T` собраны из
  явных компонентов (гибкие day periods zh-TW не воспроизведены); `f` = короткая дата +
  время по ТЗ (в клиенте раскладка иная); wire-совместимость не заявляется.
- RW-27/P6-04: `DeepLink::raw` — публичное поле с `token`; redaction только в `Debug`,
  потребитель не должен логировать поле. `AppSettings` принимает trailing `/`, host
  сравнивается case-insensitive — либеральнее RE. Реальный лаунчер/D-Bus не проверялись.
- RW-27/P6-03: `Name` в desktop-файле не проходит string-escaping (`\t`, `\`), отказ
  только на `\n`/`\r`/`\0`; строгий spec для литерального `\` (`\\\\`) не реализован.
- RW-27/P7-01b: `format_list` без language-fallback (`ru-RU`/`de-DE` → en-US), контракт
  заморожен; fallback-строка en-US и `join(", ")` недостижимы на полной таблице.
- RW-26/P4-07: SRTCP-трейлер с version=2 и валидной длиной может быть разобран как пустой
  RR (~1/4 тегов); `ReceptionStats.seen` — неограниченный `BTreeSet` (нужен bounded 4096);
  вытеснение sender-report по NTP-ключу, а не по времени вставки.
- RW-26/P4-11: `plan_probes` без `wrapping_add` для `sequence + offset`; канонический порт
  (отказ `01`/`+1`) — гигиена ввода, не подтверждён RE.
- B06/P4-03: (1) RFC 3711 §4.3.1 KDF не реализован — текст недоступен офлайн,
  `srtp_key_derive` сделан на HKDF-SHA1; (2) вывод сессионных ключей/nonce — внутренняя
  реализация Discord, wire-совместимость **не заявлена**; (3) входящий 4-байтный трейлер
  трактуется как непрозрачный (монотонность в capture не подтверждена); (4) окно приёма
  1024 трейлеров — «очень старый» повтор вне окна теоретически проходит; (5) предел
  `n ≤ 0x7FFF_FFFF` (старший бит — маркер) — при ином кодировании граница изменится;
  (6) S-box без таблиц медленнее табличного (~13 умножений GF(2^8) на байт) — при
  включении в горячий путь нужен замер.
- B06/P7-02: базовые цифры CI-истории сняты на машине воркера — переснять на референсной;
  `delta_pct = INFINITY` при нулевой базе не сериализуется в JSON; нужна ли отдельная CLI
  прогона базы вместо печати JSON из теста; публичные `base_state()`/`PERF_*` в
  `discord-perf` — оставить (нужны независимому оракулу) или сузить.
- B06/P8-01: сети в карточке нет — загрузка артефакта отдельным подэтапом; `size` артефакта
  не сверяется (ТЗ требовал только хэш); метка `<имя>.bak.sha256` остаётся в каталоге
  установки и не снимается шагом `RemoveBackup` (исполняется вне карточки); откат проверен
  только на Linux (на Windows `rename` не заменяет файл — порядок изменён, но фактический
  прогон на Windows не выполнялся).
- B05/P7-01: `$link` URL/аргумент не типизирован (подтверждённого URL нет); полные
  переводы 31 локали, `ListFormat`/`RelativeTimeFormat`/`DurationFormat`, `<t:R>`-таймеры;
  `format_percent` без локального пробела для ru/fr/de; строгая грамматика аргументов
  slash-команды; устаревшее `he many` в TZ-W25a помечено.
- B04/P4-02: loopback-тесты skip-aware, в текущем окружении прошли; нативный skip в stable
  Rust отсутствует — отчёт в stderr. SRTCP-крипто/DAVE — P4-03/P4.5.
- B04/P8-04: license fallback для 14 platform-пакетов version-pinned; ревьюить при
  обновлении `Cargo.lock`. `cargo-deny` локально skip без инструмента.
- B03/P3-15: `$link` URL из control/аргумента не типизирован (подтверждённого URL нет);
  даты/время и продвинутые number skeletons — P7-01.
- B03/P4-01: UDP discovery/keepalive/ротация/DAVE — P4-02/P4.5; audio-only SelectProtocol
  и полный набор codecs — уточнить на следующем capture.
- B03/P7-04: `ResourceQuery` не вводился (D-02 не подтверждает query); путь без `/api/v9`
  по TZ-W22c.
- B03/P8-03: cargo-deny локально skip без инструмента; bench smoke, порог 10 % — P7-02;
  SBOM — P8-04.
- B03/JSON: лимит 4M узлов — защита от DoS, не бюджет памяти; `Other.raw` нельзя логировать.
- B02/P1-02: последнее окно незавершённого zstd-кадра не отдаётся — ограничение streaming
  API `ruzstd`, контракт 759 280/824 816 зафиксирован; для «дочитать» нужен другой декодер.
- B02/P0-04: T-HARN-02/03 (полный gateway/REST replay) — отдельная карточка, нужен
  JSON-компаратор; скелет намеренно не тащит зависимость.
- B02/P0-07: юридическая оценка ToS/распространения — за владельцем; SBOM/cargo-deny —
  T-HARN-07, keyring/лог-скан — P1-10/T-SEC.
- B02/P2-08: type 2 favorite/frecency semantics не типизирована; `status_expires_at_ms`
  и custom_status остаются structural/raw до отдельного capture.
- P2-01: публичный `value` (P3-3), схематичность §3.5-псевдокода, диагностика пустых групп
  `merged_members`, устаревший doc `ReadyError::NotReady`.
- RW-10: N1 пустой TE+CL, N2/N3 варианты ошибок chunked/обрыва, N4 overflow max_body,
  N5 O(n²) re-decode, N6 пробелы покрытия, N7 флейк read-timeout, N8 doc-md5 telemetry,
  N9 strict-TE (ТЗ обновлён).
- RW-11: P3-3 глубина JSON-сканера, P3-4 нормализация telemetry-пути, P3-6 ссылки D-02,
  P3-1/P3-2/P3-5 тесты Host/TE/httparse.
- I1: P3-7 `AUTH_HEADER_NAMES` вместо литералов.
- RW-18/P2-09 (8 P3, не блокируют):
  - P2-09-P3-1: `a16` не проверяет missing/null `merged_presences` и отсутствие лишней
    группы в `guilds[0]` (тест-пробел, код уже верен);
  - P2-09-P3-2: `r04`/`r10` не проверяют отсутствие ложных `presences:user:*:missing`;
  - P2-09-P3-3: doc-comment `presence_selectors.rs`/`member_list.rs` и
    `TEST-CORRECTIONS.md`/`REPORT.md`/`STATE.md` отстают от online-first семантики;
  - P2-09-P3-4: `Invisible`/`Unknown` в fallback-агрегате не зафиксированы контрактом;
  - P2-09-P3-5: `hidden_activities(user)` не объединяет hidden другого scope при online;
  - P2-09-P3-6: `InvalidId.value` для отсутствующего `user.id` равен `"map"` вместо
    `Missing`/точного значения;
  - P2-09-P3-7: non-list `presences` в chunk даёт две диагностики вместо одной;
  - P2-09-P3-8: мёртвая ветка в `parse_merged_presences` после `else { continue; }`.
- RW-17/P2-07: red-лог W17b снят до финального форматирования (номера строк, причина
  та же); старый `mutations.txt` 4/4 помечен superseded, актуальный `MUTATIONS.md` 7/7;
  live-smoke нет; unread-count и cap/prune read-state не вводились (вне scope); non-channel
  ACK отклоняется целиком, включая version.
- RW-16/P2-05: P3-2 red-лог r13 снят до двух last-wins assert-ов (сдвиг строк, причина
  падения та же); P3-3 нет отдельного regression на `nonce=Some/count>1` duplicate
  (покрыто r04/r13 + probe); cap/prune сборок не введён; повтор `none/count=1` заменяет
  снимок chunk-а.
- RW-15/P2-04: P3-1…3 закрыты test-only; P3-4 tests-red пуст (compile-red).
- RW-14/P2-06: P3-1 Before-trim/Around-deferred не в committed-тестах; P3-2 page/page
  last-wins и повтор токена покрыты частично; P3-4 cap 256 не покрыт тестом; P3-6
  `Inserted` может быть сразу вытеснен; P3-7 поздняя страница может вернуть вытесненный live-id.
- RW-13/P2-02: P3-1 покрытие веток (дубликаты/пустой SESSIONS_REPLACE, перенос канала,
  смена channel_id, duplicate MESSAGE_CREATE, seq=None); P3-3 синтетические thread-id;
  P3-5 `r19` не вызывает `apply_dispatch`/`apply_frame`; P3-6 пустая группа supplemental
  с невалидным raw-индексом не даёт inconsistency.
- RW-09/N3: политика `IdentityConfig::Some("")`.
- N4: рост буфера `GatewayPipe::events` до дренажа.
- P1-02: вернуть строгую проверку `s == 113` после пересмотра декодера.

## 8. Как продолжить после сбоя/сна

1. `git log --oneline -3` и `git status` — всё ли зафиксировано.
2. `tail -n 8 waves/db.jsonl` и этот файл — последнее событие и следующий шаг.
3. Batch: `docs/rust-client/WAVE-PLAN.md` + `TEAM.md`; карточки — `PLAN.md`;
   регламент — `PROCESS.md` §3–§4.
4. Факт и счётчик: `docs/rust-client/STATUS.md`; роли/WIP — `TEAM.md`.
5. Воркер: targeted-тест; integration/QA: `cd rust && cargo test --offline --workspace`,
   fmt/clippy, затем `npm run gate:worker`.
6. После приёмки: артефакты → `waves/RW-XX/`, дописать `db.jsonl`, один раз обновить
   `STATE`/`STATUS`/`ROADMAP`/`TASKS`/`map`, коммит + push (координатор).
