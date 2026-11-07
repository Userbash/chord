# STUB-27-CORE-ORACLE-FIX — отчёт по закрытию P1 из ревью (RW-40, G1 Core)

Дата: 2026-10-30. Воркер: **G1 Core Rust Architecture**. Карточка: `STUB-27-CORE-ORACLE-FIX` (исправление к `STUB-27-CORE-STREAM-OPCODES` по `waves/RW-40/PM/stub27-core/REVIEW.md`).
Базовый коммит: `4e240481e59ff4d49732ac8bd5ba08a006363748`.

---

## 1. Закрытие замечаний из `REVIEW.md`

1. **P1 (чтение внешнего RE-бандла в `t_stream_00_anchors_match_contract_and_oracle_bundle`)**:
   - В `rust/crates/discord-core/tests/stream_gateway_contract.rs` удалено превращение ошибки `fs::read(lab_path).ok()` в `None` и условные блоки `if let Some(ref bytes) = artifact_bytes`.
   - Чтение эталонного RE-артефакта `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` сделано безусловно обязательным (`fs::read(lab_path).unwrap_or_else(|e| panic!(...))` с явным сообщением об обязательности внешнего RE-оракула по `TEST-POLICY.md`).
   - Проверка длины бандла (`expected_bytes == 12_410_287` Б и `bytes.len() == expected_bytes`) и побайтовая сверка всех 17 якорей (`&bytes[offset..offset + len] == quote`) теперь выполняются безусловно на каждом запуске `t_stream_00_anchors_match_contract_and_oracle_bundle`.
2. **P1 (`pub mod stream;` в `rust/crates/discord-core/src/lib.rs`)**:
   - Подключение `pub mod stream;` в `rust/crates/discord-core/src/lib.rs` официально подтверждено и принято Team Lead; файл не изменялся в рамках данного фикса (md5 сохранён: `e847d7c368fe2ff7e3615dd745ac65a9`).

---

## 2. Таблица изменения теста (по `TEST-POLICY.md` §5)

| Место | Old | New | Основание |
|---|---|---|---|
| `stream_gateway_contract.rs:126-160` (`t_stream_00_anchors_match_contract_and_oracle_bundle`) | `let artifact_bytes = fs::read(lab_path).ok();` + `if let Some(ref bytes) = artifact_bytes { ... }` (молчаливый пропуск при `Err`) | `assert_eq!(expected_bytes, 12_410_287);` + обязательное `fs::read(lab_path).unwrap_or_else(|e| panic!(...))` + безусловная сверка `bytes.len() == 12_410_287` и всех 17 срезов `&bytes[offset..offset + len] == quote` | Закрытие P1 из `waves/RW-40/PM/stub27-core/REVIEW.md`: усиление теста и запрет ложнозелёного прохождения без RE-оракула (`TEST-POLICY.md` §2 п. 3) |

---

## 3. Проверка и доказательства (evidence)

| Команда | Exit code | Результат | Артефакт |
|---|---:|---|---|
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core --test stream_gateway_contract) > waves/RW-40/PM/stub27-core/green-fix.log 2>&1` | `0` | **11 passed; 0 failed** (включая безусловную побайтовую сверку 17 якорей и длины `12_410_287` Б в `t_stream_00_anchors_match_contract_and_oracle_bundle`) | `waves/RW-40/PM/stub27-core/green-fix.log` |
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo clippy --offline -p discord-core --test stream_gateway_contract -- -D warnings)` | `0` | 0 предупреждений | — |
| `(cd rust && rustfmt --edition 2024 --check crates/discord-core/tests/stream_gateway_contract.rs)` | `0` | чисто | — |

---

## 4. Файлы и контрольные суммы (md5)

| Файл | Статус в фиксе | md5 |
|---|---|---|
| `rust/crates/discord-core/tests/stream_gateway_contract.rs` | modified (усилен `t_stream_00`) | `99a5577644f436a467bac50a676dff3d` |
| `rust/crates/discord-core/src/stream.rs` | unchanged | `e4e0450559bcf947261ca7bc91aea91f` |
| `rust/crates/discord-core/src/lib.rs` | unchanged (принято Team Lead) | `e847d7c368fe2ff7e3615dd745ac65a9` |
| `waves/RW-40/PM/stub27-core/green-fix.log` | new | `6cfd6b4e81e66979b9da9191cac47789` |

---

## 5. Риски

- Другие файлы репозитория не затрагивались, `git commit` не выполнялся.
- Внешний RE-бандл `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`12 410 287` Б) прочитан и побайтово сверен по всем 17 якорям (`A-OPCODES`…`A-REST`).
