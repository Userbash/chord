# REVIEW-50A — task-136 / BE-P1-02, FORWARDED_DISPATCHES (F-GW-03)

Дата: 2026-10-30. Ревьюер: Технический ревьюер (DeepSeek), Review 50A.
Ветка: `feature/task-136-net-forwarded-dispatches`, worktree
`/home/user/orca/workspaces/project/feature-task-136-net-forwarded-dispatches`,
HEAD `4e24048`. Режим: чтение кода + целевые тесты + мутационные прогоны.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 3 (follow-up, вне write-scope
этой карточки), P3 = 1.

Срез закрывает F-GW-03 там, где находка его и описывает: `net.rs` больше не
отбрасывает 15 из 21 канонических типов ядра. Продуктовое следствие
(«live-сообщения не доходят до экранов») закрывается только после карточки на
UI-фильтр — см. P2-1.

## Что проверено

- Изменения в `rust/gui-spike/src/net.rs` и `rust/gui-spike/tests/net_contract.rs`
  против HEAD; других правок в коде rust-воркспейса нет.
- `FORWARDED_DISPATCHES` (`net.rs:114-137`): 22 имени = 21 имя `DispatchKind::ALL`
  (`rust/crates/discord-core/src/dispatch.rs:62-84`, файл в worktree не изменён)
  плюс `MESSAGE_REACTION_REMOVE`. Дублей нет, порядок алфавитный, размер
  массива совпадает с числом элементов.
- Фильтр `map_dispatch` (`net.rs:265-268`) не менялся: точное сравнение имени
  провода с константой, регистр значим. Список и фильтр по-прежнему один
  источник истины.
- Секреты, дедуп по `s`, `try_send`, счётчики `DispatchStats` и doc-комментарии
  не изменили поведения; правки в них — только текст. Политика
  `is_secret_key` (`net.rs:189-200`) не ослаблена: новые типы, несущие
  `session_id` (`SESSIONS_REPLACE`), проходят через тот же `redact_secrets`.
- Оракул тестов независим от литерала адаптера: `DispatchKind::ALL` и
  `DispatchKind::from_name` из ядра. Имена подаются op0-кадром через
  `route_dispatch`, а не готовым `DispatchEvent`.
- Ослаблений нет: в `net_contract.rs` нет `.only`, `#[ignore]`, новых skip.
  Две правки старых тестов (`MESSAGE_CREATE` → `MESSAGE_DELETE` в
  `filtered_and_unknown_kinds_do_not_panic_and_are_not_forwarded:571` и
  `non_forwarded_kind_is_counted_as_filtered:892`) вынуждены сменой семантики:
  `MESSAGE_CREATE` теперь форвардится, и старый вход перестал быть
  «отфильтрованным». Проверяемое свойство сохранено, assertions не удалены.

## Прогоны

Команда:

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline \
  --manifest-path rust/gui-spike/Cargo.toml --test net_contract --test ipc_contract
