# PLAT-11-META — отчёт воркера G4 QA/Security

- batch/карточка: RW-40 / PLAT-11-META (`waves/RW-40/QUALITY/PLAT-11-AUDIT.md` §3 PLAT-11-A, §4 T1), PM — DeepSeek.
- дата: 2026-10-24. HEAD: `0239543`.
- режим: **без live**, без сети, без окна, без commit. Продукт не менялся (`rust/gui-spike/src/main.rs` не тронут).
- логи: `waves/RW-40/PM/plat-11-meta/*.log`.

## 1. Что сделано

| файл | изменение | md5 |
|---|---|---|
| `tests/meta/ipc-path.test.mjs` | **new**: мета-инвариант «IPC не принимает путь» — `scan()` сигнатур `#[tauri::command]`, 3 сьюта / 12 подтестов, red-proof встроен | `8cc2b1454043640988743075e4e38a04` |
| `TDD-PLAN.md` | §8: одна строка для нового мета-теста (требование `tests/meta/plan-sync.test.mjs`) | `354eb58dd6f413eaefea48fb834893d9` |

Прод read-only: `main.rs` — `6e5b0adb6113383b2b010d1be823ff82` (тот же, что в начале сессии).
`rust/gui-spike/` в индексе git отсутствует целиком (`?? rust/gui-spike/`) — состояние дерева
до моей работы, не следствие правок.

## 2. Что именно проверяет тест

1. **Сигнатуры, не тела.** `scan(source)` вырезает комментарии (`//`, `///`, `//!`, `/* */`,
   вложенные) и содержимое строковых литералов, затем берёт блок после `#[tauri::command]`
   (в т.ч. с параметрами атрибута), находит `fn`, разбирает список аргументов с учётом
   `<>`/`()`/`[]` и одиночного `:` (не `::`). Так `//! Команды (#[tauri::command])` в `lib.rs`
   и строки `.find("#[tauri::command]")` в wiring-тестах не дают ложных блоков.
2. **Два независимых правила отказа:** имя аргумента (`path|file|filepath|filename|dir|dirname|folder|root|location`,
   по сегментам `_`/`-`, поэтому `profile`/`cursor` не краснеют) и тип (`PathBuf|Path|File|OsString|OsStr`, включая `&Path` и `std::path::Path`).
3. **Покрытие файлов:** рекурсивно все `rust/gui-spike/src/**/*.rs` (не только `main.rs`).
4. **Оракул регистрации:** набор найденных команд сравнивается с `generate_handler![...]`
   (команда без регистрации и регистрация без `#[tauri::command]` = красный).
5. **Конфигурация:** нет `tauri-plugin-fs`/`tauri-plugin-dialog` в `Cargo.toml`, `Cargo.lock`,
   `tauri.conf.json`; нет `tauri_plugin_fs`/`tauri_plugin_dialog` в `src/**`; capability окна
   ровно `["core:default"]`.

## 3. Red-proof (мутация)

Инвариант сейчас выполняется, поэтому «красный по коду» невозможен by design; чувствительность
доказана мутацией. Встроенные подтесты: синтетическая команда `fn attachment_upload(path: String)`
→ violation; `source: std::path::PathBuf`, `target: &Path`, `handle: File` → violation;
`root`/`dir`/`file_name` → violation; `profile`/`cursor` → чисто; комментарий/строка с
`path: String` → чисто. Плюс мутация **реального** текста `main.rs` в памяти (файл не пишется):
вставка `fn probe_path(path: String)` → ровно 1 violation.

Внешний прогон (`red-proof.log`, дерево не менялось):

```text
RED-PROOF baseline: commands=35 violations=0
MUTANT path: String: violations=1 [... "attachment_upload","arg":"path",...,"rule":"name"]
MUTANT source: PathBuf: violations=1 [... "capture_open","arg":"source",...,"rule":"type"]
MUTANT root: String: violations=1 [... "scan_tree","arg":"root",...,"rule":"name"]
MUTANT file_name: String (не path): violations=1 [... "preview","arg":"file_name",...,"rule":"name"]
RED-PROOF OK: все мутанты пойманы, дерево не изменялось
```

## 4. Проверки

