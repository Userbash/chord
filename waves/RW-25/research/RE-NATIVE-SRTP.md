# RE-NATIVE-SRTP — статический разбор `discord_voice.node` (Discord 1.0.165)

Seat: RE-1 (protocol research). Дата: 2026-10-22. Волна: B06 / RW-25, карточка **P4-03**.
Объект: `/home/sanya/.config/discord/app-1.0.165/modules/discord_voice-1/discord_voice/discord_voice.node`
(127 791 736 Б, sha256 `baa9574f88aed51aff0e59a57d5733bb1916b601db5d79e38e0fcf4c18b5b0c5`).

Метод: **статический, read-only** — `strings`, `readelf`, `nm`, `gdb` по DWARF. Живой процесс
Discord **не** подключался, `device.spawn`/attach не выполнялись (окно владельца не
открывалось). Проверено отдельно: старый PID 95828 из дампа уже не существует, запущен
1.0.165 (PID 16632).

## 1. Стек шифрования — libsodium + mlspp + WebRTC

Из путей в DWARF (`.debug_info` присутствует, символьная таблица stripped):

```text
discord_common/native/third_party/libsodium/src/libsodium/crypto_aead/aes256gcm/aesni/aead_aes256gcm_aesni.c
discord_common/native/third_party/libsodium/src/libsodium/crypto_aead/aes256gcm/aead_aes256gcm.c
discord_common/native/third_party/libsodium/src/libsodium/crypto_aead/xchacha20poly1305/sodium/aead_xchacha20poly1305.c
discord_common/native/third_party/libsodium/src/libsodium/crypto_box/curve25519xchacha20poly1305/box_curve25519xchacha20poly1305.c
discord_common/native/third_party/libdave/cpp/src/frame_processors.cpp
discord_common/native/third_party/mlspp/lib/hpke/src/aead_cipher.cpp
discord_common/native/third_party/llvm/libcxx/...
discord_desktop/build_x64/.../third_party/webrtc/.../d1_srtp.cc
```

Символы libsodium, присутствующие в модуле (gdb, DWARF):

- `crypto_aead_aes256gcm_encrypt` / `_decrypt` / `_encrypt_detached` / `_beforenm` (+ AES-NI-вариант);
- `crypto_aead_xchacha20poly1305_ietf_encrypt` / `_decrypt` / `_encrypt_detached`;
- `crypto_aead_aes256gcm_is_available`.

Вывод: голосовое AEAD — **libsodium** (AES-256-GCM с AES-NI и XChaCha20-Poly1305-IETF),
DAVE/MLS — **mlspp** (HPKE), SRTP-профили — **WebRTC** (`d1_srtp.cc`).

## 2. Режимы шифрования — точный список

Строки режимов в модуле (совпадают с логом движка из `docs/re/voice-deep.md` §3):

```text
aead_aes256_gcm
aead_aes256_gcm_rtpsize
aead_xchacha20_poly1305_rtpsize
xsalsa20_poly1305
xsalsa20_poly1305_lite
xsalsa20_poly1305_lite_rtpsize
xsalsa20_poly1305_suffix
```

Имена элементов перечисления: `AEAD_AES256_GCM`, `AEAD_AES256_GCM_RtpSize`,
`AEAD_XChaCha20_Poly1305_RtpSize`.

Отображение режим↔строка — `discord::encryption::ModeToString(Mode)` /
`ModeFromString(char const*)` / `ModeNames()` (`discord_common/native/encryption/mode.cpp`).

## 3. Собственный криптер Discord — `discord_common/native/encryption/packet_crypter.cpp`

Функции (gdb по DWARF, `discord_common/native/encryption/packet_crypter.cpp`):

| Строка | Функция | Что означает |
|---|---|---|
| 69 | `static size_t HeaderLength(unsigned char const*, unsigned long, Mode)` | **длина открытой (не зашифрованной) части пакета для данного режима** |
| 94 | `PacketCrypter::PacketCrypter(State)` | состояние криптера |
| 108 | `PacketCrypter::GetEncryptionModes(vector<string>&)` | список поддерживаемых режимов |
| 122 | `PacketCrypter::SpaceRequiredToEncrypt(unsigned long)` | **сколько дополнительных байт нужно под шифрование** (тег + трейлер) |
| 145 | `PacketCrypter::Decrypt(unsigned char*, unsigned long&, unsigned long)` | расшифровка |
| 258 | `PacketCrypter::Encrypt(unsigned char*, unsigned long&, unsigned long)` | шифрование |

**Это ровно та модель, что выведена из capture** (`RE-SRTP-LAYOUT.md`):

- `HeaderLength(...)` ⇒ для режимов `*_rtpsize` открытая часть = RTP-заголовок
  (12 Б + CSRC + заголовок/данные расширения), она же AAD; для остальных режимов
  открытой части нет.
- `SpaceRequiredToEncrypt(...)` ⇒ к выводу добавляются тег AEAD (16 Б) и 4-байтный трейлер
  (счётчик) — ровно +20 Б, что даёт 47 Б у 24-Б открытой части и 3-Б payload.
- `Encrypt/Decrypt` с `unsigned long&` (длина) — длина выходного буфера вычисляется
  функцией, что согласуется с «clear || ciphertext || tag || counter».

## 4. Что осталось неподтверждённым

- **Нет ключевого материала** и трассировки вывода ключей/nonce ⇒ схема KDF остаётся
  внутренней реализацией Discord и **не заявляется совместимой** (см. `TZ-W26a.md` §2).
- Входящий 4-байтный трейлер в capture не монотонен ⇒ его семантика на приёме не
  подтверждена (в коде — непрозрачные 4 байта).
- 43-байтовые пакеты первых talkspurt не соответствуют формату `PacketCrypter`
  (нет трейлера, битое расширение) ⇒ трактуются как ошибка ввода.

## 5. Ограничения RE-инструментов в этой среде

- **REA MCP**: `open_binary` на `discord_voice.node` → Ghidra 12.1.2 не успел за
  `REA_GHIDRA_STARTUP_TIMEOUT_MS = 330000` мс (`provider_timeout`, бинарь 127 МБ с
  полным DWARF). `inspect_binary_layout` недоступен: не задан `REA_PWNTOOLS_PYTHON`.
  `hopper`/`ida`/`kuna` в этой среде не сконфигурированы. Увеличение таймаута требует
  перезапуска сервера REA — в этой сессии не выполнялось.
- Поэтому применён обычный статический путь: `strings` + `readelf` + `nm` + `gdb` по DWARF
  (даёт имена функций, файлы и строки). Результат выше получен без запуска цели.

## 6. Связанные документы

- Раскладка байт по capture: `waves/RW-25/research/RE-SRTP-LAYOUT.md`.
- Контракт карточки: `waves/RW-25/TZ-W26a.md`.
- Исходные RE-отчёты: `docs/re/voice-deep.md`, `docs/re/native-re.md`.
