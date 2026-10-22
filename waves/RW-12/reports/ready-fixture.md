# RW-12 / precondition P-1 — JSON→`EtfValue` адаптер для READY-фикстур

**Дата:** 2026-10-20. **Владелец:** координатор (не W12a/W12b/W12c).
**Назначение:** снять блокер W12a/W12b/W12c — тесты не могли читать реальный
`fixtures/gateway/by-type/002-READY.json` как ETF-терм без `serde_json`.

## 1. Файлы

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/tests/support/mod.rs` | `9eaa57db534e4a9874203879bf329e1a` |
| `rust/crates/discord-core/tests/support/ready_fixture.rs` | `6b7dd87ae187a2d05734029e075eb405` |
| `rust/crates/discord-core/tests/ready_fixture.rs` (проверка, 3 теста) | `3ee7fc07d4dd003a571ff6b8ad266448` |

## 1a. Precondition P-6 — якорь `d.user.id` (red-first, координатор)

Без этого кейс B10 (top-level `user.id="abc"` → `InvalidId`) недостижим: композиция глушила
`parse_user` через `unwrap_or_default()`. Изменения — только в координаторских файлах:

- композиция `Ready::parse` (`snapshot.rs`): `parse_user(...)` → `None` ⇒
  `ReadyError::InvalidId { path: "d.user.id" }`;
- заглушка `social::parse_user`: валидный `id` (через `value::unsigned`) ⇒
  `User { id, ..Default::default() }`, отсутствие/битый id ⇒ `None`. W12b заменит тело,
  сохранив контракт `None` для битого id.

- **Red:** `cargo test --offline -p discord-core --test ready_fixture` → `2 passed; 1 failed`,
  `# EXIT=101` (`ready-anchor-red.log`; md5 теста `ready-anchor-red.md5`; красная копия
  `ready-fixture-test.red.rs`).
- **Green:** `3 passed; 0 failed`, `# EXIT=0` (`ready-anchor-green.log`); после red — только
  rustfmt-перенос строки в тесте, семантика не менялась.

Адаптер общий для тестовых бинарей, поэтому в `tests/support/mod.rs` осознанно разрешён
`dead_code` (иначе `clippy -D warnings` падал бы в бинарях, где помощник не востребован).

## 2. Red → green

- **Red:** `cargo test --offline -p discord-core --test ready_fixture` до создания
  `tests/support/` → `error[E0583]: file not found for module support`, `# EXIT=101`
  (`ready-fixture-red.log`).
- **Green:** после адаптера → `2 passed; 0 failed`, `# EXIT=0`
  (`ready-fixture-green.log`).
- `cargo fmt --check` и `clippy --workspace --all-targets -- -D warnings` — чисто.
- Правка rustfmt после red — только форматирование теста (`ready-fixture-fmt.diff`,
  17 строк); red-копия обновлена до финального текста, контракт кейсов не менялся.

## 3. Соответствие форме захвата

- JSON `null` → `EtfValue::Nil` (`NIL_EXT`); `"nil"`/`"true"`/`"false"` → `EtfValue::Atom`
  (`0x64`), остальные строки → `Str`; ключи объектов → `Atom`; массивы → `List`;
  целые → `Int`, дробные → `Float`; big-int вне `i64` остаётся десятичной строкой
  (`ready::value::unsigned` разбирает такие ID).
- Поддержаны `\uXXXX` (включая surrogate pairs), 224 экранированные кавычки, UTF-8.

## 4. Независимый оракул

`tests/ready_fixture.rs` сверяет адаптер с jq-числами дизайна A1: 40/7/272/132/363/51,
17 приватных, 146 users, 60 связей, `merged_members` 7×1, `user.id=384299970632876032`,
`member_count=24228` у ITSKIDON, атомы и `unsigned` на реальном файле.
