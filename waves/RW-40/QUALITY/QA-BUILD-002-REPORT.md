# Отчёт централизованного тестирования QA — Сборка `BUILD-002`

- **Задача QA:** `qa/task-120-qa-build-002` (`task_b08ae872706d`, `ctx_b606d5a5b3e3`)
- **Роль:** Тестировщик QA (`G4 QA`)
- **Рабочая ветка:** `qa/task-120-qa-build-002`
- **Номер сборки:** `BUILD-002` ([waves/RW-40/BUILDS/BUILD-002.md](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/waves/RW-40/BUILDS/BUILD-002.md))
- **Дата и время тестирования:** 2026-10-30, 20:26–20:44 +05:00
- **Финальный вердикт по сборке `BUILD-002`:** **QA Passed** (`P0 = 0`, `P1 = 0`, `P2 = 3` — неблокирующие замечания)

---

## 1. Окружение и синхронизация ветки

| Параметр | Значение |
|---|---|
| **ОС / Ядро** | `Linux 7.3.0-5-cachyos #1 SMP PREEMPT_DYNAMIC Fri, 23 Oct 2026 14:38:02 +0000 x86_64` |
| **Git HEAD (QA)** | `4e240481e59ff4d49732ac8bd5ba08a006363748` (`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`) |
| **Git HEAD (`testing`)** | `4e240481e59ff4d49732ac8bd5ba08a006363748` — совпадает |
| **Расхождение веток** | `git rev-list --left-right --count testing...HEAD` → `0	0` (побайтово одинаковый коммит) |
| **Node.js / npm** | `v26.13.0` / `11.20.0` |
| **TypeScript (`tsc`)** | `Version 7.0.2` |
| **Rust (`rustc` / `cargo`)** | `rustc 1.99.0 (b940084d7 2026-09-28)` / `cargo 1.99.0 (5f94df478 2026-08-27)` |
| **CPU** | 16 логических ядер |
| **Режим сборки** | `--offline`, профиль `test` (debug) |
| **Сырые логи прогона** | `/tmp/opencode/qa-build-002/` (`rust-contracts.log`, `rust-bin.log`, `rust-lib-search.log`, `meta.log`, `tsc-root.log`, `tsc-ui.log`, `unit.log`, `workspace.log`) |

Ветка QA не содержит собственных правок: тестировался ровно тот коммит, что лежит в `testing`. Ни один файл вне `waves/RW-40/QUALITY/QA-BUILD-002-REPORT.md` не менялся.

---

## 2. Состав `BUILD-002` и сверка MD5

Новые задачи поверх `BUILD-001` — `task-110` (`M-13-SEARCH-202-NSFW-BACKEND` Pass 2, [PR-04](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/waves/RW-40/PR/PR-04-task-110-search-202-be.md)) и `task-109` (`BUGFIX-IPC-ORACLE-STRICT`, [PR-05](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/waves/RW-40/PR/PR-05-task-109-ipc-oracle-strict.md)). Дополнительно проверены артефакты `task-116` ([PR-06](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/waves/RW-40/PR/PR-06-task-116-qa-build-001.md)) и `task-118` ([PR-07](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/waves/RW-40/PR/PR-07-task-118-parity-wave10.md)).

| Файл | MD5 в `BUILD-002.md` | Фактический MD5 | Итог |
|---|---|---|---|
| `rust/gui-spike/src/search.rs` | `3ec29ae3219be411465ce3b2cc749287` | `3ec29ae3219be411465ce3b2cc749287` | **совпал** |
| `rust/gui-spike/tests/search_contract.rs` | `f113b75de5727b82fe1ca5dc0f57eb3b` | `f113b75de5727b82fe1ca5dc0f57eb3b` | **совпал** |
| `rust/gui-spike/src/main.rs` | `24bb37a8f233213ba7c59dc7b9fb1b22` | `24bb37a8f233213ba7c59dc7b9fb1b22` | **совпал** |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `01929cf0e635a2e4ef946e416a61801f` | `01929cf0e635a2e4ef946e416a61801f` | **совпал** |
| `waves/RW-40/PARITY/MERGE-READINESS-14.md` | `6f80719b979a9cc6f168bdcc31dc3087` | `6f80719b979a9cc6f168bdcc31dc3087` | **совпал** |

