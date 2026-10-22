# Отчёт воркера W26a — карточка P4-03 (SRTP/SRTCP-крипта)

- **batch/карточка:** B06 / RW-25, лейн A, **P4-03** (крейт `discord-voice`).

- **сделано:**
  - `rust/crates/discord-voice/src/crypto/mod.rs` — константы размеров, `CryptoError`
    (`Display` + `std::error::Error`, ключевой материал в ошибках не переносится).
  - `rust/crates/discord-voice/src/crypto/aes.rs` — AES-256 (FIPS-197): раскладка ключа
    (60 слов), `encrypt_block`/`decrypt_block`; S-box и обратный S-box считаются
    константными функциями (инверсия в GF(2^8) + аффинное преобразование), а не
    «перепечатаны». `Aes256Key` — `Debug` без байт ключа.
  - `rust/crates/discord-voice/src/crypto/gcm.rs` — AES-256-GCM (NIST SP 800-38D):
    CTR + GHASH над GF(2^128), AAD, тег 16 Б, сравнение тега без раннего выхода.
  - `rust/crates/discord-voice/src/crypto/chacha.rs` — ChaCha20-Poly1305 (RFC 8439) и
    XChaCha20-Poly1305 (draft-irtf-cfrg-xchacha): HChaCha20, ChaCha20, Poly1305.
  - `rust/crates/discord-voice/src/crypto/kdf.rs` — HKDF-SHA1 (RFC 5869) с внутренней
    реализацией SHA-1/HMAC-SHA1; `srtp_key_derive` (label-KDF на базе HKDF-SHA1).
  - `rust/crates/discord-voice/src/srtp.rs` — `EncryptionMode`, `FrameKind`, `SrtpError`,
    `SrtpContext`: раскладка `[clear][ciphertext][tag16][counter4 LE]`, исходящий счётчик
    `0x80000000 | n` (сквозной, общий для RTP и SRTCP), AAD = clear-область в режимах
    `*_rtpsize`, разбор RTP-заголовка (V, CC, X, длина расширения), явные ошибки вместо
    паник.
  - Тесты: `tests/crypto_vectors.rs` (14 KAT) и `tests/srtp_contract.rs` (20 контрактных).
  - Внешних зависимостей не добавлено; `unsafe` нет; сеть не использовалась.

- **red:**
  - `cargo test --offline -p discord-voice --test crypto_vectors` → сборка теста падает:
    `no Aes256Key in crypto::aes`, `no Gcm in crypto::gcm`, `no hkdf_sha1 in crypto::kdf`
    и т.д. → **exit code 101** (лог `waves/RW-25/red-W26a.log`).
  - `cargo test --offline -p discord-voice --test srtp_contract` → `no SrtpContext in srtp`,
    `no EncryptionMode in srtp` → **exit code 101**.
  - Лог: `waves/RW-25/red-W26a.log` (команды + exit code).

- **green:**
  - `cargo test --offline -p discord-voice --test crypto_vectors` → 14 passed; 0 failed → **exit code 0**.
  - `cargo test --offline -p discord-voice --test srtp_contract` → 20 passed; 0 failed → **exit code 0**.
  - `cargo test --offline -p discord-voice` → 61 passed (1 unit + 14 KAT + 11 signaling + 20 contract + 15 udp);
    0 failed → **exit code 0**.
  - `cargo fmt -p discord-voice` → exit 0; `cargo fmt --all -- --check` → exit 0;
    `cargo clippy --offline -p discord-voice --all-targets -- -D warnings` → exit 0.
  - Лог: `waves/RW-25/green-W26a.log` (команды + exit code + числа тестов).

- **targeted:** `waves/RW-25/green-W26a.log` — 34 новых теста (14 KAT + 20 контрактных),
  все зелёные; полный пакет — 61 тест.

- **gate:worker:** не запускал (воркер полный gate не запускает — batch-гейт у Integration/QA).
  В рамках карточки прогнаны targeted-команды, `fmt` и `clippy` пакета.

- **файлы (md5):**
  | файл | md5 |
  |---|---|
  | `rust/crates/discord-voice/src/crypto/mod.rs` | `c89ef7b1448e7958feff4329e5fb463c` |
  | `rust/crates/discord-voice/src/crypto/aes.rs` | `3c83c0376d06bd01b72f0c26c17ddaeb` |
  | `rust/crates/discord-voice/src/crypto/gcm.rs` | `88d54101f6344b94d22cc08107d36295` |
  | `rust/crates/discord-voice/src/crypto/chacha.rs` | `bf8b9b22fc5190ae33ecda85172372c9` |
  | `rust/crates/discord-voice/src/crypto/kdf.rs` | `596094464fabfdbc557a7fa84beb7424` |
  | `rust/crates/discord-voice/src/srtp.rs` | `b154f94d4d5af9bc03f6f6af883052a8` |
  | `rust/crates/discord-voice/tests/crypto_vectors.rs` | `d69ac1dccb21ffa70e8258c3a9385993` |
  | `rust/crates/discord-voice/tests/srtp_contract.rs` | `c349522a07bfc662d8217a6427675efb` |
  | `waves/RW-25/red-W26a.log` | `1306ce8d60522f719ed43b423bc0f55a` |
  | `waves/RW-25/green-W26a.log` | `570d3855870bfe6c7f085c06a4622019` |

  Не тронуто (правки TL-2 из интеграционного патча, не мои): `Cargo.toml`, `Cargo.lock`,
  `src/lib.rs`, `src/udp.rs`, чужие крейты, доки, `waves/db.jsonl`, `STATE.md`, `STATUS.md`,
  `PLAN.md`, `TASKS.md`, `docs/map.md`.

