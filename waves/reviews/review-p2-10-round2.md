# Независимое ревью RW-19 / P2-10 — round 2 (закрытие P2-1)

Дата: **2026-10-21**. Ревьюер: **REV-P2-10-R2**, независимый (не автор карточки).
Режим: **read-only** по продукту, тестам, докам и golden; запись только в
`waves/reviews/review-p2-10-round2.md` и `waves/reviews/p2-10-round2/` (логи прогонов).
Cargo — только последовательно под `flock /tmp/opencode/discord-cargo.lock`. Полный
workspace/gate повторно не перезапускались; targeted/fmt/clippy перезапущены на финальном
дереве. Контракт: `waves/RW-19/DESIGN-P2-10.md` §1/§4.2/§5, `TEST-MATRIX-P2-10.md`,
`TEST-CORRECTIONS.md` C4. Предыдущее ревью: `waves/reviews/review-p2-10.md`.

## Вердикт — **accepted**

**P0 = 0, P1 = 0, P2 = 0, P3 = 11** (P3-1…P3-8 перенесены из предыдущего ревью,
P3-9…P3-11 новые; P3-6 расширен пунктом про устаревший md5 теста). P3 не блокируют.

P2-1 закрыт: аддитивность `READY_SUPPLEMENTAL`, last-wins дубликатов `(guild,user)` и
remove-ветка `channel_id: None` теперь проверяются тестами; оба мутанта
(clear-before-loop, first-wins) дают exit 101 и **независимо воспроизведены** на финальном
дереве в изолированной копии. Продуктовый код не менялся (md5 совпадает с `w19-report.md`
§5 и `INTEGRATION.md` §6); новых P0/P1/P2 нет.

## 1. Проверки

| # | Проверка | Команда / артефакт | Результат |
|---|---|---|---|
| 1 | md5 voice-продукта | `md5sum rust/crates/discord-core/src/state/voice/*.rs` → `p2-10-round2/md5-product.txt` | все 5 совпадают с `w19-report.md` §5 и `INTEGRATION.md` §6; reducer `8e3ff3ad7d2016463847101e1336973b` |
| 2 | md5 shared-файлов | `md5sum src/state/{mod,types,reducer}.rs src/lib.rs tests/state_{types,reducer}.rs` | совпадает с `INTEGRATION.md` §6 (6/6) |
| 3 | fixture `003` | `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` | md5 `55983830547abaf8d1b122798fdcf568`; `s=2`, 1 voice-запись, значения прежние |
| 4 | v09b — аддитивность | `tests/voice_state_reducer.rs:208-230` | live join другого `(guild,user)` (`s=1`), затем реальный `003` (`s=2`); ждёт 2 записи и оба исходных канала; проверяет именно merge, не replace |
| 5 | v09c — last wins | `tests/voice_state_reducer.rs:232-249` | два элемента одного `(guild,user)` в synthetic `READY_SUPPLEMENTAL`; ждёт последний канал/`connected_at=200`, 1 запись, пустые `duplicates`; парсер порядок сохраняет, дедупликации нет (`parser.rs:48-49,127-133`) |
| 6 | v09d — remove | `tests/voice_state_reducer.rs:251-280` | прямой `apply_snapshot` с `channel_id: None` удаляет существующую запись; ветка §5, недостижимая через парсер |
| 7 | targeted на финальном дереве | `cargo test --offline -p discord-core --test voice_state_types --test voice_state_reducer --test voice_state_selectors` → `p2-10-round2/targeted-rerun.log` | **exit 0**, 8+4+6 = **18/18**; тот же test-binary hash, что в сохранённых логах |
| 8 | fmt | `cargo fmt --all -- --check` → `p2-10-round2/fmt-check.log` | **exit 0**, пустой вывод |
| 9 | clippy | `cargo clippy --offline -p discord-core --all-targets -- -D warnings` → `p2-10-round2/clippy-core.log` | **exit 0** |
| 10 | workspace-лог | `waves/B01/logs/p2-10-workspace-final.log` → `p2-10-round2/workspace-gate-audit.txt` | 68 `test result: ok`, сумма **548**, `FAILED` = 0; reducer 8, selectors 4, types 6; mtime 01:31:05 (после финального fmt) |
| 11 | gate | `waves/B01/logs/p2-10-gate-worker.log`, `.gate/worker-2026-10-20T20-31-37-021Z.json` | `verdict: passed`; meta 25/25, unit 169/169, smoke 20/20. Скопированный `waves/B01/gate-worker.json` устарел — P3-9 |
| 12 | мутант clear-before-loop | изолированная копия, `cargo test … voice_state_reducer v09b_snapshot` → `p2-10-round2/mutation-clear-before-loop-repro.log` | **exit 101**, `left: 1 right: 2` на `voice_state_count()`; совпадает с сохранённым логом |
| 13 | мутант first-wins | изолированная копия, `cargo test … voice_state_reducer v09c_snapshot` → `p2-10-round2/mutation-first-wins-repro.log` | **exit 101**, `left: Some(1176785373989445703) right: Some(…704)`; совпадает с сохранённым логом |
| 14 | восстановление продукта | `md5sum /tmp/opencode/p2-10-reducer-backup.rs rust/…/reducer.rs` | идентичны, `8e3ff3ad7d2016463847101e1336973b`; после моего повтора md5 не изменился (before/after в `p2-10-round2/`) |
| 15 | матрица/коррекции | `TEST-CORRECTIONS.md` C4, `TEST-MATRIX-P2-10.md:23-34` | C4 и матрица совпадают с тестами: v09b/v09c/v09d, reducer 8, voice targeted 18; ожидания выведены из DESIGN §1/§4.2/§5, не из реализации |
| 16 | golden/секреты/сеть | `git status --porcelain -- logs functions evidence`; `grep` по `state/voice` | golden пуст; новых зависимостей/сети/паник в продукте нет; `Debug` не печатает member payload |

