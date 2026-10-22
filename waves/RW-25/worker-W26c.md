# Отчёт воркера W26c — B06 / RW-25, карточка P8-01 (собственный updater, крейт `discord-updater`)

- **batch/карточка**: RW-25 (волна B06), лейн C, карточка **P8-01** — «Свой updater |
  манифест+Ed25519, staged-замена, откат | T-SYS-11 | P7 | L».
- **ТЗ**: `waves/RW-25/TZ-W26c.md` (замороженный API §3, тест-кейсы §4).

## Сделано

Реализован крейт `discord-updater` (модули `manifest.rs` и `plan.rs`) по замороженному
API ТЗ, без изменения `Cargo.toml`/`Cargo.lock`/`src/lib.rs`.

`src/manifest.rs`:
- `Version` (`major.minor.patch[-pre]`) с собственным разбором и сравнением — без `semver`:
  `Version::parse` отвергает пустую строку, `1.2`, `1.2.3.4`, `1.2.x`, `v1.2.3`, ведущие нули
  (`01.2.3`), пустой pre-release (`1.2.3-`); `Version::is_newer_than` — pre-release младше
  своей же стабильной версии, pre-суффиксы сравниваются лексикографически.
- `Artifact`, `Manifest`, `ManifestError` — ровно по ТЗ §3 (11 вариантов ошибки).
- `parse_manifest` — **не** проверяет подпись; проверяет валидный JSON, наличие
  `version`/`artifacts`/`name`/`url`/`sha256`/`size`, 64 hex-символа у `sha256`,
  уникальность имён артефактов, `size >= 0` (отрицательное/дробное ⇒ `InvalidJson`).
- `verify_manifest` — длина блоба < 64 ⇒ `SignatureMissing`; несовпадение подписи
  (в т.ч. подменённое тело и чужой ключ) ⇒ `InvalidSignature`. Подпись проверяется
  **до** разбора тела, поэтому подмена даёт `InvalidSignature`, а не ошибку JSON.
- `sign_manifest` — `body || signature[64]`; `sha256_hex` — 64 символа нижнего регистра.
- Подпись Ed25519 через `ring 0.17`: `ring::signature::ED25519`,
  `UnparsedPublicKey::new(...).verify(...)`, `Ed25519KeyPair::from_seed_unchecked`,
  `Ed25519KeyPair::sign`. Новых зависимостей нет (`ring`, `sha2`, `serde_json` уже в
  `Cargo.toml` крейта).

`src/plan.rs`:
- `PlanStep` (6 вариантов), `UpdatePlan`, `PlanError` — по ТЗ §3; `PlanError::Manifest`
  обёртка с `impl From<ManifestError>`.
- `plan_update` — ищет артефакт по имени (`UnknownArtifact`), проверяет новизну версии
  (`NotNewer`), строит 5 шагов: `Download` → `VerifyDigest` → `Stage` → `Swap` →
  `RemoveBackup`. Раскладка внутри `install_dir`: `stage/<имя>.download`,
  `stage/<имя>.new`, `<имя>` (цель), `<имя>.bak` (backup).
- `apply_local_steps` — сверка `sha256_hex(source)` с ожидаемым хэшем из шага
  `VerifyDigest` **до любых перемещений файлов** (`DigestMismatch`), копирование в
  staging, увод прежней цели в backup, переименование staged-файла на место цели; при
  сбое подмены backup возвращается на место, ошибка возвращается вызывающему.
- `rollback` — возвращает прежнее содержимое цели из backup и снимает backup.
- Ошибок глотания нет: все файловые операции через `std::fs`, всё через `Result`;
  в продуктовом коде нет `unwrap`/`expect`/`panic!`/`unsafe` (проверено grep).

## Red-прогон

Лог: `waves/RW-25/red-W26c.log`.

