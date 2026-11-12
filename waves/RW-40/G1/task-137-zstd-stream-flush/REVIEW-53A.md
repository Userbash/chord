# Review 53A — task-137 (F-GW-01): потоковый flush `ZstdStream`

- **Ревьюер:** Технический ревьюер (DeepSeek), RW-40 / G1.
- **Карточка:** task-137 (BE-RE-01), находка F-GW-01 из `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §2.2.
- **Ветка / worktree:** `feature/task-137-zstd-stream-flush`, HEAD `4e24048`, worktree
  `/home/user/orca/workspaces/project/feature-task-137-zstd-stream-flush`.
- **Дата:** 2026-10-30.
- **Вердикт: PASS / Ready for Merge** — P0 = 0, P1 = 0, P2 = 1 (доки вне write-scope),
  P3 = 3. Целевые тесты 623/623, мутации 5/5 killed, red-фаза воспроизведена независимо.

## 1. Что проверено

| Файл | md5 (совпадает с REPORT.md) |
|---|---|
| `rust/crates/discord-codec/src/zstd.rs` | `6563379e4a71ec6875ad1b678d31975b` |
| `rust/crates/discord-codec/Cargo.toml` | `0708cbb594c521dd30e4b6870159d238` |
| `rust/crates/discord-codec/tests/zstd_stream.rs` | `bb8799618a270753621c0bdd0bdd4508` |
| `rust/crates/discord-core/tests/dispatch_router.rs` | `8236b56cd3ba488f82226ed205b3b395` |
| `rust/crates/discord-core/tests/ws_transport.rs` | `9fd81c876440fd6c2b60a1df7e37d44c` |
| `rust/Cargo.lock` | `6ac855a6516669d44d67ec5b5b7da334` |

Проверял: реализацию (`ZstdStream` на `zstd::stream::raw::Decoder` + `InBuffer`/`OutBuffer`),
зависимость крейта, три новых юнит-теста `flush_*`, обновлённые тесты-оракулы в `discord-core`,
отчёт `waves/RW-40/G1/task-137-zstd-stream-flush/REPORT.md`, логи `red.log`/`green.log`/
`clippy.log`/`fmt.log`, формулировку F-GW-01.

Отдельно: файлы `rust/crates/discord-codec/src/lib.rs`, `rust/crates/discord-core/src/ws.rs`,
`rust/Cargo.toml` вне write-scope воркера — не правились, см. F-01.

## 2. Независимая проверка (не по реализации)

1. **Синтетическая flush-фикстура.** Node 26.13 `zlib.createZstdDecompress()` по одному
   чанку на push: выход 39/64/64/64/64/64 Б, то есть ровно по сообщению на push; md5 конкатенации
   сжатых `8e542e5805676d15c93e302be9ee4cbe`, распакованных `db695b2285bd9b18c54e5314e90aab17`,
   длины сжатых 48/67/13/14/14/14 — всё совпало с REPORT.md. `end()` Node не вызывал: поток
   намеренно не завершён, финальный `end()` даёт `unexpected end of file` — это и есть эталон
   шлюза.
2. **Lab-снимок `gw-proto/zstd-frames-95974_1389.hexl`.** Тот же Node-декодер: 137 кадров,
   104 066 сжатых Б, **824 816 Б** распаковки, первый байт `0x83`. Это число — независимое
   подтверждение новой границы в тестах (было 759 280).
3. **Оракул роутера.** `msgs-run2-95974_1389.jsonl` разобран отдельным скриптом: 137 термов,
   ops `{0: 113, 10: 1, 11: 23}`, `s = 1..113` без разрывов, 11 типов op0 —
   GUILD_MEMBER_LIST_UPDATE 51, PRESENCE_UPDATE 40, MESSAGE_UPDATE 6, GUILD_MEMBERS_CHUNK 4,
   MESSAGE_CREATE 3, SESSIONS_REPLACE 2, THREAD_LIST_SYNC 2, MESSAGE_ACK 2, READY 1,
   READY_SUPPLEMENTAL 1, CHANNEL_INFO 1 (сумма 113). Новая таблица `LAB_STREAM_COUNTS`
   совпадает с этим источником один в один; `assert_type_counts` сверяет и число типов, и
   каждое значение, поэтому подгонка «под реализацию» исключена.
4. **Red-фаза воспроизведена.** Взял HEAD-реализацию (`ruzstd`) + текущий тест-модуль
   (с заглушкой `pending_bytes()` на накопителе, иначе тест не компилируется) и прогнал
   `cargo test -p discord-codec`: `3 passed; 2 failed` — падают
   `flush_releases_each_message_immediately` и `flush_keeps_window_history_across_chunks`,
   третий (`flush_then_frame_end_releases_everything`) зелёный. Совпало с `red.log` и с
   объяснением в отчёте, что третий тест сторожит контракт RW-02, а не F-GW-01.
   Лог: `mutants/red-replay.log`.

## 3. Тесты и линты (прогон ревьюера)

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline \
  --manifest-path rust/Cargo.toml -p discord-codec -p discord-core
→ 623 passed, 0 failed, exit 0 (59 «test result: ok», 0 warning)
```

