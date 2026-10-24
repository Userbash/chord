# PLATFORM-AUDIT — G4b Security / G5 Platform (read-only)

Дата: 2026-10-24. Аудитор: субагент G4b+G5 (ТЗ `Q-AUDIT-PLATFORM`), PM — DeepSeek.
HEAD: `0239543` (дерево грязное: `rust/gui-spike/**` и `docs/rust-client/SECURITY.md` — untracked,
чужие правки не трогались). Режим: **только чтение**; live/сеть/окно/установки не запускались.

Основание: `waves/RW-40/QUALITY/STRATEGY.md` §2–§9, `G1-AUDIT.md`, `G2-AUDIT.md`,
`waves/RW-40/STUB-MAP.md` (STUB-13/17/26/27/28), `docs/rust-client/SECURITY.md` §1–§8,
`TEST-POLICY.md`. Запрет ТЗ соблюдён: FFI **не** предлагается убирать.

## 0. Что проверено и чем

| Проверка | Результат | Артефакт |
|---|---|---|
| census `unsafe {` по `rust/crates/**` | **62 блока**, 6 `unsafe extern "C"` блоков, 73 `SAFETY` | `PM/qa-platform/unsafe-census.txt` (+`.py`) |
| `cargo test --offline -p discord-platform --test keyring_contract --test spellcheck_contract` | exit **0**, 12 + 13 passed, 0 skip | `PM/qa-platform/platform-tests.log` |
| `cargo test --offline -p discord-platform --test dbus_contract` | exit **0**, 23 passed (f01–f13 в т.ч.) | `PM/qa-platform/dbus-tests.log` |
| `cargo test --offline -p discord-store` | exit **0**, 14 + 1 passed | `PM/qa-platform/store-tests.log` |
| системная база для G5 (read-only) | `libopus/libasound/libpipewire/libv4l2` + заголовки есть; `/dev/video*` нет; PipeWire-сокет есть; `pw-cli/wpctl/pactl/gst-launch` есть | §4, §4.1 |

Оговорка по evidence: тесты keyring на этом хосте реально выполнились (Secret Service доступен,
0 skip в отчёте) — но кейсы `k01–k08`, включая **leak-тест** `k02`, при `DISCORD_LAB_KEYRING_LIVE!=1`
делают ранний `return`, а `skip()` только печатает строку. Для cargo они **passed**, для гейта —
«12 passed, 0 skipped». Это PLAT-02 (P1), а не «всё хорошо».

## 1. Вывод в одну строку

**P0 не найдено.** Секрет не уходит в `Debug`/`Display`/логи/IPC, паники через FFI нет (ни одна
Rust-функция не передаётся в C), double-free не найден, `unsafe` изолирован и на всех 62 блоках
есть `SAFETY`. Найденные P1 — не утечка секрета, а **отказ доступности и потеря диагностики**:
keyring-операции идут под общим мьютексом без единого таймаута, а их ошибки маскируются под сеть.

## 2. Реестр `unsafe`/FFI (62 блока)

### 2.1 Где SAFETY есть, где нет

| Метрика | Значение |
|---|---|
| `unsafe {}` блоков | **62** (`store/ffi.rs` 24, `platform/ffi_secret.rs` 21, `ffi_hunspell.rs` 9, `dbus/fd.rs` 8) |
| блоков **без** `// SAFETY:` в ≤3 строках выше | **0** (максимальная дистанция — 3 строки) |
| `unsafe extern "C"` блоков | 6 (по SAFETY на каждый) |
| `SAFETY` всего | 73 (26+26+12+9) |
| `unsafe` вне этих 4 модулей | 0 (`deny(unsafe_code)` в Cargo.toml) |
| Rust-callback, переданный в C | 0 (`sqlite3_exec` всегда `None`) |
| `catch_unwind` | в продакшн-коде 0; сейчас не нужен, т.к. Rust-функция в C не передаётся (условие сохранения — PLAT-06) |

Уточнение к G1-14: «73 на 62» — арифметика верна, но 4 алиаса `type ... unsafe extern "C" fn`
(`ffi_secret.rs:60-62`, `store/ffi.rs:50`) — это **типы**, а не операции; SAFETY им не нужен.
Итого носителей инварианта 68 (62 блока + 6 extern). Мета-тест должен считать именно так,
иначе он либо ругается на легальные алиасы, либо пропускает extern-блок.

### 2.2 Легенда инвариантов

