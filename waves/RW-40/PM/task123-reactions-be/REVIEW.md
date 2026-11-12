# REVIEW — RW-40 / task-123-reactions-extended-be

- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` (ветка `feature/task-123-reactions-extended-be`)
- **Ревью:** Review 45A, независимое. Продуктовый код и тесты не правились; временный пробник
  `tests/tmp_review_probe.rs` удалён после прогона, рабочее дерево вернулось к baseline-md5.
- **Объект:** `rust/gui-spike/src/reactions.rs` (+331/−21), `rust/gui-spike/tests/reactions_contract.rs`
  (+494/−1), артефакты `waves/RW-40/PM/task123-reactions-be/{REPORT,red,green,mutations}`
- **Оракулы:** `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md` (§1, §3.2.4–§3.2.5, §6.1–§6.2,
  `SPEC:583`, `SPEC:795-798`, `SPEC:827-834`), `REACTIONS-EXTENDED-SPEC-REVIEW.md` (§4.2 п.1, п.5,
  P1-5, P1-6), `rust/gui-spike/src/rest_failure.rs`, `rust/crates/discord-core/src/rest/{client,rate_limit}.rs`
- **Вердикт:** **PASS** — P0 = 0, P1 = 0, P2 = 6. Карточка реализована, мутации 5/5 убиты
  независимым прогоном, дефектов, ломающих IPC/REST-поведение, нет. Замечания — про синхронизацию
  спеки (429-дефолт, «сырой» 429), два непокрытых правила валидации и неоднозначную семантику
  `active` в новых публичных хелперах.

## 1. Чек-лист ТЗ

| # | Требование | Итог | Где |
|---|---|---|---|
| 1 | Percent-encoding Unicode (`👍`, `👋`, `❤️`) без plain-unicode в пути | PASS | `src:314-337`, `src:446-459`; тест `reactions_contract.rs:220-280` |
| 2 | Кастомные `name:id`, `a:name:id`, обёртки `<:name:id>` / `<a:name:id>`, `:` → `%3A` | PASS | `src:400-444`; тест `:230-248` (точные строки `party%3A…`, `a%3Aparty_blob%3A…`) |
| 3 | Маршрут `/channels/{c}/messages/{m}/reactions/{emoji}/@me` | PASS | `src:321-337`; точные `target` в тесте `:396-418` |
| 4 | `PUT` c телом `b"{}"` при `active=true`, `DELETE` с пустым телом при `active=false` | PASS | `src:208-224`; тест `:358-419` (`RecordedHttpCall` сверяет тело побайтно) |
| 5 | `200`/`204` → `Ok(())`, без чтения тела | PASS | `src:461-467`; тест `:358-419` (ответы `204` и `200`) |
| 6 | `401`/`403`/`404`/`5xx`/`Network` типизированы, токен не утекает | PASS | `src:60-76`, `rest_failure.rs:51-101`; тесты `:421-551`, `:121-129` |
| 7 | `429` → `RateLimited { retry_after_ms }` из JSON-тела и `Retry-After`/`x-ratelimit-reset-after`, минимум `1` мс | PASS (см. P2-1, P2-3) | `src:339-378`; тест `:421-500` (1.5→1500, 0.25→250, header 2.5→2500, `0.0004`→1) |
| 8 | Идемпотентные хелперы: `active=false` → PUT и `Ok(true)`, `active=true` → DELETE и `Ok(false)` | PASS | `src:267-311`; тест `:554-629` (методы вызовов 0..4) |
| 9 | Предсетевая валидация: пустые/нечисловые ID, `emoji` пустой/`>256`, control, `/`, `\`, `.`/`..`, битые `name:id` | PASS | `src:387-444`; тест `:282-356` + `transport.calls().is_empty()` |
| 10 | Обратная совместимость `ReactionApi` / `toggle_reaction` / IPC `main.rs` | PASS | `src:242-265`; `main.rs:393-408` не менялся (`send_reaction` → `toggle_reaction`), семантика `active` = целевое состояние сохранена |
| 11 | `cargo test --test reactions_contract`, `--lib reactions`, `clippy`, `rustfmt` | PASS | 11/11, 4/4, clippy scoped 0, `rustfmt --check` exit 0 (см. §4); `clippy --all-targets` красный по чужим файлам — P2-6 |
| 12 | Мутационный порог 5/5 | PASS | независимо убиты 5/5, md5 M1/M5 совпали с отчётом (см. §4) |

## 2. P1 — нет

Ни одного дефекта, который ломает REST-маршрут, кодирование, IPC-контракт или наблюдаемое
поведение UI, не найдено. Red-first, отсутствие ослаблений тестов и защита от утечки токена
проверены отдельно (§4).

## 3. P2 — исправить, приёмку не блокируют

### P2-1. `retry_after` при отсутствии поля: спека `1000` мс, код `0`

`SPEC:583` требует «дефолт при отсутствии поля `1000` мс». `parse_retry_after_ms`
(`src:339-364`) возвращает `0`, если ни JSON, ни заголовки не дали положительного значения.
Мой пробник подтвердил: `parse_retry_after_ms(429, no headers, b"{}") == 0` и
`{"message":"rate limited"}` → `0`.

Код при этом следует фактическому контракту остальных модулей: `rest_failure.rs:20-24`
(«задержку не выдумываем», контракт G-03a), `invites.rs:700`, `notifications.rs:1203`,
`pins_threads.rs:408` — там тоже `0`. То есть расходится не реализация с проектом, а текст
спеки с проектом. UI-эффекта нет: `remainingSec = Math.max(1, ceil(0/1000)) = 1` совпадает с
`1000` мс. Нужно решение: либо зафиксировать `0` + floor в `DECISIONS`/спеке (как уже просит
`REACTIONS-EXTENDED-SPEC-REVIEW.md` P1-6), либо поставить `1000` и добавить тест на отсутствующее
поле. Сейчас тест `:523-534` пинит `0` для невалидных значений, но случая «поля нет» в тестах
нет вообще, поэтому правило ничем не закреплено.

### P2-2. Тесты не покрывают `trim()` и пробелы внутри эмодзи

Два независимых мутанта выжили на финальном наборе (11/11 green):

- `R45A-X1`: `let trimmed = emoji.trim();` → `let trimmed = emoji;` — SURVIVED;
- `R45A-X2`: отключение ветки `candidate.chars().any(char::is_whitespace)` (`src:438`) — SURVIVED.

`SPEC:797-798` прямо требует `emoji.trim()` до проверки длины и кодирования. Пробник
подтвердил, что `build_reaction_path(..., " 👍 ")` в текущем коде даёт корректный
`.../reactions/%F0%9F%91%8D/@me`. Добавить в `strict_validation...` два случая:
`" 👍 "` → `Ok(/%F0%9F%91%8D/)` (trim работает) и `"👎 👍"` → `Err(InvalidArg)` (внутренний
пробел). Иначе правило держится только на чтении кода.

### P2-3. Разбор «сырого» 429 в `reactions.rs` недостижим в проде и расходится с общей политикой

`decode_response` (`src:461-466`) разбирает тело/заголовки 429, но боевой транспорт
`ApiReactionTransport` (`src:107-134`) получает от `ApiClient` уже классифицированную ошибку:
`client.rs:400-427` при 429 всегда повторяет запрос (`rate_limit.rs:232`: `RateLimited → true`),
после исчерпания возвращает `ApiError::RateLimited { retry_after, attempts }`, а
`RestFailure::from_api_error` (`rest_failure.rs:92-96`) отдаёт реальные миллисекунды. Значит
ветка `decode_response(429)` исполняется только через `MockReactionTransport`, то есть
«разбор `Retry-After`/тела» в реакциях — синтетика, а живой путь идёт через ядро.

Дополнительно: `rest_failure.rs:20-24` утверждает, что «эти модули не разбирают» тело/заголовки
сырого 429, а `reactions.rs` теперь разбирает — две политики на один статус. И приоритет
источников обратный принятому в `invites.rs:700` / `notifications.rs:1203` (там заголовок раньше
тела, здесь JSON раньше заголовка; при разных значениях результат разный — мой пробник:
`{"retry_after":1.5}` + `Retry-After: 5` → `1500`).

Ничего не ломается, но заявленный в отчёте пункт «разбор 429 из JSON-тела и заголовков» следует
описать как fallback для мок-транспорта либо переиспользовать `rate_limit.rs:143-175` вместо
третьей копии. Заодно синхронизировать шапку `rest_failure.rs`.

### P2-4. Два публичных toggle с противоположной семантикой `active` в одном модуле

- `toggle_reaction(api, …, active)` (`src:254-265`) — `active` это **целевое** состояние
  (`true` → PUT), так работает IPC (`main.rs:393-408`);
- `toggle_reaction_from_active(api, …, active)` (`src:289-300`), `flip_reaction` (`src:302-311`),
  `DiscordReactionApi::toggle_from_active` (`src:187-196`) — `active` это **текущее** состояние,
  функция инвертирует его и возвращает новое.

Одинаковые имена параметра и типа при противоположном смысле — ловушка: если следующий воркер
подключит `toggle_from_active` к IPC или перепутает хелперы, реакция начнёт ставиться вместо
снятия. Сейчас новые хелперы не используются вне тестов (grep по `rust/`, `tests/` — только
`reactions.rs` и `reactions_contract.rs`). Переименовать параметр в `currently_active` (и/или
убрать `flip_reaction` как дубль) до того, как API кто-то начнёт потреблять.

### P2-5. Валидация строже оракула и принимает битую обёртку `<:a:name:id>`

- `is_numeric_id` (`src:387-391`) требует ещё и `parse::<u64>()`; `SPEC:795` описывает только
  «непустая строка из ASCII-цифр». Тест `:292,296` пинит отказ для `99999999999999999999999`.
  Для снежинок Discord безвредно, но это не документированное ужесточение — либо записать в
  спеку, либо убрать проверку.
- `normalize_and_validate_emoji` (`src:413-429`) принимает `<:a:party:123>` как animated
  `a:party:123` (мой пробник подтвердил `Ok("a%3Aparty%3A…")`). Discord такую обёртку не
  отдаёт; вход мусорный, но безопасный. Не блокирует, стоит закрыть проверкой, что `a:`-ветка
  приходит только из `<a:…>`.
- `MAX_EMOJI_CHARS` считается от trimmed-строки (`src:407-412`), что совпадает со `SPEC:797`;
  пробник с 200 пробелами + `👍` + 200 пробелами проходит — это ожидаемо, но тоже без теста.

### P2-6. `clippy --all-targets` красный по чужим файлам; `red.log` снят с другой ревизии тестов

- `cargo clippy --all-targets -- -D warnings` падает на `src/main.rs:1703`
  (`type_complexity`), `tests/net_contract.rs:782` (`type_complexity`) — это файлы других
  карточек (`FakePutTransport`, `DeferredReadyGateway`), к task-123 отношения не имеют.
  Скоуп-прогон из отчёта (`--test reactions_contract --lib`) чистый, я его повторил. Для
  `gate:worker` на интеграционном лейне эти два предупреждения нужно закрыть отдельно, иначе
  гейт упадёт не по адресу.
- `red.log` ссылается на строки `reactions_contract.rs:237/434/541`, текущий файл — `:242/462/573`.
  Набор и число падений совпадают (5 из 11, ровно новые тесты), мутанты на финальном файле
  убиты, поэтому red-first по существу подтверждён; но формально лог снят с более ранней
  ревизии теста (между прогонами добавилось ~30 строк внутрь `strict_validation...`). Для
  строгого T0-следа хранить копию теста на момент red-прогона.

## 4. Доказательства

**Мои прогоны** (`CARGO_TARGET_DIR=/home/user/orca/project/rust/target`,
`--manifest-path rust/gui-spike/Cargo.toml`):

- `cargo test --test reactions_contract` → `11 passed; 0 failed; 0 ignored`, exit 0;
- `cargo test --lib reactions` → `4 passed; 0 failed` (`decode_429_...`, `custom_emoji_...`,
  `decode_maps_2xx_...`, `api_error_is_classified_...`), exit 0;
- `cargo clippy --test reactions_contract --lib -- -D warnings` → exit 0, 0 предупреждений;
- `rustfmt --edition 2021 --check` по обоим файлам → exit 0;
- `cargo clippy --all-targets -- -D warnings` → red по `main.rs:1703` и `net_contract.rs:782`
  (см. P2-6).

**Независимые мутации** (`/tmp/opencode/mutate.py`, восстановление после каждого мутанта с
проверкой md5; baseline `753a4f2c…`):

| Мутант | md5 мутанта | Итог | Кем убит |
|---|---|---|---|
| `R45A-1`: `:` не кодируется в `percent_encode_emoji` | `951d271e84bf0bc873cfba66bf440715` | KILLED (3 теста) | `emoji_path_...`, `custom_and_unicode_...`, `discord_reaction_api_transport_...` |
| `R45A-2`: снят фильтр control/`/`/`\` | `6e1473c7593f5b1c0d9eb81883be2736` | KILLED | `strict_validation_...` |
| `R45A-3`: отклонение `a:name:id` | `995e7b639553beec3897cc1343a662f8` | KILLED (2 теста) | `custom_and_unicode_...`, `discord_reaction_api_transport_...` |
| `R45A-4`: `429` → `retry_after_ms: 0` | `5548cbf6a877d7dc28e171f6ca4484f2` | KILLED | `http_429_...` |
| `R45A-5`: нет инверсии в `toggle_reaction_from_active` | `150e009718a20a4c9d21af44177cda67` | KILLED | `idempotent_toggle_...` |
| `R45A-X1`: без `trim()` | `c3eae358f3f813967d73e0ae30412870` | **SURVIVED** | — (P2-2) |
| `R45A-X2`: пробелы внутри эмодзи разрешены | `7330de93dc1884cd2a8360b60e8fc835` | **SURVIVED** | — (P2-2) |
| `R45A-X3`: отсутствие поля → `1000` | `4e34a2cbfb691ad3d74287e7cd4ef8f3` | KILLED | `http_429_...` (пинит `0` для `Retry-After: -1`) |

Мутанты M1 и M5 побайтово совпали с отчётом воркера (`951d271e…`, `150e0097…`) — идея та же;
M2–M4 убиты независимо, md5 отличается только формой правки. Итог: **5/5 KILLED**, плюс два
выживших мутанта на непокрытых правилах валидации.

**Сверка артефактов:** md5 совпадают с `REPORT.md`: `reactions.rs` `753a4f2ce2bc60c369857cd2df94ac21`,
тест `d5b1da8f8d2783b209d2a51773913a29`, `red.log` `8bd362d9e5e93c0a7af1a4aeeaa155e7`,
`green.log` `fd0785f6b00b7098a8f5af4c75c86fb0`, `mutations.log` `ff3ced53f7a91f2e734640876f7fc760`.

**Честность тестов:** в `reactions.rs` и `reactions_contract.rs` нет `.skip`/`.only`/`#[ignore]`;
в тестовом файле удалена ровно одна строка — переписанный `use`; существующие тесты не ослаблены
(`git diff` показывает только добавления вокруг них).

