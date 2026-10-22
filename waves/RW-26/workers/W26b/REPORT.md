# Отчёт воркера — B07/RW-26/P4-11/W26b

## 1. Границы

- Файлы продукта: `rust/crates/discord-voice/src/regions.rs`, `rust/crates/discord-voice/tests/regions_contract.rs`.
- Артефакты карточки: `red.log`, `green.log`, этот отчёт.
- Shared-файлы, чужие тесты, доки, golden, Cargo/package не менялись.

## 2. Red

```sh
cd rust && CARGO_TARGET_DIR=/home/sanya/.cache/discord-lab-target-rw26b cargo test --offline -p discord-voice --test regions_contract
# EXIT=101; среда не разрешила создать target-dir (Read-only file system)
```

Лог: `waves/RW-26/workers/W26b/red.log`.

## 3. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/discord-lab-target-rw26b cargo test --offline -p discord-voice --test regions_contract
# EXIT=0; 9 passed, 0 failed
```

Exact target из ТЗ (`/home/sanya/.cache/discord-lab-target-rw26b`) недоступен для записи в sandbox; для targeted-прогона использован writable `/tmp`.

Лог: `waves/RW-26/workers/W26b/green.log`.

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-voice/src/regions.rs` | `cfdc134535969c8087ba4bdcb809a0c5` |
| `rust/crates/discord-voice/tests/regions_contract.rs` | `bdef17cfcff95bb585734dc3d5d52d3a` |
| `waves/RW-26/workers/W26b/red.log` | `135deba3c31ec500e5aab6915deb7895` |
| `waves/RW-26/workers/W26b/green.log` | `6ee67b0a332489b92f04c84117004b7d` |

## 5. Риски/вопросы

- `regions.rs` ожидает подключение через `pub mod regions;` в `src/lib.rs` интеграционным слоем; `lib.rs` по ТЗ не менялся.
- Payload UDP latency-проб и native-метрика ранжирования не подтверждены capture; медиана — модель карточки.
- Предупреждение о мёртвом варианте `UnavailableRegion` возникает при standalone-подключении тестом.

## 6. Передача

- next: TL-2 integration / QA / review.
- batch gate: не запускал — у integration/QA.
