# Отчёт воркера W26c (fix) — B06 / RW-25, карточка **P8-01** (updater, крейт `discord-updater`)

- **batch/карточка**: RW-25 (волна B06), лейн C, карточка **P8-01** — «Свой updater |
  манифест+Ed25519, staged-замена, откат | T-SYS-11 | P7 | L».
- **Основание**: приёмка с замечаниями — codex-ревью (F-C1, F-C2), локальное LLM-ревью (F-C4).
- **Исходный ТЗ**: `waves/RW-25/TZ-W26c.md`; исходный отчёт воркера: `waves/RW-25/worker-W26c.md`.

## Сводка

| замечание | severity | статус | red (exit code) | green (exit code) |
|---|---|---|---|---|
| **F-C1** path traversal через имя артефакта | P0 | **закрыто** | 101 (`red-W26c-fix-FC1.log`) | 0 |
| **F-C2** план по неподписанному манифесту | P0 | **закрыто** | 101 (`red-W26c-fix-FC2.log`) | 0 |
| **F-C3** откат поверх существующего файла | P0 | **закрыто** | 101 (`red-W26c-fix-FC3-FC4.log`) | 0 |
| **F-C4** целостность backup перед откатом | P2 | **закрыто** | 101 (`red-W26c-fix-FC3-FC4.log`) | 0 |

Итог: **34 теста** в `tests/updater_contract.rs` (было 23) + 1 doc-тест `compile_fail`
(автоматическая проверка FC2(в)) — всё зелёное, clippy `-D warnings` чистый, `cargo fmt -p
discord-updater -- --check` — 0.

Логи: `red-W26c-fix-FC1.log`, `green-W26c-fix-FC1.log`, `red-W26c-fix-FC2.log`,
`green-W26c-fix-FC2.log`, `red-W26c-fix-FC3-FC4.log`, `green-W26c-fix-FC3-FC4.log`,
итоговый `green-W26c-fix.log`.

---

## F-C1 (P0) — path traversal через имя артефакта

### Что было

`artifact.name` из подписанного (но всё равно сетевого) манифеста подставлялся прямо в
пути `stage/<имя>.new`, `<имя>`, `<имя>.bak`. Имя вида `../../outside` выводило запись за
пределы `install_dir` — вектор произвольной записи файлов.

### Red

`waves/RW-25/red-W26c-fix-FC1.log` (exit code **101**).

Чтобы red был честным, а не «красным только из-за отсутствующего варианта ошибки»,
проверка имени и проверка путей были временно отключены в коде (`if false && …`), и
зафиксирован реальный красный прогон:

```
test plan_rejects_unsafe_artifact_name_k ... FAILED
небезопасное имя артефакта должно отвергаться: UpdatePlan { … artifact: "../../outside",
steps: [ Download { to: ".../stage/../../outside.download" }, …,
Swap { to: ".../4/../../outside", backup: ".../4/../../outside.bak" } …] }
test result: FAILED. 29 passed; 1 failed
EXIT_CODE=101
```

Вывод падения наглядно показывает уход путей за `install_dir` (`.../4/../../outside`).

### Что изменилось

`src/plan.rs`:
- новая вариация ошибки `PlanError::UnsafeArtifactName { name: String }` (санкционировано
  Team Lead);