**Store (`S`)** — `discord-store/src/ffi.rs`: S1 handle открыт в `open`, один владелец `Db`,
закрыт ровно один раз в `Drop`; S2 `&str → CString` (NUL → `FfiError`), указатель живёт на время
вызова; S3 out-параметр инициализирован `null`, результат проверяется на null; S4 `SQLITE_TRANSIENT`
— SQLite копирует, указатель не хранится; S5 `Stmt<'a>` не переживает `Db`, `finalize` один раз
(в т.ч. на error-path `prepare`); S6 колонки читаются только после `Step::Row`, `pointer+bytes` →
копия в `Vec` до сдвига строки; S7 `error_message` из `exec` освобождается `sqlite3_free` один раз;
S8 коды C → `FfiError`, паник в блоках нет.

**Secret (`K`)** — `platform/ffi_secret.rs`: K1 `GError` читается до `g_error_free`, free ровно один
раз на каждом пути; K2 у `Service`/`Schema`/`Table`/`Value` один владелец, `unref` один раз; K3
hash-table владеет ключами/значениями (`g_free`-деструкторы), таблица имён живёт до `Schema`;
K4 значение копируется (`slice`/`g_strndup`) до `secret_value_unref`, наружу — только `Vec<u8>`;
K5 строка + точная длина (NUL не входит в `length`), `content_type` NUL-terminated; K6 null-check
после каждого C-вызова; K7 текст ошибки — только код+glib, секрет не печатается.

**Hunspell (`H`)** — `ffi_hunspell.rs`: H1 handle только из `create` (non-null), `destroy` один раз;
H2 слова через `CString`, NUL → `false`/пусто (не обрезаем); H3 массив `count` указателей читается
≤ `limit`, `free_list` с **исходным** `count`; H4 паник нет.

**fd (`F`)** — `dbus/fd.rs`: F1 `close_fd` игнорирует `fd<0`, владелец закрывает ровно один раз;
F2 `iov`/control-буфер живы на время `sendmsg`, `MSG_NOSIGNAL`, ancillary только к первому байту;
F3 `cmsg_len`/`CMSG_SPACE` согласованы с размером буфера; F4 обход cmsg ограничен
`msg_controllen`, `MSG_CTRUNC` → `Protocol`; F5 при ошибке наружу не уходит ни один осиротевший fd.

### 2.3 Реестр: блок → инвариант → тест

