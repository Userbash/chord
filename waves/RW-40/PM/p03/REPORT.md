# P-03 — backend `POST /channels/{id}/typing` (D-108)

Воркер: G1 Backend. Дата: 2026-10-24. Статус: готово к приёмке.
Live/сеть/окно/commit не запускались. UI не менялся.

## Решение по маршруту (D-108)

`/typing` отсутствует в frozen D-02 (47 live-строк). Чтобы не размывать оракул D-02 и не
править чужие тесты каталога (`rest_routes.rs`, 574 core-теста остаются без изменений),
маршрут добавлен в `core/src/rest/routes.rs` как явное `[S]`-расширение:

- `pub const TYPING_PATH = "/channels/{id}/typing"`;
- `static TYPING_ROUTE: RouteSpec { method: Post, body_keys: [] }`;
- `find()` резолвит frozen `ROUTES` **или** это расширение;
- `ROUTES` остаётся 49 (D-02 + POST сообщения), `rest_routes.rs` не тронут.

Новый `tests/typing_contract.rs` доказывает отдельность оракула: `/typing` отсутствует в
`tests/data/rest-routes-47.tsv` (D-02) и в `ROUTES`, но резолвится `find()`.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/typing.rs` (new) | `e62d22fedf30877efc9e96ac791879c2` |
| `rust/gui-spike/tests/typing_contract.rs` (new) | `4ab8e1b5494f730b0026361dd3047193` |
| `rust/gui-spike/src/lib.rs` | `c3bad55bde14bf7f57b1ec79ee5f972d` |
| `rust/gui-spike/src/main.rs` | `0ba38d3b8e5880797a60a9ee57ab6937` |
| `rust/crates/discord-core/src/rest/routes.rs` | `286f40409208c537497dd9d20355a7e9` |

## Red (strict TDD)

`waves/RW-40/PM/p03/red-gui.log`:

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test typing_contract
error[E0432]: unresolved import `discord_gui_spike::typing`
error[E0425]: cannot find value `TYPING_PATH` in module `routes`
error: could not compile `discord-gui-spike` (test "typing_contract") due to 2 previous errors
```

## Green

| Команда | Результат |
|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test typing_contract` | **13 passed**, 0 failed (`green-typing.log`) |
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **217 passed** = 202 прежних + 13 контрактных + 2 unit в `typing.rs`, 0 failed (`green-gui.log`) |
| `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **574 passed**, 0 failed — регрессий нет (`green-core.log`) |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | чисто (`clippy-gui.log`) |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | чисто (`fmt-gui.log`) |
| доп.: `cargo clippy -p discord-core --all-targets -- -D warnings`, `cargo fmt -p discord-core -- --check` | чисто (`clippy-core.log`, `fmt-core.log`) |

## Покрытие контракта

- POST без тела на `/channels/{id}/typing`; fake-транспорт фиксирует target и body (body пуст,
  без query); ровно один запрос.
- 200 с пустым/whitespace-телом → `Ok(None)` (не падаем); 200 с JSON — оба поля, частичный,
  `{}`, неизвестные ключи; wire snake_case → IPC camelCase
  (`messageSendCooldownMs`/`threadCreateCooldownMs`); поля не сериализуются, если `None`.
- 200 с не-объектом (`[]`, `null`, строка) и невалидным типом поля → `InvalidResponse`
  (serde не принимает массив за позиционную структуру); 201/204 → `InvalidResponse`
  (контракт `[S]` подтверждает только 200).
- 401/403/404/429/400/500/503 → `Unauthorized`/`Forbidden`/`NotFound`/`RateLimited{0}`/
  `Client{400}`/`Server{500}`/`Server{503}`; транспортная ошибка → `Network`.
- Валидация channel id (снежинка, ≤ u64) до сети — и в свободной функции, и в trait-методе.
- Нет/пустой токен → `Unauthorized`; ключинг через `token_or` в Tauri-команде, ровно одно
  чтение `backend.token()`, keyring-сбой → `Storage`, токен не принимается из фронтенда,
  `println!/eprintln!` в команде отсутствуют; `Debug` клиента не раскрывает токен.
- Контракт каталога: `find(Post, "/channels/{id}/typing")` → `body_keys == []`,
  `render_path` → реальный путь, `/typing` нет в D-02 TSV и в `ROUTES`.

## Остаток / на приёмку

1. **Форма каталога.** Маршрут зарегистрирован как `[S]`-расширение, а не строкой в `ROUTES`
   (сохранён оракул D-02 и 574 core-теста). Если PM хочет typing внутри `ROUTES` — потребуется
   правка 3 мест в `rust/crates/discord-core/tests/rest_routes.rs` (len 49→50, исключения в
   `catalog_matches_live_capture` и `method_distribution_matches_capture`); не делал без
   разрешения, т.к. этого файла нет в матрице.
2. `TypingError::Storage` — аддитивный внутренний код (нужен `token_or`, SEC-02), не догадка
   о контракте Discord; остальные варианты — ровно из задания.
3. Пустое тело ответа Tauri отдаёт как `null` (JSON-представление `void`); UI должен проверять
   falsy. Объект с кулдаунами — camelCase по G6 §3.6.
4. TTL/троттлинг (`TYPING_RESEND_MS`, ≤5 каналов), индикатор «Печатает…» — вне scope backend
   (UI/G2), не реализовывались.
5. Политика повторов клиента — `max_attempts: 1` (typing fire-and-forget, UI троттлит сам).
