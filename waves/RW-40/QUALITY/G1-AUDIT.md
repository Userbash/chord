# G1-AUDIT — Core/Backend (production Rust), read-only

Дата: 2026-10-24. Аудитор: субагент G1 (по ТЗ Q-AUDIT-G1), PM — DeepSeek.
Статус: **read-only**, код/тесты/доки проекта не менялись; изменений в `git` нет.
Основание: `waves/RW-40/QUALITY/STRATEGY.md` §2–§9, `waves/RW-40/QUALITY/BASELINE-QUALITY.md`,
`TEST-POLICY.md`, `waves/RW-40/PARITY/G6-CHARTER.md`.

## 0. Scope и метод

Читалось: `rust/gui-spike/src/**`, `rust/crates/discord-*/src/**` (кроме generated
`discord-i18n/*_data.rs`), контрактные тесты — только для проверки «оракул уже есть».

- `rg` по `unwrap/expect/panic!/allow/unsafe/let _`, ручное чтение ветвей ошибок;
- production/tests разделены скриптом: всё после первого `#[cfg(test)]` в файле — тесты;
- один `cargo` за раз; live/сеть не запускались, `logs/`, `functions/`, `evidence/` не трогались.

Что **не** делалось: RE-подтверждение поведения Discord, запуск живого окна, mutation-прогон
(это следующий шаг после приёмки findings; в отчёте указан оракул для каждого фикса).

## 1. Прогоны (evidence)

| Команда | Exit | Лог |
|---|---:|---|
| `cd rust && cargo clippy --offline -p discord-core --all-targets -- -D warnings` | **0** | `waves/RW-40/PM/qa-g1/clippy-core.log` |
| `cd rust && cargo clippy --offline --manifest-path gui-spike/Cargo.toml --all-targets -- -D warnings` | **0** | `waves/RW-40/PM/qa-g1/clippy-gui-spike.log` |
| `cd rust && cargo test --offline --manifest-path gui-spike/Cargo.toml` | **0** (161 passed / 0 failed) | `waves/RW-40/PM/qa-g1/test-gui-spike.log` |

Замечание по evidence: `cargo test -p discord-gui-spike` из workspace не работает
(`package ID ... did not match`, exit 101) — gui-spike не входит в workspace. Прогон только
через `--manifest-path`; это стоит зафиксировать в `PROCESS.md`/гейте, иначе следующий воркер
снова получит ложный красный.

## 2. Нормализованный census `panic`-точек (production vs tests)

Baseline §2 даёт 106 `unwrap` / 30 `expect` / 2 `panic!` — это **prod+tests**. После отделения
`#[cfg(test)]` и тестовых модулей:

| Класс | Production | Где |
|---|---:|---|
| `unwrap()` | **14** | `discord-ui`: tray 8, dialog 3, window 2, navigation 1 — все на `SystemTime`/`HashMap` |
| `expect()` | **10** | `core/state/reducer.rs` 2, `discord-i18n/lib.rs` 6, `i18n/markdown.rs` 1, `gui-spike/main.rs` 1 |
| `panic!()` | **2** | `discord-codec/etf.rs:260,275` (в `encode`/`encode_term`) |

Вывод: «106 unwrap» — артефакт методики. В реальных путях пользовательского ввода, сети и
файлов паник **нет** (это подтверждено и чтением, и clippy). Остаются 16 однотипных
`SystemTime`-unwrap в скелете `discord-ui` (G1-07) и публичный `etf::encode` (G1-10).

## 3. Сводная таблица findings