| # | file:line | fn | inv | тест-оракул |
|---:|---|---|---|---|
| 1 | `store/ffi.rs:148` | `Db::open` | S1,S2,S3 | `s01`,`s11` |
| 2 | `:177` | `Db::errmsg` | S1,S6 | `s11` |
| 3 | `:188` | `extended_errcode` | S1 | `s06`,`s11` |
| 4 | `:206` | `Db::exec` | S2,S3,S7 | `s01`,`s10` |
| 5 | `:219` | `Db::exec` (CStr) | S6,S7 | `s06`,`s11` |
| 6 | `:223` | `Db::exec` (free) | S7 | `s06`,`s11` |
| 7 | `:238` | `busy_timeout` | S1 | `s01` (косвенно) |
| 8 | `:253` | `Db::prepare` | S1,S2,S3 | `s02`,`s13` |
| 9 | `:266` | `prepare` (error finalize) | S5 | `s11` (косвенно) |
| 10 | `:276` | `changes` | S1 | `s02`,`s07` |
| 11 | `:286` | `Drop for Db` | S1 | `s09` (reopen), `s11` |
| 12 | `:321` | `bind_text` | S2,S4 | `s13` (NUL), `s10` |
| 13 | `:336` | `bind_i64` | S1 | `s03`,`s06` |
| 14 | `:343` | `bind_null` | S1 | `s04`,`s07` |
| 15 | `:350` | `step` | S1 | все `s*` |
| 16 | `:362` | `reset` | S1 | **нет** (`#[allow(dead_code)]`) |
| 17 | `:370` | `clear_bindings` | S1 | **нет** (`#[allow(dead_code)]`) |
| 18 | `:378` | `column_count` | S1 | **нет** (`#[allow(dead_code)]`) |
| 19 | `:385` | `column_is_null` | S1,S6 | `s04`,`s07` |
| 20 | `:395` | `column_text` | S1,S6 | `s13`,`s02` |
| 21 | `:401` | `column_text` (2 C-вызова) | S6 | `s02`,`s05` |
| 22 | `:422` | `column_text` (slice) | S6 | `s13` (UTF-8/large) |
| 23 | `:432` | `column_i64` | S1,S6 | `s03`,`s04` |
| 24 | `:440` | `Drop for Stmt` | S5 | `s02`,`s13` |
| 25 | `ffi_secret.rs:175` | `take_error` | K1,K6 | `k02` (success), `k09`–`k12` |
| 26 | `:187` | `take_error` (free) | K1 | `k09`–`k12` |
| 27 | `:196` | `duplicate` | K6 | `k06` (unicode) |
| 28 | `:209` | `Table::new` | K2,K3 | `k02`,`k04` |
| 29 | `:239` | `put_string` (free key) | K2,K3 | `k10` (NUL → ранний выход) |
| 30 | `:245` | `put_string` (insert) | K3 | `k02`,`k05` |
| 31 | `:259` | `put_name` | K3 | `k01`,`k03` |
| 32 | `:278` | `Drop for Table` | K2 | `k02`,`k11` |
| 33 | `:294` | `Service::connect` | K1,K6 | `k01` |
| 34 | `:324` | `Service::store` (`secret_value_new`) | K5,K6 | `k02`,`k06` |
| 35 | `:338` | `store` (`store_sync`) | K2,K6 | `k02`,`k05` |
| 36 | `:352` | `store` (unref) | K2 | `k02` |
| 37 | `:374` | `Service::lookup` | K1,K6 | `k03`,`k08` |
| 38 | `:392` | `lookup` (`secret_value_get`) | K4,K6 | `k02` |
| 39 | `:395` | `lookup` (unref on null) | K2 | `k03` (косвенно) |
| 40 | `:401` | `lookup` (slice) | K4 | `k02`,`k06` |
| 41 | `:403` | `lookup` (unref) | K2 | `k02` |
| 42 | `:419` | `Service::clear` | K1,K6 | `k04`,`k11` |
| 43 | `:439` | `Drop for Service` | K2 | `k02`,`k12` |
| 44 | `:459` | `Schema::new` | K3,K6 | `k01` |
| 45 | `:473` | `Drop for Schema` | K2 | `k02`,`k12` |
| 46 | `ffi_hunspell.rs:87` | `Handle::create` | H1,H2 | `h01`,`h10`,`h12` |
| 47 | `:101` | `check` | H2 | `h02`,`h03` |
| 48 | `:115` | `suggest` | H2,H3 | `h04` |
| 49 | `:123` | `suggest` (slice) | H3 | `h04`,`h11` |
| 50 | `:130` | `suggest` (CStr) | H3 | `h04` |
| 51 | `:137` | `suggest` (free_list) | H3 | `h11` |
| 52 | `:148` | `add` | H2 | `h05` |
| 53 | `:158` | `remove` | H2 | `h05` |
| 54 | `:166` | `Drop for Handle` | H1 | `h11` |
| 55 | `dbus/fd.rs:62` | `close_fd` | F1 | `f01`,`f04` |
| 56 | `:99` | `send_with_fds` (zeroed msghdr) | F2 | `f09` |
| 57 | `:110` | `send_with_fds` (cmsg) | F3 | `f09`,`f13` |
| 58 | `:131` | `send_with_fds` (sendmsg) | F2 | `f09` |
| 59 | `:150` | `send_with_fds` (ptr arith) | F2 | `f09` |
| 60 | `:175` | `recv_exact_with_fds` (zeroed) | F4 | `f04`,`f06` |
| 61 | `:184` | `recv_exact_with_fds` (recvmsg) | F4 | `f05`,`f06` |
| 62 | `:202` | `recv_exact_with_fds` (cmsg parse) | F4,F5 | `f04`–`f11`,`f13` |

Чего в реестре не хватает как **документа**: файла-артефакта «блок → инвариант → тест» в
`docs/rust-client/` (SECURITY.md §4 — чек-лист, не таблица). Эта таблица — её первая версия;
владельцу G5 остаётся перенести §2.3 в `docs/rust-client/UNSAFE-REGISTRY.md` и не терять при
следующих правках.

### 2.4 Ownership / double-free / утечки — что проверено по существу

| Проверка | Результат |
|---|---|
| двойное закрытие `Db`/`Stmt` | невозможно: `NonNull` + `!Clone` + `&'a Db` у `Stmt` |
| двойной `close(fd)` | нет: каждый fd владеет один держатель; `discard()` закрывает и очищает вектор |
| утечка `GError` | нет: `take_error` вызывается на каждом пути, включая success |
| утечка `SecretValue` в `store` | нет: `unref` до проверки результата |
| утечка `SecretValue` в `lookup` | нет по контракту glib (`NULL` при ошибке); теоретическое окно — PLAT-05 |
| утечка `fd` при `MSG_CTRUNC`/битом cmsg | нет: `discard` закрывает уже собранные |
| unbounded-буферы | `MAX_FDS_PER_MESSAGE=16`, `SCM_MAX_FD=253`, `RECEIVE_WINDOW=1024` (SRTP) — ограничены |
| panic через FFI | нет: в C не передаётся ни одна Rust-функция; все `unwrap` в FFI-модулях — `unwrap_or` |
| `unsafe impl Send/Sync` | 0 в репозитории; `Keyring` дополнительно фиксирует `!Send` через `PhantomData<*mut ()>` |

## 3. Keyring / secret (G4b)