Все пять контрольных сумм из манифеста сборки сошлись. Проверялся именно тот код, который закреплён в `BUILD-002.md`.

---

## 3. Шаги воспроизведения: ожидание и факт

### Шаг 1. Контрактные тесты Rust (`BUILD-002.md` §2.1)

- **Команда:**
  ```sh
  cargo test --manifest-path rust/gui-spike/Cargo.toml --offline \
    --test search_contract --test ipc_wiring_contract \
    --test notifications_contract --test attachments_contract
  cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --bin discord-gui-spike
  cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --lib search
  ```
- **Ожидаемый результат (§2.1):** `search_contract` `23/23`, `ipc_wiring_contract` `9/9`, `notifications_contract` `23/23`, `attachments_contract` `4/4`, `--bin discord-gui-spike` `12/12`, `--lib search` `2/2`, во всех наборах `0 failed`, код возврата `0`.
- **Фактический результат:** все наборы зелёные, `0 failed`, код возврата `0`. Число тестов совпало по сумме (`59` контрактных + `12` bin + `2` lib), но два набора пришли с обратным распределением: `notifications_contract` — `4/4`, `attachments_contract` — `23/23`. Подробности — замечание **P2-1**.
- **Вывод лога (контрактные наборы):**
  ```text
  Running tests/attachments_contract.rs
  running 23 tests
  test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  Running tests/ipc_wiring_contract.rs
  running 9 tests
  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

  Running tests/notifications_contract.rs
  running 4 tests
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

  Running tests/search_contract.rs
  running 23 tests
  test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
  ```
- **Вывод лога (`--bin discord-gui-spike`):**
  ```text
  running 12 tests
  test wiring_tests::invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input ... ok
  test wiring_tests::invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts ... ok
  test wiring_tests::every_network_command_reads_token_through_one_helper ... ok
  ...
  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
  ```
- **Вывод лога (`--lib search`):**
  ```text
  running 2 tests
  test search::tests::retry_after_header_parses_positive_integer_seconds_with_5000ms_fallback ... ok
  test search::tests::m13_constants_and_indexing_error_format_match_spec ... ok
  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s
  ```

**Функциональное покрытие `PR-04` (search 202) подтверждено тестами:** `returns_immediate_indexing_envelope_on_http_202_without_sleeping_or_reading_body`, `retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results`, `retries_http_202_against_local_mock_http_server_and_parses_200_ok`, `returns_typed_indexing_error_when_all_5_retries_return_202`, `decodes_documents_indexed_from_200_ok_and_defaults_to_zero_when_omitted`, `includes_nsfw_and_attempts_query_params_and_validates_attempts_range`, `enforces_max_search_offset_9975_and_rejects_above_ceiling_before_transport`.

**Функциональное покрытие `PR-05` (строгий IPC-оракул) подтверждено тестами:** `channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field`, `every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`, `tauri_main_registers_and_wires_all_six_ipc_commands_via_token_or`, `tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or`, `ipc_error_kinds_serialize_to_expected_frontend_tags_without_leaking_secrets`.