```
$ cd /home/sanya/orca/project/rust
$ export CARGO_TARGET_DIR=/tmp/opencode/target-w26c
$ cargo test --offline -p discord-updater --test updater_contract
→ error[E0432]: unresolved imports `discord_updater::manifest::Manifest`, ... `parse_manifest`,
  `sha256_hex`, `sign_manifest`, `verify_manifest`
→ error[E0432]: unresolved imports `discord_updater::plan::PlanError`, `PlanStep`,
  `UpdatePlan`, `apply_local_steps`, `plan_update`, `rollback`
→ error: could not compile `discord-updater` (test "updater_contract") due to 2 previous errors
EXIT_CODE=101
```

Red зафиксирован до написания продуктового кода: тест-файл компилировался только
против API, которого ещё не было (модули были заглушками). Первые два прогона теста
также дали red из-за двух опечаток в самом тесте (`//!`-строка с `*` вместо `-` и
non-ASCII в `b"..."`); они исправлены в тесте до продуктового кода, лог red содержит
финальный «красный» прогон против заглушек.

## Green-прогон

Лог: `waves/RW-25/green-W26c.log`.

```
$ cargo test --offline -p discord-updater --test updater_contract
→ test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
EXIT_CODE=0

$ cargo test --offline -p discord-updater
→ test result: ok. 23 passed; 0 failed (интеграционные) + 0 doc-тестов
EXIT_CODE=0

$ cargo clippy --offline -p discord-updater --all-targets -- -D warnings
→ Finished `dev` profile
EXIT_CODE=0

$ cargo fmt -p discord-updater -- --check
EXIT_CODE=0
```

## Targeted-тесты

`rust/crates/discord-updater/tests/updater_contract.rs` — **23 теста**, все зелёные.
Покрытие кейсов ТЗ §4 (все 20 обязательных + 3 дополнительных):

| кейс ТЗ | тест |
|---|---|
| manifest_parse_k | `manifest_parse_k`, `manifest_parse_pre_release_k` |
| manifest_invalid_json_k | `manifest_invalid_json_k` |
| manifest_missing_field_k | `manifest_missing_field_k` |
| manifest_invalid_version_k | `manifest_invalid_version_k` |
| manifest_invalid_digest_k | `manifest_invalid_digest_k` |
| manifest_duplicate_artifact_k | `manifest_duplicate_artifact_k` |
| signature_ok_k | `signature_ok_k` |
| signature_bad_k | `signature_bad_k` |
| signature_wrong_key_k | `signature_wrong_key_k` |
| signature_tampered_body_k | `signature_tampered_body_k` |
| signature_missing_k | `signature_missing_k` |
| version_order_k | `version_order_k` |
| version_parse_rejects_k | `version_parse_rejects_k` |
| plan_not_newer_k | `plan_not_newer_k` |
| plan_unknown_artifact_k | `plan_unknown_artifact_k` |
| plan_steps_k | `plan_steps_k` |
| digest_mismatch_k | `digest_mismatch_k` |
| staged_swap_k | `staged_swap_k` |
| rollback_k | `rollback_k` |
| swap_failure_rollback_k | `swap_failure_rollback_k` |
| (сверка с внешним оракулом) | `sha256_hex_known_k`, `rfc8032_test_vector_k` |

Оракулы (не собственная реализация):
- SHA-256 — векторы `sha256sum` для `""`, `"abc"`, `"hello"` (NIST FIPS 180-4).
- Ed25519 — вектор **RFC 8032 TEST 1**: seed `9d61b19d…7f60`, публичный ключ
  `d75a9801…511a`, подпись над пустым сообщением `e5564300…100b`. Публичный ключ для
  этого seed дополнительно сверен с `openssl pkey -pubout` (совпал байт в байт).
- План и staged-замена — сверяются с фактическим состоянием файлов в temp-каталоге.

Файловые операции тестов — только под `std::env::temp_dir()` с уникальным суффиксом
(`pid` + наносекунды + счётчик), каталог удаляется в `Drop`.

## Файлы (+ md5)