| ID | P | file:line | Симптом |
|---|---|---|---|
| G1-01 | **P1** | `gui-spike/src/{messages.rs:302,reactions.rs:155,search.rs:203,guilds.rs:217}` + decode-блоки | 4 копии `map_api_error` + 4 копии status-декодера; всё, кроме 401/429, → `Network` |
| G1-02 | **P1** | `gui-spike/src/main.rs:150,168,185,201,218,274` (+`guild_token:234`) | 6 копий чтения токена; `AuthError::Keyring` → `Network`/`Unauthorized` |
| G1-03 | P2 | `gui-spike/src/auth.rs:1502-1528` (lock `1264`, `1284`) | мьютекс состояния удерживается через сетевой `/users/@me` |
| G1-04 | P2 | `gui-spike/src/auth.rs:1075-1093` | `mpsc::recv()` без таймаута: зависший Secret Service = вечный `spawn_blocking` |
| G1-05 | P2 | `crates/discord-ui/src/reply_thread.rs` (436) + 5×`dialog_window.rs.bak*` | модуль не объявлен в `lib.rs` ⇒ не компилируется/не тестируется; 59 КБ бэкапов в `src/` |
| G1-06 | P2 | `gui-spike/src/data.rs` 1814, `auth.rs` 1539, `core/src/ws.rs` 776 | несколько ответственностей в одном файле, швы уже размечены комментариями |
| G1-07 | P3 | `discord-ui/{tray_manager.rs:130,152,168,194,210,258,265,272, window_manager.rs:124,164, navigation.rs:49, dialog_window.rs:182,213}` | 12× `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` |
| G1-08 | P3 | `core/src/ready/snapshot.rs:392-414`, `discord-ui/src/dialog_window.rs` 11× | 11 `#[allow(unused_imports)]` без причины; 11 `#[allow(dead_code)]` на `pub`-элементах |
| G1-09 | P3 | `gui-spike/src/main.rs:243-262`, `guilds.rs:183-188` | два Tauri-command на один контракт + alias-функция; UI не вызывает ни один |
| G1-10 | P3 | `discord-codec/src/etf.rs:260,275` | публичный `encode` паникует на внешних данных, `encode_checked` рядом |
| G1-11 | P3 | `core/src/rest/identity.rs:45-50` | сбой `/dev/urandom` глотается → UUID v4 из нулей |
| G1-12 | P3 | `gui-spike/src/net.rs:258-267` | `StopReason::ReconnectLimit` → `Closed(4013)` («invalid intents») |
| G1-13 | P3 | `gui-spike/src/data.rs:1489-1505` | `ensure_day` принимает `2026-99-99`, `2026-02-31` |
| G1-14 | P2 | `discord-store/src/ffi.rs`, `discord-platform/src/{ffi_secret,ffi_hunspell,dbus/fd}.rs` | SAFETY-комментарии есть (73 на 62 блока), но нет реестра «блок → инвариант → тест» |

## 4. Детали

### G1-01 — REST error-mapping: 4 копии и потерянные 403/404/5xx

`file:line`: `gui-spike/src/messages.rs:302-311` (`map_api_error`), `248-257` (`decode_action`),
`312-321` (`decode_receipt`); `reactions.rs:155-164`, `165-175`; `search.rs:203-212`, `181-190`
(status-ветки inline в `execute`); `guilds.rs:217-226`, `202-214`; плюс `auth.rs:320-331`,
`863-872` (две «свои» версии).

Симптом:

```rust
// 4 почти одинаковых тела
ApiError::Unauthorized => X::Unauthorized,
ApiError::RateLimited { retry_after, .. } => X::RateLimited {
    retry_after_ms: u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX) },
_ => X::Network,                       // ← сюда падают Server{5xx}, Client{403,404}, InvalidRequest
```

Почему плохо:
1. **Пользовательский дефект.** `send/edit/delete/reply/toggle_reaction/search` при 403
   («нет права писать в канал»), 404 («сообщение/канал удалён») и `ApiError::InvalidRequest`
   показывают «сеть недоступна» — UI может предлагать retry вместо «нет доступа/не найдено».
   `guilds.rs` уже умеет 403/404 — значит, это не ограничение модели, а разъезд копий.
2. **Дубли.** 4×10 строк маппера + 4×10 строк декодера + 2 свои версии в `auth.rs`; правка
   политики ошибок требует 6 синхронных изменений (уже разъехались: `reactions::decode`
   отдаёт `InvalidResponse` на 5xx, `messages::decode_action` — `Network`).
3. `429` в decode-ветках всегда `retry_after_ms: 0`, хотя `ApiError::RateLimited` несёт реальную
   задержку: два разных ответа на один и тот же статус в одном файле.