| ID | file:line | риск | доказательство | P | фикс/инвариант | тест |
|---|---|---|---|---|---|---|
| **SEC-01** | `gui-spike/src/auth.rs:1089-1092` (+`main.rs:324-332`, `auth.rs:1115-1120`) | `ThreadedStore::call` ждёт `recv()` без таймаута: зависший/заблокированный Secret Service = вечное ожидание `spawn_blocking`; `Drop` делает `join()` тоже без предела | `receiver.recv()` без `recv_timeout`; ни одного `timeout/Duration` в `gui-spike/src/{auth,main}.rs` (`rg` — 0, кроме `thread::sleep` в тест-хуке) | **P1** | `recv_timeout(KEYRING_TIMEOUT=5s)`, параметр `with_timeout` для тестов; ошибка → `storage`-домен; `Drop` — `join` с пределом или `detach`+лог | `ThreadedStore::spawn(\|\| Ok(Box::new(BlockingStore)))`, timeout 50 мс → `Err("…не ответил")`; мутация «убрать `recv_timeout`» должна вешать тест (запускать под внешним `timeout`) |
| **PLAT-01** | `auth.rs:1231-1244` (`status`), `1249-1253` (`token`), `1262-1273`→`1502-1527` (`login`/`restore` + `validator.validate`), `1468-1497` (`accept_token`: сеть + `store.store`), `1310-1319` (`logout`) | `MutexGuard<Inner>` удерживается через **keyring I/O и сетевой `/users/@me`**. Любая команда, которой нужен токен, встаёт на мьютексе внутри `spawn_blocking`; пул блокирующих потоков выедается, UI «висит» без объяснения | `lock()` берётся до `store.load()`/`store.store()`/`validate()`; `restore` — `lock` → `load` → сеть; `token()` — `lock` → `load`; `accept_token` — `lock` → `validate` → `store` | **P1** | не держать guard через I/O: снять копию состояния/generation, отпустить guard, выполнить I/O, вернуть guard и применить результат только при совпадении `generation`; `status()` — читать keyring **до** `lock()` | fake `TokenStore`, который блокируется на канале: поток A `status()`, поток B `token()` обязан вернуться < 100 мс (сейчас висит); fake `TokenValidator` со `sleep(200ms)` для сетевой части; characterization на 39 `auth_contract` |
| **SEC-02** | `main.rs:150-160` (`token_or`), вызовы `171,190,208,225,243,267,288,308` | ошибка keyring (`Secret Service недоступен`, коллекция заблокирована) превращается в `Network`/`unavailable`: пользователь чинит сеть вместо хранилища; причина (единственный диагностический след) выбрасывается | `map_err(\|_\| unavailable())` на `AuthError`; `AuthError::Keyring{message}` не доходит ни в один `kind` | **P1** | аддитивный `kind:"storage"` (или `MissingToken` vs `Unavailable`) + текст причины (уже без секрета); до контрактного решения — счётчик+`eprintln` в одном месте | поведенческий: store → `Err("Secret Service недоступен")` → команда отдаёт `storage` и текст; characterization на текущий `Network` приложить к карточке (диф теста = PM) |
| **SEC-04** | `auth.rs:1075-1092` (после фикса SEC-01) | таймаут делает операцию **неопределённой**: запрос остаётся в очереди и может выполниться после того, как вызывающий увидел ошибку (`Store`/`Clear`) | `serve()` (`1124-1143`) обрабатывает запрос даже если `reply` уже недоступен (`let _ = reply.send(...)`) | P2 | инвариант: таймаут = «состояние неизвестно», UI пишет «не удалось подтвердить», а не «не сохранено»; отменённые запросы помечать и пропускать в `serve` (поколение запроса) | тест: `Store` с задержкой → таймаут → затем `Clear`; после `serve` keyring пуст (детерминированный fake store, без сети) |
| **SEC-03** | `auth.rs:731-739` (`wipe_string`/`wipe_bytes`), `705-715` (`login_body`), `1100-1103` (`token.to_string()` в канал), `1477` (`token.clone()` для `ValidationRequest`) | «пароль/код затираются best-effort» — гарантия уже́, чем звучит: копии живут в `serde_json::Value`/теле запроса/канале `Request::Store`/`ValidationRequest` и умирают без затирания; `String` с запасом ёмкости затирается только по `len` | `login_body` строит `serde_json::json!` (промежуточные `String`), `Request::Store(String)` пересылается по каналу, `token.clone()` в валидатор | P2 | либо честная формулировка в доке («гарантия не заявляется; zeroize не используем»), либо один тип `Secret` с затиранием в `Drop` (без новых зависимостей) — решение PM | unit: тело запроса после вызова — нули; тест-док: `rg serde_json::json!` в auth — ожидаемое исключение с комментарием |
| SEC-05 | `auth.rs:551-575` (`LoginView::Mfa.ticket`) | `#[derive(Debug)]` на `LoginView` печатает `ticket` (краткоживущий креденшл) в отличие от redacted `LoginOutcome` (`496-520`) | `derive(Debug)` без ручного impl; `ticket` — поле структуры | P3 | ручной `Debug` с `ticket: "<redacted>"` (как в `LoginOutcome`) | `format!("{:?}", LoginView::Mfa{..})` не содержит ticket |
| SEC-06 | `ffi_secret.rs:282` | док-комментарий: «`!Send + !Sync` обеспечивает вызывающий слой» — неверно: `NonNull` уже `!Send + !Sync`; формулировка провоцирует будущий `unsafe impl Send` «раз слой обеспечивает» | `struct Service { raw: NonNull<SecretService> }` | P3 | переписать: «тип `!Send + !Sync` автоматически; `unsafe impl` запрещён» + строка в UNSAFE-REGISTRY | meta-grep: `unsafe impl (Send\|Sync)` → 0 |