```

- `net_contract`: **27 passed, 0 failed**.
- `ipc_contract`: **20 passed, 0 failed**.
- `cargo clippy --offline --all-targets -- -D warnings`: exit 0.
- `cargo fmt -- --check`: exit 0.
- MD5 файлов совпадают с REPORT.md: `net.rs`
  `3e463b632d9e177392d10046b8b81d24`, `net_contract.rs`
  `9ff8127759546e4404cff7096306796a`.
- Red-лог `red-net-contract.log` соответствует заявленному: 24 passed,
  3 failed, падают ровно те три теста, что перечислены в отчёте.

## Мутации (5/5 пойманы)

Каждая мутация применялась к `net.rs`, прогонялся только `--test net_contract`,
затем файл восстанавливался. После всех прогонов MD5 `net.rs` равен исходному
`3e463b632d9e177392d10046b8b81d24`.

| # | Мутация | Результат | Поймавшие тесты |
|---|---|---|---|
| M1 | удалить `MESSAGE_CREATE` из списка (22→21) | FAILED, 24 passed / 3 failed, exit 101 | `forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove`, `every_core_dispatch_kind_reaches_the_kind_data_envelope`, `all_core_kinds_reach_the_ui_channel_in_order` |
| M2 | удалить `MESSAGE_REACTION_REMOVE` (22→21) | FAILED, 25 / 2, exit 101 | `reaction_kinds_use_wire_name_and_forward_data_as_is`, `dispatch_reaches_ui_channel_in_order_without_changing_status` |
| M3 | фильтр: точное совпадение → `starts_with` | FAILED, 26 / 1, exit 101 | `wire_names_outside_the_core_router_are_filtered` |
| M4 | фильтр: точное совпадение → `eq_ignore_ascii_case` | FAILED, 26 / 1, exit 101 | `wire_names_outside_the_core_router_are_filtered` |
| M5 | добавить raw `MESSAGE_DELETE` в список (22→23) | FAILED, 23 / 4, exit 101 | `forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove`, `wire_names_outside_the_core_router_are_filtered`, `filtered_and_unknown_kinds_do_not_panic_and_are_not_forwarded`, `non_forwarded_kind_is_counted_as_filtered` |

M1 — прямое требование задания: удаление события из `FORWARDED_DISPATCHES`
ловится. M3/M4 — поломка фильтра (префикс, регистр) ловится. M2 и M5
показывают, что список не может разъехаться с контрактом ни в минус по
raw-имени, ни в плюс по неразбираемому ядром имени.

## Findings

**P0:** нет.

**P1:** нет.

**P2-1. UI-фильтр `FORWARDED_KINDS` всё ещё из 6 имён.**
`rust/gui-spike/ui/features/dispatch-model.js:19-26` содержит старые шесть
имён; новые события дойдут до Tauri-канала, но модель UI их отбросит.
Заявлено в REPORT §5 п.3 как отдельная карточка. Блокирует не мерж этого
среза, а закрытие F-GW-03 на продуктовом уровне: до правки UI находку нельзя
помечать «закрыта» по следствию «сообщения не доходят до экранов».

**P2-2. `CONTRACTS/dispatch.md` §4 противоречит коду.**
Строка 64 и таблица ниже описывают 6 имён и прямо утверждают, что
`MESSAGE_CREATE`/`MESSAGE_UPDATE`/`TYPING_START` «не форвардятся». После мержа
документ будет врать. Файл вне write-scope воркера (Team Lead), заявлено в
REPORT §5 п.4. Правку нужно внести в том же batch, до публикации среза.

**P2-3. Восемь имён провода остаются отфильтрованными.**
`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE/UPDATE/DELETE`,
`THREAD_CREATE/UPDATE/DELETE` ядро отдаёт raw (`kind: None`), в
`DispatchKind::ALL` их нет. Карточка и F-GW-03 требуют ровно 21 тип, поэтому
срез формально верен, а тест
`wire_names_outside_the_core_router_are_filtered` фиксирует границу явно.
Но F-GW-03 в тексте называет удаления среди потерянных сценариев — нужна
отдельная карточка на расширение core-роутера, иначе «удаления не доходят»
останется открытым.

**P3-1. Незадекларированная правка `package-lock.json`.**
В worktree `package-lock.json` отличается от HEAD на 6 строк (добавлены `bin`
и `engines`), в отчёте файл не указан, в основной чекаут правка не попала.
Похоже на побочный `npm install`: lockfile догоняет `package.json`, вреда нет.
Перед интеграцией либо исключить файл из коммита, либо задекларировать
отдельной строкой.

## Границы доказательств

Проверен backend-срез: `net.rs`, конверт `{kind, data}`, redaction, доставка в
UI-канал на fake-эмиттере, статистика. Не проверялись: реальный Tauri-канал,
модель/экраны UI (там свой фильтр, P2-1), живой Discord и полный
`gate:worker` — по процессу его делает integration/QA на batch. Тест
`all_core_kinds_reach_the_ui_channel_in_order` ждёт через `Condvar` с дедлайном
5 с и без sleep-гонок; прогон стабилен, но это один прогон, не серия.
