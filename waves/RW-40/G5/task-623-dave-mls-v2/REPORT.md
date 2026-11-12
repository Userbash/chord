# Отчёт воркера — TASK-623: DAVE MLS Ratchet Key Derivation & Commit Verification (`G5 Voice & Platform`)

- **Карточка / Задача:** `TASK-623` (`task_8f6f84d732d6`, dispatch `ctx_72fa84ef20c0`)
- **Отдел / Роль:** `G5 Voice & Platform`
- **Рабочее дерево:** `/home/user/orca/workspaces/project/task-623-dave-mls-v2`
- **Ветка:** `feature/task-623-dave-mls-v2`

---

## 1. Сделано

1. **Сверка с эталонным RE-оракулом (`reference-oracle/native-re/raw/discord_voice/strings-a-n6.txt` и `nm-all-demangled.txt`):**
   - Извлечены точные контракты `libdave` (`mls_key_ratchet.cpp`, `cryptor_manager.cpp`, `mls/session.cpp`) и `mlspp::HashRatchet`:
     - Метка экспорта пользовательских медиа-ключей: `DAVE_MLS_USER_MEDIA_KEY_BASE_LABEL = b"Discord Secure Frames v0"` (`strings-a-n6.txt:76085`).
     - Метки продвижения `mlspp::HashRatchet`: `MLS 1.0 key`, `MLS 1.0 nonce`, `MLS 1.0 secret` (`strings-a-n6.txt:76715-76730`, `238131-238136`).
     - Константы и функции управления поколениями в `cryptor_manager.cpp`: `DAVE_RATCHET_GENERATION_SHIFT_BITS = 24` (`truncated_nonce >> 24`), `DAVE_MAX_GENERATION_GAP = 250`, `DAVE_MAX_LIFETIME_GENERATIONS = 256`, `compute_wrapped_generation` (`ComputeWrappedGeneration`), `compute_wrapped_big_nonce` (`ComputeWrappedBigNonce`).
     - Строгие проверки верификации MLS Proposal / Commit / Welcome в `mls/session.cpp` (`strings-a-n6.txt:76000-76085`): `ValidateProposalMessage`, `ProcessProposals`, `ProcessCommit`, `VerifyWelcomeState`, `ProcessWelcome`, `GetLastEpochAuthenticator`.
2. **Реализована иерархическая деривация ключей поколений `MlsKeyRatchet` в `rust/crates/discord-voice/src/srtp.rs` и `src/dave.rs`:**
   - Созданы структуры `MlsKeyRatchet` и `MlsRatchetKeyAndNonce` (с редактированием ключевого материала `[REDACTED]` в `Debug`).
   - Поддержано однонаправленное продвижение секрета по поколениям (`get_key_and_nonce`, `get_key`), кэширование промежуточных поколений для приёма out-of-order пакетов, защита от DoS по разрыву поколений (`> DAVE_MAX_GENERATION_GAP (250)` → `SrtpError::MalformedHeader`), контроль времени жизни (`>= DAVE_MAX_LIFETIME_GENERATIONS (256)` → `SrtpError::CounterOverflow`) и удаление отработанных ключей (`delete_key`, `delete_keys_up_to`) для обеспечения Forward Secrecy (повторный запрос удалённого поколения возвращает `SrtpError::CounterRewind`).
   - В `DaveMediaCipher` интегрирован `MlsKeyRatchet` (`DaveMediaCipher::with_ratchet`, `ratchet`, `ratchet_mut`, `set_counter`) с автоматическим удалением старых поколений на стороне отправителя при переходе через границу `2^24` и распаковкой поколения через `compute_wrapped_generation` на стороне получателя.
3. **Реализована структурированная верификация MLS Proposal / Commit / Welcome и деривация `epoch_authenticator` в `DaveMlsEngine`:**
   - Добавлены типы `MlsProposalType` (`Add = 1`, `Remove = 2`) и `MlsVerificationError` (`MissingExternalSender`, `InvalidWireFormat`, `UnexpectedGroupId`, `EpochMismatch`, `InvalidExternalSenderSignature`, `InvalidProposalType`, `UnrecognizedUserId`, `DuplicateIdentityInRoster`, `UnrecognizedProposalRef`, `CommitWithoutQueuedProposals`, `UnexpectedExternalSenderCount`, `UnexpectedExternalSender`, `ConfirmationTagMismatch`), а также конструкторы `build_structured_mls_proposal`, `build_structured_mls_revoke`, `build_structured_mls_commit`, `build_structured_mls_welcome`.
   - В `DaveMlsEngine` добавлена константная по времени (`ct_eq`) проверка тегов подписи `external_sender` и `confirmation_tag`, проверка `group_id == channel_id`, сверка целевой эпохи, валидация отсутствия дубликатов и вхождения всех участников в `recognized_users`, очередь `pending_proposals` с поддержкой отзыва по `proposal_ref`, автоматический сброс с отправкой `op:31 InvalidCommitWelcome` + свежего `op:26 KeyPackage` (до `DAVE_MAX_RESETS = 3`), а также вычисление `epoch_authenticator()`, 30-значного `voice_privacy_code()` и пользовательских `user_key_ratchet(user_id)`.

---

## 2. Red-прогон (Strict TDD)

