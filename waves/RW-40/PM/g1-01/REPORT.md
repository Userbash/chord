# G1-01 — единый classifier ошибок REST (red-first)

Дата: 2026-10-24. Воркер: G1 Backend. PM: DeepSeek.
Статус: **готово к приёмке**. Live/окно/сеть/commit не использовались.

## Проблема

`map_api_error` и status-декодеры были скопированы в `messages`, `reactions`,
`search`, `guilds`; хвост `_ => X::Network` превращал 403/404/5xx в
«сеть недоступна». `guilds` уже различал 403/404, копии разъехались
(`reactions::decode` давал `InvalidResponse` на 5xx, `messages::decode_action` —
`Network`).

## Сделано

Новый модуль `rust/gui-spike/src/rest_failure.rs` — единственный источник политики:

| Вход | `RestFailure` |
|---|---|
| 2xx | `None` (успех) |
| 401 | `Unauthorized` |
| 403 | `Forbidden` |
| 404 | `NotFound` |
| 429 | `RateLimited { retry_after_ms }` (0 из «сырого» статуса; из `ApiError` — реальная задержка) |
| прочие 4xx | `ClientOther { status }` |
| 5xx | `Server { status }` |
| 1xx/3xx | `Network` (не подтверждённый успех API, как до фикса) |
| транспорт/таймаут | `Network` |

`RestFailure::from_status(u16)` и `RestFailure::from_api_error(&ApiError)`.
Домены конвертируют через `impl From<RestFailure>`.

Изменены доменные enum'ы (аддитивно, существующие kind'ы сохранены):

- `MessageError`: +`Forbidden`, `NotFound`, `Client{status}`, `Server{status}`;
- `ReactionError`: +`Forbidden`, `NotFound`, `Client{status}`, `Server{status}`;
- `SearchError`: +`Forbidden`, `NotFound`, `Client{status}`, `Server{status}`;
- `EnterGuildError`: +`Client{status}`, `Server{status}` (403/404 уже были);
- у всех — `Display` на новые варианты и serde `{kind: camelCase}` (`client`/`server`
  несут `status`).

`messages`, `reactions`, `search`, `guilds` больше не содержат своих мапперов:
`from_api_error` вызывается из одного места, статусы декодируются одним
`RestFailure::from_status`. `guilds::decode` сохраняет контракт G-03a:
204 → `Ok`, иной 2xx → `Parse`.

`429 retry_after = 0` для сырого статуса **не менялся** (тело не разбирается,
задержку не выдумываем); `ApiError::RateLimited` по-прежнему несёт
`retry_after` ядра и сатурируется в `u64::MAX` без паники.

## RED

Тесты написаны до реализации: `waves/RW-40/PM/g1-01/red.log`, exit 101,
19 ошибок компиляции — `E0432/E0433` (`rest_failure` отсутствует) и `E0599`
(нет вариантов `Forbidden`/`NotFound`/`Client`/`Server`), в т.ч.:

```
error[E0432]: unresolved import `crate::rest_failure`
error[E0599]: no variant named `Server` found for enum `messages::MessageError`
```

Старые неверные оракулы заменены **осознанно, с комментарием в тесте** (не
подгонка):

- `search_contract.rs`: было `403/404/500/503 → SearchError::Network`
  (`maps_other_http_errors_to_network...`), стало `Forbidden`/`NotFound`/`Server{status}`;
- `guilds_contract.rs`: было `500/400 → Network`, стало `Server{status}`/`Client{status}`.

## GREEN