| команда | результат | лог |
|---|---|---|
| `node --test tests/meta/ipc-path.test.mjs` | **12/12 pass, 0 fail** (3 сьюта) | `green-ipc-path.log` |
| `npm run test:unit` | **606/606 pass, 0 fail** (порог QA-AUDIT 588 не ослаблен, +18 с чужих карточек) | `unit.log` |
| `node --test 'tests/meta/**/*.test.mjs'` | 64/65 pass; **1 красный — чужой** (см. §5) | `meta-suite.log` |
| `npm run typecheck` | **красный — чужой файл** (см. §5) | `typecheck.log` |
| `tsc` без чужого файла (`/tmp/opencode/tsconfig.plat11.json`, extends штатного) | **exit 0** | `typecheck-isolated.log` |
| `tsc -p tsconfig.agent.json` | exit 0 | `typecheck.log` |
| `plan-sync` в копии дерева, где чужому тесту добавлена его строка §8 | **5/5 pass** | `plansync-sim.log` |

Нарушений инварианта: **0** на **35** командах; `scanned == registered` (35/35).

## 5. Красное вне моей карточки (не правил)

`tests/unit/read-state-ui.test.mjs` — **untracked**, mtime 14:11, карточка read-state UI другого
воркера. Он не описан в §8 (plan-sync: `actual: ['tests/unit/read-state-ui.test.mjs']`) и даёт
5 ошибок `tsc` (`previousAuthor`/`previousDay` implicitly any, `TS2345`). Мои файлы в этих
падениях не участвуют: изолированный `tsc` по штатному конфигу без этого файла — exit 0,
plan-sync в копии с его строкой — 5/5. Чужой файл не трогал (правило «только свои файлы»).
Зелёный `plan-sync`/`typecheck` в общем дереве наступит после того, как его владелец добавит
строку §8 и типы; повторный прогон — на приёмке.

## 6. Наблюдения по аудиту PLAT-11

- В аудите: «29 команд». Фактически в `generate_handler!` и в исходнике — **35** (`auth_login_password`,
  `auth_mfa`, `auth_sms`, `net_connect/status/disconnect`, `settings_load/save` уже есть).
  Число в §0/§1 аудита устарело; инвариант от этого не страдает (0 path-аргументов).
- md5 `main.rs` в аудите (`6ea7941d…`) ≠ текущий (`6e5b0adb…`): файл менялся после аудита.
  Я его только читал (mtime `14:05:07` не изменился за сессию, md5 совпадает до/после).
  Тест от конкретной ревизии не зависит: он читает файл при запуске и падает только на
  path-аргументе; число команд в проверке — порог `>= 29`, а не жёсткая константа.

## 7. Риски и границы

| риск/граница | что делать |
|---|---|
| Эвристика имён: запрещены сегменты `path/file/dir/root/...`; легитимное `filename` в IPC теперь нельзя | так и задумано: путь не должен приходить из webview; для файлов — handle (`u64`) по PLAT-11 §2 п.2 |
| Мультиредакторный `#[tauri::command]` в другом крейте/workspace | тест сканирует `rust/gui-spike/src`; при появлении второго Tauri-бинаря добавить его каталог в список |
| Не покрывает T2 (`send_message` кладёт `channelId` в URL-путь) и T3 (symlink/canonicalize) | отдельные карточки PLAT-11-B / T3, аудит §4 |
| Тест не доказывает отсутствие рантайм-инъекции обработчиков | вне области: `generate_handler!` статичен, регистрация сверяется отдельно |
| Ложный зелёный, если атрибут написан не как `#[tauri::command]` (например, `tauri::command` через `use`) | в спайке такого нет; при рефакторинге — расширять регэксп, а не ослаблять тест |

## 8. Для приёмки

DoD PLAT-11-A: T1 существует и доказан мутацией — **выполнено** (§3), дерево чистое,
продукт не менялся. §8 TDD-PLAN содержит строку:

```text
| `tests/meta/ipc-path.test.mjs` | — | мета: IPC не принимает путь — `scan` сигнатур `#[tauri::command]` (`PathBuf`, `File`, `path`/`root`/`dir`), `tauri-plugin-fs`/`tauri-plugin-dialog`, 0 violations |
```