### 3.1 Гигиена реестра (P2/P3, доказательства из §2.3)

| ID | file:line | риск | доказательство | P | фикс/инвариант | тест |
|---|---|---|---|---|---|---|
| **PLAT-02** | `discord-platform/tests/keyring_contract.rs:30-33,61-68,192` | security-кейсы FFI **молча пропускаются**: `k01–k08` (включая leak-тест `k02` на 2000 итераций и interop с `secret-tool` `k07/k08`) при `LIVE_ENV != 1` делают `return`, а `skip()` только `println!`-ит. Cargo считает их **passed**, гейт видит «12 passed, 0 skipped»; причина пропуска не видна ни гейту, ни приёмке (`TEST-POLICY` §9 DoD: «skip только с причиной и не на дисплее»; сам skip-aware-подход разрешён `AGENTS.md`) | прогон: `platform-tests.log` — `12 passed; 0 failed; 0 ignored`, при этом `k02` не выполнялся (`keyring(true)` → `skip`) | **P1** | security-набор — отдельная фаза гейта, обязательная в live-окне; при `LIVE=1` ошибка среды = **fail**, не skip; в отчёте волны — явный список пропущенных live-кейсов | прогон `DISCORD_LAB_KEYRING_LIVE=1 … --test keyring_contract` в окне; gate-отчёт обязан печатать `skipped: N` для этого файла |
| PLAT-03 | `store/ffi.rs:359,367,375` | 3 FFI-слота (`reset`, `clear_bindings`, `column_count`) помечены `#[allow(dead_code)]` и не вызываются ни продом, ни тестами: в реестре это «блок без оракула», а `dead_code` скрывает, что контракт не проверен | `rg 'column_count\(\|\.reset\(\)\|clear_bindings\(' rust` — только объявления в `store/ffi.rs` (плюс несовместимый `Backoff::reset` в `discord-core`) | P2 | либо контрактные тесты на reuse prepare (reset+clear_bindings+повторный bind) и `column_count`, либо удалить слоты (решение владельца FFI-контракта) | тест reuse: prepare → bind → step → Done → reset → bind → step → Row; `column_count` = ожидаемое число |
| PLAT-04 | `dbus/mod.rs:121-127` (`from_abstract_name`), `195-206` (`connect`) | клиент принимает `unix:abstract=…` из `DBUS_SESSION_BUS_ADDRESS`: абстрактное имя в Linux не защищено правами ФС (в отличие от сокета-пути) — локальный процесс может занять имя и отвечать «как шина» (подмена ответов `Inhibit`/`Notify`/portal). Это свойство окружения, но выбор адреса нигде не предпочтён и не логируется | `parse` возвращает первый поддержанный адрес; `a04`/`a05` закрепляют abstract как равноправный | P3 | инвариант: в продакшн-пути предпочитать `unix:path=`; abstract — только по явной конфигурации; в диагностику — тип адреса без значения (решение PM, т.к. меняет порядок выбора) | тест: список `abstract;path` → выбран `path`; сейчас characterization фиксирует обратное |
| PLAT-05 | `ffi_secret.rs:289-301` (`connect`), `383-388` (`lookup`) | при «`Some(error)` + не-null указатель» объект не освобождается (утечка). По контракту glib (`transfer full, nullable` + NULL при ошибке) недостижимо, но инвариант нигде не записан ⇒ следующий правщик не знает, можно ли доверять | `let Some(error) = take_error(error) { return Err(error); }` до проверки `raw`/`value` | P3 | записать инвариант в UNSAFE-REGISTRY: «на error-path glib отдаёт NULL; unref не нужен, но и не вредит» + защитный `if !raw.is_null() { unref }` (нулевой риск) | тест невоспроизводим без подмены FFI; достаточно строки реестра + code-review чек-листа SECURITY §4 |
| PLAT-06 | `store/ffi.rs:49-50,206-214` | объявлен `ExecCallback`, но в C всегда передаётся `None`; инвариант «никогда не отдавать Rust-функцию в C без `catch_unwind`» не зафиксирован — будущий `sqlite3_exec` с callback'ом даст unwind через FFI (UB) | `None` в вызове `sqlite3_exec`; `catch_unwind` в продакшн-коде 0 (единственное совпадение — тест `discord-json/tests/json_contract.rs:165`) | P3 | строка в UNSAFE-REGISTRY + комментарий у типа: «callback только `extern "C"` без паник, иначе `catch_unwind`» | meta-grep: `sqlite3_exec` вызывается только с `None` |

