# SEC-01 — `ThreadedStore::call` без вечного ожидания (G1 Backend)

**Карточка:** `waves/RW-40/QUALITY/PLATFORM-AUDIT.md` SEC-01 (P1) · **Режим:** red-first, headless
(без live/сети/окна/D-Bus) · **Дата:** 2026-10-24

## Сделано (минимальный объём карточки)

Только caller-timeout + текст ошибки; успешные `load`/`store`/`clear`, `AuthError` и UI не менялись.

| Файл | Изменение |
|---|---|
| `rust/gui-spike/src/auth.rs` | `pub const KEYRING_TIMEOUT: Duration = 15 s`; поле `ThreadedStore::timeout`; билдер `with_timeout(Duration)` (короткий таймаут в тестах); `call`: `recv()` → `recv_timeout(timeout)`; `Timeout` → `Err("keyring не ответил за N с/мс")`, `Disconnected` → прежний текст; `timeout_text()` без секретов; доккомментарии `call`/`Drop` |
| `rust/gui-spike/tests/auth_contract.rs` | fake `BlockingStore` (condvar-«зависание» вместо D-Bus) + тест `threaded_store_times_out_instead_of_waiting_forever` |

Сообщение таймаута: `keyring не ответил за 15 с` (для тестовых 50 мс — `… за 50 мс`). Тайм-аут
приходит в `AuthError::Keyring { message }` уже существующим путём (`status`/`token`/`restore`) —
новых типов ошибок не потребовалось.

## Red

1. **red-1 (API ещё нет)** — `auth_contract.rs` c `with_timeout` не компилируется:
   `E0599: no method named with_timeout found for struct ThreadedStore` → `red-1-compile.log`.
2. **red-2 (API есть, `recv()` остался)** — тест виснет: `has been running for over 60 seconds`,
   внешний `timeout 240` убил прогон → `EXIT=124` (`red-2-hang.log`).
3. **Мутация после фикса** — `recv_timeout` → `recv`, `timeout 25` → `EXIT=124`
   (`mutation-no-timeout.log`). Т.е. тест действительно держит инвариант «не ждать вечно».

## Green

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test auth_contract threaded_store
running 3 tests ... 3 passed; 0 failed        (green-targeted.log)
```

`threaded_store_times_out_instead_of_waiting_forever`: ошибка содержит «не ответил», не содержит
токен, `elapsed < 5 s` при таймауте 50 мс; поток отпускается condvar'ом до `Drop`.

## Гейты

| Проверка | Результат |
|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **186 passed, 0 failed** (было 185 + 1 новый; `auth_contract` 40) — `gui-tests.log` |
| `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **574 passed, 0 failed** — `core-tests.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | `EXIT=0` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | `EXIT=0` |

Frontend (228) не запускался и не менялся: правки только в двух файлах выше.

## MD5

```
1c58651d9a432a0f777be4028a96c8da  rust/gui-spike/src/auth.rs
b75e023e54f337bd42a709d3135b8fe2  rust/gui-spike/tests/auth_contract.rs
```

## Остаток (не выдаётся за fix)

1. **`Drop::join` по-прежнему без предела** — `drop(ThreadedStore)` ждёт поток, а тот может стоять
   в libsecret/D-Bus (prompt/заблокированная коллекция). Таймаут в `call` этот `join` не
   ограничивает: закрытие приложения всё ещё может зависнуть. Зафиксировано в доккомментарии
   `Drop`; нужен отдельный шаг (detach + лог либо `join` с пределом) — вне scope SEC-01.
2. **Зависший D-Bus остаётся** — теперь его видит только вызывающий (ошибка вместо вечного
   спиннера), сам поток-владелец и соединение не выздоравливают: после таймаута store может
   остаться непригодным до перезапуска (нет reconnect/health-check).
3. **SEC-04 (P2) стал явным**: после таймаута запрос остался в очереди и может выполниться позже
   (`store`/`clear` — «состояние неизвестно»). Отмены/поколений запросов в этой карточке нет.
4. **Live-сценарий не проверен** — «коллекция заблокирована / prompt» на реальном Secret Service
   требует live-окна; здесь только fake-store.
5. Контракт `TokenStore` отдаёт `String`, отдельного typed-варианта `Timeout` нет — при появлении
   UI-различения (SEC-02, `kind:"storage"`) текст придётся либо парсить, либо заводить variant.