| файл | md5 |
|---|---|
| `rust/crates/discord-updater/src/manifest.rs` | `3d42884339f4609e0cddcfd43fec14ab` |
| `rust/crates/discord-updater/src/plan.rs` | `d7508386fdab51aedcc5db9d5c58dce3` |
| `rust/crates/discord-updater/tests/updater_contract.rs` | `9c74ca5259accffb68c3d8f577b143b3` |
| `waves/RW-25/red-W26c.log` | `61d3cb4fdb848bf57554aeca38cf9e4a` |
| `waves/RW-25/green-W26c.log` | `1ca64c8d5a5902adba5918c822862116` |

Не трогались (по границам ТЗ §1): `Cargo.toml`, `Cargo.lock`, `src/lib.rs` крейтов,
другие крейты, `waves/db.jsonl`, `STATE.md`, `STATUS.md`, `PLAN.md`, `TASKS.md`,
`docs/map.md`, `logs/`, `functions/`, `evidence/`.

## Риски / открытые вопросы

1. **`cargo fmt --all` затронул два чужых файла.** По ТЗ команда запускается из `rust/`
   после зелёной фазы; на тот момент лейн W26a параллельно писал
   `rust/crates/discord-voice/tests/crypto_vectors.rs` и
   `rust/crates/discord-voice/tests/srtp_contract.rs`. Мой `cargo fmt --all` их
   переформатировал (изменение только форматирующее, без изменения логики). Эти файлы
   не в git (untracked), поэтому точечно откатить их нельзя; повторный
   `cargo fmt --all -- --check` по workspace сейчас показывает diff только в
   `srtp_contract.rs` — это уже последующая правка того же лейна. **Риск для приёмки:
   изменение вне моего набора файлов** — нужно подтвердить у TL-2/ревьюера, что W26a
   не считает форматирование своей правкой. Мои файлы отформатированы
   (`cargo fmt -p discord-updater -- --check` — 0).
2. **Шаг `RemoveBackup` в плане не исполняется в этой карточке** — по ТЗ §3 backup
   снимается после успешного запуска обновлённого приложения, а запуск вне карточки
   (сеть/запуск вне контура). `apply_local_steps` оставляет backup на месте; `rollback`
   его снимает. Это соответствует формулировке ТЗ, но следующий подэтап должен
   исполнить `RemoveBackup` явно.
3. **`PlanStep::Rollback` не порождается в `plan_update`** — вариант есть в
   замороженном enum и используется как контракт операции отката, но построитель плана
   его не добавляет: откат выполняется функцией `rollback(plan)` по шагу `Swap`.
   Если TL считает, что `Rollback` должен присутствовать в `steps`, требуется правка
   ТЗ (пути отката отличаются от путей прямого плана).
4. **Ожидаемый хэш берётся из шага `VerifyDigest` плана**, а не из отдельного
   аргумента-манифеста: `apply_local_steps` по ТЗ принимает только `(plan, source)`.
   Значение `expected` в плане — это и есть хэш из манифеста (план строится из него),
   так что семантика «сверить с манифестом» соблюдена. Фактический хэш (`actual`)
   вычисляется при исполнении и в плане остаётся пустым — поле по ТЗ не уточняет,
   когда оно заполняется.
5. **`size` артефакта в манифесте не сверяется с фактическим размером файла** при
   исполнении — ТЗ §3 требует только сверки хэша. Если нужна проверка размера,
   это отдельный подэтап.
6. Полный `cargo test --workspace`, `npm run gate:worker`, независимое ревью и
   мутации — не выполнялись воркером (по `PROCESS.md` §3.13–§3.15 это зона
   Integration/QA на batch).

## Следующий шаг

Передать Team Lead / Integration (лейн batch B06) на приёмку: red/green-логи, отчёт и
md5 — на месте. Отдельно требуется решение по п. 1 (чужие файлы после `cargo fmt --all`)
и подтверждение по пп. 3–4 (семантика `Rollback`/`actual` в плане).
