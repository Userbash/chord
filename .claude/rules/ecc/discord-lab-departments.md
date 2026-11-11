# ECC × discord-lab: Архитектура отделов, маршрутизация и план интеграции (P0–P2)

Этот документ связывает фреймворк **ECC (Everything Claude Code / ECC 2.0 Control Plane)** с правилами репозитория `discord-lab` (`AGENTS.md`, `TEAM.md`, `PROCESS.md`, `TEST-POLICY.md`) и локальной базой эталонных проектов (`docs/reference-projects/`).

---

## 1. Отделы специалистов `discord-lab` (Specialist Departments)

В каждом запуске ECC и Orca Orchestration задачи распределяются строго по отделам с непересекающимися файловыми контурами (`write-scope`):

| Профиль в `ecc2.toml` | Отдел `discord-lab` | Зона ответственности (`write-scope`) | Целевые T0-проверки |
|---|---|---|---|
| `g1_backend` | **G1 — Backend (Rust Core & Tauri BFF)** | `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs` (кроме `voice_devices.rs`), `rust/gui-spike/tests/*_contract.rs` | `cargo test -p discord-media`, `cargo test --test <slice>_contract`, `cargo clippy -D warnings` |
| `g2_frontend` | **G2 — Frontend (WebView UI & DOM/A11y)** | `rust/gui-spike/ui/**/*.js`, `rust/gui-spike/ui/**/*.css`, `rust/gui-spike/ui/index.html`, `tests/unit/*.test.mjs` | `node scripts/test-impact.mjs --run`, `npm run typecheck`, `tests/meta/ipc-path.test.mjs` |
| `g3_design` | **G3 — Design (UX/UI Specs & Tokens)** | `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`, CSS-токены и брейкпоинты | `node --test tests/meta/design-breakpoints.test.mjs tests/meta/test-policy.test.mjs` |
| `g4_qa` | **G4 — QA (Test Automation & Verification)** | Централизованное тестирование ветки `testing`, `scripts/test-impact.mjs`, `tests/{meta,unit,smoke,e2e}/`, `QA-BUILD-*.md` | Полный матричный прогон `npm run gate:worker` + `cargo test --workspace` |
| `g5_voice_platform` | **G5 — Voice & Platform** | `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, `rust/gui-spike/src/voice_devices.rs`, опкоды `18..=22`, SRTP/DAVE, `discord-ipc-0` | `cargo test -p discord-voice -p discord-platform`, `--test voice_devices_contract` |
| `g6_tech_writer` | **G6 — Tech Writer & Parity** | `waves/RW-*/PARITY/MERGE-READINESS-*.md`, `RELEASE-NOTES-*.md`, матрицы паритета | `node --test tests/meta/docs-sync.test.mjs tests/meta/plan-sync.test.mjs` |
| `g7_re_rea` | **G7 — Reverse Engineering (`G7 RE / REA`)** | Read-only сверка с `/home/sanya/re-deep/`, `docs/reference-projects/` и `rea`; отчёты `waves/RW-*/RE-AUDIT/RE-GAP-*.md` | Инструменты MCP `rea` и `understand` |
| `devops` | **DevOps / Release & Integration** | `scripts/{release.sh,ci.sh,gate.mjs}`, SBOM, `SHA256SUMS`, конвейер `unstable -> testing -> beta -> main` | `bash scripts/release.sh --verify`, `npm run gate:release` |
| `pm_review` | **Product Manager (PM)** | Приёмка `ACCEPT.md`, проверка пользовательских сценариев и DoD | Сверка 9 измерений тестирования и DoD |
| `tech_reviewer` | **Technical Reviewer (DeepSeek)** | Независимое ревью `REVIEW.md` и уничтожение `5/5` мутантов | Перезапуск тестов, MD5, мутационный гейт `5/5` |

---

## 2. Лимит памяти, слоты и маршрутизация моделей

1. **Бюджет ОЗУ (`WORKER_RAM_LIMIT <= 25 GB`):**
   - Максимум **5 параллельных активных сессий/воркеров** (`max_parallel_sessions = 5`, `max_parallel_worktrees = 5` в `ecc2.toml`).
   - Не запускать несколько `cargo` одновременно на одном `target-dir`.
2. **Маршрутизация моделей:**
   - **Полный запрет `codex`** (`--agent codex` никогда не используется).
   - **Antigravity (`--agent antigravity --model gemini-4-argon-high --effort high`):** первичная разработка новых фич (`G1`, `G2`, `G5`) и работа в будние дни (Пн–Пт) до 18:00 по пекинскому времени (`TZ=Asia/Shanghai`).
   - **DeepSeek (`--agent opencode --model deepseek/deepseek-v4.1-flash`):** техническое ревью, QA, мутации и рефакторинг (Pass 2/3) строго в разрешённом окне (Пн–Пт с 18:00 `Asia/Shanghai`, Сб–Вс круглосуточно).
3. **Иерархия веток:**
   - Все рабочие ветки воркеров создаются от актуальной ветки `unstable`: `<type>/task-<id>-<slug>` -> PR в `unstable` -> `testing` -> `beta` -> `main`.

---

## 3. Приоритетный план интеграции эталонных проектов (`Serein`, `LemonCord`, `rust-discord-activity`)

### Приоритет 1 (P0): Видеоконвейер Go Live, 60 FPS и нативный RTP/H.264/AV1 (из `ViceVerse-cz/Serein`)
- **Задача 1 (`task-601-go-live-dual-session`, отдел `G5 Voice & Platform`):**
  - **Источник:** `docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs` (`run_stream_inner` строки 885–1390, `announce_video` строки 1435–1485).
  - **Целевые файлы:** `rust/crates/discord-voice/src/session.rs`, `rust/crates/discord-voice/src/udp.rs`.
  - **Суть:** выделенная вторая сессия Go Live (`STREAM_SERVER_UPDATE` со своим `stream_key`, `endpoint`, `token`, вторым Voice WS и вторым UDP-сокетом, `video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`), точный контракт Voice Opcode 12 (`Video`) с массивом `streams` и Voice Opcode 15 (`Media Sink Wants`).
- **Задача 2 (`task-602-rtp-assembler-rtx-rtcp`, отдел `G5 Voice & Platform`):**
  - **Источник:** `docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs` (`Assembler::push` строки 52–165, `Receivers::restore_rtx` строки 390–445, `spawn_decoder` строки 560–650).
  - **Целевые файлы:** `rust/crates/discord-voice/src/media/rtp.rs`, `rust/crates/discord-voice/src/rtcp.rs`.
  - **Суть:** сборщик фрагментированных RTP-кадров H.264 (`Single NAL`, `STAP-A`, `FU-A`) и AV1 (`OBU`) по RTP timestamp, распаковка ретранслированных пакетов RFC 4588 (`restore_rtx`), мгновенная генерация RTCP NACK (PT 205, FMT 1), PLI (PT 206, FMT 1), FIR (PT 206, FMT 4) и REMB (PT 206, FMT 15).
- **Задача 3 (`task-603-media-pipeline-60fps-slot`, отдел `G1 Backend`):**
  - **Источник:** `docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs` (`store_frame` строки 54–65, `Watch::poll` строки 158–226), `crates/discord-voice/src/screen.rs` (`Worker::start`, `encode_loop` строки 70–250), `crates/discord-voice/src/video.rs` (`packetize`, `Pacer`, `Rate`, `History`), `apps/desktop/src/gpu_video.rs` (`upload_gpu` строки 295–421).
  - **Целевые файлы:** `rust/crates/discord-media/src/lib.rs`, `rust/gui-spike/src/main.rs`.
  - **Суть:** паттерн «Latest-Frame Atomic Slot» (`Arc<Mutex<Option<VideoFrame>>>`), двухпоточный конвейер захвата/кодирования с `sync_channel(2)` и неблокирующим `try_send`, кольцевой буфер `History` (512 пакетов для ответов RTX на чужие NACK), адаптивный регулятор битрейта `Rate` по REMB и подготовка `NV12`/`I420 -> RGBA` конвертера без промежуточного JPEG.

### Приоритет 2 (P1): Изоляция громкости стрима и защита от эха системного звука (из `ViceVerse-cz/Serein`)
- **Задача 4 (`task-604-stream-volume-mixer-antiecho`, отдел `G5 Voice & Platform`):**
  - **Источник:** `docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs` (`Mixer::pop_with_volumes` строки 95–165), `stream_playout.rs` (`Playout::next`), `screen/audio_linux.rs` (`OwnApplication::allows`).
  - **Целевые файлы:** `rust/crates/discord-voice/src/reception.rs`, `rust/crates/discord-voice/src/devices.rs`, `rust/gui-spike/src/voice_devices.rs`.
  - **Суть:** раздельный микшер громкости участников (`HashMap<UserId, f32>`) и независимого коэффициента громкости аудиопотока демонстрации экрана с мягким лимитером `tanh`, отдельный стерео джиттер-буфер для стрима и фильтрация собственного PID процесса (`application.process.id != std::process::id()`) при захвате звука на Linux.

### Приоритет 3 (P1): Сквозное шифрование DAVE MLS E2EE в голосовом/видео-транспорте (из `ViceVerse-cz/Serein`)
- **Задача 5 (`task-605-dave-mls-e2ee-srtp`, отдел `G5 Voice & Platform`):**
  - **Источник:** `docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs` (`Dave`, `Encryption::seal`, `Encryption::open`, строки 140–310) и `transport.rs` (бинарные Voice Opcodes `21..31`).
  - **Целевые файлы:** `rust/crates/discord-voice/src/srtp.rs`, `rust/crates/discord-voice/src/session.rs`.
  - **Суть:** двухслойный криптоконвейер `DAVE E2EE + SRTP (_rtpsize)` (`dave.encrypt` перед `cipher.seal` и `dave.decrypt` после `cipher.open`) и диспетчеризация MLS-опкодов Voice Gateway (`Op 21..31`).

### Приоритет 4 (P2): Локальный сокет `discord-ipc-0` для Rich Presence (из `rust-discord-activity` и `Serein`)
- **Задача 6 (`task-606-discord-rpc-ipc-socket`, отдел `G5 Voice & Platform`):**
  - **Источник:** `docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs` (`DiscordClient`), `models/activity.rs`, `models/client/payload.rs` и `docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs` (`serve_ipc`, `read_frame`, `write_frame` строки 446–610).
  - **Целевые файлы:** `rust/crates/discord-platform/src/rpc_ipc.rs`, `rust/crates/discord-platform/src/lib.rs`, `rust/gui-spike/src/profile_ext.rs`.
  - **Суть:** сервер и клиент локального RPC-сокета `$XDG_RUNTIME_DIR/discord-ipc-0..9` с бинарным фреймингом `[u32 LE opcode][u32 LE length][JSON payload]` (Op 0 Handshake -> `READY`, Op 1 `SET_ACTIVITY` -> трансляция в Gateway Op 3 `PresenceUpdate`).

### Приоритет 5 (P2): Навигационный фильтр внешних ссылок для WebView (из `japandotorg/LemonCord`)
- **Задача 7 (`task-607-webview-navigation-guard`, отдел `G5 Voice & Platform` / `G1 Backend`):**
  - **Источник:** `docs/reference-projects/sources/LemonCord/src/main.rs` (`discord`, строки 50–99).
  - **Целевые файлы:** `rust/crates/discord-platform/src/deep_link.rs`, `rust/gui-spike/src/main.rs`.
  - **Суть:** безопасный классификатор и перехватчик навигации WebView (`navigation_handler`), удерживающий главное окно внутри локального UI/OAuth-контура и открывающий внешние ссылки в системном браузере.
