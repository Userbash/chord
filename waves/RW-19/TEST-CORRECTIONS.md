# TEST-CORRECTIONS P2-10 — voice states

Дата: 2026-10-21. Повод: независимое ревью `waves/reviews/review-p2-10.md`, P2-1.
Расширение матрицы не меняет замороженный контракт и не подгоняет ожидания под код: оно
добавляет проверки уже описанных в DESIGN §1/§4.2/§5 веток.

## C4 — аддитивность снимка, last-wins дубликатов, remove-ветка

Добавлены в `tests/voice_state_reducer.rs`:

| ID | Вход | Ожидание | Почему не было раньше |
| --- | --- | --- | --- |
| v09b | live join другого `(guild,user)`, затем реальный `003-READY_SUPPLEMENTAL.json` | в store **2** записи; существующая не потеряна, snapshot-запись добавлена | v09 применял `003` к пустому store, поэтому `clear()` перед циклом проходил |
| v09c | synthetic `READY_SUPPLEMENTAL` с двумя элементами одного `(guild,user)` и разными каналами | побеждает последний канал; `duplicates` пуст; в store 1 запись | не было дубликата, поэтому first-wins проходил |
| v09d | прямой `apply_snapshot` с `channel_id: None` для существующей записи | запись удалена | remove-ветка §5 недостижима через snapshot-парсер (он не создаёт `None`) |

`v09` остаётся проверкой «snapshot → READY reset». Итого reducer-тестов: **8** (было 5),
voice targeted: **18** (types 6 + reducer 8 + selectors 4).

## Oracle

- v09b — реальная фикстура `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json`
  (`md5=55983830547abaf8d1b122798fdcf568`, `s=2`); live join идёт с `s=1`, чтобы снимок
  не был stale.
- v09c/v09d — synthetic-кадры через `tests/support/state_fixture`, повторяющие форму провода;
  ожидания взяты из DESIGN §1/§4.2/§5, а не из реализации.

## Red-first evidence (мутанты)

| Мутант | Файл | Тест | Лог | Результат |
| --- | --- | --- | --- | --- |
| clear-before-loop (`state.voice_states.clear()` в `apply_snapshot`) | `state/voice/reducer.rs` | v09b | `waves/B01/mutations/p2-10-clear-before-loop.log` | exit 101, ожидалось 2, получено 1 |
| first-wins (`insert` только если ключа нет) | `state/voice/reducer.rs` | v09c | `waves/B01/mutations/p2-10-first-wins.log` | exit 101, остался первый канал |

После каждого мутанта файл восстановлен из `/tmp/opencode/p2-10-reducer-backup.rs`;
md5 продукта до/после — `8e3ff3ad7d2016463847101e1336973b`. Продуктовый код не менялся.
Новые тесты на исходном коде зелёные (поведение корректно); красный доказывается мутантами,
как требует ревью.

## Прогоны

- `cargo test --offline -p discord-core --test voice_state_reducer` → 8/8, exit 0
  (`waves/B01/logs/p2-10-v09bcd-green.log`).
- Полный voice targeted 18/18 и batch gate — после расширения, см. отчёт волны.