- `is_safe_component()` — имя артефакта принимается только как **один безопасный
  компонент пути**: непустое, ≤ 255 байт, ASCII-печатное (`0x21..=0x7E`), без `/` и `\`,
  не `.` и не `..`;
- проверка имени выполняется **до** построения путей (сразу после поиска артефакта);
- вторая линия защиты: `is_inside(install_dir, path)` — сравнение по компонентам
  (`Component::ParentDir`/`RootDir` в хвосте запрещены), без `canonicalize` (файлы могут
  ещё не существовать). Применяется ко всем четырём путям плана.

`src/manifest.rs`: имя артефакта в манифесте не санируется — это осознанно: манифест
разбирается «как есть», а безопасное имя гарантирует **слой планирования** (единственное
место, где из имени строится путь).

### Green

`waves/RW-25/green-W26c-fix-FC1.log` (exit code **0**), 30 тестов.

Новые тесты:
- `plan_rejects_unsafe_artifact_name_k` — 9 небезопасных имён (`../../outside`, `../evil`,
  `sub/dir`, `sub\dir`, `""`, `.`, `..`, `with space`, `bell\u{7}`) ⇒ `UnsafeArtifactName`;
- `plan_keeps_all_paths_inside_install_dir_k` — ни один путь плана не выходит за `install_dir`;
- `plan_accepts_safe_artifact_name_k` — валидные имена (`app`, `discord-1.2.3.bin`,
  `app_2026-10-22`, `a.b.c`) ⇒ план строится, все пути внутри `install_dir`.

### Изменения контракта

`PlanError` получил вариант `UnsafeArtifactName { name: String }`. Остальное API ТЗ §3
сохранено.

---

## F-C2 (P0) — план строился по неподписанному манифесту

### Что было

`plan_update` принимал `&Manifest`, а `parse_manifest` подпись не проверяет ⇒ неподписанный
манифест из сети можно было подсунуть в план. Требование «манифест подписан» держалось на
соглашении, а не на типах.

### Red

`waves/RW-25/red-W26c-fix-FC2.log` (exit code **101**):

```
error[E0432]: unresolved import `discord_updater::manifest::VerifiedManifest`
error[E0599]: no method named `version` found for struct `…::Manifest`
error[E0599]: no method named `artifacts` found for struct `…::Manifest`
EXIT_CODE=101
```

### Что изменилось

`src/manifest.rs`:
- новый тип `VerifiedManifest` с **приватными** полями (`version`, `released_at`,
  `artifacts`) и аксессорами `version() -> &Version`, `artifacts() -> &[Artifact]`,
  `artifact(name) -> Option<&Artifact>`;
- `verify_manifest(signed, public_key)` теперь возвращает `Result<VerifiedManifest,
  ManifestError>`; получить `VerifiedManifest` можно **только** через успешную проверку
  подписи — приватные поля исключают ручную конструкцию;
- `parse_manifest` по-прежнему возвращает обычный `Manifest` и используется только для
  разбора/инспекции.

`src/plan.rs`:
- `plan_update(current: &Version, manifest: &VerifiedManifest, artifact_name: &str,
  install_dir: &Path)` — принимает исключительно подписанный манифест.

### Проверка «в) на уровне API»

Требование «попытка собрать план из `parse_manifest` не компилируется» закрыто
**автоматически**, а не только словами: в doc-комментарии `plan_update` добавлен
`compile_fail`-doctest, который передаёт `&Manifest` в `plan_update` и обязан не собраться.

```
$ cargo test --offline -p discord-updater
   Doc-tests discord_updater
