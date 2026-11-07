# REVIEW-56A — task-143 (BE-RE-05 / F-VC-01: каноническое шифрование SRTP `*_rtpsize`)

- **Карточка / задача:** `RW-40 / G5 / task-143` (`BE-RE-05`, закрытие находки `F-VC-01` из `RE-GAP-BACKEND-PROTOCOL.md` §6.2 и `RE-P1-06 + BE-P1-06` из `BACKLOG-RE-BACKEND.md`)
- **Ветка:** `feature/task-143-srtp-raw-key-trailer-nonce` (HEAD `4e24048`)
- **Проверяемый файл:** `rust/crates/discord-voice/src/srtp.rs` (md5 `aabb1c02af3c7b9366368d6c1215fb80`)
- **Вердикт:** **PASS / Ready for Merge** (`P0 = 0`, `P1 = 0`, `P2 = 0`, `P3 = 3`)

---

## 1. Сверка диффа и RE-оракула (`F-VC-01`)

Независимо проверены отчёт `REPORT.md`, находка `F-VC-01` в `RE-GAP-BACKEND-PROTOCOL.md` (§6.2), дампы `/home/sanya/re-deep/voice-deep.json` / `voice-deep.md` и дизассемблирование нативного модуля `discord_voice.node` (`objdump -d -C`):

1. **Прямой 32-байтный `secret_key` из OP 4 `SESSION_DESCRIPTION` без HKDF:**
   - Оракул (`discord::SocketClient::SetEncryptionSettings` @ `0x3f84f0`): проверяет `secret_key.size() == 0x20` (32 Б), копирует 32 байта ключа напрямую в состояние `PacketCrypter` (`offset 0x04..0x24`) без KDF/соли и инициализирует исходящий счётчик `0x80000000` (`movl $0x80000000, 0x24(%rsp)`).
   - Код (`srtp.rs:218-245`): в режимах `mode.covers_header()` (`aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`) `session_key = *master_key` передаётся напрямую в `Aes256Key::from_array` и `XChaCha20Poly1305::new`. Добавлены конструкторы `SrtpContext::from_secret_key` и `SrtpContext::from_secret_key_and_mode`.
2. **Формирование nonce из 4-байтного LE-трейлера `nonce_counter` с нулями в старших байтах:**
   - Оракул (`discord::encryption::PacketCrypter::Encrypt` @ `0x46d6c0`, `Decrypt` @ `0x46d4c0`): обнуляет 24-байтный буфер nonce на стеке (`xorps %xmm0, %xmm0`), записывает 4-байтный LE-счётчик в `nonce[0..4]`, вызывает `crypto_aead_aes256gcm_encrypt` (12 Б nonce) или `crypto_aead_xchacha20poly1305_ietf_encrypt` (24 Б nonce) и дописывает 4 байта `nonce[0..4]` в конец пакета; при расшифровке читает последние 4 байта пакета в `nonce[0..4]` при нулевых `nonce[4..]`.
   - Код (`srtp.rs:550-570`): `gcm_nonce` при `self.mode.covers_header()` кладёт `trailer` в `nonce[0..4]` и оставляет `nonce[4..12] = 0`; `xchacha_nonce` кладёт `trailer` в `nonce[0..4]` и при `self.mode.covers_header()` оставляет `nonce[4..24] = 0`.
3. **Заголовок RTP/SRTCP как AAD и раскладка кадра:**
   - Раскладка `[clear RTP/SRTCP header (AAD)][ciphertext][16B AEAD tag][4B LE nonce_counter]` сохранена для RTP (`clear_len = rtp_header_len(packet)`, включая CSRC и расширение `0xBEDE`) и SRTCP (`SRTCP_CLEAR_LEN = 8`).
   - Защита от повтора nonce на приёме (`check_receive_trailer` / `remember_receive_trailer`, окно `1024`) и запрет перемотки счётчика назад (`CounterRewind`) работают без регрессий.

---

## 2. Независимые прогоны тестов и статического анализа

| Команда | Результат |
|---|---|
| `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-voice` | **144 passed; 0 failed; 0 ignored** (9 unit + 135 integration в 10 тест-раннерах), exit `0` |
| `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-voice --all-targets -- -D warnings` | exit `0`, предупреждений нет |
| `cd rust && cargo fmt --check` | exit `0` |
| `grep -n "ignore\|\.only\|todo!\|unimplemented!\|skip" rust/crates/discord-voice/src/srtp.rs` | пусто (нарушений `TEST-POLICY.md` нет) |

Все 3 новых модульных теста в `srtp::tests` (`aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce`, `xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce`, `rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key`) и все 22 контрактных теста `tests/srtp_contract.rs` зелёные.

---