- **Тестовый файл:** `rust/crates/discord-voice/tests/srtp_contract.rs`
- **Команда:**
  ```sh
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice --test srtp_contract
  ```
- **Лог:** `waves/RW-40/G5/task-623-dave-mls-v2/red.log`
- **Результат Red-фазы (до реализации в `srtp.rs` и `dave.rs`):**
  ```text
  error[E0432]: unresolved imports `discord_voice::dave::DAVE_MAX_GENERATION_GAP`, `discord_voice::dave::DAVE_MAX_LIFETIME_GENERATIONS`, `discord_voice::dave::DAVE_RATCHET_GENERATION_SHIFT_BITS`, `discord_voice::dave::MlsKeyRatchet`, `discord_voice::dave::compute_wrapped_big_nonce`, `discord_voice::dave::compute_wrapped_generation`
  error[E0432]: unresolved imports `discord_voice::dave::DAVE_MAX_RESETS`, `discord_voice::dave::MlsProposalType`, `discord_voice::dave::MlsVerificationError`, `discord_voice::dave::build_structured_mls_commit`, `discord_voice::dave::build_structured_mls_proposal`, `discord_voice::dave::build_structured_mls_revoke`, `discord_voice::dave::build_structured_mls_welcome`
  error: could not compile `discord-voice` (test "srtp_contract") due to 23 previous errors
  ```

---

## 3. Green-прогон, Targeted и Гейт пакета

- **Targeted-прогон (`srtp_contract`):**
  ```sh
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice --test srtp_contract
  ```
  Результат: **`24 passed; 0 failed`**.
- **Полный прогон пакета `discord-voice` (`waves/RW-40/G5/task-623-dave-mls-v2/green.log`):**
  ```sh
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice
  ```
  Результат: **`227 passed; 0 failed`** (включая `35` unit-тестов и `192` интеграционных/контрактных теста по всем 11 тестовым наборам `discord-voice`).
- **Форматирование и статический анализ (`rustfmt` + `clippy`):**
  ```sh
  rustfmt --edition 2024 --check rust/crates/discord-voice/src/dave.rs rust/crates/discord-voice/src/srtp.rs rust/crates/discord-voice/tests/srtp_contract.rs
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --all-targets -- -D warnings
  ```
  Результат: **`0 warnings, 0 errors`**.

---

## 4. Мутационное тестирование (`5/5 KILLED`)

Лог: `waves/RW-40/G5/task-623-dave-mls-v2/mutations.log`.

| ID | Файл | Мутация | Статус | Убивающий тест |
|---|---|---|---|---|
| `M1` | `src/srtp.rs` | Граница разрыва поколений `> DAVE_MAX_GENERATION_GAP` → `>= DAVE_MAX_GENERATION_GAP` | **KILLED** | `dave_mls_key_ratchet_forward_securing_out_of_order_cache_and_generation_gap_bounds` |
| `M2` | `src/srtp.rs` | Пропуск удаления ключа из кэша в `MlsKeyRatchet::delete_key` | **KILLED** | `dave_mls_key_ratchet_forward_securing_out_of_order_cache_and_generation_gap_bounds` |
| `M3` | `src/srtp.rs` | Замена `wrapping_sub` на `saturating_sub` в `compute_wrapped_generation` | **KILLED** | `dave_mls_key_ratchet_forward_securing_out_of_order_cache_and_generation_gap_bounds` |
| `M4` | `src/srtp.rs` | Обход побайтового сравнения тегов подписи/подтверждения в `ct_eq` | **KILLED** | `dave_mls_structured_proposal_commit_and_welcome_verification_and_epoch_authenticator` |
| `M5` | `src/srtp.rs` | Отключение проверки непустой очереди предложений `pending_proposals.is_empty()` при обработке Commit | **KILLED** | `dave_mls_structured_proposal_commit_and_welcome_verification_and_epoch_authenticator` |

Контрольные суммы MD5 до и после мутаций полностью совпадают:
- `d0f5f56ba528504152abbae89b9af5b9  rust/crates/discord-voice/src/srtp.rs`

---

## 5. Изменённые и созданные файлы + MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/dave.rs` | modified | `ec8781c4d4ffff09497a9b789d15066f` |
| `rust/crates/discord-voice/src/srtp.rs` | modified | `d0f5f56ba528504152abbae89b9af5b9` |
| `rust/crates/discord-voice/tests/srtp_contract.rs` | modified | `8436a399a5f85d0aeb437e4eeef4021f` |
| `waves/RW-40/G5/task-623-dave-mls-v2/red.log` | added | артефакт волны |
| `waves/RW-40/G5/task-623-dave-mls-v2/green.log` | added | артефакт волны |
| `waves/RW-40/G5/task-623-dave-mls-v2/mutations.log` | added | артефакт волны |
| `waves/RW-40/G5/task-623-dave-mls-v2/REPORT.md` | added | артефакт волны |

---

## 6. Риски и примечания

- `#![forbid(unsafe_code)]` в крейте `discord-voice` полностью сохранён.
- Внешние зависимости и `Cargo.toml` не изменялись.
- Сохранена 100% обратная совместимость со всеми существующими тестами `voice_call_state_machine_contract`, `rtp_video_contract` и модульными тестами `srtp::tests` / `session::tests`.