## 4. G5 Platform: что есть, чего нет (факты)

| Область | Состояние | Доказательство |
|---|---|---|
| D-Bus клиент (auth/EXTERNAL, fd) | готов, покрыт: 23 теста, mock-шина, `f01–f13` с проверкой утечек через `/proc/self/fd` | `PM/qa-platform/dbus-tests.log` |
| logind inhibitor сна | готов (`Inhibit -> h`, владение fd), тесты `power_contract` | `power.rs:172-212` |
| уведомления | готов отправитель (8 hints), **не подключён** к приложению; DND/история/click→channel отсутствуют — «уровень приложения» | `notifications.rs:9-12`; `gui-spike` не зависит от модуля (`rg discord_platform::` — только `keyring`) |
| autostart / deep link | готовы, экранирование `Exec`, control-chars и лимит 4096 | `autostart.rs:103-108`, `deep_link.rs:18-32` |
| keyring | готов, 12 тестов, redacted `Debug`, leak-тест live-gated | §3, `k02` |
| **voice devices/capture** | **нет ни одного пути**: `discord-voice` — сигнализация/RTP/SRTP/jitter/stats, без устройств и Opus; `discord-media` — 26 строк констант | `discord-voice/src/**`, `discord-media/src/lib.rs:11-17` |
| **video/Go Live** | нет; `STUB-27` | `STUB-MAP.md:53` |
| системная база (этот хост) | есть `libopus`, `libasound`, `libpipewire-0.3`, `libspa-0.2`, `libv4l2` + заголовки; активен PipeWire-сокет; `/dev/video*` отсутствует; `pw-cli/pw-dump/wpctl/pactl/gst-launch` есть | `ldconfig -p`, `/usr/include/{opus,alsa,pipewire-0.3,spa-0.2}`, `ls /dev/video*` |

### 4.1 Feasibility capture (без live)

| Шаг | Можно без live? | Условия / инвариант | Тест-оракул |
|---|---|---|---|
| Portal marshalling `org.freedesktop.portal.ScreenCast`/`Camera` (D-Bus) | **да** | только через `discord-platform::dbus`, mock-шина как в `a04/a05`; никаких путей/шелл-спавна | contract на mock-шине: `CreateSession → SelectSources → Start → OpenPipeWireRemote`, разбор `(u, a{sv})`, отказ пользователя → типизированная ошибка |
| Захват экрана/камеры (реальные кадры) | **нет** | диалог согласия портала = интерактив; нужен экран владельца | live-окно: пользователь подтверждает, приходит PipeWire node |
| Микрофон (voice) | частично | ALSA (`libasound`) или PipeWire; готового Rust-пути нет. Минимум FFI-поверхности — ALSA PCM | loop-тест с `snd_pcm_open("null")`/`plug` без железа; тест ошибок `-ENODEV` |
| Opus encode/decode | да | `libopus` есть; PT 120 / 48 кГц / 20 мс уже зафиксированы (`media/rtp.rs:16-33`) | KAT: encode(тишина) → decode → длина/каденс; ошибки `opus_encoder_create == NULL` |
| v4l2 напрямую | не рекомендуется | в env нет `/dev/video*`, доступ требует группы `video`; камера — через portal | — |
| Go Live (стрим) | **нет** | нужен live voice ws + rid/stream key | live |

Порядок, который не создаёт долга: (1) portal-маршаллинг + mock-тесты; (2) `libopus` FFI-модуль с
KAT; (3) ALSA-capture с consent-индикатором; (4) только потом live-окно с реальным захватом.
Каждый шаг — отдельный `#[allow(unsafe_code)]`-модуль + строка в UNSAFE-REGISTRY + error-path тест.

### 4.2 Consent / permissions (инварианты, которые надо записать до кода)

1. Захват не стартует без явного действия пользователя и видимого индикатора; индикатор = фактическое
   состояние потока кадров, а не флаг UI (см. PLAT-10).
2. Источник — только portal (ScreenCast/Camera); прямые устройства — запрещены по умолчанию.
3. `restore_token` портала — capability, не секрет аккаунта, но и не публичный: файл в XDG-state
   с правами `0600`, не в логах; при logout — не удалять (иначе UX), но и не шарить между профилями.
