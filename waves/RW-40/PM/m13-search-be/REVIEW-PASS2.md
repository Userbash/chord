# REVIEW PASS 2 — RW-40 / M-13-SEARCH-202-NSFW-BACKEND

- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` (ветка `rw40/m13-search-202-be`)
- **Режим:** независимое ревью, read-only по коду и golden-каталогам; тесты и `rustfmt` прогнаны заново.
- **Объект:** `rust/gui-spike/src/search.rs`, `rust/gui-spike/tests/search_contract.rs`, отчёт `waves/RW-40/PM/m13-search-be/REPORT.md`.
- **Оракул:** `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` §3.1 (строки 436–458), §3.2, `waves/RW-40/SEARCH-202-NSFW-RE.md` §5.1–§5.4, §6.
- **Вердикт:** **PASS** — P0=0, P1=0, P2=2 (оба межкарточные/остаточные, не блокируют приёмку этой карточки).

## 1. Закрытие P1-1 и P2-1…P2-5

| Пункт | Статус | Доказательство |
|---|---|---|
| P1-1 | **закрыт** | `execute()` на `202` сразу возвращает `SearchResponse { indexing: true, retry_after_ms: Some(..), documents_indexed: 0, messages: [] }` (`src/search.rs:493-503`), тело не парсится и не логируется. IPC-путь `search_remote` → `RemoteSearch::search` → `execute` (`src/main.rs:229-241`, `src/search.rs:511-519`) не спит и не повторяет. Это Вариант B: конверт в Rust, поллинг в UI. |
| P2-1 | **закрыт** | `documents_indexed: u64` с `#[serde(default, rename = "documents_indexed")]` (`src/search.rs:179-181`); тест `decodes_documents_indexed_from_200_ok_and_defaults_to_zero_when_omitted` (`tests/search_contract.rs:758-796`) проверяет `590` и `0` при отсутствии поля. |
| P2-2 | **закрыт** | `attempts: Option<u32>` (`src/search.rs:66`) с десериализацией из корня и `query` (`:165`), валидацией `> 5 → InvalidArg` до транспорта (`:409-414`) и `attempts={n}` в URL (`:484-486`). `execute_with_retry` ставит `attempts = 1..5` на повторах (`:359-387`): 6 запросов, `attempts=1..5` со второго по шестой. Проверено точными URL в тестах (`:663-672`, `:696-706`, `:739-749`) и по проводу локального TCP-мока (`:1043-1056`). |
| P2-3 | **закрыт для IPC-пути** | Цикла в `execute` больше нет, 25-секундной блокировки на команде нет. Остаточный риск — в §4. |
| P2-4 | **закрыт** | `m13_constants_and_indexing_error_format_match_spec` сверяет полную подстроку `"после 5 попыток; повтор через 2000 мс"` (`src/search.rs:604-616`), а не отдельные цифры. |
| P2-5 | **закрыт** | `skip_serializing_if` с `include_nsfw` убран, вместо него doc-комментарий о ручной сборке query (`src/search.rs:61-63`); URL добавляет параметр только при `Some(true)` (`:481-483`), `Some(false)`/`None` опускаются. |

Дополнительно сверил семантику повторов: `Retry-After` читается только из заголовка в целых секундах, `*1000`, fallback `5000` (`:521-529`); на шестом `202` возвращается `Indexing { retry_after_ms, attempts: 5 }` с задержкой из последнего заголовка, лишнего `sleep` нет. Заголовок `Retry-After`, тело `202`, `documents_indexed` из `202` не смешиваются с телом `200`. Утечки токена в новых ветках нет: `Indexing` несёт только числа.

## 2. md5 против `REPORT.md`

Все пять значений совпали:

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/search.rs` | `3ec29ae3219be411465ce3b2cc749287` |
| `rust/gui-spike/tests/search_contract.rs` | `f113b75de5727b82fe1ca5dc0f57eb3b` |
| `waves/RW-40/PM/m13-search-be/red.log` | `e9e16189cb373e7460ffa3950aa18152` |
| `waves/RW-40/PM/m13-search-be/green.log` | `334a7fd4ace05320700309cac8035ca8` |
| `waves/RW-40/PM/m13-search-be/mutations.log` | `98cca7b2ff60094cf2cf172dfdc796d3` |

## 3. Прогоны (мои, 2026-10-30)

- `cargo test --offline -p discord-gui-spike --test search_contract` → `ok. 23 passed; 0 failed`, 1.00 с.
- `cargo test --offline -p discord-gui-spike --lib search` → `ok. 2 passed; 8 filtered out`.
- `cargo test --offline -p discord-gui-spike` целиком → все наборы `ok`, регрессий в других модулях нет.
- `node --test tests/meta/test-policy.test.mjs` → `tests 17 / pass 17 / fail 0`, `exit=0`.
- `rustfmt --edition 2021 --check src/search.rs tests/search_contract.rs` → `exit=0`.
- `red.log` согласован с отчётом: Pass 1 — 22 ошибки компиляции, `exit=101`; Pass 2 — `17 passed; 6 failed`.
- `mutations.log` — 5/5 `KILLED`, у каждого мутанта падают заявленные тесты, после каждого `restored … (match=True)` с baseline md5. `search_contract.rs` в diff: `812` добавленных строк, `1` удалённая (расширение списка импортов), ослабления проверок нет; `.only`/`#[ignore]`/`skip` отсутствуют.

## 4. Остаточные P2 (не блокируют)

### P2-A. `execute_with_retry` не вызывается из продакшена

Метод публичный и покрыт тестами, но по всему `rust/` у него нет ни одного вызова, кроме `tests/search_contract.rs`: IPC-команда идёт через `search()`/`execute()`. То есть retry-цикл и `SearchError::Indexing` сейчас живут только в тестах. Риск — дрейф между двумя реализациями поллинга и соблазн позже подключить метод к IPC: тогда команда снова начнёт блокироваться до 25 с, а отмены (`cancel`) в цикле нет, что противоречит §3.1. Решение: либо оставить как явно тестовый хелпер с комментарием, либо удалить до появления реального потребителя; в отчёте это стоит назвать прямо.

### P2-B. Замороженный UI не умеет `indexing`-конверт

`rust/gui-spike/ui/features/search-overlay.js:1016-1051` читает только `messages`/`total_results`/`doing_deep_historical_index`, полей `indexing`/`retry_after_ms` не знает. На `202` Rust теперь отдаёт успешный пустой ответ, и оверлей отрисует пустую серверную выдачу с `remoteExhausted = true` — без баннера «Индексация», без «Попытка N из 5» и без повтора. §3.2–§3.3 спеки в UI пока не реализованы ни одной карточкой. Контракт бэкенда при этом соответствует §3.1, поэтому блокировать эту карточку не за что, но UI-карточку на баннер индексации нужно поставить в очередь до того, как состояние `202` станет видимым пользователю.

## 5. Итог

Все пункты исходного ревью закрыты, отчёт точен, артефакты и md5 сходятся, тесты сильные и не ослаблены. Ветка готова к приёмке по backend-части M-13; два остаточных P2 выше относятся к соседним карточкам и к судьбе тестового хелпера, а не к корректности IPC-контракта.
