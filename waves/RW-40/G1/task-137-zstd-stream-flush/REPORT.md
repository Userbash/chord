# task-137 / BE-RE-01 (F-GW-01) — ZstdStream отдаёт все распакованные байты сразу

- **batch/карточка:** RW-40 / task-137, код `BE-RE-01` — находка **F-GW-01** из
  `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §2.2 и карточка BE-P1-03 из
  `waves/RW-40/PM/BACKLOG-RE-BACKEND.md` §1.
- **worktree:** `/home/sanya/orca/workspaces/project/feature-task-137-zstd-stream-flush`,
  ветка `feature/task-137-zstd-stream-flush`, HEAD `4e24048`.
- **scope (расширен Team Lead в ходе работы, подтверждено дважды):**
  `rust/crates/discord-codec/src/zstd.rs`, `rust/crates/discord-codec/Cargo.toml`,
  `rust/crates/discord-codec/tests/zstd_stream.rs`, `rust/crates/discord-core/tests/dispatch_router.rs`,
  `rust/crates/discord-core/tests/ws_transport.rs`. Другие файлы не трогались.

## Сделано

1. **Диагноз (почему нельзя починить в рамках ruzstd).** Публичный streaming API `ruzstd` 0.9.1
   (`FrameDecoder::decode_from_to` / `read` / `collect`) отдаёт распакованное только при
   `is_finished()`, то есть когда прочитан `last_block`; до этого `DecodeBuffer` удерживает
   последние `window_size` байт. Слить удержанный хвост, сохранив окно, нельзя: `DecodeBuffer`
   и `block_decoder` — `pub(crate)`, а `drain()`/`drain_to_writer()` обнуляют историю.
   `decode_from_to` с пустым источником тоже возвращает только «за окном».
   Проверено экспериментом (`/tmp/opencode/zexp`, энкодер `zstd` 0.14.3, `WindowLog(16)`,
   `flush` на границе сообщения): 6 сообщений по 39…64 Б дают у ruzstd **0 Б на каждый push**,
   у `zstd::stream::raw::Decoder` (libzstd, `ZSTD_decompressStream`) — ровно сообщение сразу.
2. **Red-тесты** (Strict TDD) добавлены в `src/zstd.rs` (модуль `zstd::tests`):
   - `flush_releases_each_message_immediately` — после каждого `push` выход **равен** распакованному
     сообщению (0 Б задержано), `pending_bytes() == 0`;
   - `flush_keeps_window_history_across_chunks` — накопленный выход равен конкатенации всех
     сообщений; чанки 3…6 сжаты в 13…14 Б (ссылки на историю окна), то есть окно сохраняется;
   - `flush_then_frame_end_releases_everything` — контракт RW-02 не сломан: пустой последний блок
     (`01 00 00`) завершает кадр и отдаёт всё, мусор после кадра — терминальный `TrailingData`.
   Фикстура вшита в тест как hex (новых тестовых зависимостей нет). Провенанс: собран libzstd
   0.14.3, уровень 3, `WindowLog(16)` (окно 64 КиБ как у шлюза), `ChecksumFlag(false)`, кадр не
   завершён, `flush()` на границе каждого сообщения; 6 чанков 48/67/13/14/14/14 Б
   (md5 конкатенации `8e542e5805676d15c93e302be9ee4cbe`), выход 39/64×5 Б
   (md5 `db695b2285bd9b18c54e5314e90aab17`).
3. **Реализация** (`src/zstd.rs`): `ZstdStream` переведён с `ruzstd::FrameDecoder` на потоковый
   декодер libzstd (`zstd::stream::raw::Decoder` + `InBuffer`/`OutBuffer`, `Operation::run`).
   libzstd отдаёт выход в буфер вызывающего и держит окно отдельно — контракт совпадает с
   эталонным `zlib.createZstdDecompress()` из RE-дампа. Сохранено:
   магия только в кадре 0 (`MissingMagic`), один непрерывный кадр, терминальность ошибок
   (`Decode`/`TrailingData` → `Poisoned`), `compressed_consumed()`, `is_finished()`,
   `is_dead()`, переиспользуемый буфер 64 КиБ. Добавлен `pending_bytes()` (DoD BE-P1-03) —
   размер сжатого хвоста, который декодер ещё не принял; в норме 0.
4. **`Cargo.toml` крейта:** `ruzstd` → `zstd = { workspace = true }` (libzstd уже в
   `Cargo.lock` и оффлайн-кэше как dev-зависимость `discord-core`). `Cargo.lock` пересобран:
   из него ушли ставшие неиспользуемыми `ruzstd` и `twox-hash`.
5. **Обновлены тесты-оракулы, закреплявшие баг** (разрешено Team Lead):
   - `discord-codec/tests/zstd_stream.rs::lab_stream_1389_matches_oracle` — теперь 824 816 Б,
     137 полных термов, `pending_bytes() == 0`, остатка нет (было 759 280 Б / 44 терма / 704 Б);
   - `discord-core/tests/dispatch_router.rs::lab_real_stream_routes_known_kinds` — 137 термов,
     113 op0 (s = 1..113), 24 не-диспатча (23 op11 + 1 op10), `LAB_STREAM_COUNTS` заменён на
     полный потиповой оракул сокета (11 типов; сумма 113) по `gw-proto/msgs-run2-95974_1389.jsonl`;
   - `discord-core/tests/ws_transport.rs::lab_capture_stream_feeds_pipe` — 137 термов,
     `last_seq() == Some(113)`, `pending_bytes() == 0`;
   - `discord-core/tests/ws_transport.rs::pipe_buffers_partial_term` — первая половина терма
     теперь честно ждёт в хвосте пайпа (`pending_bytes() == 9`), а не «съедается окном».

## Red

`CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-codec`
→ `test result: FAILED. 3 passed; 2 failed`, exit 101. Падают оба новых теста:
`flush_releases_each_message_immediately` (`push 0: left: ""`, ожидалось 39 Б) и
`flush_keeps_window_history_across_chunks` (накопленный выход пуст). Третий тест
(`flush_then_frame_end_releases_everything`) в red-фазе зелёный: завершение кадра ruzstd тоже
отдавал целиком — он сторожит контракт RW-02, а не F-GW-01.
Лог: `red.log`.

Отдельно, уже после фикса и **до** правки тестов-оракулов, полный `-p discord-core` показал
падения `dispatch_router::lab_real_stream_routes_known_kinds` (137 термов вместо 44) и
`ws_transport::lab_capture_stream_feeds_pipe` (137 вместо 44; `last_seq` 113 вместо 36;
`pending_bytes` 0 вместо 704), `ws_transport::pipe_buffers_partial_term` (`pending_bytes` 9
вместо 0). Все три закрепляли старое удержание окна; после обновления оракулов зелёные.

## Green

`CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-codec -p discord-core`
→ **623 passed, 0 failed**, exit 0 (57 тестовых бинарей + 2 doc-test). Лог: `green.log`.
Ключевые: `zstd::tests::flush_*` (3/3), `lab_stream_1389_matches_oracle`,
`lab_real_stream_routes_known_kinds`, `lab_capture_stream_feeds_pipe`,
`pipe_buffers_partial_term`, все 13 e2e-тестов `ws_transport`.

- `cargo clippy --offline --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings`
  → exit 0 (лог `clippy.log`).
- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` → exit 0 (лог `fmt.log`).
- `gate:worker` не запускался: по процессу полный гейт делает integration/QA на batch
  (`PROCESS.md` §3.13–§3.15), а этот worktree — отдельная карточка.