4. Конец сессии портала / lock / suspend → немедленно закрыть поток и снять индикатор (связка с
   `power.rs`/logind).
5. Никакой отправки кадров до `Voice Op 1 Select Protocol`-этапа и без выбранного кодека.

## 5. Security-риски заглушек

| ID | STUB | риск | доказательство | P | инвариант/фикс | тест |
|---|---|---|---|---|---|---|
| PLAT-10 | STUB-17 mute/deafen | кнопка mute/deafen — только `aria-pressed`, реального voice-state нет; после появления захвата «выключено» может не соответствовать потоку | `STUB-MAP.md:38`; `ui/features/actions.js` (toggle только `aria-pressed`) | P1 (после включения capture) / P2 сейчас | пока голоса нет — `aria-disabled`+`title`; когда появится — `aria-pressed` из фактического состояния (mute гасит кадры, не иконку) | тест: клик по mute без voice-state не меняет `aria-pressed`; с fake-состоянием — меняет и останавливает фейк-поток |
| PLAT-11 | STUB-13 attachments | при реализации «по аналогии» появится IPC-команда с путём от рендерера → чтение любого локального файла (`~/.ssh`, cookies) при XSS/компрометации webview | сейчас команд с путём нет — это контроль-гэп, а не дефект; `ATTACH-RE.md:23-33` — upload **blocked** | P1 (контроль) / P2 (код) | путь принимается только как handle из `tauri-plugin-dialog`; канонизация + allowlist + лимит размера; upload не пишется до `ATTACH-RE-live` | meta-grep: ни одна `#[tauri::command]` не содержит `path`/`PathBuf`/`&str` с именем пути; контракт dialog-handle на fake |
| PLAT-12 | STUB-26/27 voice/video | появление новых FFI (opus/alsa/pipewire/v4l2) без дисциплины: `unsafe` в обычном модуле, паника в потоке кадров, неограниченные буферы | `Cargo.toml` voice = `forbid`, media = `forbid`; в env есть библиотеки ⇒ соблазн «быстро прилинковать» | P2 | новые модули только `#[allow(unsafe_code)]` + UNSAFE-REGISTRY; лимиты кадров/буферов; ошибки C → typed; capture только по §4.2 | mutation-runbook: новая FFI-функция без SAFETY → meta-тест краснеет |
| PLAT-13 | STUB-28 DND/уведомления | DND не реализован; `Notification` содержит `body` (текст сообщения) и `derive(Debug)` → PII в логах при отладке; нет click→channel | `notifications.rs:9-12,37-47` | P2 | DND-состояние на уровне приложения (`suppress-sound`/`transient`), «не показывать текст» по умолчанию на lock-экране; `Debug` для `body` — длина/hash | тест: `format!("{:?}", Notification{body:"secret"})` не содержит текста; DND=true → `suppress-sound=b:1`, urgency=low |
| PLAT-14 | secret storage (db/settings) | SQLite-файл истории создаётся по umask (обычно `0644`) → другой локальный пользователь читает переписку; `settings_store` прямо говорит «права из umask» | `rg '0o600\|set_permissions' rust` — 0 совпадений; `settings_store.rs:25` | P2 (до подключения store к приложению) | каталог `0700`, БД/WAL `0600` (`set_permissions` после создания), проверить до первого `PRAGMA`; токенов в БД нет — и не должно появиться | тест: после `Store::open` `metadata().permissions().mode() & 0o077 == 0`; `ls -l` в oracle-скрипте |
| PLAT-15 | инфраструктура гейта | нет `unsafe_code` deny в `discord-json` и `gui-spike`; нет meta-теста на реестр/`unsafe impl`/CSP | `rust/Cargo.toml` и `gui-spike/Cargo.toml` без `[lints.rust]`; `csp: null` в `tauri.conf.json` | P2 | добавить `unsafe_code = "deny"`; meta-тест: каждый FFI-крейт `deny\|forbid`, `unsafe impl (Send\|Sync)` = 0, `csp != null`, capability без `remote.urls` | `tests/meta/unsafe-inventory.test.mjs` (новый) — падает на мутации «добавить `unsafe {}` в gui-spike» |

Отдельно (не дефект, но зафиксировать): capability `default.json` содержит только `core:default` и
**не** объявляет `remote.urls` — это правильно, IPC с чужого origin закрыт. Но `csp: null` +
`withGlobalTauri: true` означает: любой будущий HTML-инъекшн сразу получает `invoke`. Сейчас
`innerHTML` в `ui/**` = 0 (G2), поэтому P2, а не P1. Триггер повышения: любое remote-HTML/SVG,
markdown-рендер или preview вложений.

## 6. Приоритеты и владельцы

