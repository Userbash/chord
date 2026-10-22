# RW-18 / P2-09 — mutation evidence (13/13 caught)

Все мутации вносились в рабочее дерево, затем исходник восстанавливался из
`/tmp/opencode/rw18-mutation-backup` (вне git). После восстановления файлы обновлены
(`touch`) и целевые тесты перезапущены: 54/54. Логи — `waves/RW-18/mutations/`.

## 1. Восстановление

| Файл | md5 до мутаций | md5 после restore |
|---|---|---|
| `rust/crates/discord-core/src/state/presence.rs` | `8f2ee74346d9dfbf6da417d3ebba0d93` | `8f2ee74346d9dfbf6da417d3ebba0d93` |
| `rust/crates/discord-core/src/state/presence_reducer.rs` | `52b8c6352e7ddcde7f932bf08b1d8996` | `52b8c6352e7ddcde7f932bf08b1d8996` |
| `rust/crates/discord-core/src/state/presence_selectors.rs` | `dab270cb7135cf347754fc02ef9fd557` | `dab270cb7135cf347754fc02ef9fd557` |
| `rust/crates/discord-core/src/state/member_list.rs` | `b06ea1fbd9560c7053cc3ace705d9e5a` | `b06ea1fbd9560c7053cc3ace705d9e5a` |
| `rust/crates/discord-core/src/state/member_chunk.rs` | `a033fbc27523881b1085806600400576` | `a033fbc27523881b1085806600400576` |
| `rust/crates/discord-core/src/state/reducer.rs` | `786dc1034f9d86a48a57b42b906ca7be` | `786dc1034f9d86a48a57b42b906ca7be` |

Полный манифест — `MD5SUMS.txt` (15 файлов: 9 продуктовых + 6 тестовых).

## 2. Результаты

Команда каждого прогона:
`cd rust && cargo test --offline -p discord-core --test presence_types --test presence_reducer --test presence_selectors`

| ID | Мутация | Поймавший тест | Итог | Лог |
|---|---|---|---|---|
| M1 | `max_by` → `min_by` в выборе scope | `s02` | CAUGHT (101) | `mutations/M1.log` |
| M2 | отключена очистка all-offline | `r07` | CAUGHT (101) | `mutations/M2.log` |
| M3 | приоритет всех activities = 0 | `s06` | CAUGHT (101) | `mutations/M3.log` |
| M4 | member-list presence не применяется | `r17` | CAUGHT (101) | `mutations/M4.log` |
| M5 | chunk scope не получает guild по умолчанию | `r13` | CAUGHT (101) | `mutations/M5.log` |
| M6 | offline+hidden больше не видим | `r09` | CAUGHT (101) | `mutations/M6.log` |
| M7 | несколько playing не сворачиваются | `s07` | CAUGHT (101) | `mutations/M7.log` |
| M8 | `is_vr_online` всегда false | `s16` | CAUGHT (101) | `mutations/M8.log` |
| M9 | убран desktop/vr guard в `is_mobile_online` | `s16` | CAUGHT (101) | `mutations/M9.log` |
| M10 | убран displayable tie-break | `s17` | CAUGHT (101) | `mutations/M10.log` |
| M11 | stale guard пропускает `seq == last_seq` | `r15` | CAUGHT (101) | `mutations/M11.log` |
| M12 | `guild_id = 0` принимается | `a18` | CAUGHT (101) | `mutations/M12.log` |
| M13 | пустой member-list `status` принимается молча | `r18` | CAUGHT (101) | `mutations/M13.log` |

Сводка: `mutations/summary.txt`. Все exit-коды — `101` (тестовое падение), не
compile-error; в каждом логе есть `test ... FAILED` соответствующего теста.

## 3. Что добавлено после ревью

M8–M13 закрывают мутационные пробелы P3-4…P3-6, P3-10, P3-12 из
`waves/reviews/review-p2-09.md`: VR/mobile guards, displayable tie-break, stale payload,
`guild_id=0` и пустой member-status. M1–M7 — исходный план.