## Оракул (независимая сверка)

- `RE/gateway-protocol.md` §3: эталонный `zlib.createZstdDecompress()` без `end()` пишет кадры
  последовательно и отдаёт накопленный вывод полностью; `perSocket[].decompressedBytes` для
  `95974.1389` = 824 816 Б.
- `gw-proto/zstd-frames-95974_1389.hexl` (137 кадров, 104 066 сжатых Б) → 824 816 Б, 137 термов,
  `compressed_consumed() == 104_066`, кадр не завершён.
- `gw-proto/msgs-run2-95974_1389.jsonl` (независимый таймлайн): 137 термов = 113 op0 + 23 op11 +
  1 op10; потиповые числа op0 — GUILD_MEMBER_LIST_UPDATE 51, PRESENCE_UPDATE 40, MESSAGE_UPDATE 6,
  GUILD_MEMBERS_CHUNK 4, MESSAGE_CREATE 3, SESSIONS_REPLACE 2, THREAD_LIST_SYNC 2, MESSAGE_ACK 2,
  READY 1, READY_SUPPLEMENTAL 1, CHANNEL_INFO 1; s = 1..113 без разрывов.

## Файлы

- `rust/crates/discord-codec/src/zstd.rs` — `6563379e4a71ec6875ad1b678d31975b`
- `rust/crates/discord-codec/Cargo.toml` — `0708cbb594c521dd30e4b6870159d238`
- `rust/crates/discord-codec/tests/zstd_stream.rs` — `bb8799618a270753621c0bdd0bdd4508`
- `rust/crates/discord-core/tests/dispatch_router.rs` — `8236b56cd3ba488f82226ed205b3b395`
- `rust/crates/discord-core/tests/ws_transport.rs` — `9fd81c876440fd6c2b60a1df7e37d44c`
- `rust/Cargo.lock` — `6ac855a6516669d44d67ec5b5b7da334`
- Логи: `red.log`, `green.log`, `clippy.log`, `fmt.log` (рядом с этим отчётом).