## 3. Мутационная проверка (5/5 KILLED)

Выполнены все 5 целевых мутаций в `rust/crates/discord-voice/src/srtp.rs` с прогоном `cargo test --offline --manifest-path rust/Cargo.toml -p discord-voice --lib --test srtp_contract` и последующим восстановлением файла:

| # | Мутация | Описание правки в `srtp.rs` | MD5 мутанта | Статус | Убившие тесты |
|---|---|---|---|---|---|
| M1 | Искажение 4-байтного LE-трейлера `nonce_counter` | В `gcm_nonce` запись трейлера смещена из `nonce[..4]` в `nonce[8..12]` | `d1aaad8e7d860220cb7ce30b77c7ddc0` | **KILLED** (`exit 101`) | `srtp::tests::aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce`, `srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key` |
| M2 | Подмена AAD заголовка | В `encrypt_rtp` вместо `&packet[..clear_len]` передан пустой срез `&[]` в `self.seal(...)` | `44ee2be15233845d2f9497ef5d41fe93` | **KILLED** (`exit 101`) | `aes256_gcm_rtpsize_byte_format_...`, `xchacha20_poly1305_rtpsize_byte_format_...`, `rtpsize_roundtrip_...`, `rtp_roundtrip_k`, `rtp_tamper_k`, `xchacha_rtpsize_k`, `incoming_counter_non_monotonic_k`, `decrypt_nonce_reuse_rejected_k` |
| M3 | Изменение размера тега AEAD | В `encrypt_rtp` запись тега усечена с 16 Б до 12 Б (`&tag[..12]`) | `2fcca987a0ecfe3711199ce4d7126f1d` | **KILLED** (`exit 101`) | `aes256_gcm_rtpsize_byte_format_...`, `xchacha20_poly1305_rtpsize_byte_format_...`, `rtpsize_roundtrip_...`, `rtp_layout_k`, `rtp_roundtrip_k`, `rtp_tamper_k`, `xchacha_rtpsize_k` |
| M4 | Инверсия байта `secret_key` | В `SrtpContext::new` для `mode.covers_header()` инвертирован первый байт ключа (`k[0] ^= 0xff`) | `d369587ed1522bdb7d4f09e10f9d5dcd` | **KILLED** (`exit 101`) | `aes256_gcm_rtpsize_byte_format_...`, `xchacha20_poly1305_rtpsize_byte_format_...`, `rtpsize_roundtrip_...` (на серверном пакете с эталонным `OP4_SECRET_KEY`) |
| M5 | Сбой round-trip | В `decrypt_rtp` искажён расшифрованный `plaintext` (`b ^ 0x01`) перед возвратом | `e6074b10bc75b76c191fb09e2dbac4bf` | **KILLED** (`exit 101`) | `srtp::tests::rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key`, `rtp_roundtrip_k`, `xchacha_rtpsize_k`, `incoming_counter_non_monotonic_k`, `decrypt_nonce_reuse_rejected_k` |

После завершения мутаций исходный файл восстановлен и проверен:
- `ORIG_MD5 = aabb1c02af3c7b9366368d6c1215fb80`
- `RESTORED_MD5 = aabb1c02af3c7b9366368d6c1215fb80` (`MATCH = True`)
- Контрольный прогон `cargo test --offline --manifest-path rust/Cargo.toml -p discord-voice` после восстановления — `144 passed, 0 failed`.

---

## 4. Замечания (не блокируют мерж)

- **P0:** 0
- **P1:** 0
- **P2:** 0
- **P3-1 (документация в смежном тест-файле):** В шапке `rust/crates/discord-voice/tests/srtp_contract.rs:9-10` остался старый комментарий от P4-03 («совместимость с wire Discord не проверяется»). Сам файл `tests/srtp_contract.rs` не входил в write-scope воркера (`srtp.rs`), при следующей ревизии контракта комментарий стоит обновить.
- **P3-2 (legacy-режим `aead_aes256_gcm` без суффикса `_rtpsize`):** Для режима `!mode.covers_header()` (`EncryptionMode::Aes256Gcm`) сохранена прежняя ветка с `hkdf_sha1_32`/`hkdf_sha1_16` ради обратной совместимости с тестом `non_rtpsize_mode_k`. На проводе Discord в OP 2 `READY` используются `aead_aes256_gcm_rtpsize` и `aead_xchacha20_poly1305_rtpsize`, которые полностью переведены на каноническую схему.
- **P3-3 (косметика doc-комментария):** В `srtp.rs:216` оставлено английское слово `directly` во фразе `/// Создаёт контекст для канонического режима Discord directly из 32-байтного`.

---

## 5. Итоговый вердикт

**PASS / Ready for Merge** (`P0 = 0`, `P1 = 0`).
