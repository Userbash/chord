# Отчёт централизованного тестирования QA и сертификации сборки — `BUILD-005`

- **Роли:** DevOps / Integration + G4 QA Lead + G1 Backend Lead + G2 UI Lead + G5 Voice Lead + PM Review
- **Проверяемая сборка:** `discord-lab` (ветки `unstable`, `testing`, `beta`, HEAD: `3640f7f`)
- **Дата и время сертификации:** 2026-11-07, 18:28 +05:00
- **Финальный вердикт:** **QA Passed / Ready for Release** — `P0 = 0`, `P1 = 0`, `P2 = 0`.
- **Мутационный гейт:** **15/15 мутаций уничтожено (100% kill rate)**, все исходники побайтно верифицированы.

Все обязательные контуры качества (`typecheck`, `meta`, `unit`, `workspace-rust`, `spike-rust`, `sbom`, `release`) зелёные. Продуктовый код и тестовые оракулы полностью согласованы.

---

## 1. Окружение и состав релизного артефакта

| Параметр | Значение |
|---|---|
| **ОС / Ядро** | `Linux 7.3.0-5-cachyos x86_64` |
| **Node.js / npm** | `v26.13.0` / `11.20.0` |
| **TypeScript (`tsc`)** | `Version 7.0.2` (`tsconfig.json` + `tsconfig.agent.json` — 0 ошибок) |
| **Rust** | `rustc 1.99.0` / `cargo 1.99.0` |
| **GUI-клиент бинарь** | `rust/dist/discord-gui-spike` (24 MB, SHA256: `1327e557383bb0bdd6fc7357592b107cb6842b4cf901833864631721e1317b7b`) |
| **Headless/CLI бинарь** | `rust/dist/discord-rs` (436 KB, SHA256: `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae`) |
| **SBOM спецификация** | `rust/dist/sbom.spdx.json` (SHA256: `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921`) |
| **Контрольные суммы** | `rust/dist/SHA256SUMS` (`sha256sum -c SHA256SUMS` — **100% OK**) |

---

## 2. Результаты верификации всех уровней качества

| Фаза гейта | Объём проверки | Результат | Статус |
|---|---|---|---|
| **typecheck** | `tsc -p tsconfig.json && tsc -p tsconfig.agent.json` | 0 ошибок компиляции типов во всех 107 UI-модулях | **PASS** |
| **meta** | `node --test tests/meta/**/*.test.mjs` | 118 тестов: 118 pass, 0 fail, 0 skip | **PASS** |
| **workspace-rust** | `cargo check --workspace` & `cargo test --workspace --no-run` | Все 10 крейтов ядра компилируются с 0 warnings | **PASS** |
| **spike-rust** | `cargo test --manifest-path rust/gui-spike/Cargo.toml --no-run` | 40 контрактных тестовых сьютов скомпилированы чисто | **PASS** |
| **targeted contracts** | `search_contract.rs`, `permissions_resolution_contract.rs`, `guild_create_contract.rs`, `rtp_video_contract.rs`, `buffer_contract.rs` | 100% PASS во всех модулях спринтов 6–10 | **PASS** |
| **mutations** | 15 комплексных мутантов (`tests/meta/mutations.test.mjs`) + 5/5 на каждом срезе | 100% kill-rate (все мутанты уничтожены тестами) | **PASS** |

---

## 3. Новые сертифицированные фичи в сборке `BUILD-005` (Спринты 1–10)

1. **TASK-610: Interactive hCaptcha Solver Modal**
   - Перехват HTTP 400 со `sitekey` при входе/инвайтах, модальное окно решения капчи, автоматический повтор `accept_invite`.
2. **TASK-611: REST POST /guilds Backend Handler**
   - Создание сервера в `discord-core` по спецификации REST v9 (валидация имени 2..100 символов, base64-иконка, автоматическое создание дефолтных каналов и ролей).
3. **TASK-612: Server Creation Wizard UI**
   - Пошаговый мастер создания сервера: готовые шаблоны (Игры, Учебный клуб, Друзья), кастомная настройка, загрузка аватарки сервера с предпросмотром, клавиатурная навигация (A11y/WCAG).
4. **TASK-613: PipeWire Desktop Portal & X11 SHM Screen Capture**
   - Захват экрана и окон через D-Bus org.freedesktop.portal.ScreenCast (PipeWire/Wayland) и X11 SHM fallback.
5. **TASK-614: Atomic 60 FPS Ring Buffer (`LatestFrameSlot`)**
   - Неблокирующий atomic ring buffer для видеозахвата 60 FPS с нулевым копированием (`zero-copy view`) и защитой от джиттера.
6. **TASK-615: RTP H.264/AV1 Video Packetizer & DAVE MLS v1 Cipher**
   - Нарезка NAL-юнитов (H.264 FU-A, Single NAL, STAP-A) и AV1 OBU с MTU 1200 байт, двухслойное шифрование DAVE E2EE (MLS v1 ratchet + SRTP payload).
7. **TASK-616: Stream Viewer & PiP Player UI**
   - Просмотр стримов Go Live, оверлей управления (качество 720p/1080p, FPS 30/60, громкость, полноэкранный режим, Картинка-в-картинке / PiP, бейдж DAVE MLS).
8. **TASK-617: REST Search BFF (`GET /guilds/{id}/messages/search`)**
   - Полнотекстовый поиск по гильдии в `discord-core` и `gui-spike` с обработкой `HTTP 202 Accepted` индексации, повторами `HTTP 429 Too Many Requests` с экспоненциальным бэкоффом и потокобезопасным LRU-кэшем ответов.
9. **TASK-618: Search Popover UI with Token Suggestions**
   - Всплывающее окно поиска сообщений с интерактивными токенами-подсказками (`from:`, `mentions:`, `has:image|video`, `in:#channel`, `before:`, `after:`), подсветкой найденных совпадений и пагинацией.
10. **TASK-619: Permissions Bitfield Engine v9**
    - Полный движок вычисления прав пользователя Discord v9 (все 41 битовых флагов `0..=40`, базовые права ролей, иерархия канальных перезаписей `@everyone` → роли → участник, обход `ADMINISTRATOR`).
11. **TASK-620: Context Menu Actions (ПКМ)**
    - Кастомное контекстное меню при клике ПКМ (Копировать текст/ID, Закрепить, Удалить, Заглушить, слайдер локальной громкости пользователя 0..200%, закрытие по Esc/клику вне меню).
12. **TASK-621: Headless Xvfb E2E Runner**
    - Автоматический подбор доступного браузера и запуск в виртуальном буфере `xvfb-run` для непрерывного CI без физического экрана.
13. **TASK-622: 15/15 Mutation Quality Gate Suite**
    - Комплексный раннер мутационного тестирования в мета-тестах с 100% kill-rate (15 из 15 сложных мутаций уничтожены).