| P | ID | Owner | Почему этот порядок |
|---|---|---|---|
| **P1** | SEC-01 | G1 | один зависший keyring = приложение не отвечает совсем; фикс локальный (`recv_timeout`) |
| **P1** | PLAT-01 | G1 | тот же сценарий делает «висит» глобальным (мьютекс); требует generation-инварианта |
| **P1** | SEC-02 | G1+PM | без него первые два не диагностируются: пользователю показывают «сеть» |
| **P1** | PLAT-02 | G4+G1 | security-кейсы FFI (leak `k02`, interop `k07/k08`) не просто вне гейта — они **выглядят зелёными** |
| **P1** | PLAT-10 / PLAT-11 | G2+G1 | ложное «mute» и путь к файлам — будущие P0-классы; дешёвые инварианты сейчас |
| P2 | SEC-04, SEC-03, PLAT-03, PLAT-12, PLAT-13, PLAT-14, PLAT-15 | G1/G5/G4 | долг, который не блокирует текущий спайк |
| P3 | SEC-05, SEC-06, PLAT-04, PLAT-05, PLAT-06 | G5 | гигиена реестра и доков; нулевой риск |

P0: **0** (обоснование в §1). Массового rewrite не требуется ни в одном пункте: все P1 — это один
таймаут, одна развязка guard'а, один аддитивный `kind` и два теста.

Номера `PLAT-07…PLAT-09` в реестр не попали: их содержание вошло в SEC-01/PLAT-01/PLAT-02
(таймаут, guard, невидимый skip) — отдельными карточками не дублируются.

## 7. Что требует live-окна (и что нет)

| Требует live | Почему |
|---|---|
| Портал ScreenCast/Camera: диалог согласия и реальные PipeWire-кадры | интерактивное разрешение пользователя + компоситор |
| Реальный микрофон (ALSA/PipeWire), проверка «mute гасит поток» | нужно железо/сессия |
| Discord voice ws (op2/op4), Go Live/`STREAM_*` | сеть + аккаунт |
| Keyring в состоянии «коллекция заблокирована / prompt» | нужен реальный Secret Service с блокировкой |
| Уведомления: фактический DND-режим DE и клик по уведомлению | DE/шина, окно |

Не требует live: SEC-01, PLAT-01, SEC-02, SEC-04 (fake store/validator), PLAT-02 (частично),
PLAT-03/05/06, PLAT-11, PLAT-13 (unit/meta), PLAT-14 (temp-каталог), portal-маршаллинг на mock-шине,
`libopus` KAT, ALSA на `null`-устройстве.

## 8. Топ-10

1. SEC-01 — `ThreadedStore::call` без таймаута (P1): вечный spinner, `Drop::join` тоже без предела.
2. PLAT-01 — мьютекс auth удерживается через keyring и `/users/@me` (P1).
3. SEC-02 — keyring-ошибка маскируется под `Network`, причина теряется (P1).
4. PLAT-02 — leak/interop тесты FFI (`k02`,`k07`,`k08`) live-gated, а `skip()` невидим гейту:
   «12 passed» при невыполненных security-кейсах (P1).
5. PLAT-10 — mute/deafen = только `aria-pressed`, без voice-state (P1 при включении capture).
6. PLAT-11 — нет инварианта «IPC не принимает путь»; STUB-13 заблокирован, но следующий воркер
   может открыть произвольное чтение файлов (P1-контроль).
7. SEC-04 — таймаут keyring делает операцию неопределённой; UI не должен врать «не сохранено» (P2).
8. SEC-03 — «best-effort wipe» слабее, чем звучит (serde-копии, канал) (P2).
9. PLAT-14 — SQLite/настройки без `0600`; `PLAT-15` — нет `unsafe_code` deny в `gui-spike`/`json`
   и нет meta-теста инвентаря (P2).
10. G5 капчер: нет ни device-, ни Opus-пути; в системе есть `libopus/libasound/libpipewire` ⇒
    реализуемо поэтапно (portal → opus → ALSA), live нужен только для согласия и кадров (P2).

## 9. Артефакты

```
waves/RW-40/QUALITY/PLATFORM-AUDIT.md        — этот отчёт
waves/RW-40/PM/qa-platform/unsafe-census.txt — census 62 блоков/73 SAFETY/∆ до SAFETY
waves/RW-40/PM/qa-platform/unsafe-census.py  — скрипт census (воспроизводимо)
waves/RW-40/PM/qa-platform/platform-tests.log— keyring 12 + spellcheck 13, EXIT=0
waves/RW-40/PM/qa-platform/dbus-tests.log    — 23 passed, EXIT=0
waves/RW-40/PM/qa-platform/store-tests.log   — 14 + 1 passed, EXIT=0
```

Изменений в коде/тестах/доках проекта не вносилось; `logs/`, `functions/`, `evidence/` не трогались;
`git status` до/после — без новых правок в отслеживаемых файлах.