Дополнительно сверил живой UI-вызов: [notifications.js](file:///home/sanya/orca/workspaces/project/qa-task-120-qa-build-002/rust/gui-spike/ui/features/notifications.js) (`submitChannelOverridePatch`, строка ~1606) передаёт `override: overridePatch` верхним полем и конверт `patch.channel_overrides[channelId]` с тем же `channelId`. Строгий путь без fallback (`main.rs:841-857`) этот вызов не ломает — регрессии на рабочем сценарии нет.

### Шаг 2. Мета-политики (`BUILD-002.md` §2.2)

- **Команда:**
  ```sh
  node --test tests/meta/*.test.mjs
  ```
- **Ожидаемый результат:** `102/102` pass, `fail 0`, `skipped 0`, код возврата `0`.
- **Фактический результат:** `102/102` pass, `17 suites`, `fail 0`, `skipped 0`, `duration_ms 926.90`, код возврата `0`.
- **Вывод лога:**
  ```text
  ✔ backlog-sync: бэклог переноса ↔ docs ↔ README/TASKS
  ✔ QA-M2-04: CI browser-job (pinned Chromium, без live)
  ✔ PLAT-02 T3: cargo подтверждает ignored-набор (867.344624ms)
  ✔ plan-sync: §8 ↔ тестовые файлы ↔ модули
  ✔ TEST-POLICY: запрет ослабления тестов
  ✔ QA-M2-01: e2e-skip не является прохождением
  ✔ waves-sync: журнал волн восстановим
  ℹ tests 102
  ℹ suites 17
  ℹ pass 102
  ℹ fail 0
  ℹ cancelled 0
  ℹ skipped 0
  ℹ todo 0
  ℹ duration_ms 926.904472
  ```

Отдельно отмечу пункт `TEST-POLICY: запрет ослабления тестов` — он прошёл, значит в наборе нет `.only`, `.todo`, пустых `assert` и `@ts-ignore`, которыми можно было бы «позеленить» прогон.

### Шаг 3. Проверка типов TypeScript (`BUILD-002.md` §2.2)

- **Команда:**
  ```sh
  npx tsc -p tsconfig.json
  npx tsc -p rust/gui-spike/ui/tsconfig.json
  ```
- **Ожидаемый результат:** оба проекта компилируются без ошибок, код возврата `0`.
- **Фактический результат:** оба вызова вернули пустой вывод и код `0` (`EXIT_ROOT=0`, `EXIT_UI=0`). Ошибок и предупреждений нет.

### Шаг 4. Регрессионный прогон unit-наборов (`BUILD-002.md` §2.3)

- **Команда:**
  ```sh
  npm run test:unit
  ```
- **Ожидаемый результат:** `879/879 pass`, `0 fail`, код возврата `0`.
- **Фактический результат:** `879/879 pass`, `173 suites`, `fail 0`, `cancelled 0`, `skipped 0`, `todo 0`, `duration_ms 2893.69`, код возврата `0`.
- **Вывод лога:**
  ```text
  ✔ walk: движок fixture (64.697795ms)
  ✔ walk: CLI через fake CDP (2038.948587ms)
  ℹ tests 879
  ℹ suites 173
  ℹ pass 879
  ℹ fail 0
  ℹ cancelled 0
  ℹ skipped 0
  ℹ todo 0
  ℹ duration_ms 2893.688449
  ```

### Шаг 5. Дополнительный регресс по всему Rust-воркспейсу (сверх §2)

- **Команда:**
  ```sh
  cargo test --manifest-path rust/Cargo.toml --workspace --offline
  ```
- **Ожидаемый результат:** `0 failed` по всем крейтам.
- **Фактический результат:** `118` тестовых бинарников, `1142 passed`, `0 failed`, `12 ignored`, код возврата `0`.
- **Что за `ignored`:** все 12 — live-кейсы `keyring_contract.rs` с явной пометкой `#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]`. Это объявленный пропуск по среде, а не молчаливый skip; мета-политика `PLAT-02 T3` такой набор подтверждает.

---

## 4. Замечания по критичности

### P0 — блокирующие
Нет.

### P1 — серьёзные
Нет.

### P2 — неблокирующие (3)

**P2-1. В `BUILD-002.md` §2.1 перепутаны счётчики `notifications_contract` и `attachments_contract`.**
Манифест обещает `notifications_contract` `23/23` и `attachments_contract` `4/4`; фактически первый набор содержит 4 теста, второй — 23 (`grep -c '^#\[test\]'`: `notifications_contract.rs` → 4, `attachments_contract.rs` → 23). Это не регресс: оба файла побайтово те же, что в `BUILD-001` (`notifications_contract.rs` `9f047fd938b856037d1c1f5cbc3324a3`, `attachments_contract.rs` `77157aadf4267ded8c225d8891c71a9f` — совпадают с `QA-BUILD-001-REPORT.md`), и прошлый отчёт фиксировал ровно такое распределение. Сумма по четырём контрактным наборам сходится (`59`), ни один тест не потерян, поведение сборки корректно. Правка нужна только в тексте манифеста — иначе следующая приёмка будет сверять несуществующие числа.

**P2-2. `execute_with_retry` не вызывается из продакшена — retry-цикл живёт только в тестах.**
Метод объявлен в `rust/gui-spike/src/search.rs:359`, но кроме `tests/search_contract.rs` вызовов по всему `rust/` нет: IPC-команда идёт через `search()`/`execute()`, то есть по Варианту B (конверт `202` в Rust, поллинг в UI). Риск — дрейф двух реализаций поллинга и соблазн позже подключить метод к IPC, где он заблокирует команду до ~25 с без отмены. Замечание перенесено из ревью `PR-04` (`REVIEW-PASS2.md` §4, P2-A) и на контракт этой сборки не влияет. Решение — либо пометить метод как тестовый хелпер комментарием, либо удалить до появления потребителя.

**P2-3. UI пока не понимает конверт `indexing`.**
`rust/gui-spike/ui/features/search-overlay.js` читает только `messages`/`total_results`/`doing_deep_historical_index` (строка 1035), полей `indexing` и `retry_after_ms` не знает; `include_nsfw` из UI не отправляется нигде. На `202` Rust отдаёт успешный пустой ответ, и оверлей отрисует пустую выдачу без баннера «Индексация» и без повтора. Бэкенд-контракт §3.1 при этом соблюдён, а UI-часть — это `task-111` (`M-13-SEARCH-202-NSFW-UI` Pass 2), которая по §3 манифеста ещё в работе и войдёт в `BUILD-003`. Замечание совпадает с P2-B из `REVIEW-PASS2.md`; блокировать `BUILD-002` нечем.

### Наблюдения без замечаний

- `notifications_contract.rs` состоит из 4 крупных тестов с множеством проверок внутри, `attachments_contract.rs` — из 23 узких. Локализация падения в первом наборе грубее, но на текущем зелёном прогоне это не влияет на достоверность.
- Задачи `task-107` и `task-111` по §3 манифеста в `BUILD-002` не входят — их отсутствие в наборе ожидаемо, а не дефект.
- Флаки не наблюдались: все наборы прошли с первого запуска, повторных прогонов не потребовалось.

---

## 5. Итог

`BUILD-002` прошёл все проверки §2 манифеста: контрактные Rust-наборы (`23/23`, `9/9`, `4/4`, `23/23`), `--bin discord-gui-spike` (`12/12`), `--lib search` (`2/2`), мета-политики (`102/102`), оба проекта `tsc` (`0` ошибок), unit-регресс (`879/879`). Контрольные суммы четырёх ключевых файлов и `MERGE-READINESS-14.md` совпали с манифестом, ветка QA побайтово равна `testing`. Дополнительный прогон всего Rust-воркспейса дал `1142 passed / 0 failed`.

Три замечания P2 не блокируют выпуск: одно — опечатка в счётчиках самого манифеста, два — перенесённые из ревью `PR-04` остаточные риски (тестовый `execute_with_retry` и ещё не сделанный UI-баннер индексации в `task-111`).

**Вердикт: QA Passed** (`P0 = 0`, `P1 = 0`, `P2 = 3`).