- `zstd::tests::flush_*` — 3/3 ok; `lab_stream_1389_matches_oracle`,
  `lab_real_stream_routes_known_kinds`, `lab_capture_stream_feeds_pipe`,
  `pipe_buffers_partial_term` — ok.
- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy -p discord-codec -p discord-core --all-targets -- -D warnings` → exit 0.
- Лог: `review-green.log`. Полный `gate:worker` воркер не запускал — по процессу его делает
  integration/QA на batch; замечаний к этому нет.

## 4. Мутационные проверки (5/5 killed)

Каждая мутация применялась к `src/zstd.rs`, гонялся тот же целевой набор
(`-p discord-codec -p discord-core`), файл восстанавливался и сверялся по md5
(`6563379e…` до и после). Логи — `mutants/M*.log`.

| # | Мутация | Результат | Убийцы |
|---|---|---|---|
| M1 | Вернуть удержание: отдавать выход только после завершения кадра | exit 101, 3 failed | `flush_releases_each_message_immediately`, `flush_keeps_window_history_across_chunks`, `flush_then_frame_end_releases_everything` |
| M2 | Терять историю окна: пересоздавать `Decoder` на каждой итерации | exit 101, 3 failed | те же три |
| M3 | Не фиксировать конец кадра (убрать ветку `hint == 0`) | exit 101, 1 failed | `flush_then_frame_end_releases_everything` |
| M4 | Убрать `TrailingData` (молча терять байты после кадра) | exit 101, 1 failed | `flush_then_frame_end_releases_everything` |
| M5 | `pending.drain(..offset)` → `pending.clear()` | exit 101, 1 failed | `flush_then_frame_end_releases_everything` |

Итог: **killed 5/5**. Ни одна мутация не выжила.

## 5. Что проверено по существу кода

- `push` крутит `ZSTD_decompressStream` по остатку накопителя (`offset` растёт на `input.pos()`),
  выход копируется в результат сразу — история окна остаётся внутри `ZSTD_DStream`. Это
  ровно контракт эталонного `createZstdDecompress()` без `end()`.
- Условие выхода корректно: `hint == 0` → кадр дочитан; `taken == 0 && produced == 0` →
  вход исчерпан. Полный буфер выдачи (64 КиБ) не обрезает выход: цикл продолжается с пустым
  входом, пока декодер отдаёт байты. Проверено на lab-потоке 824 816 Б.
- Терминальность сохранена: `Decode`/`TrailingData` → `dead`, дальше `Poisoned`;
  ошибка после кадра не «оживает». `pending.drain(..offset)` при `TrailingData` оставляет
  ровно хвост, байты не теряются молча.
- Магия проверяется только на первых 4 байтах потока; `MissingMagic` на префиксе короче 4 Б
  не выдумывается — поведение прежнее, покрыто старыми тестами `zstd_stream.rs`.
- `compressed_consumed()` — сумма принятых байт, на снимке 104 066; совпадает с суммой длин
  кадров.
- `unsafe_code = "forbid"` в `discord-codec/Cargo.toml` соблюдён, `unsafe` в нашем коде нет.
- Тесты не ослаблены: в диффе нет новых `.only`/`skip`, нет удалённых проверок; изменены
  только числа оракулов (подтверждены независимыми источниками, §2) и doc-комментарии.

## 6. Находки

### F-01 (P2, владелец — Team Lead, вне write-scope воркера). Доки противоречат коду

После перехода на libzstd в общих файлах остались утверждения про `ruzstd` и удержание окна:

- `rust/crates/discord-codec/src/lib.rs:9-13` — «поверх `ruzstd`», «удержание окна … не потеря»;
- `rust/crates/discord-core/src/ws.rs:51-57` — раздел «Известное ограничение (RW-02: удержание
  окна ruzstd)» с числами 759 280 / 44 терма / `s = 36`; `ws.rs:343` — «удержания окна» в
  описании `pending_bytes`;
- `rust/Cargo.toml:1-5,32-33` — шапка про «ruzstd 0.9.1» и «`zstd` — только для стабов тестов»;
  после этой задачи `zstd` — продуктовая зависимость `discord-codec`.

Это не ломает поведение, но прямо описывает снятый баг и провоцирует откат. Правит Team Lead
в integration-патче (файлы общие). Вердикт не блокирует: P1 нет.

### F-02 (P3). В worktree изменён `package-lock.json`, не относящийся к задаче

`git status` показывает `M package-lock.json` (+6 строк: `bin`, `engines`). Правка появилась
в 21:48:37, до работы по zstd (21:57) и совпадает по времени с provisioning worktree;
`package.json` в HEAD уже содержит эти поля, то есть lock просто синхронизировали. В
REPORT.md написано «другие файлы не трогались» — формально верно, но при коммите всего
worktree файл уедет в task-137. Координатору: не включать в коммит задачи или оформить
отдельным «lock sync».

### F-03 (P3). `red.log` снят с более ранней версии теста

В `red.log` есть `warning: unused variable: message` (`zstd.rs:298`), а в финальном тесте
переменная используется в `assert_eq!`. То есть лог — от промежуточной редакции теста.
Red-first это не отменяет: я воспроизвёл падение независимо (§2.4). Просьба на будущее —
перезаписывать `red.log` финальной версией теста, иначе след не сходится с кодом.

### F-04 (P3). `pending_bytes()` не покрыт ненулевым значением

Все проверки — `pending_bytes() == 0`. По факту libzstd забирает частичные заголовки в
собственный буфер, поэтому ненулевой накопитель возможен только на пути ошибки. Геттер нужен
по DoD BE-P1-03 и безвреден, но тест `push` с заведомо битым блоком + `assert!(pending_bytes() > 0)`
сделал бы его не тавтологичным. Не блокер.

### Наблюдение (не находка)

`Decoder::new().expect("ZSTD_createDStream не может отказать…")` — единственная паника в
конструкторе (OOM). Раньше паники не было вовсе; для инициализации это приемлемо, но
фразу «Паник нет» в doc-комментарии `push` стоит читать как «нет паник на входных данных».

## 7. Остаточные риски (приняты, не блокируют)

1. **C-зависимость libzstd.** `discord-codec` больше не pure Rust: релизная сборка тянет
   `zstd-sys` (bundled C, `cc`). Решение Team Lead (вариант А), зафиксировано в REPORT.md
   §Риски. Если политика «pure Rust на чтении» вернётся — нужен отдельный тикет.
2. **Live-проверки не было** (`E3 нет`): паритет подтверждён оффлайн — lab-снимок `.1389`
   и синтетический flush-поток. Живой Discord в этой задаче не поднимался, сеть не трогалась.
3. **Один кадр на соединение.** Контракт RW-02 сохранён: второй кадр/мусор после `last_block`
   — терминальный `TrailingData`, тихой потери нет. Если Discord начнёт слать несколько
   кадров, это будет видно как ошибка, а не как пропуск данных.
4. **`Cargo.lock` потерял `ruzstd`/`twox-hash`.** При мерже с ветками, где ruzstd ещё нужен,
   lock пересоберётся; пакеты в локальном кэше есть (`ruzstd-0.9.1.crate`), оффлайн-сборка
   возможна — проверено red-реплеем.

## 8. Чек-лист мержа

- [x] Находка F-GW-01 закрыта функционально: 824 816 Б / 137 термов / 113 диспатчей вместо
      759 280 / 44 / 36.
- [x] Оракулы обновлены по независимым источникам, а не по выходу реализации.
- [x] Red-фаза воспроизводима, green 623/623, clippy/fmt чисто.
- [x] Мутации 5/5 killed.
- [x] Нет ослабления тестов, `.only`/`skip` не добавлены.
- [ ] F-01: обновить доки в `lib.rs`, `ws.rs`, `rust/Cargo.toml` (Team Lead).
- [ ] F-02: не тащить `package-lock.json` в коммит задачи.

**Итог: PASS / Ready for Merge** (P0 = 0, P1 = 0).
