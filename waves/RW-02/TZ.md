### ТЗ RW-02 — zstd-stream декодер (прототип), P0-03 / T-ENC-04

**Цель**: инкрементальный декодер непрерывного zstd-потока шлюза Discord поверх `ruzstd 0.9.1`
(вариант B, согласован с владельцем): `push` WS-кадров → новые распакованные байты; контекст
между кадрами сохраняется, `end()` не вызывается; магия только в кадре 0; кадры 2+ продолжают
тот же поток.

**Файлы (только эти)**:
- `rust/crates/discord-codec/src/zstd.rs` — новый модуль (реализация + доккомментарии по-русски).
- `rust/crates/discord-codec/src/lib.rs` — точечная правка: `pub mod zstd;`, re-export
  `{ZstdError, ZstdStream}`, обновить строку статуса (ныне строки 9–10).
- `rust/crates/discord-codec/tests/zstd_stream.rs` — новый тест-файл.
- `rust/crates/discord-codec/tests/data/synth-zstd-frame.hexl` — новая строка тест-данных (hex;
  скопировать из `reference-oracle/gw-proto/synth-zstd-frame.hexl`, сверить md5
  `0a7742ae83a58088f3f80a16be77ad83`).

**Запрещено**: любые другие файлы (другие крейты, `tests/etf_*.rs`, `Cargo.toml`/`Cargo.lock` —
зависимость уже добавлена координатором, `docs/`, `TASKS.md`, `fixtures/`), golden (`logs/`,
`functions/`, `evidence/`), сеть/установки, git-коммиты. Формат — только `cargo fmt -p discord-codec`
(не трогать другие крейты). §8 не трогать.

**Публичный контракт** (`src/zstd.rs`, re-export в `lib.rs`):

```rust
pub enum ZstdError {
    MissingMagic,                    // поток не начинается с 28 b5 2f fd
    Decode { msg: String },          // ошибка ruzstd (Display)
    TrailingData { bytes: usize },   // после завершения кадра остались байты
}
pub struct ZstdStream;
impl ZstdStream {
    pub fn new() -> Self;
    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZstdError>;
    pub fn compressed_consumed(&self) -> u64; // == decoder.bytes_read_from_source()
    pub fn is_finished(&self) -> bool;        // == decoder.is_finished()
}
impl Default for ZstdStream;
impl fmt::Display for ZstdError;  // русские формулировки, в стиле EtfError (см. etf.rs)
impl std::error::Error for ZstdError;
```

**Алгоритм `push`** (проверен пробником координатора `/tmp/opencode/rw-02/probe`; внутреннее
состояние: `ruzstd::decoding::FrameDecoder`, `pending: Vec<u8>`, флаг проверки магии,
переиспользуемый буфер выдачи ≥64 КиБ):

1. `pending.extend_from_slice(frame)`.
2. Магия: пока не проверена — при `pending.len() >= 4` сверить `crate::ZSTD_FRAME_MAGIC`;
   нет магии → `Err(MissingMagic)`; иначе пометить проверенной. При `pending.len() < 4` → `Ok(пусто)`.
3. Цикл: если `pending` пуст — стоп; если декодер ещё не инициализирован и `pending.len() < 18` —
   стоп (18 = максимальный размер заголовка zstd-кадра); вызвать
   `decoder.decode_from_to(&pending, &mut target)`; **продвижение считать дельтой
   `decoder.bytes_read_from_source()`** (не возвращённым кортежем — у ruzstd есть ветка
   с некорректным возвратом при добирании контрольной суммы); `pending.drain(..advanced)`;
   копить выданное; при нулевом прогрессе — стоп.
4. `Err` из ruzstd → `ZstdError::Decode { msg: e.to_string() }` (поток далее — терминальный).
5. После цикла: если `decoder.is_finished()` и `pending` не пуст → `Err(TrailingData { bytes })`.

**Семантика (задокументировать в доккомментариях модуля!)**:
- Возврат `push` — только новые распакованные байты; байты, декодированные в вызове, который
  завершился ошибкой, теряются (поток мёртв после `Err`).
- Удержание окна: пока кадр не завершён, ruzstd выдаёт не более `распаковано − окно`; у потока
  шлюза окно 64 КиБ (window descriptor `0x30` → 2¹⁶). Для сокета 1389: 824 816 − 65 536 = 759 280 Б.
  Это не потеря: байты выдадутся по мере поступления следующих кадров; хвост до закрытия
  соединения удерживается (ограничение ruzstd; кандидат на пересмотр в P1-02).