| Проверка | Итог | Лог |
|---|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **174 passed**, 0 failed (baseline 164, +10) | `green-gui.log` |
| `cargo test --offline -p discord-core` (из `rust/`) | **574 passed**, 0 failed (без изменений) | `green-core.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | exit 0 | `clippy-gui.log` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | exit 0 | `fmt.log` |

Новые/обновлённые тесты:

- `tests/rest_failure_contract.rs` (6): 401/403/404/429/400/418/500/503/2xx;
  `ApiError` → `RestFailure` (включая `Server`, `Client{403/404}`, `Timeout`,
  `Transport`, `InvalidRequest`), сатурация `retry_after`;
  конверсия во все 4 домена; serde-`kind` + непустой `Display`; сохранность
  legacy kind'ов (`unauthorized`/`rateLimited`/`network`/`invalidResponse`).
- `messages_contract.rs` / `reactions_contract.rs` / `search_contract.rs`:
  новые варианты проходят через публичные контракты, тело ответа не утекает.
- `messages.rs` / `reactions.rs` (`#[cfg(test)]`): wiring приватных декодеров —
  2xx → `Ok`, 401/403/404/429/400/500/503 → типизированные ошибки
  (иначе HTTP-путь этих модулей не проверить без сети).

## LOC и дубли

| Файл | Prod LOC до | Prod LOC после |
|---|---:|---:|
| `src/messages.rs` | 321 | 337 |
| `src/reactions.rs` | 175 | 200 |
| `src/search.rs` | 225 | 250 |
| `src/guilds.rs` | 227 | 252 |
| `src/rest_failure.rs` | — | 111 |
| **Итого prod** | **948** | **1150** |

Убрано копий: **4** `map_api_error` + **5** status-декодеров (messages ×2,
reactions, search inline, guilds) = **9 блоков** → 1 classifier + 4 явных
`From`-адаптера. Prod LOC вырос на 202 строки — это новый модуль с документацией
и аддитивные варианты/`Display`/`From`; политика теперь правится в одном месте,
а не в шести (включая `auth.rs`, см. риски).

## Файлы и md5

```
bb74c43e5b23c3efd6bace2b7cf1c1fa  rust/gui-spike/src/rest_failure.rs   (new)
7edae4d978bd59002038e7cb36ad214e  rust/gui-spike/src/lib.rs
abab904e764576fc93377499d4c80914  rust/gui-spike/src/messages.rs
b5afe518e2f336722a29a2269498aef8  rust/gui-spike/src/reactions.rs
b24db68053f10bdf81b7c3855364af0f  rust/gui-spike/src/search.rs
99713006351c570cc9dd5440a6bd11cd  rust/gui-spike/src/guilds.rs
b0194eb65c59be4fa4f8f3c280c1dd71  rust/gui-spike/tests/messages_contract.rs
dd134924fb6d57e09098de23fc9dd9a0  rust/gui-spike/tests/reactions_contract.rs
93e801a1f2e0c7647ef017be34e1eeee  rust/gui-spike/tests/search_contract.rs
5087dca37b4d18ad55e470fb25c41060  rust/gui-spike/tests/guilds_contract.rs
050e3c03c4265b8ff2987322de735c73  rust/gui-spike/tests/rest_failure_contract.rs (new)
```

## Риски / открытые вопросы

1. **UI не менялся** (по ТЗ), но `ui/features/search-overlay.js` держит закрытый
   список `SEARCH_ERROR_KINDS`: новые `forbidden`/`notFound`/`client`/`server`
   не входят в него, поэтому `isSearchError` вернёт `false` и поиск уйдёт в
   локальный fallback. Нужна отдельная UI-карточка (добавить kind'ы и тексты);
   в Rust-контракте это уже типизировано.
2. `auth.rs` сохраняет **свои** две версии маппинга (строки ~320, ~863) — вне
   FILES этой карточки, относится к G1-02.
3. `ApiError::InvalidRequest/InvalidBase/TelemetryBlocked` остаются `Network`:
   у них нет HTTP-статуса, новый `kind` для них — отдельное контрактное решение.
4. `429` из `RestFailure::from_status` несёт `retry_after_ms = 0` (как до фикса);
   изменение требует подтверждения и разбора заголовков/тела.
5. 1xx/3xx по-прежнему `Network` (успехом API не являются); поведение сохранено.
