# BRIEF W39i — backend P3-02: поля сообщений/участников + локальный поиск

Владелец: W39i (Backend). PM: DeepSeek. **Запуск: после приёмки W39e2** (он правит `data.rs`).
Вход: `HANDOFF.md` §6, `DESIGN/SPEC-P3-02.md`, `fixtures/README.md`, `fixtures/MANIFEST.json`,
`src/data.rs`, `tests/data_contract.rs`.

## Файлы (только эти)
- `rust/gui-spike/src/data.rs`, `rust/gui-spike/tests/data_contract.rs`
- логи: `waves/RW-39/PM/w39i/`
READ-ONLY: `Cargo.toml` (зависимости не добавлять), `ui/**`, `fixtures/**` (только читать),
`ipc_contract.rs`, `launch.rs`.

## Требования (по данным фикстур, без выдумывания полей)
1. `messages_page`: добавить в элемент `embeds[]`, `attachments[]`, `reactions[]`,
   `reply_to`, `edited_at`, `day` (дата для разделителя), `group` (start/continuation) —
   только если поле есть в фикстуре; иначе `None`/пустой список, не фейк.
2. `members_page`: `roles[]`, `group` (роль/статус), стабильный порядок, счётчики групп.
3. `search_page(query, filters, cursor, limit)` — локальный поиск по загруженным фикстурам;
   фильтры: автор, канал, содержит, упоминание, дата, тип, закреплено (7 из R-10);
   пустой результат валиден; `limit 1..=100`.
4. Сохранить R1–R4 из W39e2 (лимиты, строгая валидация, типизированные ошибки).
5. **R5**: ограничить `settings(section?)` тем же `MAX_INPUT_LEN=256` → `InvalidArg`
   (остаток из D-12); red-тест на 10 000 символов.
6. Контрактные тесты: позитив на реальных фикстурах + негатив (битый JSON, отсутствующий id,
   слишком длинный фильтр, пустой результат). Без `unwrap` на входе.

## Проверки
```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Окно/probe/gate/сеть/commit — запрещены; один cargo за раз.

## Отчёт
`waves/RW-39/PM/w39i/REPORT.md`: поле → источник в фикстуре → тест, покрытие негатива,
md5, что осталось (например, если в фикстурах нет embeds — честно).