test crates/discord-updater/src/plan.rs - plan::plan_update (line 149) - compile fail ... ok
```

### Green

`waves/RW-25/green-W26c-fix-FC2.log` (exit code **0**), 27 тестов.

Новые тесты: `verified_manifest_from_signed_bytes_k` (валидная подпись ⇒ `VerifiedManifest` с
корректными аксессорами, из него строится план), `verified_manifest_requires_valid_signature_k`
(битая подпись ⇒ ошибка, `VerifiedManifest` не получен), `plan_from_verified_manifest_k`,
`parse_manifest_stays_plain_manifest_k` (разбор остаётся обычным `Manifest`).

### Изменения контракта

- `verify_manifest` → `Result<VerifiedManifest, ManifestError>` (вместо `Manifest`);
- `plan_update` принимает `&VerifiedManifest` (вместо `&Manifest`);
- новый тип `VerifiedManifest` (приватные поля + 3 аксессора).

Изменения санкционированы Team Lead в замечании F-C2.

---

## F-C3 (P0) — откат поверх существующего файла

### Что было

`rollback` делал `rename(backup → target)`. На Windows `std::fs::rename` **не заменяет**
существующий файл ⇒ откат после успешной установки провалился бы и оставил бы новую
версию. Поведение зависело от платформы.

### Red

`waves/RW-25/red-W26c-fix-FC3-FC4.log` (exit code **101**): тесты FC3/FC4 не собирались
(отсутствовала вариация `PlanError::BackupCorrupted`).

**Честная оговорка о red:** на Linux (текущая платформа) `rename` заменяет существующий
файл, поэтому «красного» поведения именно на этой ОС не видно — старый код на Linux давал
бы верный результат. Red зафиксирован по отсутствующему API, а **поведение зафиксировано
тестом** `rollback_replaces_existing_target_k` (плюс два теста на сбой), который будет
красным на Windows-семантике и на любой платформе, где `rename` не заменяет файл.

### Что изменилось

`src/plan.rs`, `rollback` — порядок операций теперь не зависит от платформы:

1. существующая цель переносится во временное имя `<имя>.failed` (рядом с целью);
2. backup переносится на место цели;
3. временный файл удаляется.

При любом сбое — `PlanError::RollbackFailed`, backup остаётся на месте; если цель уже была
уведена во временный файл, она возвращается на место (откат не оставляет систему в
полусломанном состоянии). Занятое временное имя ⇒ отказ до любых перемещений.

### Green

`waves/RW-25/green-W26c-fix-FC3-FC4.log` (exit code **0**), 34 теста.

Новые тесты:
- `rollback_replaces_existing_target_k` — цель существует, backup существует ⇒ после
  отката цель содержит содержимое backup, backup отсутствует, временный файл отсутствует;
- `rollback_refuses_occupied_temp_name_k` — занятое `<имя>.failed` ⇒ отказ, цель и backup
  нетронуты;
- `rollback_missing_backup_keeps_target_k` — backup недоступен ⇒ отказ, исходная цель не
  уничтожена, временный файл не создавался.

### Изменения контракта

Публичный API не менялся. Внутренний порядок операций `rollback` изменён; добавлена
внутренняя функция `temp_path()`.

---

## F-C4 (P2) — целостность backup перед откатом

### Что было

`rollback` восстанавливал backup без проверки: повреждённый backup (сбой питания в момент
подмены) молча уходил бы на место цели.

### Red

`waves/RW-25/red-W26c-fix-FC3-FC4.log` (exit code **101**) — общий red с FC3:
`error[E0599]: no variant named `BackupCorrupted` found for enum `PlanError``.

### Что изменилось

`src/plan.rs`:
- новая вариация `PlanError::BackupCorrupted { artifact: String }` (явная ошибка, как
  просил замечание; альтернатива `DigestMismatch` не подошла — там «хэш фактических
  данных не совпал с ожидаемым» про скачанный артефакт, здесь про прежнюю версию файла);
- `apply_local_steps` при уводе прежней цели в backup **фиксирует хэш** прежнего
  содержимого в метке `<имя>.bak.sha256` (рядом с backup, внутри `install_dir`);
- `rollback` перед восстановлением сверяет фактический хэш backup с зафиксированным:
  несовпадение ⇒ `BackupCorrupted`, **backup и цель не трогаются**;
- при отсутствии метки (backup сделан не нашей подменой) сверка не выполняется — сверять
  не с чем; это явно оговорено в doc-комментарии;
- при успешном откате/сбое подмены метка удаляется, чтобы не оставлять мусор.

### Отклонение от буквы замечания (обосновано)

Замечание предлагало сверять хэш backup с `artifact.sha256` **из манифеста**. Это
семантически невозможно: по контракту ТЗ §3 backup — это **прежняя** версия файла
(«Атомарно заменить целевой файл, сохранив прежний в backup»), а `artifact.sha256` в
манифесте — хэш **нового** артефакта. Буквальная сверка отказывала бы в **каждом**
корректном откате (хэш прежней версии ≠ хэш нового артефакта), то есть ломала бы рабочее
поведение. Поэтому ожидаемый хэш backup фиксируется в момент подмены — это и есть та
«эталонная величина», которая ловит именно описанный в замечании сбой (сбой питания в
момент swap ⇒ backup повреждён ⇒ хэш не совпал ⇒ отказ). Поведение, требуемое замечанием
(«backup с изменённым содержимым ⇒ отказ, цель не заменена»), реализовано полностью;
источник ожидаемого хэша отличается от предложенного — решение задокументировано здесь и
в doc-комментариях `apply_local_steps`/`rollback`.

### Green

`waves/RW-25/green-W26c-fix-FC3-FC4.log` (exit code **0**).

Новый тест: `rollback_rejects_corrupted_backup_k` — backup с изменённым содержимым ⇒
`BackupCorrupted`, цель не заменена (`hello` на месте), backup не тронут
(`corrupted-bytes` на месте).

---

## Итоговые прогоны (гейт)

`waves/RW-25/green-W26c-fix.log`:

```
$ cargo test --offline -p discord-updater
→ test result: ok. 34 passed; 0 failed  (tests/updater_contract.rs)
→ test result: ok. 1 passed; 0 failed   (doc-test compile_fail)
EXIT_CODE=0