### Проверка тестов на «подгонку»

- v09b начинает с **непустого** store (live join `OTHER_GUILD/OTHER_USER`), затем применяет
  реальный `003`; проверяет, что старая запись сохранилась, а snapshot-запись добавилась.
  При `clear()` перед циклом остаётся 1 запись — тест падает (воспроизведено).
- v09c использует **два** элемента одного ключа и проверяет именно последний канал и
  `connected_at`; при first-wins остаётся первый канал — тест падает (воспроизведено).
- v09d вызывает публичный `apply_snapshot` напрямую; при игнорировании `None` запись
  остаётся — тест падает.
- Все три теста используют публичный `State`/reducer API, реальную фикстуру или форму
  провода из `tests/support/state_fixture`; продуктовые файлы и фикстура не менялись.
  Старые v07…v09 сохранены; сдвиг их строк объясняется добавленными helper’ами
  (`snapshot_voice`, `supplemental_frame`) перед тестами, а не ослаблением ассертов.

### Red-first evidence

Сохранённые `waves/B01/mutations/p2-10-*.log` и мой повтор совпадают по семантике: те же
тесты, те же значения `left/right`, exit 101. Отличие только в номерах строк
(`228/250` в сохранённых логах против `221/245` сейчас) — тестовый файл был переформатирован
`rustfmt` после снятия мутантов; после нормализации путь/PID/время/номера строк diff пуст
(`p2-10-round2/mutation-repro-summary.txt`). Это traceability-нит (P3-10), не дефект
red-first: на финальном дереве оба мутанта снова падают.

## 2. P0/P1/P2

P0 и P1 не найдены. Новых P2 нет.

Единственный ранее блокировавший P2-1 закрыт в объёме условий
`review-p2-10.md` §6 (остаточная ниша same-guild — P3-11):

1. **Аддитивность** — v09b, red на clear-before-loop (exit 101, 1 vs 2).
2. **Last-wins дубликатов** — v09c, red на first-wins (exit 101, первый канал).
3. **Remove-ветка** — v09d, прямой вызов с `channel_id: None`.
4. **Матрица** — расширение зафиксировано `TEST-CORRECTIONS.md` C4; строки v09b/v09c/v09d
   и счётчики 8/18 совпадают с тестами.
5. **Продукт** — md5 не изменился; восстановление после мутантов подтверждено.

Замечания, которые могли бы выглядеть как P2, но не являются ими:

- **Номера строк в mutation-логах** отличаются от текущего файла после `rustfmt`. Повтор на
  финальном дереве даёт те же падения и те же значения; логи не подделаны и не устарели
  семантически. P3-10.
- **`waves/B01/gate-worker.json`** — копия гейта 19:58Z, а финальный гейт 20:31Z лежит в
  `.gate/` и в `p2-10-gate-worker.log` (оба `passed`). Артефакт-копия не обновлена; на
  вердикт не влияет. P3-9.
