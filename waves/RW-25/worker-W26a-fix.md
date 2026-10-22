# Отчёт воркера W26a-fix — замечания к приёмке карточки P4-03 (SRTP/SRTCP-крипта)

- **batch/карточка:** B06 / RW-25, лейн A, **P4-03** (крейт `discord-voice`).
- **предмет:** закрытие замечаний F-A1…F-A4 по итогам приёмки (F-A1 — независимое ревью
  codex, F-A2/F-A3 — ревью Team Lead, F-A4 — независимое ревью codex).
- **правило:** на каждое замечание — отдельный красный прогон (зафиксирован в логе),
  затем реализация, затем зелёный прогон.

## F-A1 (P0) — табличный S-box утекает ключ через кэш

**Что изменено** (`src/crypto/aes.rs`):
- Удалены `static SBOX: [u8; 256]`, `static INV_SBOX: [u8; 256]`, компиляторные
  `build_sbox`/`build_inv_sbox` и `gf_pow`. В коде больше нет обращений к памяти
  по индексу, зависящему от секрета (проверено grep: `SBOX`/`sbox[` отсутствуют).
- S-box считается алгебраически: `sbox(x) = affine(x^254)`, где обратный элемент
  `x^254` — фиксированная последовательность squaring/multiplication (x², x⁴, x⁸, x¹⁶,
  x³², x⁶⁴, x¹²⁸ и семь умножений), ветка «x == 0» не нужна (`0^254 = 0`).
- `gf_mul` — «русский крестьянин» **без ветвлений по данным**: выборка по младшему
  биту через маску `(b & 1).wrapping_neg()`, редукция по модулю — через маску
  `(a >> 7).wrapping_neg()`.
- Аффинное преобразование: `y ^ rotl(y,1) ^ rotl(y,2) ^ rotl(y,3) ^ rotl(y,4) ^ 0x63`;
  обратное — `rotl(y,1) ^ rotl(y,3) ^ rotl(y,6) ^ 0x05`.
- Обратный S-box: `inv_sbox(y) = (A⁻¹(y))^254` (проверено: обратное аффинное
  преобразование ≠ аффинное от обратного, вариант выбран по свойству `inv(sbox(x)) = x`).
- Публичные `pub const fn sbox(x: u8) -> u8` и `pub const fn inv_sbox(y: u8) -> u8`
  (нужны для проверки и для внешней верификации).
- `sub_bytes`, `inv_sub_bytes`, `expand_key` (SubWord) переведены на вычисление.

**red:** `cargo test --offline -p discord-voice --test crypto_vectors` →
`error[E0425]: cannot find function 'sbox' in module 'aes'` → **exit code 101**
(лог `waves/RW-25/red-W26a-fix1.log`).

**green:** тот же прогон → **16 passed; 0 failed; exit code 0**
(лог `waves/RW-25/green-W26a-fix1.log`; на момент реализации F-A1 — 15 passed).

**Оракул теста:** `sbox_matches_fips197_k` — таблица S-box FIPS-197 (Fig. 7) держится
**в тесте** как эталон (256 значений), проверены все 256 значений: `sbox(x) == таблица`,
`inv_sbox(таблица[x]) == x`, `inv_sbox(y) == обратная перестановка`, `inv(sbox(x)) == x`.
Источник таблицы: опубликованная FIPS-197, перепроверенная по **независимой**
реализации (aws-lc / BoringSSL, `aes_ref_sub_byte`, локальный офлайн-кэш cargo) и по
KAT FIPS-197 App. C.3 — таблица совпала с алгебраическим вычислением побайтово.
Существующие KAT (FIPS-197 C.3, GCM TC13–15) остались зелёными — это независимая
проверка корректности нового вычисления.

**Замечание по ходу реализации (поймано тестом):** первая версия маски в `gf_mul`
использовала `(a & 0x80).wrapping_neg() == 0x80` (а не 0xFF) — редукция по модулю не
срабатывала, KAT-тесты `aes256_k`/`gcm_k`/`sbox_matches_fips197_k` падали; маска
исправлена на `(a >> 7).wrapping_neg()`. Это штатный цикл red→green TDD.

