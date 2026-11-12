# RW-41: План интеграции эталонных проектов (`Serein`, `LemonCord`, `rust-discord-activity`) под управлением ECC 2.0 + Orca Orchestration

- **Orca Run ID:** `run_0409327aea82`
- **Конфигурация ECC 2.0:** [`ecc2.toml`](file:///home/user/orca/project/ecc2.toml), [`.agents/rules/ecc-discord-lab-departments.md`](file:///home/user/orca/project/.agents/rules/ecc-discord-lab-departments.md)
- **Эталонная документация:** [`docs/reference-projects/README.md`](file:///home/user/orca/project/docs/reference-projects/README.md), [`SEREIN-FUNCTION-MAP.md`](file:///home/user/orca/project/docs/reference-projects/SEREIN-FUNCTION-MAP.md), [`LEMONCORD-FUNCTION-MAP.md`](file:///home/user/orca/project/docs/reference-projects/LEMONCORD-FUNCTION-MAP.md), [`RUST-DISCORD-ACTIVITY-MAP.md`](file:///home/user/orca/project/docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md)
- **Лимит слотов и памяти:** 4 активных воркера (`<= 5` слотов, `WORKER_RAM_LIMIT <= 25 GB`)
- **Базовая ветка:** `unstable`

---

## Матрица распределения задач по отделам и рабочим веткам

| Ветка воркера | ECC Session | Отдел | Приоритет | Задачи плана | Write-Scope (не пересекается) |
|---|---|---|---|---|---|
| `feature/task-601-go-live-dual-session` | `eaaa77a5` / `caf5ccdc` | **G5 — Voice & Platform** | **P0.1 + P1.5** | 1. Выделенная вторая сессия Go Live (`run_stream_inner` + `announce_video`, Op 12 `Video`, Op 15 `Media Sink Wants`)<br/>5. Двухслойный криптоконвейер `DAVE MLS E2EE + SRTP (_rtpsize)` и Voice Opcodes `21..31` | `rust/crates/discord-voice/src/session.rs`<br/>`rust/crates/discord-voice/src/udp.rs`<br/>`rust/crates/discord-voice/src/srtp.rs` |
| `feature/task-602-rtp-assembler-rtx-rtcp` | `0c8357ad` | **G5 — Voice & Platform** | **P0.2** | 2. Сборщик фрагментированных RTP-кадров H.264 (`Single NAL`, `STAP-A`, `FU-A`) и AV1 (`OBU`) `Assembler::push`, восстановление RTX (`restore_rtx`, RFC 4588) и генерация RTCP `NACK` / `PLI` / `FIR` / `REMB` | `rust/crates/discord-voice/src/media/rtp.rs`<br/>`rust/crates/discord-voice/src/rtcp.rs` |
| `feature/task-603-media-pipeline-60fps-slot` | `756a0974` | **G1 — Backend** | **P0.3** | 3. Паттерн «Latest-Frame Atomic Slot» (`store_frame` / `Watch::poll` 60 FPS), конвейер без обратного давления `sync_channel(2)`, буфер RTX `History` (512 пакетов), адаптивный регулятор битрейта `Rate` по REMB и конвертер `NV12`/`I420 -> RGBA` | `rust/crates/discord-media/src/lib.rs` |
| `feature/task-604-stream-mixer-and-rpc-ipc` | `43a84ffc` / `aa466197` / `0c58b29c` | **G5 — Voice & Platform** | **P1.4 + P2.6 + P2.7** | 4. Раздельный микшер голоса и аудио демонстрации экрана (`Mixer::pop_with_volumes` с `tanh`, анти-эхо по PID `OwnApplication::allows`)<br/>6. Сервер/клиент локального RPC-сокета `discord-ipc-0..9` (`DiscordClient` / `serve_ipc`)<br/>7. Навигационный фильтр внешних ссылок `navigation_handler` (из `LemonCord`) | `rust/crates/discord-voice/src/reception.rs`<br/>`rust/crates/discord-voice/src/devices.rs`<br/>`rust/crates/discord-platform/src/rpc_ipc.rs`<br/>`rust/crates/discord-platform/src/deep_link.rs`<br/>`rust/crates/discord-platform/src/lib.rs` |