- **Отчёт `w19-report.md`** в шапке и §5 всё ещё говорит 15/15 и старый md5 reducer-теста
  `66b82ef2…`; addendum внизу описывает 18/18 и новые тесты. Это доковая несинхронность
  (P3-6), не код.

## 3. P3 backlog (не блокирует `accepted`)

| ID | Замечание | Статус |
|---|---|---|
| P3-1 | Ветки ошибок `parse_voice_states` (не-list, битый элемент, fallback/несогласованный `guild_id`, `channel_id:nil`) не покрыты табличным тестом | перенесено, не блокирует |
| P3-2 | V2 по-прежнему инвертирует guard `member_user.id == user_id`, а не удаляет его; настоящий мутант «upsert без сравнения» не проверен | перенесено |
| P3-3 | v03 ставит `self_mute=false`, хотя в capture s=51 `self_mute=true`; матрица говорит «флаги как в capture» | перенесено |
| P3-4 | Набор мутаций не покрывает M3/M4/M5/M8 из DESIGN §9 (`discoverable` default, `connected_at` без ×1000, timer nil, stale) | перенесено |
| P3-5 | Nil optional bool даёт default без `ignored`, хотя §4.1 буквально говорит «иначе default + ignored» | перенесено |
| P3-6 | Неточности `w19-report.md`: «37 ошибок» вместо 64 (6+21+37); `apply_snapshot` не мержит `member_user`; stale-логика в shared reducer. Дополнительно: шапка «15/15», §5 и `INTEGRATION.md` §6 — старый md5 `66b82ef2…` для reducer-теста (текущий `9d019883…`) | перенесено + расширено |
| P3-7 | Матрица v01 обещает «diagnostics пусты», тест проверяет только voice-диагностику (в `003` есть не voice-дубль `merged_members`) | перенесено |
| P3-8 | Behavior-red лог отсутствует; red-first подтверждён compile-red и mutation-red | перенесено |
| P3-9 | `waves/B01/gate-worker.json` — копия гейта 19:58Z; финальный гейт 20:31Z только в `.gate/worker-2026-10-20T20-31-37-021Z.json` и `p2-10-gate-worker.log`. Обновить копию перед журналом волны | новое, процесс |
| P3-10 | В `waves/B01/mutations/p2-10-*.log` номера строк panics (228/250) относятся к pre-rustfmt версии теста; текущий файл даёт 221/245. Логи воспроизводимы, но для аудита стоит сохранить post-fmt повтор или пометку в `MUTATIONS.md` | новое, traceability |
| P3-11 | v09b проверяет аддитивность на **другой** гильдии. Запись той же гильдии, отсутствующая в снимке, отдельно не проверена; мутант «очистить только гильдии, присутствующие в снимке» пройдёт v09b. Полный clear и first-wins закрыты, контракт §5 прямо запрещает очистку карты; это остаточная ниша для следующего batch | новое, test-strength |

## 4. Ограничения ревью

- Сохранённый `p2-10-targeted-final.log` снят в 01:29:46, до финального fmt в 01:30:28;
  поэтому targeted перезапущен на финальном дереве (18/18, см. §1). Workspace-лог 548 снят
  уже после fmt (01:31:05) и содержит reducer 8/8.
- Полные `cargo test --workspace` и `gate:worker` повторно не запускались (режим ревью);
  проверены сохранённые логи, суммы и `verdict`. Targeted/fmt/clippy перезапущены на
  финальном дереве.
- Мутанты повторялись не в рабочем дереве, а в изолированной копии
  `/tmp/opencode/p2-10-round2/rust` (crates + Cargo.toml/lock, фикстуры скопированы);
  продуктовые файлы репозитория не менялись. После повтора md5 reducer сверен с backup.
- Live Discord, SQLite/UI/keyring, сеть и запись в golden не запускались. Секреты отдельным
  сканером не проверялись; в voice-коде логирования `session_id`/member payload нет.
- P3-1…P3-8 остаются backlog’ом предыдущего ревью; их закрытие не требуется для `accepted`.

## 5. Итог

**accepted.** P2-1 закрыт тестами и воспроизводимыми mutation-red логами; продукт не изменён;
новых P0/P1/P2 нет. Перед записью в журнал волны желательно (не обязательно) обновить
`waves/B01/gate-worker.json` и md5-таблицы отчёта — это P3-9/P3-6.