**Остаточный риск:** вычисление S-box без таблиц в ~100 раз медленее табличного
(13 умножений GF(2⁸) на байт). Для медиа-пакета Discord (47 Б) это единицы
микросекунд; крейт в медиа-путь пока не включён. Табличный вариант не возвращаем —
это и есть смысл замечания.

## F-A2 (P1) — `set_counter` допускал перемотку назад

**Что изменено** (`src/srtp.rs`):
- Новая вариация ошибки (санкционировано Team Lead):
  `SrtpError::CounterRewind { current: u64, requested: u64 }` + `Display`.
- `set_counter`: `counter < self.counter` ⇒ `Err(CounterRewind)` (счётчик не меняется);
  `counter > MAX_COUNTER` ⇒ `Err(CounterOverflow)`; иначе присваивание.
  Порядок проверок не конфликтует: `self.counter <= MAX_COUNTER` всегда.

**red:** `cargo test --offline -p discord-voice --test srtp_contract` →
`error[E0599]: no variant named 'CounterRewind' found for enum 'SrtpError'` →
**exit code 101** (лог `waves/RW-25/red-W26a-fix2.log`).

**green:** тот же прогон → **22 passed; 0 failed; exit code 0**
(лог `waves/RW-25/green-W26a-fix2.log`; на момент реализации F-A2 — 21 passed).

**Тест:** `counter_rewind_rejected_k` — перемотка `0x10 → 0x02` ⇒ `CounterRewind
{ current: 0x10, requested: 0x02 }`, счётчик остался `0x10`; движение вперёд (`0x20`)
разрешено. Прежний риск «повтор nonce GCM из-за перемотки» закрыт на уровне API.

## F-A3 (P1) — расшифровка допускала повтор nonce

**Что изменено** (`src/srtp.rs`):
- Новая вариация ошибки (санкционировано Team Lead):
  `SrtpError::NonceReuse { trailer: u32 }` + `Display`.
- Контекст помнит **окно приёма**: `VecDeque<u32>` (порядок вытеснения) + `HashSet<u32>`
  (быстрая проверка), `RECEIVE_WINDOW = 1024` последних трейлеров; при переполнении
  вытесняется самый старый.
- `decrypt_rtp` / `decrypt_srtcp` (все ветки режимов): если трейлер уже в окне ⇒
  `Err(NonceReuse { trailer })` **до** попытки расшифровки; при успешной расшифровке
  трейлер запоминается (неуспешная попытка окно не «отравляет» — иначе мусорный кадр
  вытеснял бы легитимные трейлеры из окна).
- Окно приёма **отдельно** от счётчика отправки: шифрование окно не смотрит, поэтому
  round-trip «зашифровал → расшифровал» одним контекстом работает.

**red:** `cargo test --offline -p discord-voice --test srtp_contract` → runtime-падение:
`decrypt_nonce_reuse_rejected_k` — «повтор nonce отклоняется» (второй `decrypt_rtp`
успешно расшифровал кадр) → **exit code 101** (лог `waves/RW-25/red-W26a-fix3.log`).

**green:** тот же прогон → **22 passed; 0 failed; exit code 0**
(лог `waves/RW-25/green-W26a-fix3.log`; на момент реализации F-A3 — 22 passed).

**Тест:** `decrypt_nonce_reuse_rejected_k` — второй `decrypt_rtp` того же кадра ⇒
`NonceReuse { trailer: 0x8000_0000 }`; кадр с другим трейлером (0x8000_0010)
принимается; для SRTCP — тот же контракт.

**Остаточный риск:** окно ограничено 1024 трейлерами — «очень старый» повтор (вне
окна) теоретически пройдёт. Для медиа-трафика (окно ≈ 20 с) достаточно; при
требовании строгой защиты нужен больший размер окна или счётчик воспроизведённых
кадров — решение за Team Lead.

## F-A4 (P1) — переполнение счётчика ChaCha20 (и GCM/CTR)