Минимальный фикс (без изменения публичных `kind`): один приватный классификатор в
`gui-spike/src/rest_errors.rs`:

```rust
pub(crate) enum RestStatus { Ok, Unauthorized, Forbidden, NotFound, RateLimited(u64), Server(u16), Client(u16), Transport }
pub(crate) fn classify(err: &ApiError) -> RestStatus;
pub(crate) fn decode(status: u16, headers: &[(String,String)]) -> RestStatus; // 2xx/401/403/404/429
```

и по одному `From<RestStatus> for XxxError` на домен. 403/404 добавлять **только** вместе с
аддитивными вариантами `Forbidden/NotFound` и обновлением `ui/` (см. риски, §7) — отдельной
карточкой после решения PM.

Тест-оракул: `tests/messages_contract.rs`, `reactions_contract.rs`, `search_contract.rs`,
`guilds_contract.rs` уже пинят текущие коды — они и есть characterization. Добавить по одному
кейсу «403 → kind», «5xx → kind» (сейчас 403 не покрыт нигде, кроме guilds).

Выигрыш: −~90 LOC дублей, один источник политики; риск — изменение `kind` для 403/404
(контрактное, требует ADR/PM).

### G1-02 — Чтение токена: 6 копий, keyring-ошибка маскируется под сеть

`file:line`: `gui-spike/src/main.rs:150-154, 168-172, 185-189, 201-205, 218-222, 274-278`;
образец уже вынесен — `guild_token` (`main.rs:234-240`).

```rust
let token = blocking(move || backend.token())
    .await.map_err(|_| MessageError::Network)?      // ← join-error spawn_blocking
    .map_err(|_| MessageError::Network)?            // ← AuthError::Keyring !!
    .ok_or(MessageError::Unauthorized)?;
```

Почему плохо:
1. `AuthBackend::token()` возвращает `AuthError::Keyring { message }` (Secret Service недоступен,
   коллекция заблокирована). Здесь причина **выбрасывается** и превращается в «сеть недоступна»,
   т.е. UI отправляет пользователя чинить сеть вместо keyring. Диагностика теряется полностью.
2. `ok_or(Unauthorized)` смешивает «нет токена» и «токен протух». В `guilds`/`search` для этого
   есть `MissingToken`, в `messages`/`reactions` — нет: одинаковое условие даёт разные `kind`.
3. 6 копий по 5 строк; любая новая команда копирует блок целиком (уже 6 из 6).

Минимальный фикс: один хелпер + один общий вариант ошибки:

```rust
async fn token_or<E: TokenError>(backend: Arc<AuthBackend>) -> Result<String, E>;
trait TokenError { fn missing() -> Self; fn storage(reason: String) -> Self; fn transport(reason: String) -> Self; }
```

`guild_token` заменяется на него; `EnterGuildError::MissingToken` сохраняется, в
`MessageError`/`ReactionError` добавляется аддитивный `Storage`/`MissingToken` (или, если PM
запретит менять контракт, причина пишется в лог + счётчик — но это хуже: UI всё равно врёт).

Тест-оракул: `AuthBackend::new(Box<dyn TokenStore>, validator)` уже позволяет подсунуть store,
возвращающий `Err("Secret Service недоступен")`; тест на команду (или на хелпер) проверяет
`kind == "storage"` и что текст причины дошёл. Characterization: `tests/messages_contract.rs:217`,
`guilds_contract.rs:182` пинят текущее поведение — их диф требует PM.

Выигрыш: −~30 LOC, устранение P1-класса «ошибка не того домена»; риск — аддитивные варианты в
трёх enum'ах (frontend увидит неизвестный `kind`; нужен fallback в `ui/`).

### G1-03 — Мьютекс авторизации удерживается через сетевой вызов

`file:line`: `auth.rs:1223-1228` (`lock`), `1248-1256` (`token`), `1260-1276` (`login`),
`1278-1299` (`restore`), `1502-1528` (`authenticate` → `self.validator.validate(&request)` под
`&mut Inner`).