**Заявленный пробник (удалён после прогона):** `" 👍 "` → Ok с trim; отсутствие `retry_after`
→ `0`; `<:a:party:123>` → animated; JSON приоритетнее заголовка (`1.5` vs `Retry-After: 5`
→ `1500`); 200 пробелов + `👍` + 200 пробелов → Ok.

**Скоуп:** диффу карточки принадлежат только два файла. Остальные ~50 изменённых файлов в
рабочем дереве (`main.rs`, `net.rs`, `search.rs`, `ui/*`, `tests/unit/*`) — работа параллельных
карточек RW-40, к этому воркеру не относятся и в ревью не входят.

## 5. Итог

Реализация соответствует карточке и §6.1–§6.2 спеки: кодирование Unicode/кастомных/анимированных
эмодзи, точный маршрут `…/reactions/{emoji}/@me`, `PUT` с `{}` / `DELETE` без тела, `204`/`200`,
типизированные `401/403/404/5xx/Network`, инверсия `active` в идемпотентных хелперах и
предсетевая валидация закрыты кодом и контрактными тестами. Мутационный порог 5/5 подтверждён
независимым прогоном, артефакты и md5 сходятся, ослаблений тестов нет.

Приёмку не блокирует ничего. Перед переносом в `beta` стоит закрыть P2-1 и P2-3 на уровне
спеки/`DECISIONS` (429-дефолт и статус «сырого» разбора 429), P2-2 — двумя тест-кейсами,
P2-4 — переименованием параметра, P2-5/P2-6 — по месту.

## Источники вердикта

`waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md` (§1, §3.2.4, §3.2.5, §6.1, §6.2),
`waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC-REVIEW.md` (§4.2 п.1, п.5, P1-5, P1-6),
`waves/RW-40/PM/task123-reactions-be/{REPORT.md,red.log,green.log,mutations.log}`,
`rust/gui-spike/src/reactions.rs`, `rust/gui-spike/tests/reactions_contract.rs`,
`rust/gui-spike/src/{rest_failure.rs,invites.rs,notifications.rs,pins_threads.rs}`,
`rust/gui-spike/src/main.rs:393-408`,
`rust/crates/discord-core/src/rest/{client.rs,rate_limit.rs}`, `TEST-POLICY.md`.