**Что изменено:**
- `src/crypto/mod.rs`: новая вариация `CryptoError::MessageTooLong { limit: usize }`
  (+ `Display`); константы `GCM_MAX_MESSAGE_LEN` (блок 16 Б) и
  `CHACHA_MAX_MESSAGE_LEN` (блок 64 Б) — оба от 32-битного счётчика (2³²−1 блоков);
  общий крейт-приватный помощник `check_message_len(len, block_size, limit)`.
- `src/crypto/gcm.rs`: публичный `Gcm::check_message_len(len)`; проверка вызывается
  в `encrypt` и `decrypt` **до** аллокации буферов.
- `src/crypto/chacha.rs`: публичные `ChaCha20Poly1305::check_message_len(len)` и
  `XChaCha20Poly1305::check_message_len(len)`; проверка в общем пути `seal`/`open`
  (покрывает оба AEAD-варианта) — до аллокации.
- Устранена тихая перемотка `block_counter.wrapping_add(1)`: при превышении предела
  возвращается ошибка, а не повтор keystream.

**red:** `cargo test --offline -p discord-voice --test crypto_vectors` →
`error[E0599]: no variant named 'MessageTooLong'` и
`no associated function or constant named 'check_message_len'` → **exit code 101**
(лог `waves/RW-25/red-W26a-fix4.log`).

**green:** тот же прогон → **16 passed; 0 failed; exit code 0**
(лог `waves/RW-25/green-W26a-fix4.log`; на момент реализации F-A4 — 16 passed).

**Тест:** `aead_rejects_overlong_message_k` — граница проверяется **по длине, без
аллокации** (можно передать `usize::MAX`): `Gcm::check_message_len(usize::MAX)` и
`check_message_len(GCM_MAX_MESSAGE_LEN + 1)` ⇒ `MessageTooLong { limit }`;
`check_message_len(GCM_MAX_MESSAGE_LEN)` и `check_message_len(1024)` ⇒ Ok; те же
проверки для `ChaCha20Poly1305` и `XChaCha20Poly1305`.

**Остаточный риск:** пределы (≈64 ГиБ для GCM, ≈256 ГиБ для ChaCha20) — следствие
32-битного счётчика AEAD; реальная длина медиа-кадра (десятки байт) далека от них.

## Итоговые прогоны

`waves/RW-25/green-W26a-fix.log`:

| команда | результат | exit code |
|---|---|---|
| `cargo test --offline -p discord-voice --test crypto_vectors` | 16 passed; 0 failed | 0 |
| `cargo test --offline -p discord-voice --test srtp_contract` | 22 passed; 0 failed | 0 |
| `cargo test --offline -p discord-voice` | 65 passed (1 + 16 + 11 + 22 + 15); 0 failed | 0 |
| `cargo fmt -p discord-voice` | — | 0 |
| `cargo clippy --offline -p discord-voice --all-targets -- -D warnings` | чисто | 0 |

Новых тестов в этом раунде: **4** (`sbox_matches_fips197_k`, `counter_rewind_rejected_k`,
`decrypt_nonce_reuse_rejected_k`, `aead_rejects_overlong_message_k`); всего в карточке
**38** тестов (16 + 22), в пакете — 65.

`unwrap`/`expect`/`panic!`/`unreachable!` в продуктовом коде — отсутствуют
(проверено grep по `src/crypto/*.rs`, `src/srtp.rs`). Новых зависимостей, сети,
`unsafe`, `git commit/push` — нет. `cargo fmt` применён только к своему пакету
(`-p discord-voice`); чужие крейты не тронуты.

## Файлы (md5, актуальные)

