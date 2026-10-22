# TZ-W26a — P4-03: SRTP/SRTCP-крипта (крейт `discord-voice`)

Ты — **W26a**, кодовый воркер лейна A волны B06/RW-25. Пиши **только** файлы из §1.
`src/lib.rs`, `Cargo.toml`, `Cargo.lock`, доки и `waves/*` — не трогать. Сначала red-тесты
(зафиксируй команду и exit code), затем минимальный green.

## 1. Файлы

Пишешь:
- `rust/crates/discord-voice/src/crypto/mod.rs`
- `rust/crates/discord-voice/src/crypto/aes.rs`
- `rust/crates/discord-voice/src/crypto/gcm.rs`
- `rust/crates/discord-voice/src/crypto/chacha.rs`
- `rust/crates/discord-voice/src/crypto/kdf.rs`
- `rust/crates/discord-voice/src/srtp.rs`
- `rust/crates/discord-voice/tests/crypto_vectors.rs`
- `rust/crates/discord-voice/tests/srtp_contract.rs`

Читаешь, но не меняешь: `rust/crates/discord-voice/src/udp.rs`, `rust/crates/discord-voice/src/lib.rs`,
`docs/re/voice-deep.md` §2.3–2.5 и §3, `waves/RW-25/WAVE-PLAN.md`, `waves/RW-25/research/`.

Запрещено: `unsafe` (крейт с `unsafe_code = "forbid"`), внешние зависимости, сеть,
`git commit/push`, доступ к живому Discord, запись в `logs/`, `functions/`, `evidence/`,
`/home/sanya/re-deep`.

## 2. Оракул (RE, capture s3)

Источник: `/home/sanya/re-deep/voice/strace-media-s3.jsonl` (только чтение), разбор —
`waves/RW-25/research/media-packet-layout.mjs`. **Подтверждённая** раскладка:

```text
RTP-медиа (47 Б, исходящие):
  0..24  clear  RTP-заголовок: 12 Б фикс + 4 Б ext-заголовок (profile 0xBEDE, len=2 слова)
                      + 8 Б данных расширения
 24..27  ciphertext (3 Б — Opus DTX/comfort-noise)
 27..43  tag (16 Б, GCM)
 43..47  счётчик 4 Б LE = 0x80000000 | n

SRTCP исходящий (60 Б, SR/PT 200):
  0..8   clear  RTCP-заголовок 4 Б (V=2, RC=0, PT=200, len=6) + SSRC 4 Б
  8..40  ciphertext (32 Б)
 40..56  tag 16 Б
 56..60  счётчик 4 Б LE = 0x80000008

SRTCP исходящий пустой RR (28 Б, PT 201):
  0..8   clear; ciphertext 0 Б; tag 16 Б; счётчик 4 Б LE = 0x80000016

SRTCP входящий (52 Б, RR/PT 201):
  0..8   clear; 8..32 ciphertext (24 Б); 32..48 tag; 48..52 счётчик = 0x00000000
```

Факты, которые обязаны отражаться в тестах:

1. Единая структура: `[clear][ciphertext][tag 16][counter 4 LE]`.
2. Исходящий счётчик = `0x80000000 | n`, где `n` — сквозной счётчик **общий для RTP и SRTCP**
   одного сокета: в capture 8 RTP-пакетов дали `n = 0..7`, затем SR-пакеты — `n = 8, 9, 10`.
3. `n = 0x16` (22) — счётчик после ротации медиа-сокета (+60 с) не сбрасывается.
4. Входящие счётчики **не монотонны** (0x00..0x0a, затем 0x76, 0x66, 0x0d) ⇒ семантика
   входящего трейлера **не подтверждена**. На расшифровке трейлер — непрозрачные 4 байта,
   монотонность не проверяется. На шифровании — наш исходящий формат `0x80000000 | n`.
5. Два первых пакета talkspurt — 43 Б: без валидного расширения (`profile != 0xBEDE`,
   `len` нелепый) и **без** 4-байтового трейлера-счётчика ⇒ это не SRTP-кадр нашего формата.
   Контракт обязан вернуть явную ошибку, а не паниковать и не «угадывать».
6. Режим из лога движка: `Selected encryption mode: aead_aes256_gcm_rtpsize`.
   Движок также перечисляет `aead_aes256_gcm`, `aead_xchacha20_poly1305_rtpsize`,
   `xsalsa20_poly1305*` — они **объявлены, но в capture не наблюдались**.
7. AAD в режиме `*_rtpsize` = clear-область (RTP-заголовок / RTCP-заголовок+SSRC):
   это и есть семантика «rtp size» — AEAD защищает полезную нагрузку, а не заголовок.