- Прототип поддерживает один непрерывный кадр; переподключение = новый `ZstdStream`.
- Никаких `unwrap`/паник на входных данных; `unsafe` запрещён линтом.

**Тест-данные**:
- `tests/data/synth-zstd-frame.hexl` — 67 Б, одна строка hex (как в файле lab).
- Payload-оракул: терм (34 Б)
  `8374000000036400026f70610a640001736400036e696c6400017464000450494e47` × 4096 = 139 264 Б;
  md5 payload `4dbd601d877520056a7bcc3ca534ad4a`.

**Кейсы (red-first; имена тестов латиницей, комментарии по-русски; только публичный API крейта)**:

1. `synth_frame_decodes_whole` — push целого кадра → `Ok`, выход == payload (139 264);
   `is_finished()`; `compressed_consumed() == 67`; `push(&[])` после → `Ok(пусто)`.
2. `synth_frame_split_in_three` — куски `[0..7)`, `[7..23)`, `[23..]` → конкатенация выходов ==
   payload; куски 1–2 не начинаются с магии (факт вектора).
3. `synth_frame_byte_by_byte` — 67 пушей по 1 байту → конкатенация == payload.
4. `missing_magic_is_rejected` — на свежем `push(&[0,1,2,3,4])` → `Err(MissingMagic)`.
5. `magic_split_across_pushes` — `push(&f[..2])`, `push(&f[2..4])`, `push(&f[4..])` → == payload.
6. `corrupted_frame_is_reported` — `f[12] ^= 0xff` → `Err(Decode { .. })`, сообщение непусто.
   (Индекс 12 проверен сканом координатора: `FailedToReadBlockBody(MalformedSectionHeader
   { expected_len: 7456, remaining_bytes: 40 })`.)
7. `trailing_bytes_after_finished_frame` — push кадра → `Ok`; `push(&[0])` →
   `Err(TrailingData { bytes: 1 })`.
8. `lab_stream_1389_matches_oracle` (skip-aware) — файл
   `{RE_LAB_ROOT:-reference-oracle/}/gw-proto/zstd-frames-95974_1389.hexl`; нет файла →
   `eprintln!("skip: …")` + return (не падать). Подать все 137 кадров; проверить: строк 137;
   кадр 0 с магией, среди 136 остальных — 0; `out.len() == 759_280`;
   `compressed_consumed() == 104_066`; `!is_finished()`; `out[..138]` == HELLO-hex (ниже);
   жадный разбор `etf::decode`: 44 полных терма, смещение 758 576, остаток 704 Б
   (`etf::decode` на остатке → `Err`).
9. `lab_midstream_socket_is_rejected` (skip-aware) — `{RE_LAB_ROOT}/gw-proto/zstd-frames-95974_8158.hexl`
   (1 кадр, 74 Б, без магии) → `push` → `Err(MissingMagic)`.

**HELLO-hex (138 Б, оракул захвата)**:
`8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c`

**Оракул/провенанс**:
- `zstd-frames-95974_1389.hexl` — снимок из `capture-run2.jsonl` скриптом
  `reference-oracle/gw-extract-frames.mjs`; md5 `9199431aabbb31f0b85145373908826a`;
  сжато 104 066 Б.
- Распаковано 824 816 Б = Σ`bytes` (`gw-proto/msgs-run2-95974_1389.jsonl`) 824 679 + 137 фрейм-байт
  `0x83`; совпало с Node `createZstdDecompress` (независимый оракул).
- 44 терма / граница 758 576 / остаток 704 — из того же msgs (границы термов).
- Синтетика — `reference-oracle/gw-make-synth-zstd.mjs` (Node `zstdCompressSync`, payload без
  данных Discord); кадр 67 Б, payload 139 264 Б, md5 payload `4dbd601d877520056a7bcc3ca534ad4a`.
- `zstd-frames-95974_8158.hexl` — 1 кадр (74 Б) из `capture-run2.jsonl`: первое сообщение
  захвачено не с начала потока (референс Node: «Unknown frame descriptor»).

**Логи**: `/tmp/opencode/rw-02/red.txt` (до реализации), `green.txt`, `clippy.txt`, `fmt.txt`,
`gate.txt`.

**DoD**: red-прогон зафиксирован (падают все 9 новых тестов); green:
`cd rust && cargo test --offline` (весь воркспейс) и `cargo test --offline -p discord-codec`;
`cargo clippy --offline -p discord-codec --all-targets -- -D warnings` чисто;
`cargo fmt -p discord-codec` затем `--check`; `npm run gate:worker` — passed; отчёт §4.2
(+ md5 файлов). §8-строка не требуется (Rust-тесты вне meta-регистра).
