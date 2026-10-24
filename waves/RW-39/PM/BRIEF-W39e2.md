# BRIEF W39e2 — закрыть 4×P1 Codex в BFF (возврат E4)

Владелец: W39e2 (Backend, продолжение W39e). PM: DeepSeek. Итерация: 2.
Основание: `waves/RW-39/PM/w39e/REVIEW-codex.md` — FAIL, P0=0, P1=4.

## Файлы (только эти)
- `rust/gui-spike/src/data.rs`
- `rust/gui-spike/tests/data_contract.rs`
- логи/отчёт: `waves/RW-39/PM/w39e2/`
`Cargo.toml`, `ui/**`, `fixtures/**`, `ipc_contract.rs`, `launch.rs` — read-only.

## Задачи (каждая red → green, отдельный тест)

1. **R1 размер фикстуры.** Ввести `MAX_FIXTURE_BYTES = 64 MiB`; до чтения проверить
   `metadata.len()`; больше — `DataError::Parse` с путём и лимитом, без паники и без полного
   чтения в память. Red: тест с файлом > лимита (создать во временном корне, не в `fixtures/`).
2. **R2 лимит входа.** `MAX_INPUT_LEN = 256` для `channel_id`/`guild_id`/`cursor`; пустой или
   длиннее — `InvalidArg`; в сообщении — усечённое значение (`…`), не вся строка.
   Red: id из 10 000 символов и cursor из 10 000; проверить длину сообщения.
3. **R3 строгий кадр.** Кадр `by-type`: `t` обязан быть непустой строкой; `d` — присутствовать
   (объект/массив/число/строка; `Null` допустим только если так в дампе — обосновать).
   Иначе `Parse` с путём и индексом. Red: `{}` и `{"t":""}` → `Parse`.
4. **R4 битый участник.** `GUILD_MEMBER_LIST_UPDATE` с item без `member.user.id` → `Parse`
   (имя/индекс item), а не тихий пропуск. Red: `{"member":{"user":{}}}`.
5. Не ослаблять существующие 17 тестов; новые — минимум +4; `DL_STRICT` сохранить.

## Проверки
```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Окно/probe/gate/сеть/commit — запрещены. Один cargo за раз.

## Отчёт
`waves/RW-39/PM/w39e2/REPORT.md`: R1–R4 → red-лог → green-лог, md5, что осталось.