**Честная граница:** в capture нет ключевого материала, поэтому **совместимость с wire
Discord не заявляется**. Реализация самосогласована, раскладка байт — по capture,
криптопримитивы — по KAT-векторам NIST/RFC. Это написать в doc-комментариях модулей.

## 3. Замороженный API

Сигнатуры обязательны буквально. Приватные хелперы внутри модуля — можно.

### `crypto/mod.rs`

```rust
pub mod aes;
pub mod chacha;
pub mod gcm;
pub mod kdf;

pub const AES_BLOCK_SIZE: usize = 16;
pub const AES256_KEY_SIZE: usize = 32;
pub const GCM_NONCE_SIZE: usize = 12;
pub const GCM_TAG_SIZE: usize = 16;
pub const CHACHA_KEY_SIZE: usize = 32;
pub const CHACHA_NONCE_SIZE: usize = 12;
pub const XCHACHA_NONCE_SIZE: usize = 24;

/// Ошибка крипто-примитива.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    InvalidKeyLength { expected: usize, actual: usize },
    InvalidNonceLength { expected: usize, actual: usize },
    InvalidTagLength { expected: usize, actual: usize },
    InvalidOutputLength,
    AuthenticationFailed,
}
```
`CryptoError` — `Display` + `std::error::Error`. `Debug` ключей не должен утекать.

### `crypto/aes.rs`

```rust
pub struct Aes256Key([u8; 32]);           // Debug — редактированный, без байт ключа
impl Aes256Key {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError>;  // ошибка длины
    pub fn as_bytes(&self) -> &[u8; 32];
}

pub struct Aes256 { /* round keys */ }
impl Aes256 {
    pub fn new(key: &Aes256Key) -> Self;
    pub fn encrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE];
    pub fn decrypt_block(&self, block: &[u8; AES_BLOCK_SIZE]) -> [u8; AES_BLOCK_SIZE];
}
```

### `crypto/gcm.rs`

```rust
pub struct Gcm { /* ... */ }
impl Gcm {
    pub fn new(key: &Aes256Key) -> Self;
    /// Возвращает `ciphertext || tag` (длина = len(plaintext) + 16).
    pub fn encrypt(&self, nonce: &[u8; GCM_NONCE_SIZE], aad: &[u8], plaintext: &[u8])
        -> Result<Vec<u8>, CryptoError>;
    /// На входе `ciphertext || tag`; при несовпадении тега — `AuthenticationFailed`.
    pub fn decrypt(&self, nonce: &[u8; GCM_NONCE_SIZE], aad: &[u8], ciphertext_and_tag: &[u8])
        -> Result<Vec<u8>, CryptoError>;
}
```
Требование: вход короче 16 Б ⇒ `InvalidTagLength`; длина `ciphertext_and_tag` без остатка 16.

### `crypto/chacha.rs`

```rust
pub struct ChaCha20Poly1305 { /* ... */ }
impl ChaCha20Poly1305 {
    pub fn new(key: &[u8; CHACHA_KEY_SIZE]) -> Self;
    pub fn encrypt(&self, nonce: &[u8; CHACHA_NONCE_SIZE], aad: &[u8], plaintext: &[u8])
        -> Result<Vec<u8>, CryptoError>;   // ciphertext || tag16
    pub fn decrypt(&self, nonce: &[u8; CHACHA_NONCE_SIZE], aad: &[u8], ciphertext_and_tag: &[u8])
        -> Result<Vec<u8>, CryptoError>;
}

pub struct XChaCha20Poly1305 { /* ... */ }
impl XChaCha20Poly1305 {
    pub fn new(key: &[u8; CHACHA_KEY_SIZE]) -> Self;
    pub fn encrypt(&self, nonce: &[u8; XCHACHA_NONCE_SIZE], aad: &[u8], plaintext: &[u8])
        -> Result<Vec<u8>, CryptoError>;
    pub fn decrypt(&self, nonce: &[u8; XCHACHA_NONCE_SIZE], aad: &[u8], ciphertext_and_tag: &[u8])
        -> Result<Vec<u8>, CryptoError>;
}
```

### `crypto/kdf.rs`

```rust
/// HKDF-SHA1 (RFC 5869): extract(salt, ikm) + expand(info, L).
pub fn hkdf_sha1(ikm: &[u8], salt: &[u8], info: &[u8], out_len: usize)
    -> Result<Vec<u8>, CryptoError>;
```
`out_len == 0` ⇒ `InvalidOutputLength`; `out_len > 255 * 20` ⇒ `InvalidOutputLength`.

### `srtp.rs`