Почему плохо: `login`/`restore` берут `MutexGuard<Inner>` и **внутри** ходят в `/users/@me`
(таймаут × попытки). В это время `token()`/`status()` из других Tauri-команд (`search_remote`,
`send_message`, `net_connect`) стоят на этом мьютексе внутри `spawn_blocking` — блокирующий пул
tokio забивается, UI-команды висят синхронно с чужим сетевым таймаутом. Это не deadlock, но
непредсказуемая задержка, которую нельзя объяснить пользователю.

Минимальный фикс: не держать guard через I/O — снять `request` (клоны identity), отпустить
guard, выполнить `validate`, затем взять guard снова и применить результат, только если
`state` не изменился (счётчик поколения `generation: u64` в `Inner`; при расхождении —
диагностика, не затирание).

Тест-оракул: fake `TokenValidator` со `sleep(200ms)`; поток A зовёт `restore()`, поток B через
20 мс зовёт `token()` и обязан вернуться < 100 мс (после фикса) — либо тест фиксирует
документированную сериализацию (до фикса). Проверка гонки: два параллельных `login` — победитель
один, второй видит `generation` mismatch.

Выигрыш: предсказуемая latency IPC; риск средний — конкурентность, обязателен characterization
на текущие `auth_contract` (39 тестов).

### G1-04 — `ThreadedStore::call` без таймаута

`file:line`: `auth.rs:1075-1093` (`receiver.recv()` на 1090).

Почему плохо: поток-владелец keyring может навсегда застрять на prompt/DBus
(`Secret Service` заблокирован). Тогда `recv()` не вернётся никогда, `spawn_blocking`-задача
висит, команда (и её `await` в Tauri) — тоже. `Drop` для `ThreadedStore` честно ждёт `join`, но
это только выход из приложения. Таймаута нет ни на одном уровне.

Минимальный фикс: `recv_timeout(KEYRING_TIMEOUT)` (константа 5 с; в тестах — параметр
`with_timeout`) → `Err("поток keyring не ответил за N мс")`, который маппится в `storage`-ошибку
(G1-02). Поток при этом не убиваем (он владеет `!Send`-объектом), но перестаём ждать.

Тест-оракул: `ThreadedStore::spawn(|| Ok(Box::new(BlockingStore)))`, где `BlockingStore::load`
спит на канале; тест с timeout 50 мс проверяет `Err` и что причина содержит «не ответил».
Мутация: убрать `recv_timeout` → тест виснет (запускать с внешним `timeout` в harness).

Выигрыш: исчезает класс «UI навсегда в spinner»; риск низкий.

### G1-05 — Осиротевший модуль и бэкапы в `src/`

`file:line`: `rust/crates/discord-ui/src/reply_thread.rs` (436 строк, P3-07 contract),
`rust/crates/discord-ui/src/lib.rs` (модуль не объявлен); `dialog_window.rs.{backup,backup2,bak2,bak3,bak_final}`
(5 файлов, 59 КБ, все в git).

Почему плохо:
1. `reply_thread.rs` **не компилируется и не тестируется** (inline `mod tests` внутри мёртвого
   файла): контракт P3-07 считается «сделанным», но его нет в бинаре. `rg reply_thread` вне
   самого файла — 0 совпадений.
2. Бэкапы в `src/` искажают любые glob-метрики (мой первый прогон `rg` по `crates/discord-*/src`
   посчитал их как production-код) и создают соблазн «поправить не тот файл».

Минимальный фикс: `git rm` пяти бэкапов; для `reply_thread` — либо `pub mod reply_thread;`
(+ прогон его тестов), либо удалить файл отдельной карточкой с обоснованием (TEST-POLICY §2.8:
удаление теста = отдельная карточка).

Тест-оракул: `git ls-files rust | rg '\.(bak|backup)'` → пусто; `cargo test -p discord-ui`
показывает тесты нового модуля; мутация — переименовать `reply_thread.rs` → сборка падает
(доказывает, что модуль подключён).

Выигрыш: −59 КБ мусора, честная coverage-картина (+436 LOC контракта либо −436 мёртвого кода).

### G1-06 — Крупные модули: где именно резать

