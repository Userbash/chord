# Отчёт по задаче task-143 (карточка BE-RE-05 / находка F-VC-01)

- **batch/карточка**: `RW-40 / task-143 (BE-RE-05 / F-VC-01 — SRTP *_rtpsize raw key & trailer nonce)`
- **сделано**:
  1. Проведена сверка схемы шифрования `aead_aes256_gcm_rtpsize` и `aead_xchacha20_poly1305_rtpsize` по `/home/sanya/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` (§6.2 `F-VC-01`), `/home/sanya/orca/project/waves/RW-40/PM/BACKLOG-RE-BACKEND.md` (`RE-P1-06 + BE-P1-06`), эталонным дампам `/home/sanya/re-deep/voice-deep.json` и `/home/sanya/re-deep/voice-deep.md`, а также по дизассемблированию `discord::SocketClient::SetEncryptionSettings` (`0x3f84f0`), `discord::encryption::PacketCrypter::Encrypt` (`0x46d6c0`) и `discord::encryption::PacketCrypter::Decrypt` (`0x46d4c0`) в нативном модуле `discord_voice.node`.
  2. По Strict TDD сначала добавлены модульные red-тесты в `rust/crates/discord-voice/src/srtp.rs` (`srtp::tests`), проверяющие побайтовый формат пакета `*_rtpsize` (`[RTP header AAD][ciphertext][16B AEAD tag][4B LE nonce trailer]`) с прямым 32-байтным `secret_key` из OP 4 `SESSION_DESCRIPTION` и `nonce` из 4-байтного трейлера с нулями в старших байтах (`12 Б` для `AES-256-GCM`, `24 Б` для `XChaCha20-Poly1305`), а также round-trip шифрование/расшифровку между двумя независимыми контекстами и приём пакета от медиа-сервера. Зафиксирован red-лог (`exit code 101`, 3 упавших теста).
  3. Реализована поддержка канонического формата `*_rtpsize` в `rust/crates/discord-voice/src/srtp.rs`:
     - в режимах `aead_aes256_gcm_rtpsize` и `aead_xchacha20_poly1305_rtpsize` (`mode.covers_header()`) 32-байтный `secret_key` используется **напрямую** как ключ AEAD (`AES-256-GCM` / `XChaCha20-Poly1305`) без промежуточного `HKDF-SHA1`;
     - 12-байтный (для `AES-256-GCM`) и 24-байтный (для `XChaCha20-Poly1305`) `nonce` формируется из 4-байтного LE-трейлера `nonce_counter` в младших байтах `nonce[0..4]` с нулями в старших байтах (`nonce[4..12] = 0` и `nonce[4..24] = 0`), а открытый заголовок RTP/SRTCP передаётся как AAD;
     - добавлены конструкторы `SrtpContext::from_secret_key` и `SrtpContext::from_secret_key_and_mode` для прямой инициализации из OP 4 `secret_key: [u8; 32]`, при сохранении обратной совместимости сигнатур `SrtpContext::new` и `SrtpContext::from_engine_name`.
  4. Проверены все тесты крейта `discord-voice` (`144 passed, 0 failed`), `cargo clippy --offline -p discord-voice --all-targets -- -D warnings` и `cargo fmt --check`.
- **red**: `cd rust && cargo test --offline -p discord-voice --lib` → `FAILED. 6 passed; 3 failed` (`exit code 101`).
- **green**: `cd rust && cargo test --offline -p discord-voice` → `ok. 144 passed; 0 failed` (`exit code 0`: 9 unit + 135 integration во всех 10 тест-раннерах крейта `discord-voice`).
- **targeted**:
  - `srtp::tests::aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce` — ok
  - `srtp::tests::xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce` — ok
  - `srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key` — ok
  - `tests/srtp_contract.rs` (22 контракта) — ok
  - `tests/crypto_vectors.rs` (16 KAT-векторов) — ok