$ cargo clippy --offline -p discord-updater --all-targets -- -D warnings
→ Finished `dev` profile
EXIT_CODE=0

$ cargo fmt -p discord-updater -- --check
EXIT_CODE=0
```

Форматирование выполнялось **только** для своего крейта (`cargo fmt -p discord-updater`),
в отличие от прошлого раунда, где `cargo fmt --all` задел два файла лейна W26a. Проверено:
за последние минуты изменены только мои три файла.

## Файлы (+ md5)

| файл | md5 |
|---|---|
| `rust/crates/discord-updater/src/manifest.rs` | `6b8f0411bbbfb44449decfc05527c1f1` |
| `rust/crates/discord-updater/src/plan.rs` | `2088cc72b4e46aa83f3adf437a1e9113` |
| `rust/crates/discord-updater/tests/updater_contract.rs` | `9efc775f7b0a7d26dfdae299999abb6c` |
| `waves/RW-25/red-W26c-fix-FC1.log` | `f57f8484a38da992aacdb43fb5de470a` |
| `waves/RW-25/green-W26c-fix-FC1.log` | `264660e9ccf03f0295e82612e4344417` |
| `waves/RW-25/red-W26c-fix-FC2.log` | `1c201d63d43941c93217a271f84c5058` |
| `waves/RW-25/green-W26c-fix-FC2.log` | `f4848e7823a81bfa016256071270e2b2` |
| `waves/RW-25/red-W26c-fix-FC3-FC4.log` | `d1cc83a492e8172fed0d6b0075459a68` |
| `waves/RW-25/green-W26c-fix-FC3-FC4.log` | `30b3039b0ff9d450b415fc02cba7f13d` |
| `waves/RW-25/green-W26c-fix.log` | `8aa783ecbb3a2cc43171167140a72ddd` |

Не трогались: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, чужие крейты, доки, `waves/db.jsonl`,
`STATE.md`, `STATUS.md`, `PLAN.md`, `TASKS.md`, `docs/map.md`, `logs/`, `functions/`,
`evidence/`. Новых зависимостей, сети, `git commit/push`, `unsafe` — нет.

## Риски / открытые вопросы

1. **Метка хэша backup — новый файл в каталоге установки** (`<имя>.bak.sha256`). Она
   внутри `install_dir`, удаляется при откате и при сбое подмены, но **не** удаляется
   шагом `RemoveBackup` (шаг описан в плане, но исполняется вне карточки — после успешного
   запуска). Следующий подэтап должен сносить метку вместе с backup. Прошу Team Lead
   зафиксировать это в контракте `RemoveBackup`.
2. **FC4: источник ожидаемого хэша** — зафиксирован при подмене, а не взят из манифеста
   (обоснование в разделе F-C4). Если TL настаивает на сверке именно с `artifact.sha256`
   из манифеста, требуется пересмотр модели swap (например, backup = копия *нового*
   артефакта) — это меняет смысл отката и выходит за рамки карточки.
3. **F-C1: имя артефакта в манифесте не санируется** на уровне `parse_manifest` —
   безопасность обеспечивается в слое планирования. Если понадобится defence-in-depth на
   уровне разбора, требуется новая вариация `ManifestError` (сейчас такой нет в
   замороженном enum).
4. **`VerifiedManifest` не сериализуется** и не кладётся в `UpdatePlan` — план хранит
   только извлечённые значения. Если следующему подэтапу понадобится повторная проверка
   подписи в момент применения плана, это отдельное решение.
5. **Windows-путь FC3 на этой ОС не проверен** — поведение зафиксировано тестом, но
   фактический прогон на Windows не выполнялся (среда Linux).
6. **Полный `cargo test --workspace`, `npm run gate:worker`, независимое ревью и мутации**
   не выполнялись воркером — по `PROCESS.md` §3.13–§3.15 это зона Integration/QA на batch.

## Следующий шаг

Передать Team Lead / Integration на приёмку: red/green-логи по каждому замечанию, отчёт и
md5 на месте. Требуют решения пп. 1 (уборка метки в `RemoveBackup`) и п. 2 (источник
ожидаемого хэша в FC4).
