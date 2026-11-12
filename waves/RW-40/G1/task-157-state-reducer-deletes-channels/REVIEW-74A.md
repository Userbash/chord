# Technical Review (Pass 74A) — task-157 (`BE-P1-02` / `F-GW-06`: State Reducer for Message Delete & Channel/Thread CRUD)

- **Карточка:** `task-157` (`BE-P1-02` / `F-GW-06`)
- **Ворктри:** `/home/user/orca/workspaces/project/feature-task-157-state-reducer-deletes-channels`
- **Проверяемые файлы:**
  * `rust/crates/discord-core/src/state/types.rs` (`76959f14e9b8e52c19f8cd930124ac64`)
  * `rust/crates/discord-core/src/state/reducer.rs` (`c792da047c48423aa2ab6d4b1096d178`)
  * `rust/crates/discord-core/tests/state_contract.rs` (`0521a09e586c0c0c37a78d6d9278162b`)
- **Статус ревью:** `PASS / Ready for Merge`
- **Блокирующие замечания (P0/P1):** 0

---

## 1. Проверка компиляции и тестов

Запущен тестовый набор с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:
```bash
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test state_contract --test state_reducer --test state_types --test state_selectors --lib
```

**Результаты прогона:**
- `unittests src/lib.rs`: 29 passed, 0 failed, 0 ignored.
- `tests/state_contract.rs`: 15 passed, 0 failed, 0 ignored (`c01`..`c15`).
- `tests/state_reducer.rs`: 19 passed, 0 failed, 0 ignored.
- `tests/state_selectors.rs`: 8 passed, 0 failed, 0 ignored.
- `tests/state_types.rs`: 15 passed, 0 failed, 0 ignored.
- **Итого:** 86/86 passed, 0 failed, 0 warnings (exit code 0).

Форматирование проверено: `rustfmt --edition 2024 --check` чисто.
Clippy для целевых артефактов проверен: `cargo clippy -p discord-core --lib --test state_contract --test state_reducer --test state_types --test state_selectors -- -D warnings` чисто.

---

## 2. Верификация ручных мутаций (5/5 KILLED)

Все 5 ручных мутаций в продуктовом коде `rust/crates/discord-core/src/state/reducer.rs` были протестированы против набора тестов `state_contract.rs`, гарантированно убиты падением соответствующих ассертов и затем исходные файлы были полностью восстановлены с контролем md5sum.

1. **Мутант 1: отключение удаления сообщений при удалении канала (`apply_channel_delete`)**
   - *Мутация:* закомментирована строка `state.messages.retain(|_, msg| msg.channel_id != channel_id);`
   - *Результат:* **KILLED** тестом `c05_channel_delete_removes_channel_and_messages` (`assertion left == right failed, left: 3, right: 1`).
   - *Восстановление:* md5 `c792da047c48423aa2ab6d4b1096d178` подтверждён.

2. **Мутант 2: пропуск обновления индекса `channel_guild` при создании/обновлении канала (`apply_channel_upsert`)**
   - *Мутация:* закомментирована строка `state.channel_guild.insert(channel_id, guild_id);`
   - *Результат:* **KILLED** тестом `c03_channel_create_and_update_in_existing_guild` (`assertion left == right failed, left: None, right: Some(100)`).
   - *Восстановление:* md5 `c792da047c48423aa2ab6d4b1096d178` подтверждён.

3. **Мутант 3: пропуск цикла удаления в `MessageDeleteBulk`**
   - *Мутация:* закомментирован цикл `for id in &ev.ids { state.messages.remove(id); }`
   - *Результат:* **KILLED** тестом `c02_message_delete_bulk_removes_multiple_messages` (`assertion left == right failed, left: 4, right: 2`).
   - *Восстановление:* md5 `c792da047c48423aa2ab6d4b1096d178` подтверждён.

4. **Мутант 4: пропуск записи несогласованности при создании канала в неизвестной гильдии (`apply_channel_upsert`)**
   - *Мутация:* закомментирована строка `state.diagnostics.inconsistencies.push(format!("guilds:{guild_id}:missing"));`
   - *Результат:* **KILLED** тестом `c04_channel_create_in_unknown_guild_records_inconsistency` (`panicked at: должна быть зафиксирована несогласованность с гильдией 999`).
   - *Восстановление:* md5 `c792da047c48423aa2ab6d4b1096d178` подтверждён.

5. **Мутант 5: пропуск удаления треда из `state.threads` в `apply_thread_delete`**
   - *Мутация:* закомментирована строка `state.threads.remove(&ev.id);`
   - *Результат:* **KILLED** тестом `c07_thread_delete_removes_thread_and_messages` (`assertion left == right failed, left: Some(Thread { ... }), right: None`).
   - *Восстановление:* md5 `c792da047c48423aa2ab6d4b1096d178` подтверждён.

---

## 3. Проверка контрольных сумм файлов (Baseline Restore Verification)

Исходные файлы проверены до и после тестирования мутаций:
- `76959f14e9b8e52c19f8cd930124ac64  rust/crates/discord-core/src/state/types.rs`
- `c792da047c48423aa2ab6d4b1096d178  rust/crates/discord-core/src/state/reducer.rs`
- `0521a09e586c0c0c37a78d6d9278162b  rust/crates/discord-core/tests/state_contract.rs`

Хеши совпадают с зафиксированными в отчёте воркера (`REPORT.md`). Изменения полностью соответствуют контрактам State/Reducer и ТЗ.

---

## 4. Итоговый вердикт

**PASS / Ready for Merge**. Реализация качественная, полностью покрыта контрактными тестами, инварианты состояния и диагностика соблюдены.