| Файл | LOC | Уже существующие швы | Предлагаемый split (без изменения поведения) |
|---|---:|---|---|
| `gui-spike/src/data.rs` | 1814 | `// ---- messages_page / members_page / search_page / settings` | `data/{mod,ready,messages,members,search,settings}.rs`, `mod.rs` = `pub use` |
| `gui-spike/src/auth.rs` | 1539 | `// ---- Состояние / Текст ошибок / Identity / LoginView / Транспорт / Валидатор / Store / Backend` | `auth/{mod,state,error,login,store,backend}.rs` |
| `core/src/ws.rs` | 776 | нормализация URL / pipe / heartbeat-энкодеры / цикл | `ws/{mod,url,pipe,heartbeat,run}.rs`, `pub use` в `ws.rs` |

Почему плохо: в одном файле живут разные причины изменения (парсинг фикстур vs HTTP-контракт vs
keyring-поток). Diff любого изменения читается целиком; `data.rs`/`auth.rs` — самые горячие файлы
BFF (по 1385/1370 строк контрактных тестов).

Минимальный фикс: **один** файл за карточку, только перемещение + `pub use`, без правки тел.
`pub use` обязателен: тесты импортируют `discord_gui_spike::data::*`, `discord_core::ws::*`.

Тест-оракул: существующие `tests/data_contract.rs` (29), `auth_contract.rs` (39),
`core` ws-тесты (`ws_transport`, `identify_ready`, `resume_reconnect`) без единой правки; плюс
`cargo doc --no-deps` (проверка внутридоковых ссылок).

Выигрыш: навигация/ревью, не LOC; риск — механические ошибки импорта, ловится компилятором.

### G1-07 — 12 копий `SystemTime … unwrap()` (P3)

`file:line`: `tray_manager.rs:130,152,168,194,210,258,265,272`; `window_manager.rs:124,164`;
`navigation.rs:49`; `dialog_window.rs:182,213` (плюс `dialog_window.rs:229` — `get_mut().unwrap()`
после `contains_key`, лишний lookup).

Почему плохо: (а) дубль 12×; (б) `duration_since(UNIX_EPOCH)` паникует, если системные часы
раньше 1970 (битый RTC/контейнер) — для UI-скелета это не «never happens», а незакрытая ветка;
(в) `?`-стиль модуля (`Result<_, String>`) уже есть, а паника его обходит.

Минимальный фикс: `fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) }`
в `discord-ui/src/lib.rs` (или `time.rs`), 12 замен, `429`-подобных решений не требуется.

Тест-оракул: unit на `now_ms()` (не 0, монотонность); для ветки ошибки — выделить
`fn epoch_ms(t: SystemTime) -> u64` и подать `UNIX_EPOCH - 1s` → `0`, без паники.

Выигрыш: −~30 LOC, −12 panic-точек; риск нулевой (только `discord-ui`).

### G1-08 — `#[allow]` без причины

`file:line`: `core/src/ready/snapshot.rs:392-414` — 11× `#[allow(unused_imports)] use … as _X;`
(22 строки). Комментарий утверждает «чтобы rustdoc видел связи», но `use X as _Y;` приватен и
на intra-doc links не влияет: связи видны через `pub use` в `ready.rs`. Это 11 из 31 allow в
проекте — треть «разрешённых» предупреждений ничего не разрешает.

Второй блок: `discord-ui/src/dialog_window.rs` — 11× `#[allow(dead_code)]` без причины, в т.ч.
на `pub fn`/`pub struct` (для `pub` в lib dead_code не срабатывает вовсе).

Минимальный фикс: удалить 392-414; в `dialog_window.rs` снять allow и прогнать clippy — что
действительно мертво, удалить/пометить `#[doc(hidden)]` с причиной.

Тест-оракул: `cargo clippy -p discord-core --all-targets -D warnings` (0), `cargo doc --no-deps`
(нет broken links), `cargo test -p discord-core`. Мутация: вернуть `use` → clippy падает
(unused_imports), т.е. удаление безопасно.

Выигрыш: −22 LOC, allow-политика снова означает «исключение с причиной»; риск нулевой.