```rust
/// Режим шифрования из лога движка (`docs/re/voice-deep.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMode {
    Aes256GcmRtpSize,
    Aes256Gcm,
    XChaCha20Poly1305RtpSize,
}
impl EncryptionMode {
    /// Разбор имени режима движка; неизвестное имя ⇒ `None`.
    pub fn from_engine_name(name: &str) -> Option<Self>;
    /// Имя режима как в логе движка.
    pub fn as_engine_name(self) -> &'static str;
    /// `true` для режимов `*_rtpsize`: AEAD защищает только полезную нагрузку,
    /// заголовок — это AAD.
    pub fn covers_header(self) -> bool;
    /// Длина тега AEAD (все режимы — 16).
    pub fn tag_size(self) -> usize;
}

/// Вид кадра.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind { Rtp, Srtcp }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrtpError {
    /// Пакет короче минимально допустимого.
    PacketTooShort { len: usize, min: usize },
    /// Некорректный RTP/RTCP-заголовок.
    MalformedHeader { reason: &'static str },
    /// Режим не поддерживается (например, `xsalsa20_poly1305` — вне карточки).
    UnsupportedMode { name: &'static str },
    /// Несовпадение AEAD-тега.
    AuthenticationFailed,
    /// Ошибка примитива.
    Crypto(CryptoError),
    /// Переполнение 32-битного счётчика.
    CounterOverflow,
}

/// Контекст SRTP: ключ, соль, режим и сквозной счётчик кадров.
pub struct SrtpContext { /* ... */ }
impl SrtpContext {
    /// `master_key` — 32 Б, `master_salt` — 14 Б (RFC 3711 master salt).
    pub fn new(master_key: &[u8; 32], master_salt: &[u8; 14], mode: EncryptionMode) -> Self;
    /// Текущий счётчик `n` (следующий кадр получит `n`, затем `n + 1`).
    pub fn counter(&self) -> u64;
    /// Шифрует RTP-кадр: результат = `[clear][ciphertext][tag16][counter4 LE]`.
    pub fn encrypt_rtp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError>;
    /// Расшифровывает RTP-кадр; при несовпадении тега — `AuthenticationFailed`.
    pub fn decrypt_rtp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError>;
    /// Шифрует SRTCP-кадр (clear = 8 Б: заголовок RTCP + SSRC).
    pub fn encrypt_srtcp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError>;
    /// Расшифровывает SRTCP-кадр.
    pub fn decrypt_srtcp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError>;
}
```

Обязательные свойства реализации:

- `encrypt_*` **не меняет** входной счётчик при ошибке (счётчик растёт только при успехе).
- Исходящий трейлер = `0x80000000 | n` в LE; `n` общий для RTP и SRTCP.
- `n` монотонен и не сбрасывается (ротация сокета — не наша ответственность).
- `decrypt_*` трейлер **не** проверяет на монотонность (входящие значения не монотонны —
  п. 4 §2).
- Режимы без `rtpsize` (`Aes256Gcm`): AAD пустой, шифруется **весь** кадр,
  clear-область в выводе отсутствует; layout `[ciphertext][tag16][counter4]`.
- `PacketTooShort`: RTP — меньше 12 Б; SRTCP — меньше 8 Б; плюс проверка, что после
  выделения clear-области и хвоста (tag+counter) остаётся неотрицательная полезная нагрузка.
- `MalformedHeader`: `V != 2`; `CC > 15`; при `X = 1` — `ext_len_words * 4` не влезает
  в пакет; для SRTCP — `V != 2` в первом байте.
- `UnsupportedMode` — для имён движка, не покрытых карточкой (`xsalsa20_poly1305*`).
  `from_engine_name("aead_xchacha20_poly1305_rtpsize") == Some(XChaCha20Poly1305RtpSize)`.
- Никаких `panic!`/`unwrap` на пользовательском вводе: только `Result`.

## 4. Тесты (test ID → кейс)

`tests/crypto_vectors.rs` — KAT по независимым оракулам (NIST/RFC; векторы вшиты в тест
как константы, не вычисляются тем же кодом):

- `aes256_k NIST FIPS-197 C.3` — один блок: ключ/блок → шифротекст.
- `aes256_decrypt_k` — обратное преобразование воспроизводит блок.
- `aes256_wrong_key_len` — `from_bytes` с 31/33 Б ⇒ `InvalidKeyLength`.
- `gcm_k NIST GCM test case 4` (128-битный ключ — **расширь на 256 не нужно**, используй
  AES-256-GCM из NIST SP 800-38D, тест-векторы для AES-256 приведены в §5) — проверь
  `encrypt`/`decrypt`, AAD привязан (изменение AAD ⇒ `AuthenticationFailed`).
- `gcm_empty_plaintext_k` — пустой plaintext ⇒ на выходе ровно 16 Б (только тег).
- `gcm_tamper_k` — правка байта шифротекста ⇒ `AuthenticationFailed`.
- `gcm_short_input_k` — вход < 16 Б ⇒ `InvalidTagLength`.
- `chacha20poly1305_k RFC 8439 §2.8.2` — полный вектор (ключ, nonce, aad, plaintext,
  ciphertext+tag).
- `xchacha20poly1305_k` — вектор XChaCha20-Poly1305 (RFC draft-irtf-cfrg-xchacha,
  A.3.1) с 24-байтным nonce.
- `hkdf_sha1_k RFC 5869 test case 1` (и проверка границ: `out_len = 0` ⇒
  `InvalidOutputLength`; `out_len = 255*20 + 1` ⇒ `InvalidOutputLength`).

`tests/srtp_contract.rs` — раскладка и поведение:

- `rtp_layout_k` — 47-байтовый эталонный RTP-кадр (заголовок 12 Б + ext 0xBEDE/2 слова +
  8 Б данных + 3 Б payload) → после `encrypt_rtp` длина 47, хвост `0x80000000`,
  tag на позиции `len-20`, clear-область первых 24 Б **побайтово равна** исходной.
- `rtp_counter_shared_k` — 8 RTP + 1 SRTCP ⇒ трейлеры `0x80000000..0x80000008` подряд,
  общий счётчик; при шифровании SRTCP значение `n` продолжает счётку RTP.
- `srtcp_layout_k` — 60-байтовый эталон SR: первые 8 Б сохраняются, ciphertext 32 Б,
  tag 16, трейлер `0x80000008`; пустой RR (28 Б) ⇒ ciphertext 0 Б, tag 16, трейлер.
- `rtp_roundtrip_k` — `decrypt_rtp(encrypt_rtp(x)) == x` и то же для SRTCP.
- `rtp_tamper_k` — правка байта в ciphertext ⇒ `AuthenticationFailed`; правка байта в
  clear-области ⇒ тоже `AuthenticationFailed` (AAD привязан).
- `malformed_43b_k` — 43-байтовый «пакет talkspurt» из capture (профиль расширения ≠
  0xBEDE) ⇒ `MalformedHeader`/`PacketTooShort`, **не паника**.
- `short_packet_k` — 5 Б ⇒ `PacketTooShort`; RTP из 12 Б без хвоста ⇒ ошибка, не паника.
- `mode_names_k` — `from_engine_name`/`as_engine_name` для всех четырёх имён движка;
  `xsalsa20_poly1305_lite` ⇒ `None`/`UnsupportedMode`.
- `non_rtpsize_mode_k` — режим `Aes256Gcm`: AAD пустой, в выводе нет clear-области.
- `counter_overflow_k` — разряд счётчика не паникует (выставить `counter` на максимум
  через внутреннее поле — если поле приватное, добавь **приватный** сеттер только для
  теста через `#[cfg(test)]`-видимость; либо проверь логику через публичный счётчик).

Минимум **16** тестов в этом файле. Каждый тест — отдельная функция с ID в имени.

## 5. Векторы

Векторы бери из RFC/NIST (NIST SP 800-38D / FIPS-197 / RFC 8439 / RFC 5869 /
draft-irtf-cfrg-xchacha). **Сеть запрещена** — если конкретный вектор не помнишь
дословно, используй те, которые знаешь наверняка, и **не выдумывай** остальные: лучше
меньше тестов с верными векторами, чем неверный вектор. Если вектор вызывает сомнение —
вынеси его в `waves/RW-25/worker-W26a.md` как открытый вопрос, а не вшивай в тест.

## 6. Red/green и сдача

```bash
cd /home/sanya/orca/project/rust
export CARGO_TARGET_DIR=/tmp/opencode/target-w26a
cargo test --offline -p discord-voice --test crypto_vectors   # red
cargo test --offline -p discord-voice --test srtp_contract    # red
cargo test --offline -p discord-voice                          # green
cargo fmt --all
cargo clippy --offline -p discord-voice --all-targets -- -D warnings
```

Логи: `waves/RW-25/red-W26a.log`, `waves/RW-25/green-W26a.log` (команда + exit code).
Отчёт: `waves/RW-25/worker-W26a.md` по шаблону `PROCESS.md` §4.2 (batch/карточка, сделано,
red-прогон, green-прогон, targeted, файлы + md5, риски, открытые вопросы).
