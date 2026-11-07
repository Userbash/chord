# Отчёт воркера — TASK-615: RTP H.264/AV1 Video Packetizer и DAVE MLS v1 (`G5 Voice & Platform`)

- **Карточка / Задача:** `TASK-615` (`task_ab20b2b54613`, dispatch `ctx_c4357b09400f`)
- **Отдел / Роль:** `G5 Voice & Platform` (Voice Lead)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/task-615-rtp-video-dave`
- **Ветка:** `feature/task-615-rtp-video-dave`

---

## 1. Сделано

1. **Сверка с эталонным RE-оракулом (`/home/sanya/re-deep/voice-deep.md` и `docs/re/voice-deep.md`):**
   - Подтверждены константы видеокодеков из `default_client_codecs`: `AV1` (`PT=101`, `RTX PT=102`), `H265` (`PT=103`, `RTX PT=104`), `H264` (`PT=105`, `RTX PT=106`), `VP8` (`PT=107`, `RTX PT=108`), тактовая частота видео `90_000` Гц (`3_000` тиков/кадр для 30 fps, `1_500` тиков/кадр для 60 fps).
   - Подтверждены опкоды видеостриминга основного шлюза Discord `18..=22`: `STREAM_CREATE (18)`, `STREAM_DELETE (19)`, `STREAM_WATCH (20)`, `STREAM_PING (21)`, `STREAM_SET_PAUSED (22)` и формат `StreamKey` (`guild:{guild_id}:{channel_id}:{owner_id}`, `call:{channel_id}:{owner_id}`).
2. **Реализован модуль `rust/crates/discord-voice/src/rtp/video.rs` (и `src/rtp/mod.rs`):**
   - **H.264 (RFC 6184):**
     - Разбор Annex-B потоков (`parse_h264_annex_b`) с поддержкой 4-байтных (`00 00 00 01`) и 3-байтных (`00 00 01`) стартовых кодов, проверкой `forbidden_zero_bit == 0` и `nal_unit_type` в `1..=23`.
     - Упаковка одиночных NAL-единиц (`SingleNalu`, `<= 1200` байт).
     - Агрегация нескольких малых NAL-единиц (например `SPS (7)` + `PPS (8)`) в пакет `STAP-A` (`type = 24`, `[max_nri | 24][u16 len_be][nal]...`, `encode_h264_stap_a`).
     - Фрагментация крупных кадров/NAL-единиц (`> 1200` байт) через `FU-A` (`type = 28`, `[fu_indicator = (F|NRI)|28][fu_header = (S<<7)|(E<<6)|nal_type][fragment]`, `encode_h264_fu_a`) с установкой бита RTP `marker = true` строго на последнем пакете кадра.
   - **AV1 (RTP Payload Format for AV1 v1.0):**
     - Разбор саморазграничивающихся потоков OBU (`parse_av1_obu_stream` с поддержкой `obu_extension_flag` и `LEB128` размеров).
     - Пакетизация `packetize_av1_obus_with_mtu` / `packetize_av1_bitstream_with_mtu` с формированием заголовка агрегации `Z/Y/W/N`:
       - `N = 1` (`0x08`) на первом пакете ключевого кадра (`OBU_SEQUENCE_HEADER = 1`);
       - агрегация малых OBU в один RTP-пакет (`W = 1..=3`, либо `W = 0` с LEB128-длинами всех элементов при 4+ OBU);
       - фрагментация крупных OBU (`> 1200` байт с заголовком) на серию пакетов с флагами продолжения `Z = 0x80` и `Y = 0x40`.
   - **Состоятельный пакетизатор `RtpVideoPacketizer` и `VideoRtpPacket`:**
     - Авто-инкремент `sequence` (с переходом через `u16::MAX -> 0`), сдвиг 90 кГц `timestamp`, сохранение однобайтового расширения заголовка RFC 5285 `0xBEDE`, генерация пакетов ретрансляции RFC 4588 RTX (`build_rtx_packet` / `encode_rtx` с 2-байтным big-endian `OSN`), 100% совместимость со сборщиком `VideoFrameAssembler` (`media::rtp`).
   - **Конструкторы JSON-кадров стриминга `18..=22`:** `build_stream_create_guild`, `build_stream_create_call`, `build_stream_delete`, `build_stream_watch`, `build_stream_ping`, `build_stream_set_paused`.
3. **Интегрировано сквозное двухслойное шифрование видеокадров DAVE MLS v1 в `discord_voice::dave` (`rust/crates/discord-voice/src/dave.rs`):**
   - Экспортированы типы и константы DAVE MLS v1 (`DAVE_PROTOCOL_VERSION_V1`, `DAVE_GCM_TAG_LEN`, `DAVE_MARKER_BYTES`, `DaveMlsEngine`, `DaveMediaCipher`, `DaveFrameCipher`, `DaveStrippedRtpPacket`, `DaveTransitionOutcome`, `has_dave_magic_marker`).
   - Добавлены методы двухслойного шифрования/дешифрования видеопотоков на `DaveMlsEngine`:
     - `encrypt_video_rtp_packets` — внутренний слой AES-256-GCM эпохи MLS с магическим маркером `[16B tag][leb128(nonce)][size][0xFA, 0xFA]` + внешний слой SRTP (`aead_xchacha20_poly1305_rtpsize` / `aead_aes256_gcm_rtpsize`) с открытым заголовком RTP и расширением `0xBEDE` в качестве AAD;
     - `packetize_and_encrypt_h264` и `packetize_and_encrypt_av1`;
     - `decrypt_video_rtp_packet` и `decrypt_and_assemble_video_packet` — двухслойная расшифровка с проверкой защиты от повтора (`SrtpError::NonceReuse`), проверкой тега аутентификации (`SrtpError::AuthenticationFailed`), снятием расширения `0xBEDE` и подачей в `VideoFrameAssembler`.

---

## 2. Red-прогон (Strict TDD)

- **Тестовый файл:** `rust/crates/discord-voice/tests/rtp_video_contract.rs`
- **Команда:**
  ```sh
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice --test rtp_video_contract
  ```
- **Лог:** `waves/RW-40/G5/task-615-rtp-video-dave/red.log`
- **Результат Red-фазы (до реализации `rtp::video` и интеграции в `dave`):**
  ```text
  error[E0433]: cannot find `rtp` in `discord_voice`
    --> crates/discord-voice/tests/rtp_video_contract.rs:11:20
  error[E0432]: unresolved imports `discord_voice::dave::DAVE_MARKER_BYTES`, `discord_voice::dave::DAVE_PROTOCOL_VERSION_V1`, `discord_voice::dave::DaveMlsEngine`, `discord_voice::dave::has_dave_magic_marker`
  error: could not compile `discord-voice` (test "rtp_video_contract") due to 2 previous errors
  ```

---

## 3. Green-прогон, Targeted и Гейт пакета

- **Targeted-прогон (`rtp_video_contract`):**
  ```sh
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice --test rtp_video_contract
  ```
  Результат: **`8 passed; 0 failed`**.
- **Полный прогон пакета `discord-voice` (`waves/RW-40/G5/task-615-rtp-video-dave/green.log`):**
  ```sh
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice
  ```
  Результат: **`225 passed; 0 failed`** (включая `35` unit-тестов и `190` интеграционных/контрактных тестов по всем 11 тестовым наборам `discord-voice`).
- **Форматирование и статический анализ (`rustfmt` + `clippy`):**
  ```sh
  rustfmt --edition 2024 --check rust/crates/discord-voice/src/dave.rs rust/crates/discord-voice/src/rtp/mod.rs rust/crates/discord-voice/src/rtp/video.rs rust/crates/discord-voice/tests/rtp_video_contract.rs
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --all-targets -- -D warnings
  ```
  Результат: **`0 warnings, 0 errors`**.

---

## 4. Мутационное тестирование (`5/5 KILLED`)

Лог: `waves/RW-40/G5/task-615-rtp-video-dave/mutations.log`.

| ID | Файл | Мутация | Статус | Убивающий тест |
|---|---|---|---|---|
| `M1` | `src/rtp/video.rs` | Граница FU-A `if nalu.len() > max_payload_len` → `>= max_payload_len` | **KILLED** | `h264_single_nalu_and_exact_1200_vs_1201_fu_a_boundary` |
| `M2` | `src/rtp/video.rs` | Пропуск бита конца фрагмента `H264_FU_HEADER_END (0x40)` в `encode_h264_fu_a` | **KILLED** | `h264_single_nalu_and_exact_1200_vs_1201_fu_a_boundary`, `h264_stap_a_and_multi_fragment_fu_a_roundtrip_with_video_frame_assembler` |
| `M3` | `src/rtp/video.rs` | Порядок байт длины NALU в `STAP-A`: `to_be_bytes()` → `to_le_bytes()` | **KILLED** | `h264_stap_a_and_multi_fragment_fu_a_roundtrip_with_video_frame_assembler` |
| `M4` | `src/rtp/video.rs` | Обнуление флага продолжения `AV1_AGGR_Z_CONTINUATION (0x80)` при фрагментации OBU | **KILLED** | `av1_single_multi_w2_w3_w0_and_fragmented_obu_roundtrip`, `dave_mls_v1_two_layer_video_encryption_roundtrip_h264_and_av1` |
| `M5` | `src/dave.rs` | Инверсия проверки магического маркера `0xFAFA` в `has_dave_magic_marker` | **KILLED** | `dave_mls_v1_two_layer_video_encryption_roundtrip_h264_and_av1`, `negative_and_boundary_cases_are_rejected_with_typed_errors` |

Контрольные суммы MD5 до и после мутаций полностью совпадают:
- `a859d6b8427a51fece7bf1866e0ce289  rust/crates/discord-voice/src/rtp/video.rs`
- `baec8e28981e824e9a01b2b1cb09944e  rust/crates/discord-voice/src/dave.rs`

---

## 5. Изменённые и созданные файлы + MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/lib.rs` | modified | `8095ced4fe3b4b01e8eb3cb9873f67d6` |
| `rust/crates/discord-voice/src/dave.rs` | modified | `baec8e28981e824e9a01b2b1cb09944e` |
| `rust/crates/discord-voice/src/rtp/mod.rs` | added | `6d90f845abc89f19703064bd9280de6b` |
| `rust/crates/discord-voice/src/rtp/video.rs` | added | `a859d6b8427a51fece7bf1866e0ce289` |
| `rust/crates/discord-voice/tests/rtp_video_contract.rs` | added | `de1b1a90096739a2dd7a94c0c04117b3` |
| `waves/RW-40/G5/task-615-rtp-video-dave/red.log` | added | артефакт волны |
| `waves/RW-40/G5/task-615-rtp-video-dave/green.log` | added | артефакт волны |
| `waves/RW-40/G5/task-615-rtp-video-dave/mutations.log` | added | артефакт волны |
| `waves/RW-40/G5/task-615-rtp-video-dave/REPORT.md` | added | артефакт волны |

---

## 6. Риски и примечания

- `#![forbid(unsafe_code)]` в крейте `discord-voice` полностью сохранён.
- Внешние зависимости и `Cargo.toml` не изменялись.
- Пакеты H.264 (Single NALU, STAP-A, FU-A) и AV1 OBU побайтово совместимы с приёмным депакетизатором `VideoFrameAssembler` и двухслойным конвейером `DaveMlsEngine` (`0xFAFA` + `_rtpsize`).