## Риски / что осталось

1. **Новая продуктовая зависимость — C libzstd.** Крейт `discord-codec` больше не «чистый
   Rust»: релизная сборка теперь компилирует `zstd-sys` (bundled C, `cc`). Это осознанное
   решение Team Lead (вариант А согласован); `unsafe` в нашем коде нет, линт крейта соблюдён.
   Если политика «pure Rust на чтении» остаётся в силе — нужен отдельный тикет на форк/вендоринг
   ruzstd с разделением «окно истории» и «буфер выхода».
2. **Устаревшие комментарии в общих файлах (нужна правка Team Lead, не мой write-scope):**
   `rust/crates/discord-codec/src/lib.rs:9-12` («поверх `ruzstd`», «удержание окна — не потеря»),
   `rust/crates/discord-core/src/ws.rs:51-57` (блок «Известное ограничение (RW-02: удержание окна
   ruzstd)», цифры 759 280 / 44 терма / `s = 36`), `rust/Cargo.toml:1-4,32-33` (шапка про ruzstd
   и «энкодер zstd — только для стабов тестов»).
3. **Live-проверка не делалась** (нет согласованного окна): паритет подтверждён только оффлайн —
   на lab-снимке сокета `.1389` и синтетическом flush-потоке. `E3 нет`.
4. **Поведение на потоке, который сменит окно/кадр.** Контракт RW-02 сохранён (один кадр,
   второй кадр/мусор = `TrailingData`, ошибка терминальна), но libzstd, в отличие от ruzstd,
   охотно начинает новый кадр из оставшихся байт; поэтому мы останавливаем цикл на `hint == 0`
   и не кормим остаток декодеру. Если Discord когда-нибудь начнёт слать несколько кадров в
   соединении, это будет видно как `TrailingData`, а не как тихая потеря.
5. `Cargo.lock` потерял `ruzstd`/`twox-hash`. При интеграции с ветками, где ruzstd ещё нужен,
   lock пересоберётся; пакеты остаются в локальном кэше, оффлайн-сборка возможна.
6. В основном чекауте `/home/sanya/orca/project` файлы `discord-codec/*` и
   `discord-core/tests/dispatch_router.rs` не изменялись другими воркерами на момент работы
   (проверено `git status`), пересечения write-scope нет.
