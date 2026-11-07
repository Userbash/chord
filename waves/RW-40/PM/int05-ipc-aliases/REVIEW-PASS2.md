# DeepSeek Review 27B: INT-05-IPC-ALIASES — независимое ревью pass 2

**Вердикт: PASS (P0 — нет, P1 — нет).**

Единственная находка pass 1 (красный batch-гейт) закрыта: `.gate/worker-2026-10-30T14-36-10-163Z.json` — `verdict: passed`, все 7 фаз зелёные. MD5 контрольных файлов совпадают с `REPORT.md`, гейт снят после последней правки кода. Замечания уровня P2 не блокируют приёмку.

## Проверка закрытия P1

Находка pass 1 (`REVIEW.md:7`): артефакт `.gate/worker-2026-10-30T13-37-49-761Z.json` имел `verdict: failed` — `meta` 101/102 (`tests/unit/voice-devices-settings.test.mjs` без записи в §8) и `smoke` 21/22 (`tests/smoke/gui-window.smoke.test.mjs`, exit 1).

| Фаза | Старый артефакт (13:37Z) | Новый артефакт (14:36Z) | Итог |
|---|---|---|---|
| typecheck | ok | ok | без изменений |
| ui-tsc | ok | ok | без изменений |
| meta | **FAIL** 101/102 | ok **102/102** | закрыто: запись `tests/unit/voice-devices-settings.test.mjs` присутствует в `TDD-PLAN.md:379` |
| unit | ok 851/851 | ok **876/876** | +25 тестов, все зелёные |
| spike-rust | ok | ok (status 0) | прогон включает бинарь `tests/ipc_wiring_contract.rs` |
| workspace-rust | ok 1142/1142 | ok **1142/1142** (1154 всего, 12 ignored) | ignored — заявленный live-набор, не e2e-skip |
| smoke | **FAIL** 21/22 | ok **22/22** | закрыто: проба `gui-window` проходит на этой машине |

- `verdict: passed`, `e2e: null` (worker-скоуп), `gitCommit: 4e240481e59ff4d49732ac8bd5ba08a006363748` — совпадает с HEAD из `REPORT.md:6`.
- `waves/RW-40/QUALITY/gate-worker-wave9.log` фиксирует тот же файл отчёта и `Вердикт: passed`, расхождений с JSON нет.

## Свежесть гейта относительно кода

- Гейт стартовал 2026-10-30T14:35:26Z (19:35:26 локально), завершился 14:36:10Z.
- `rust/gui-spike/src/main.rs` — mtime 19:17:08, `rust/gui-spike/tests/ipc_wiring_contract.rs` — 19:12:41, то есть обе правки карточки старше старта гейта.
- Файлов под `rust/`, `tests/`, `scripts/`, `src/`, изменённых после старта гейта: один — `rust/dist/sbom.spdx.json` (сборочный артефакт, не исходник и не тест). Дрейфа исходников после зелёного прогона нет.
- Гейт снят на dirty-дереве (`git.dirty: true`), что для batch-лейна ожидаемо: артефакт подтверждает состояние дерева целиком, а не отдельный коммит.

## MD5-сверка с REPORT.md

| Файл | MD5 на диске | REPORT.md §5 | Итог |
|---|---|---|---|
| `rust/gui-spike/src/main.rs` | `a9b2c9164e121deb3497e5cfac195ad6` | `a9b2c9164e121deb3497e5cfac195ad6` | совпадает |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `2c6efc012048d839b26b95cffdc18edc` | `2c6efc012048d839b26b95cffdc18edc` | совпадает |
| `waves/RW-40/PM/int05-ipc-aliases/red.log` | `dbbb671c86bd24f8a2ccda9555b23c50` | `dbbb671c86bd24f8a2ccda9555b23c50` | совпадает |
| `waves/RW-40/PM/int05-ipc-aliases/green.log` | `d9c7ed611552f6b37e25b54dc148fdb3` | `d9c7ed611552f6b37e25b54dc148fdb3` | совпадает |
| `waves/RW-40/PM/int05-ipc-aliases/mutations.log` | `e751e9b279d82041e912615a7f63ea4a` | `e751e9b279d82041e912615a7f63ea4a` | совпадает |

## Точечная проверка контракта (без запуска Cargo)

- `attachments_upload_slots`, `create_invite`, `pins_list`, `threads_list`: у каждого ровно одно объявление `fn <имя>(` и ровно одна строка в списке `tauri::generate_handler![...]` в `rust/gui-spike/src/main.rs` — дублей и пропусков нет.
- `rust/gui-spike/tests/ipc_wiring_contract.rs` содержит 7 атрибутов `#[test]`, что совпадает с заявленными в `REPORT.md:50` `7 passed`.
- В фазе `spike-rust` гейта присутствует прогон бинаря `tests/ipc_wiring_contract.rs`, exit-статус фазы 0, значит новые контрактные тесты действительно исполнялись в зелёном прогоне.

## Замечания P2 (не блокируют)

1. **Смоук `gui-window` зависит от окружения.** Это live-проба AT-SPI/X11 (`tests/smoke/gui-window.smoke.test.mjs`), на машине без дисплея она уходит в skip с причиной. Зелёный результат подтверждён только для текущего окружения; при переносе приёмки на другую машину фазу нужно перепрогнать.
2. **`main.rs` — общий файл.** Он изменён поверх `INT-01` и `B2b`, поэтому контрольную сумму в сводном `ACCEPT.md` Team Lead синхронизирует отдельно (это уже отмечено в `REPORT.md:89` как `P1-1` из `QA-AUDIT-WAVE8.md` и относится к приёмке волны, а не к этой карточке).
3. **`rust/dist/sbom.spdx.json` попал в dirty-набор гейта** как сборочный артефакт. На вердикт не влияет, но в приёмке стоит держать его вне диффа.

## Границы ревью

Исходники, тесты и логи не изменялись; записан только этот файл. Cargo (build/test/clippy) и гейт не запускались — проверка выполнена по сохранённым артефактам, MD5 и чтению исходников, как требует задание.