- **оракулы (провенанс):**
  - KAT-константы вшиты в тест как константы и сверены **независимыми** реализациями
    (OpenSSL/Node.js `crypto`, Python `cryptography`), а не кодом крейта:
    FIPS-197 App. C.3 (AES-256), NIST SP 800-38D GCM test cases 13–15 (AES-256-GCM),
    RFC 8439 §2.8.2 (ChaCha20-Poly1305), draft-irtf-cfrg-xchacha A.2/A.3.1 (HChaCha20,
    XChaCha20-Poly1305), RFC 5869 A.2/A.3 (HKDF-SHA1). Все 12 констант совпали с оракулом.
  - Раскладка SRTP — реальные байты capture `strace-media-s3.jsonl`: RTP 47 Б
    (clear 24 = 12 Б фикс. заголовка + ext `0xBEDE`/2 слова + 8 Б данных, ct 3, tag 16,
    трейлер `0x80000000`), SRTCP 60 Б (clear 8, ct 32, tag 16, трейлер `0x80000008`),
    пустой RR 28 Б (трейлер `0x80000016`), входящий SRTCP 52 Б (трейлер `0x00000000`),
    43-Б пакеты talkspurt (расширение с профилем ≠ `0xBEDE` и нелепой длиной).

- **риски:**
  1. **Совместимость с wire Discord не заявляется.** В capture нет ключевого материала:
     формат самосогласован (раскладка — по capture, примитивы — по RFC/NIST). Заявлено
     в док-комментариях `crypto/mod.rs` и `srtp.rs`.
  2. **Nonce GCM привязан к трейлеру кадра.** Уникальность nonce держится на
     монотонном счётчике. Публичный `set_counter` (для синхронизации/границ) позволяет
     перемотать счётчик назад ⇒ повтор nonce под тем же ключом (катастрофа для GCM).
     Риск задокументирован в док-комментарии метода; в рабочем коде вызывается только
     вперёд.
  3. **В режиме `aead_aes256_gcm` (не `rtpsize`) заголовок не разбирается** — он внутри
     шифротекста; ошибки `MalformedHeader` в этом режиме не возникают. Симметрично на
     шифровании и расшифровке.
  4. `xsalsa20_poly1305*` и `aead_xchacha20_poly1305_rtpsize` в capture не наблюдались
     (только объявлены движком) — режим XChaCha реализован самосогласованно, без
     претензии на wire-совместимость.
  5. `cargo fmt --all` не применялся: он тронул бы файлы лейна W26c (`discord-updater`).
     Форматирование выполнено `cargo fmt -p discord-voice`; `cargo fmt --all -- --check`
     сейчас чист (exit 0).

- **отклонения от замороженного API (добавления, ничего не изменено):**
  1. `SrtpContext::set_counter(&mut self, u64) -> Result<(), SrtpError>` — ТЗ §4 требует
     кейс `counter_overflow_k` и разрешает «приватный сеттер», но `#[cfg(test)]`-видимость
     не достаёт до интеграционных тестов в `tests/`, поэтому метод сделан `pub` с
     док-комментарием. Все сигнатуры из ТЗ §3 реализованы буквально.
  2. `SrtpContext::from_engine_name(master_key, master_salt, mode_name) -> Result<Self, SrtpError>` —
     делает вариант `SrtpError::UnsupportedMode` достижимым (иначе он был бы мёртвым).
  3. `crypto::kdf::srtp_key_derive` — есть в контрактной таблице `WAVE-PLAN.md` §2; реализован
     как label-KDF на базе HKDF-SHA1 (метка в `info`).
  4. `impl Debug for SrtpContext` (без ключевого материала) — требуется `expect_err` в тестах.
  5. `impl Display`/`Error` для `SrtpError` (для `CryptoError` это требование ТЗ).
  6. Крейт-приватные хелперы: `Aes256Key::from_array`, `hkdf_sha1_16`, `hkdf_sha1_32`.

- **открытые вопросы (нужен ответ Team Lead / Tech Lead):**
  1. **RFC 3711 §4.3.1 (label-KDF)** не реализован: текст RFC в офлайн-режиме недоступен,
     а выдумывать криптографический KDF нельзя. `srtp_key_derive` — HKDF-SHA1-вариант.
     Если нужна совместимость с RFC 3711, требуется текст §4.3.1 (или решение принять
     расхождение и зафиксировать его в контракте).
  2. **Семантика входящего трейлера.** ТЗ §2 п.4 называет 4 байта «непрозрачными», но
     уникальность nonce требует привязки к кадру. Реализация: трейлер используется как
     32-битный идентификатор кадра для построения nonce; биты не интерпретируются
     (старший бит на приёме не маскируется), монотонность не проверяется. Нужно подтверждение,
     что такое прочтение допустимо.
  3. **Предел счётчика.** `CounterOverflow` срабатывает при `n > 0x7FFF_FFFF` (старший бит
     трейлера — маркер `0x80000000`). Если движок использует иное кодирование счётчика,
     граница изменится.
  4. **Поля RTCP.** В capture длина RTCP-пакета в заголовке (`len=6` для 60-Б кадра) не
     соответствует длине clear+ciphertext — это проприетарное обрамление Discord. Проверку
     согласованности длины RTCP намеренно не делаем (иначе эталонный 60-Б кадр не проходит).
  5. **`cargo fmt --all` на уровне волны** — Team Lead: сейчас `cargo fmt --all -- --check`
     чист; лейн W26c отформатировал свои файлы.

- **следующий шаг:** Team Lead — приёмка карточки (независимый прогон + ревью),
  затем Integration/QA: batch-гейт (`cargo fmt --all -- --check`, clippy workspace,
  `cargo test --offline --workspace`), мутации по новым кейсам, независимое ревью.
