# RW-40 / PLAT-11a — allowlist `channelId` в `send_message` до I/O (G1 Rust Backend)

- **batch/карточка:** RW-40 / PLAT-11a (P2; `waves/RW-40/QUALITY/PLAT-11-AUDIT.md` §1.2, §3 PLAT-11-B, §4.2 T2).
- **роль:** воркер G1 Backend, единственный владелец `cargo` в batch. PM — DeepSeek.
- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` (ветка рабочего дерева).
- **границы:** тронуты только `rust/gui-spike/src/messages.rs`, `rust/gui-spike/tests/messages_contract.rs`
  и этот каталог. `ui/**`, чужие модули/тесты, golden (`logs/`, `functions/`, `evidence/`),
  общие доки и `waves/*` вне `PM/plat-11a/` не менялись. Live/окно/сеть/спавн/commit не запускались.
- **режим:** batch — targeted-прогоны; полный `gate:worker` остаётся за integration-лейном.

## Сделано

1. **`validate_channel_id`** (`messages.rs`): allowlist `^[A-Za-z0-9_-]{1,64}$` по байтам —
   ASCII-цифры/буквы, `_`, `-`; длина 1..=64 байта. Отвергает `.`, `/`, `\`, `%`, `?`, `#`,
   NUL, CR/LF, пробелы и не-ASCII (в т.ч. multi-byte UTF-8). Снежинки проходят как цифры,
   фикстуры UI (`news`, `c1`) — как безопасный идентификатор. Это ровно формула §4.2 T2,
   а не «только цифры» (иначе ломается `chat.js:432`).
2. **Два места вызова:** free-функция `messages::send_message` (валидация до делегирования
   в `MessageSender`, поэтому fake-sender видит 0 вызовов) и `DiscordMessageSender::send`
   (defense-in-depth на случай прямого вызова трейта). Прежняя ad-hoc проверка
   `is_empty`/`{}`/CRLF заменена общим helper'ом — она была подмножеством allowlist.
3. **Тесты** (`tests/messages_contract.rs`, +3):
   - `send_message_rejects_path_traversal_in_channel_id_before_sender` — все 10 значений из ТЗ
     (`../../users/@me`, `..%2f..%2fusers/@me`, `/etc/passwd`, `10/../../users/@me`, `10?x=1`,
     `10\..\x`, `10\0`, `10\n`, `""`, `"   "`) → `Err(InvalidArg)` и 0 вызовов sender.
   - `send_message_accepts_safe_channel_ids_and_enforces_64_byte_bound` — регресс UI
     (`news`, `c1`, `10`, снежинка, `a_b-C9`), граница 64 байта проходит, 65 байт и не-ASCII
     (`é`) отвергаются.
   - `sender_rejects_path_traversal_channel_id_before_http` — прямой вызов трейта
     `DiscordMessageSender` отвергает id до HTTP; `tokio::time::timeout(500 ms)` держит тест
     офлайн и не даёт уйти в сеть, если защита исчезнет.

## Red (до реализации)

| # | команда | результат | лог |
|---|---|---|---|
| 1 | `cd rust/gui-spike && cargo test --offline --test messages_contract` | **exit 101**, `12 passed; 2 failed`: `send_message_rejects_path_traversal_...` — `left: Ok(MessageReceipt…)`, `right: Err(InvalidArg)` на `id="../../users/@me"`; `..._enforces_64_byte_bound` — 65 `a` прошли как `Ok` | `red.log` |
| 2 | та же команда, только `sender_rejects_path_traversal_channel_id_before_http`, исходник из `git show HEAD:…/messages.rs` (md5 `8da412cf…`) | **exit 101**: тест дошёл до HTTP (`A Tokio 1.x context was found, but IO is disabled`) ⇒ sender-guard отсутствовал | `red.log` (секция «additional red-proof») |

Оракул red-фазы — исходник до правки (`git show HEAD`), а не текущая реализация.

## Green (после реализации)

| проверка | результат | лог |
|---|---|---|
| `cargo test --offline --test messages_contract` | **15 passed, 0 failed**, exit 0 (было 12, +3) | `green.log` |
| `cargo test --offline` (весь gui-spike, доп. контроль) | **294 passed, 0 failed** в 21 сьюте, exit 0 | `suite.log` |
| `cargo fmt --all -- --check` | exit 0 | `fmt.log` |
| `cargo clippy --offline -- -D warnings` | exit 0, без предупреждений | `clippy.log` |

`green.log` содержит три прогона: до mutation-серии, финальный после restore и повторный
после red-proof №2 — все 15/15, exit 0.

## Targeted

`tests/messages_contract.rs` — 15 тестов, из них 3 новых (`send_message_rejects_path_traversal_…`,
`send_message_accepts_safe_channel_ids_…`, `sender_rejects_path_traversal_…_before_http`).

## Оракул

- `PLAT-11-AUDIT.md` §2 п.5 (в id-строках запрещены `/`, `\`, `..`, `%2f`/`%2e`, `?`, `#`,
  NUL, CR/LF) и §4.2 T2 (allowlist `^[A-Za-z0-9_-]{1,64}$`, сохранить `news`/`c1`).
- Существующий UI-контракт: `ui/features/chat.js:432` шлёт `channelId: "news"`; тесты
  `messages_contract.rs:60,82` фиксируют `"c1"`. Новый allowlist эти значения сохраняет.
- Транспорт как контр-оракул: `transport.rs:217-224` пропускает `..`, `/`, `?`, `%`, поэтому
  запрет обязан стоять до `render_path`/`post_once`, а не в транспорте.
- Генерация: не применимо (входы теста — фиксированный список литералов, без генератора).

## Мутации

5/5 **KILLED**, baseline зелёный (15/15). Полный лог — `mutations.log`.

| mutant | изменение | тест, убивший мутанта | вердикт |
|---|---|---|---|
| MUT-1-drop-empty-check | убрано `bytes.is_empty()` | `send_message_rejects_path_traversal_in_channel_id_before_sender` | KILLED |
| MUT-2-drop-64-byte-ceiling | убрано `bytes.len() > MAX_CHANNEL_ID_BYTES` | `send_message_accepts_safe_channel_ids_and_enforces_64_byte_bound` | KILLED |
| MUT-3-allow-slash | в allowlist добавлен `b'/'` | `send_message_rejects_path_traversal_in_channel_id_before_sender` | KILLED |
| MUT-4-drop-fixture-charset | убраны `b'_'`/`b'-'` | `send_message_accepts_safe_channel_ids_and_enforces_64_byte_bound` | KILLED |
| MUT-5-drop-sender-guard | убран `validate_channel_id(channel_id)?` из `DiscordMessageSender::send` | `sender_rejects_path_traversal_channel_id_before_http` | KILLED |

Survivors / INCONCLUSIVE / эквивалентных — нет. Все мутанты компилировались и падали
именно ожидаемым тестом (`test result: FAILED. 14 passed; 1 failed`).

## Baseline / restore

`messages.rs` до правки: `8da412cf1a629ac03636d61c5e58be3d` (= `git show HEAD`).
Baseline реализации (после green): `c449a06c783c795effb33c7fc763614e`.

| mutant | md5 мутанта | md5 после restore | совпал с baseline |
|---|---|---|---|
| MUT-1 | `1ddfefe3201bcd174b99ea9f87fdd0cf` | `c449a06c…` | да |
| MUT-2 | `d226bc0b85ee123ecffb8556a43e7f54` | `c449a06c…` | да |
| MUT-3 | `68f62c044a006e51c3a5f9ab869c21a3` | `c449a06c…` | да |
| MUT-4 | `c97a49b6f6c4ab25204dc2667ee6425d` | `c449a06c…` | да |
| MUT-5 | `07b742f620f775585e502f5781a8e51f` | `c449a06c…` | да |

Restore — копия из `/tmp/opencode/plat11a/messages.rs.orig`, затем `touch` и `md5sum`.
Финальное состояние: `c449a06c…`, зелёный `15/15`, exit 0. Skip'ов в прогонах нет.

## Стабильность

Все тесты детерминированы (литералы, fake-sender, без сети/ФС/времени). Targeted-набор
прогнан 4 раза (red, green, после mutation-restore, после red-proof swap) — 15/15 каждый раз.
Специальные повторы для гонок не требовались. `sender_rejects_…` ограничен таймаутом 500 мс,
поэтому не зависит от доступности сети.

## gate:worker

Воркер полный `gate:worker` не запускает (правило batch): вердикт — за integration-лейном
после сведения патча. Код-ревью — отдельному агенту, автор свой код не ревьюит.

## Файлы

| файл | md5 |
|---|---|
| `rust/gui-spike/src/messages.rs` | `c449a06c783c795effb33c7fc763614e` |
| `rust/gui-spike/tests/messages_contract.rs` | `0f92348a5e3e15563603faec0917c4b5` |
| `waves/RW-40/PM/plat-11a/red.log` | `ae246ea2bde623a115df5813460fea5a` |
| `waves/RW-40/PM/plat-11a/green.log` | `74aafc690c148ef49aa6aa3a9dc2ff57` |
| `waves/RW-40/PM/plat-11a/mutations.log` | `5607e564f5395122ba143a10a5e99423` |
| `waves/RW-40/PM/plat-11a/fmt.log` | `fedf20fbf10036ba2419c70b755b8c39` |
| `waves/RW-40/PM/plat-11a/clippy.log` | `289444aa36e30aba0c2af129f82b9b4e` |
| `waves/RW-40/PM/plat-11a/suite.log` | `7b6252869dee4748461ddd27fa2a3f0b` |
| `waves/RW-40/PM/plat-11a/REPORT.md` | этот файл; md5 после записи меняется — снимается при приёмке |

`git status` по моим путям: `M rust/gui-spike/src/messages.rs`,
`M rust/gui-spike/tests/messages_contract.rs`, `?? waves/RW-40/PM/plat-11a/`. Других
изменений воркер не вносил.

## Риски / вопросы

1. **`nonce` не валидируется** — уходит в JSON-тело, не в путь; вне scope PLAT-11-B.
   Если PM считает `nonce` частью того же класса — отдельная карточка.
2. **Совместимость id.** Allowlist шире снежинки, но уже текущего поведения: каналы с
   точкой/пробелом в id (не встречаются в фикстурах и Discord) теперь получат `InvalidArg`.
   Это и есть цель карточки; `news`/`c1` проверены регрессом.
3. **Дублирование правила.** `validate_ids` (только цифры) для edit/delete/reply и
   `validate_channel_id` (allowlist) для send — намеренно разные: ужесточение остальных
   команд требует миграции UI (PLAT-11-D), в эту карточку не входит.
4. **Смежная неоднородность** (`PLAT-11-D`): `reactions.js` шлёт `"news"` в `toggle_reaction`,
   который требует цифры. Не трогал — отдельная карточка паритета.
5. **`sender_rejects_…` использует `DiscordMessageSender::new`**, который строит identity из
   `SystemSources` (локально, без сети); тест не делает HTTP — при отсутствии guard'а
   падает на отключённом IO в тестовом рантайме (см. red-proof №2).

## Следующий шаг

Передать Team Lead / G4 на приёмку: red/green/mutations/fmt/clippy логи и md5 — в этом
каталоге. После приёмки batch — integration-лейн запускает `gate:worker`; затем PM закрывает
T2 в DoD PLAT-11 (п.3) и решает по waiver'у T3.
