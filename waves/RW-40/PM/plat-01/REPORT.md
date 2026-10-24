# PLAT-01 / G1-03 — не держать `Mutex<Inner>` через `/users/@me` и keyring I/O

Batch: RW-40, PM/plat-01. Воркер G1 Backend.
Карточка: `waves/RW-40/QUALITY/PLATFORM-AUDIT.md` PLAT-01 (P1), `waves/RW-40/QUALITY/G1-AUDIT.md` G1-03 (P2).
Red-first, без live/сети/окна/commit. Изменены только:

- `rust/gui-spike/src/auth.rs`
- `rust/gui-spike/tests/auth_contract.rs`

Публичный API `AuthBackend`/`AuthError`/`AuthState` не менялся: добавлены только
приватные `store`/`commit`/`generation` и приватный enum `ValidationOutcome`.
Единственная новая строка ошибки использует существующий `AuthError::Internal`.

## 1. Дизайн (минимальный: generation + commit)

Было: один `Mutex<Inner>` c `store` внутри; `status`/`token`/`login`/`restore`/`logout`/
`accept_token` держали guard через `store.load/store/clear` и `validator.validate`
(`/users/@me`). Любой IPC за токеном стоял на этом мьютексе.

Стало — три лока; `inner` **никогда** не удерживается через сеть или keyring I/O:

| лок | что защищает | когда держится |
|---|---|---|
| `inner: Mutex<Inner>` | только `state`, `keyring_checked`, `generation` | короткие чтения/записи состояния |
| `store: Mutex<Box<dyn TokenStore + Send>>` | keyring (`TokenStore` не `Sync`) | только сам вызов `load/store/clear` |
| `commit: Mutex<()>` | «keyring-запись/очистка + применение состояния» как транзакция | коммит login и logout; **не** сеть, **не** чтение keyring в status/restore |

- `generation: u64` растёт в `begin_validation` (старт `login`/`restore`/`accept_token`),
  в `logout` и в `login("")`. Результат сетевой проверки применяется только при
  совпадении поколения; иначе — `Superseded`: состояние/`keyring` более новой операции
  не перезаписываются. `begin_validation`/`logout` берут `commit`, поэтому проверка
  поколения + запись keyring + состояние атомарны относительно друг друга.
- `status()`: быстрый путь без локов, если `keyring_checked`. Иначе keyring читается
  **до** `inner`/`commit`; результат применяется под `commit` только если keyring ещё
  никем не проверен — поздний `stored` не воскрешает logout, а зависший Secret Service
  не задерживает `login`/`logout`.
- `token()`: только `store`, мьютекс состояния не берётся вообще.
- `login`/`accept_token`: `Validating` под коротким локом → `validate` без локов →
  коммит под `commit` (keyring.store + `Ready`). `restore`: `begin_validation` →
  keyring.load (только `store`) → `validate` без локов → применение по поколению.
  `stored` и `keyring_checked` сохраняются на error-path.
- `logout_with_disconnect`: `disconnect()` вызывается **до** локов (порядок
  «disconnect → clear» сохранён), затем `commit` → `clear` → `idle/error`.
- `accept_token` при проигранной гонке возвращает существующий
  `AuthError::Internal { message }` («вход отменён более новой операцией авторизации»),
  чтобы `LoginView::from_outcome` не объявил `Ready` при чужом logout. Новых вариантов
  `AuthError` нет.

## 2. Red (до реализации)

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test auth_contract -- \
  blocked_validation_does_not_block_status_or_token \
  logout_during_blocked_validation_wins_and_token_is_not_persisted \
  status_read_does_not_resurrect_state_after_logout \
  concurrent_logins_commit_only_the_newest_generation
```

```
test result: FAILED. 0 passed; 4 failed; 40 filtered out
---- blocked_validation_does_not_block_status_or_token ----
panicked: status()/token() ждут мьютекс, удерживаемый сетевым login: Timeout
---- logout_during_blocked_validation_wins_and_token_is_not_persisted ----
panicked: logout ждёт сетевую валидацию под мьютексом: Timeout
---- concurrent_logins_commit_only_the_newest_generation ----
panicked: login B вошёл в validate: Timeout
---- status_read_does_not_resurrect_state_after_logout ----
panicked: тест обязан отпустить хранилище
```

Честная оговорка: первые три падения — красный по PLAT-01 (все ждут `Mutex<Inner>`).
Четвёртый кейс на старом коде не воспроизводил потерю состояния (старый код
сериализовал всё одним локом); после починки тестового гейта (хранилище «запирается»
только на первом чтении) он стал регрессионным тестом нового merge-пути. Гонки
детерминированы каналами (`mpsc`), без `sleep`: fake-валидатор/хранилище сигналят о
входе и висят до явного `release`; таймауты — только страховка теста.

## 3. Green

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test auth_contract
test result: ok. 44 passed; 0 failed; 0 ignored   (40 прежних + 4 новых)

cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
gui-spike: 190 passed; 0 failed   (было 186, +4 контрактных PLAT-01)

cd rust && cargo test --offline -p discord-core
discord-core: 574 passed; 0 failed   (без изменений)

cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
Finished (0 warnings)

cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
exit 0
```

Новые тесты (20 прогонов подряд — 20/20 стабильно):

1. `blocked_validation_does_not_block_status_or_token` — `login` висит в валидаторе;
   `status()` и `token()` из другого потока возвращаются < 1 c и видят `Validating` /
   токен; после release login становится `Ready`.
2. `logout_during_blocked_validation_wins_and_token_is_not_persisted` — logout
   завершается, пока `/users/@me` ещё висит; поздний login возвращает `Idle`, токен не
   сохранён, keyring пуст.
3. `status_read_does_not_resurrect_state_after_logout` — `status` читает keyring
   параллельно logout; итог детерминирован: `Idle/stored=false`, keyring пуст.
4. `concurrent_logins_commit_only_the_newest_generation` — два параллельных login:
   коммитится только начавшийся позже, keyring содержит только его токен, проигравший
   видит актуальное `Ready`.

## 4. Файлы и md5

| файл | md5 |
|---|---|
| `rust/gui-spike/src/auth.rs` | `5d5f6794ae5408675c2d9dfc78b59deb` |
| `rust/gui-spike/tests/auth_contract.rs` | `f17a7381ce4a8bf7034ffd3958379644` |

## 5. Остаток / риски

- `commit` удерживается на время keyring-записи/очистки в коммите login и logout: это
  необходимо, чтобы keyring и состояние не разъехались с параллельным logout. Сеть и
  чтение keyring (`status`/`restore`) `commit` не держат.
- SEC-04 (неопределённость операции после таймаута keyring) не входил в карточку и не
  менялся: запрос остаётся в очереди `ThreadedStore`.
- `ThreadedStore::drop` по-прежнему `join`-ит поток без таймаута (SEC-01 остаток).
- UI/`main.rs` не менялись: команды вызывают те же методы; `spawn_blocking` больше не
  выедается чужим сетевым таймаутом.