| файл | md5 |
|---|---|
| `rust/crates/discord-voice/src/crypto/mod.rs` | `5286b0d8fa2e3657a0dc8476b329e08e` |
| `rust/crates/discord-voice/src/crypto/aes.rs` | `77cf89a01bd3a001785b43f67d550974` |
| `rust/crates/discord-voice/src/crypto/gcm.rs` | `5f0308ccbe7189736ab04cd58dc0f129` |
| `rust/crates/discord-voice/src/crypto/chacha.rs` | `f292f3cdb95317a7a37ee7a24563a7ae` |
| `rust/crates/discord-voice/src/crypto/kdf.rs` | `596094464fabfdbc557a7fa84beb7424` (без изменений) |
| `rust/crates/discord-voice/src/srtp.rs` | `f06531afd891e4c38bf873b808cd9eb9` |
| `rust/crates/discord-voice/tests/crypto_vectors.rs` | `9b91ade1070d3d19db50e20123451c78` |
| `rust/crates/discord-voice/tests/srtp_contract.rs` | `b8b5a183189903ba4fa6f97b5d9ff29f` |
| `waves/RW-25/red-W26a-fix1.log` | `78fc5dde52e92d0171b020911a76e134` |
| `waves/RW-25/red-W26a-fix2.log` | `9e80dbdb12e7e0154237ff26b6b8c771` |
| `waves/RW-25/red-W26a-fix3.log` | `afeb6300bc1fa9d862f7fdef4efbad28` |
| `waves/RW-25/red-W26a-fix4.log` | `c41a28b4d8de2acc498c595c2862ed3e` |
| `waves/RW-25/green-W26a-fix1.log` | `c8b4620624712506357fa0b1ea007b1c` |
| `waves/RW-25/green-W26a-fix2.log` | `beb100ec574bf1cddc2842b9c4bc0213` |
| `waves/RW-25/green-W26a-fix3.log` | `135238b7be83c6ed9ea557e8e7699a3c` |
| `waves/RW-25/green-W26a-fix4.log` | `086e9995fca1d0da28c2c646bdf609e2` |
| `waves/RW-25/green-W26a-fix.log` | `dc031972378e544569f2f1c70afb69ae` |

md5 в `waves/RW-25/worker-W26a.md` — состояние **до** этого раунда (историческая запись,
не пересчитывалась).

## Изменения замороженного контракта (санкционированы Team Lead)

1. `SrtpError::CounterRewind { current: u64, requested: u64 }` — новая вариация.
2. `SrtpError::NonceReuse { trailer: u32 }` — новая вариация.
3. `CryptoError::MessageTooLong { limit: usize }` — новая вариация.
4. `crypto::GCM_MAX_MESSAGE_LEN`, `crypto::CHACHA_MAX_MESSAGE_LEN` — публичные константы.
5. `Gcm::check_message_len`, `ChaCha20Poly1305::check_message_len`,
   `XChaCha20Poly1305::check_message_len` — публичные проверки длины (до аллокации).
6. `crypto::aes::sbox`, `crypto::aes::inv_sbox` — публичные вычисления S-box.
   Остальные сигнатуры ТЗ §3 не менялись.

## Риски / открытые вопросы

1. **Скорость AES без таблиц**: S-box считается на каждый байт (~13 умножений GF(2⁸)).
   Для медиа-кадров приемлемо, но при включении крейта в горячий путь (десятки
   кадров/с) — замерить; альтернатива (constant-time таблицы в регистрах) требует
   отдельного решения. Оставлено как есть: требование «нет утечки через кэш» важнее.
2. **Окно приёма 1024 трейлера** — «очень старый» повтор вне окна теоретически
   проходит. Размер окна — решение Team Lead (см. F-A3).
3. **Пределы длины сообщения** — следствие 32-битного счётчика AEAD; при переходе на
   64-битные счётчики пределы надо пересчитать.
4. Изменения контракта (п. «Изменения замороженного контракта») требуют подтверждения
   на приёмке и синхронизации `TASKS.md`/`docs/map.md` — это делает Team Lead после
   accepted, я документы не трогал.

## Следующий шаг

Team Lead — приёмка раунда доработки (F-A1…F-A4) и синхронизация контракта в
`TASKS.md`/`docs/map.md`; затем Integration/QA — batch-гейт волны и мутации по новым
кейсам (`sbox_matches_fips197_k`, `counter_rewind_rejected_k`,
`decrypt_nonce_reuse_rejected_k`, `aead_rejects_overlong_message_k`).