### G1-09 — `join_guild` / `enter_guild`: эталонный дубль

`file:line`: `main.rs:243-251` (`join_guild`), `253-262` (`enter_guild`), `guilds.rs:183-188`
(`join_guild` = `enter_guild(...).await`), регистрация `main.rs:541-542`.

Почему плохо: одна операция (`PUT /guilds/{id}/members/@me?lurker=true`) доступна по двум IPC
именам; `invoke_handler` и любой allow-list команд растут; тест `guilds_contract.rs:290`
требует оба имени — т.е. дубль закреплён. При этом `gui-spike/ui/**` **не вызывает ни одного**
из них (`rg join_guild|enter_guild` по `ui/` — 0), значит второй имя сейчас не нужно никому.

Минимальный фикс: оставить `enter_guild` (имя совпадает с доменом и `guilds::enter_guild`),
`join_guild` из `invoke_handler` убрать, в `guilds.rs` удалить alias; если UI когда-нибудь
позовёт `join_guild` — тонкий JS-alias, а не второй command.

Тест-оракул: `guilds_contract.rs` — один тест «регистрируется ровно одно имя» + вызов команды;
диф теста = изменение контракта, требуется PM (TEST-POLICY §5).

Выигрыш: −14 LOC, −1 IPC-поверхность; риск низкий, но правка теста (не ослабление: список
имён меняется осознанно).

### G1-10 — Публичный `etf::encode` паникует (P3, «landmine»)

`file:line`: `discord-codec/src/etf.rs:260,275`; вызовы в проде — `core/src/ws.rs:718-719`
(`encode_heartbeat`/`encode_qos_heartbeat`). Док прямо говорит «для доверенных
значений; для внешних — `encode_checked`».

Почему плохо: API-ловушка: `encode` выглядит как обычный энкодер, но паникует при
binary > u32 / atom > u16 / depth > MAX_DEPTH. Сейчас в `ws` значения ограничены (int
сатурируется, `reasons` — константы), т.е. **достижимость не доказана** (unknown/RE), но
любой будущий вызов с данными из состояния получит panic в сетевом потоке.

Минимальный фикс: не убирать `encode`; пометить `#[doc(hidden)]`/`#[deprecated]` и перевести
`ws` на `encode_checked` с `Result`-веткой (`TransportError::Frame`/новый `Encode`), плюс тест
«heartbeat для `u64::MAX` и максимального `reasons` не паникует».

Тест-оракул: property/unit в `discord-codec/tests/etf_full.rs` (уже есть границы глубины) +
`core/tests/ws_transport.rs` на `encode_*` c крайними значениями.

Выигрыш: снят класс «panic в сетевом потоке»; риск нулевой (тесты кодинга уже есть).

### G1-11 — `random_bytes` глотает сбой `/dev/urandom`

`file:line`: `core/src/rest/identity.rs:45-50` (`let _ = file.read_exact(out)`), док трейта —
«fallback — оставить буфер как есть»; вызывающий — UUID v4 (`identity.rs:224+`).

Почему плохо: при недоступном/недочитываемом urandom `X-Super-Properties.client_uuid` станет
`00000000-0000-4000-8000-000000000000` у всех клиентов, без ошибки и без счётчика. Это не
криптография (не токен), но тихая деградация, которую невозможно заметить в логах.

Минимальный фикс: `random_bytes` → `bool`/`Result`; при `Err` — заполнить из
(time, pid, адрес стека) либо вернуть `None` для uuid и пометить в диагностике. Никаких новых
зависимостей.

Тест-оракул: fake `IdentitySources` с `random_bytes`, который ничего не пишет → uuid либо
не-нулевой (fallback), либо явная ошибка/флаг; мутация: заменить fallback на нули → тест краснеет.

Выигрыш: убирает тихий ноль; риск нулевой.

### G1-12 — `StopReason` → `Closed(4013)` (P3)

`file:line`: `gui-spike/src/net.rs:258-267`, пин: `tests/net_contract.rs:88-92`.
`Stopped(StopReason::ReconnectLimit { .. }) => GatewayEvent::Closed(4013)`.

