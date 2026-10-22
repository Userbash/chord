# B01 — mutation evidence (16/16 caught)

Дата: 2026-10-21. Lane: QA. Скрипт: `/tmp/opencode/b01-mutations.sh` (вне репозитория).
Бэкап: `/tmp/opencode/b01-mutation-backup` (вне git). После каждого мутанта файлы
восстанавливались и md5 сверялся с бэкапом. Cargo — последовательно под
`flock /tmp/opencode/discord-cargo.lock`.

## 1. Первый прогон: 14/16

Первый прогон (до QA-тестов) дал два выживших мутанта:

| ID | Мутация | Итог | Причина |
| --- | --- | --- | --- |
| V3 | `voice_start_time >= 0` → `> 0` | SURVIVED | не было теста на `Some(0)` — валидный epoch |
| V5 | `voice_users_in_channel` без проверки guild | SURVIVED | не было одинакового `channel_id` в двух гильдиях |

Red-first закрытие:

- v10: `Some(0)` → `Some(0)` в store, `elapsed_ms == 1_500` при `now=1_500`;
- v13: второй guild получает тот же `channel_id=10`, проверяется `1/3` и `2/1`,
  а `voice_users_in_channel(1,10)` не содержит чужого пользователя.

Red-логи на мутантах (до восстановления): `mutations/V3-red-after-new-test.log`,
`mutations/V5-red-after-new-test.log`, оба exit 101. После восстановления — targeted 15/15.

## 2. Финальный прогон

| ID | Мутация | Целевые тесты | Итог | Лог |
| --- | --- | --- | --- | --- |
| V1 | `channel_id Some/None` инвертирован | voice 3 файла | CAUGHT (101) | `mutations/V1.log` |
| V2 | upsert member без проверки `user_id` | voice 3 файла | CAUGHT (101) | `mutations/V2.log` |
| V3 | `voice_start_time >= 0` → `> 0` | voice 3 файла | CAUGHT (101) | `mutations/V3.log` |
| V4 | условие непустого status инвертировано | voice 3 файла | CAUGHT (101) | `mutations/V4.log` |
| V5 | users in channel игнорирует guild | voice 3 файла | CAUGHT (101) | `mutations/V5.log` |
| V6 | `saturating_sub` → `-` (underflow) | voice 3 файла | CAUGHT (101) | `mutations/V6.log` |
| V7 | `voice_guild_count` считает записи | voice 3 файла | CAUGHT (101) | `mutations/V7.log` |
| V8 | snapshot очищает store вместо upsert | voice 3 файла | CAUGHT (101) | `mutations/V8.log` |
| T1 | TTL = 0 (истекает сразу) | typing 3 файла | CAUGHT (101) | `mutations/T1.log` |
| T2 | `MESSAGE_CREATE` не удаляет пару | typing 3 файла | CAUGHT (101) | `mutations/T2.log` |
| T3 | `prune_expired` `<=` на границе | typing 3 файла | CAUGHT (101) | `mutations/T3.log` |
| T4 | `is_active` `<=` на границе | typing 3 файла | CAUGHT (101) | `mutations/T4.log` |
| T5 | селектор показывает истёкшие | typing 3 файла | CAUGHT (101) | `mutations/T5.log` |
| T6 | guild-фильтр игнорирует точный id | typing 3 файла | CAUGHT (101) | `mutations/T6.log` |
| T7 | channel_count считает записи | typing 3 файла | CAUGHT (101) | `mutations/T7.log` |
| T8 | upsert typing-member без проверки id | typing 3 файла | CAUGHT (101) | `mutations/T8.log` |

Сводка: `mutations/summary.txt`; общий лог прогона: `/tmp/opencode/b01-mutations-run-2.log`.
Все 16 — exit 101; ни один мутант не остался в рабочем дереве (проверено md5 после каждого
восстановления). P0/P1/P2 по мутациям не выявлено; два начальных выживших закрыты тестами
до приёмки.

## 3. P2-10 review addendum (2026-10-21)

После ревью `waves/reviews/review-p2-10.md` P2-1 добавлены v09b/v09c/v09d и две
семантические мутации, которых не было в первом прогоне:

| ID | Мутация | Целевой тест | Итог | Лог |
| --- | --- | --- | --- | --- |
| V8b | `apply_snapshot` очищает `voice_states` перед циклом | v09b | CAUGHT (101) | `mutations/p2-10-clear-before-loop.log` |
| V8c | `apply_voice_state` вставляет только если ключа ещё нет (first wins) | v09c | CAUGHT (101) | `mutations/p2-10-first-wins.log` |

Оба мутанта восстановлены из `/tmp/opencode/p2-10-reducer-backup.rs`; md5 продукта
`8e3ff3ad7d2016463847101e1336973b` до и после. Итого по B01: **18/18** (16 базовых + 2
семантических snapshot-мутанта).

## 4. Ограничения

- Мутации точечные (8 на карточку, в пределах 5–8), не исчерпывающий mutation score.
- V6 проверяет underflow в debug-профиле; release-поведение не заявляется.
- V8 — грубая мутация snapshot; семантические ветки parser не мутировались, они покрыты
  таблицами v04/v05 и typing t04.