- **oracle**:
  - `/home/sanya/re-deep/voice-deep.json` и `/home/sanya/re-deep/voice-deep.md` (§3 Режимы шифрования, строки движка `aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`, исходник `/workspace/discord_voice/dave/src/discord/packet_crypter.cpp`);
  - `/home/sanya/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` (§6.2 `F-VC-01`);
  - дизассемблирование `discord_voice.node`:
    - `discord::SocketClient::SetEncryptionSettings` (`0x3f84f0`): проверяет `secret_key.size() == 32`, копирует 32-байтный `secret_key` напрямую в `PacketCrypter::State` (`offset 0x04..0x24`) без KDF/соли и инициализирует `state.nonce = 0x80000000` (`offset 0x24`);
    - `discord::encryption::PacketCrypter::Encrypt` (`0x46d6c0`): обнуляет 24-байтный буфер `nonce` на стеке (`xorps %xmm0, %xmm0`), записывает 4-байтный `u32` счётчик в `nonce[0..4]`, вызывает `crypto_aead_aes256gcm_encrypt` / `crypto_aead_xchacha20poly1305_ietf_encrypt` с прямым 32-байтным ключом и `AAD = packet[..header_len]`, затем дописывает 4 байта `nonce[0..4]` в конец пакета;
    - `discord::encryption::PacketCrypter::Decrypt` (`0x46d4c0`): обнуляет 24-байтный буфер `nonce`, копирует последние 4 байта пакета (`trailer`) в `nonce[0..4]` и расшифровывает `packet[header_len..len-4]` с `AAD = packet[..header_len]`.
- **генерация**: не применимо
- **мутации**: не запускались в воркер-лейне (по процессу выполняются отдельным агентом)
- **baseline/restore**: не применимо
- **стабильность**: детерминированные криптографические тесты без таймеров/сетевого I/O; повторы не требовались
- **gate:worker**: в рамках воркер-задачи проверены `cargo test --offline -p discord-voice` (exit 0), `cargo clippy --offline -p discord-voice --all-targets -- -D warnings` (exit 0) и `cargo fmt --check` (exit 0)
- **файлы**:
  - `rust/crates/discord-voice/src/srtp.rs` — `aabb1c02af3c7b9366368d6c1215fb80`
- **риски/вопросы**: нет; все существующие тесты `discord-voice` (включая `tests/srtp_contract.rs`) зелёные, предупреждений `clippy`/`dead_code` нет.
- **следующий шаг**: передать координатору / Team Lead на приёмку карточки `task-143` (`BE-RE-05`).

---

## Приложение A. Red-прогон (до реализации)

Команда:
```sh
cd rust && cargo test --offline -p discord-voice --lib
```
Exit code: `101`

Вывод:
```text
running 9 tests
test reception::tests::extension_sequence_wraps_forward ... ok
test reconnect::tests::attempt_saturates_at_u32_max_without_overflow ... ok
test rtcp::tests::i24_sign_extension ... ok
test reconnect::tests::backoff_index_is_clamped_to_last_entry ... ok
test stats::tests::ntp_middle_is_high_word ... ok
test tests::voice_constants_match_re ... ok
test srtp::tests::xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce ... FAILED
test srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key ... FAILED
test srtp::tests::aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce ... FAILED

failures:

---- srtp::tests::xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce stdout ----

thread 'srtp::tests::xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce' (1531426) panicked at crates/discord-voice/src/srtp.rs:690:9:
assertion `left == right` failed: aead_xchacha20_poly1305_rtpsize должен формировать [RTP header AAD][ciphertext][16B tag][4B trailer] на прямом 32B secret_key и 24B nonce = [trailer, 0..0]

---- srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key stdout ----

thread 'srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key' (1531425) panicked at crates/discord-voice/src/srtp.rs:715:22:
receiver decrypt_rtp: AuthenticationFailed

---- srtp::tests::aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce stdout ----

thread 'srtp::tests::aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce' (1531424) panicked at crates/discord-voice/src/srtp.rs:658:9:
assertion `left == right` failed: aead_aes256_gcm_rtpsize должен формировать [RTP header AAD][ciphertext][16B tag][4B trailer] на прямом 32B secret_key и 12B nonce = [trailer, 0..0]

test result: FAILED. 6 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

---

## Приложение B. Green-прогон и проверки качества

Команды:
```sh
cd rust && cargo test --offline -p discord-voice
cd rust && cargo clippy --offline -p discord-voice --all-targets -- -D warnings
cd rust && cargo fmt --check
```
Exit codes: `0`, `0`, `0`.