Почему плохо: 4013 в Discord — «invalid intents», терминальный код. «Исчерпан лимит
реконнектов» — другая причина с другим UX (показать «нет связи, повторите позже»). Сейчас UI
close-коды не читает (`rg 4013 gui-spike/ui` — 0), поэтому это латентный дефект, но он
зафиксирован тестом и переедет в прод как «правда».

Минимальный фикс: `GatewayEvent::Stopped(StopReason)` (типизированно) либо `Closed(4013)`
только для `InvalidClient`, а `ReconnectLimit` → `Error("лимит реконнектов")`. Требует правки
теста-пина (контрактное изменение, PM).

Тест-оракул: `net_contract.rs` — таблица `StopReason → GatewayEvent` для всех 3 вариантов;
мутация: подменить маппинг → тест краснеет.

Выигрыш: корректный UX при исчерпании реконнектов; риск — правка теста.

### G1-13 — `ensure_day` проверяет только форму (P3)

`file:line`: `gui-spike/src/data.rs:1489-1505`.

Симптом: `2026-99-99`, `2026-02-31` проходят проверку (10 символов, дефисы на 4/7, цифры) и
дают пустую выдачу вместо `InvalidArg`. Тест `data_contract.rs:1283` пиняет только
`19.10.2026` → неверный формат.

Минимальный фикс: после формы — `month 1..=12`, `day 1..=days_in_month(year, month)` (без
chrono; 15 строк). Тест-оракул: `2026-13-01`, `2026-02-30` → `InvalidArg`, `2026-02-28` → Ok.

Выигрыш: ранняя типизированная ошибка вместо «пусто»; риск нулевой (новый кейс, существующие
не меняются).

### G1-14 — `unsafe`/FFI: комментарии есть, реестра нет

`file:line`: `discord-store/src/ffi.rs` (24 блока / 26 SAFETY), `discord-platform/src/ffi_secret.rs`
(21/26), `ffi_hunspell.rs` (9/12), `dbus/fd.rs` (8/9); `#[allow(unsafe_code)]` точечно на
модулях (`lib.rs:17,19`, `store/lib.rs:25`) — как требует `docs/rust-client/SECURITY.md` §4.

Что хорошо: SAFETY-комментарии объясняют владение/границы (`ffi.rs:144-149,168-172,213-220`),
ошибки C → `FfiError`, `Drop` закрывает handle, `deny(unsafe_code)` на крейтах.

Чего нет: **traceability**. SECURITY.md §4 — чек-лист, но не таблица «блок → инвариант → тест».
Нельзя ответить, какие из 62 блоков покрыты контрактным тестом error-path, а какие — только
ревью. Это Q-08 из baseline (P1 security) — по факту материала хватает на P2.

Минимальный фикс (без кода, G5): сгенерировать `docs/rust-client/UNSAFE-REGISTRY.md` скриптом
(`rg -n 'SAFETY' -B2`) → колонки `file:line | библиотека | инвариант | тест | owner`, для
пропусков — карточки. Тест-оракул: meta-тест «каждый `unsafe {` имеет SAFETY в пределах N
строк» (уже почти выполняется: 73 SAFETY на 62 блока — проверить адаччи).

## 5. Топ-10 по приоритету

1. **G1-01** (P1) — error-mapping: 4 дубля + 403/404/5xx → `Network`.
2. **G1-02** (P1) — токен 6×, keyring-ошибка → `Network`, `MissingToken`/`Unauthorized` разъехались.
3. **G1-03** (P2) — мьютекс auth через сетевой `/users/@me`.
4. **G1-04** (P2) — `ThreadedStore::call` без таймаута (вечный spinner).
5. **G1-05** (P2) — `reply_thread.rs` не компилируется; 5 бэкапов в `src/`.
6. **G1-06** (P2) — split `data.rs`/`auth.rs`/`ws.rs` по готовым швам.
7. **G1-07** (P3) — 12× clock-unwrap, один `now_ms()`.
8. **G1-09** (P3) — merge `join_guild`/`enter_guild`.
9. **G1-10** (P3) — `etf::encode` panic-API, перевести `ws` на `encode_checked`.
10. **G1-14** (P2) — реестр unsafe-инвариантов (docs, G5).

Backlog без слота: G1-08, G1-11, G1-12, G1-13.

## 6. Три быстрых выигрыша (≤30 мин каждый, без контрактных изменений)

1. **G1-05a**: `git rm` 5 бэкапов в `discord-ui/src/` (−59 КБ, ноль риска).
2. **G1-08**: удалить блок `392-414` в `ready/snapshot.rs` (−22 LOC, −11 allow); clippy/doc/test
   как оракул.
3. **G1-07**: `now_ms()` + 12 замен в `discord-ui` (−30 LOC, −12 panic-точек, только скелет).

## 7. Три рискованных места (не трогать без PM/ADR)

1. **Контракт ошибок REST** (G1-01/G1-02): добавление `Forbidden/NotFound/Storage/MissingToken`
   меняет `kind`, который читает `ui/`; нужен ADR + fallback в JS + правка 4 контрактных тестов.
   Нельзя «заодно» — только отдельной карточкой с red-тестом на новый `kind`.
2. **Конкурентность `AuthBackend`** (G1-03): снятие мьютекса через I/O требует поколения состояния;
   ошибка даст гонку «login перезаписал logout». Обязателен characterization на текущие 39
   `auth_contract` + новый тест на гонку, иначе риск > выгоды.
3. **Split `data.rs`/`auth.rs`** (G1-06): 1814+1539 строк и 60+ `pub`-элементов; тесты
   импортируют по путям. Только перемещение + `pub use`, по одному файлу за карточку, иначе
   «полурефактор» (STRATEGY §11).

## 8. Проверено и признано здоровым (чтобы не «переписывать»)

- `ws.rs`: на данных провода нет `unwrap`; неполный терм → ожидание, рассинхрон → ошибка;
  счётчики не сбрасываются; `elapsed_ms`/`seq` сатурируются; ошибки не глотаются.
- `data.rs`: `NotFound`/`Parse`/`Io`/`InvalidArg` разделены, лимит 64 МиБ по `metadata` до
  чтения, битый кадр не теряется молча, курсор чужой → `InvalidArg`.
- `auth.rs`: токен/пароль/код не попадают в `Debug`/`Display`/`Serialize`, `sanitize` вырезает
  токен из чужих текстов, `wipe_string`/`wipe_bytes`, login 401 не маскируется (идёт через
  `post_once`, а не через `execute`).
- `state/reducer.rs`: `expect` после `contains_key`/`Vacant` — локальные инварианты с сообщением
  (P3-кандидат на `entry`, но не дефект).
- `i18n`: `try_into().expect("8 bytes")` под проверкой `i + 8 <= len`; `markdown` — под
  непустым остатком. Ложных срабатываний нет.
- `discord-core` в целом передаёт `now_ms` параметром (никаких обращений к системным часам в
  логике) — часовых паник в ядре нет.
- FFI: SAFETY на каждом блоке, `deny(unsafe_code)` с точечными allow, ошибки C типизированы.

## 9. Открытые вопросы к PM

1. Можно ли менять `kind` ошибок (аддитивно) в `messages/reactions/search`? Если нет — G1-01/02
   ограничиваются дедупликацией без 403/404, а маскировка остаётся осознанным долгом в `BUGS.md`.
2. `reply_thread.rs`: подключить (`pub mod`) или удалить карточкой? Контракт P3-07 заявлен как
   сделанный — нужно решение о статусе.
3. `join_guild`: подтвердить, что UI-алиас в JS допустим (иначе оставить два command и закрыть
   Q-01 как «не дубль, а два глагола»).

## 10. Артефакты

```
waves/RW-40/QUALITY/G1-AUDIT.md          — этот отчёт
waves/RW-40/PM/qa-g1/clippy-core.log     — clippy discord-core, EXIT=0
waves/RW-40/PM/qa-g1/clippy-gui-spike.log— clippy gui-spike, EXIT=0
waves/RW-40/PM/qa-g1/test-gui-spike.log  — 161 passed / 0 failed, EXIT=0
```
