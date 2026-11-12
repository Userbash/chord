# Техническое ревью 88A — PR-59 / task-174 (BE-P2-14: NotificationRouter)

- **Ревьюер:** Технический ревьюер, Review 88A (`deepseek/deepseek-v4.1-flash`)
- **Дата прогонов:** 2026-10-31 00:14–00:19 (+05:00), rustc/cargo 1.99.0, rustfmt 1.10.0-stable
- **Ветка:** `feature/task-174-platform-notifications-router`, HEAD `4da329b` (база `unstable` = `38f7b5c`)
- **PR:** PR-59 `feature/task-174-platform-notifications-router -> unstable`
- **PM-приёмка:** `ACCEPT.md`, статус `Ready for Technical Review`

## Вердикт: **APPROVED (Ready for Merge)**

- **P0 = 0**
- **P1 по коду PR = 0**
- **Мутации: 5/5 обязательных убиты; расширенный набор 8/8 убит**
- Тесты: `34/34` в `notifications_contract` + `1/1` lib; полный крейт `126 passed / 12 ignored / 0 failed`
- Clippy с `-D warnings`: чисто

## 1. Границы изменений

`git status --short` — ровно два файла, оба из write-scope ТЗ, ничего лишнего:

```
 M rust/crates/discord-platform/src/notifications.rs
 M rust/crates/discord-platform/tests/notifications_contract.rs
```

`git diff --stat`: `notifications.rs` +293 строки, `notifications_contract.rs` +343/-1. Коммитов воркер не делал (`git log` HEAD = `unstable`), рабочее дерево — источник ревью.

md5 совпадают с §4 `REPORT.md` и не изменились после мутационных прогонов:

| файл | md5 (REPORT.md) | md5 (после ревью) |
| --- | --- | --- |
| `rust/crates/discord-platform/src/notifications.rs` | `aa6bb85544c51f204d54bfd83db26ea3` | `aa6bb85544c51f204d54bfd83db26ea3` |
| `rust/crates/discord-platform/tests/notifications_contract.rs` | `f1d7e0042017de50a3967059d4da3fc8` | `f1d7e0042017de50a3967059d4da3fc8` |

## 2. Прогоны

Обе команды из ТЗ на изолированном таргете `/tmp/opencode/target-review-174`:

```
cargo test   --manifest-path rust/Cargo.toml -p discord-platform --test notifications_contract --lib
  -> unittests src/lib.rs: 1 passed; 0 failed
  -> tests/notifications_contract.rs: 34 passed; 0 failed   (r01..r09 + n01..n16 + e01..e07 + q01..q07)

cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --test notifications_contract --lib -- -D warnings
  -> Finished `dev` profile, 0 warnings, 0 errors
```

Дополнительно весь крейт (`cargo test -p discord-platform`): `autostart 9`, `dbus 23`, `deep_link 17`,
`keyring 0/12 ignored`, `keyring_offline 3`, `notifications 34`, `power 16`, `spellcheck 13`, `tray 10`,
`lib 1` — **126 passed, 0 failed, 12 ignored (skip-aware, внешняя среда)**. Регрессий в соседних
модулях крейта нет; `NotificationEvent` используется только внутри `notifications.rs` и его тестов,
других исчерпывающих `match` по нему в workspace нет (`grep -rn "NotificationEvent"`).

Red-first подтверждён не по отчёту, а по содержимому `red.log` (E0432: отсутствуют
`NotificationRouter`/`NotificationDispatchDecision`/`redact_secrets`) — это ошибка отсутствия
реализации, а не подгонка тестов под код. `green.log` воспроизведён мной независимо (34/34).

## 3. Мутационное тестирование: 8/8 убито

Каждая мутация вносилась в `notifications.rs`, прогонялся `cargo test -p discord-platform --test
notifications_contract --lib`, файл восстанавливался из бэкапа (md5 сверен, `match=True`).

| # | Мутация | Убита тестом | Итог |
| --- | --- | --- | --- |
| M1 | `if false && self.dnd_enabled` — при DND уведомление уходит | `r02_router_suppresses_dnd` | KILLED |
| M2 | `if false && window_focused && active_channel_id == channel` — уведомление для активного сфокусированного канала | `r03_router_suppresses_active_channel_only_when_window_focused` | KILLED |
| M3 | `if false && seen_messages.contains(message_id)` — нет дедупликации | `r05_router_dedup_and_bounded_fifo_eviction` | KILLED |
| M4 | `body: msg.content.clone()` — сырой токен вместо `<redacted>` | `r07_router_redacts_tokens_mfa_base64_bearer` | KILLED |
| M5 | `ActionInvoked => None` — нет `deep_link_url` по действию | `r08_router_register_sent_and_handle_event` | KILLED |
| M6 | `NotificationClosed` не удаляет запись (`get` вместо `remove`) | `r08_router_register_sent_and_handle_event` | KILLED |
| M7 | `if false && self_user_id == author_id` — уведомление о своём сообщении | `r01_router_suppresses_self_author` | KILLED |
| M8 | `if false && !mentions_self && !is_dm` — уведомление без упоминания вне ЛС | `r04_router_suppresses_when_not_mention_and_not_dm` | KILLED |

Обязательные по ТЗ пять мутаций — M1, M2, M3, M4, M5/M6 — убиты все.
Выживших мутантов нет.

## 4. Ручная проверка семантики

1. **Порядок фильтров** (`evaluate`): self-author → DND → активный канал (только при
   `window_focused`) → «не упоминание и не ЛС» → дубликат. Порядок существует раньше дедупликации:
   подавленное сообщение не попадает в `seen_messages`, поэтому после снятия DND оно ещё сможет
   отправиться. Это осознанное поведение, не баг.
2. **Дедупликация:** `HashSet` + `VecDeque`, вытеснение по `dedup_capacity` (по умолчанию 256,
   `with_capacity(0)` → 1). Память ограничена: `seen_messages.len() <= dedup_capacity`.
3. **Deep link:** `discord://-/channels/@me/{channel_id}` для ЛС (в том числе когда `guild_id`
   отсутствует, но `is_dm=false`), `discord://-/channels/{guild_id}/{channel_id}` для гильдий.
   Формат совпадает с каноническим парсером `deep_link.rs` (`/channels/@me/...`, `/channels/{g}/{c}`,
   контракт `deep_link_contract.rs` §100–124) и с наблюдаемым в RE-дампе маршрутом
   `reference-oracle/rest-d02.json:1810-1811` (`https://discord.com/channels/@me/<id>`).
4. **Обработка событий:** `Action`/`ActionInvoked` возвращают зарегистрированный URL без удаления
   (повторный клик по тому же уведомлению работает), `Closed`/`NotificationClosed` удаляют запись и
   возвращают `None`. Неизвестный id → `None` (`r09`).
5. **Красная зона безопасности (`redact_secrets`)** проверена вживую на изолированной копии крейта
   (`/tmp/opencode/redact-probe`, пробник не добавлен в репозиторий):

```
IN : token: mfa.abc123def          OUT: token: <redacted>            OK
IN : Bearer token_secret_value_999999   OUT: Bearer <redacted>      OK
IN : header: Bearer=token_secret_value_999999  OUT: header: Bearer=<redacted>  OK
IN : token:mfa.abc123def           OUT: token:mfa.abc123def         ЛЕК (см. P2-1)
IN : x=mfa.abc123def               OUT: x=mfa.abc123def             ЛЕК (см. P2-1)
IN : https://x/?t=<3-part token>   OUT: без маскирования             ЛЕК (см. P2-1)
IN : some.thing.else               OUT: <redacted>                   ложное срабатывание (P2-1)
IN : www.example.com               OUT: www.example.com              OK
```

## 5. Находки

### P0 — нет

### P1 по коду PR — нет

### P2 (не блокируют merge, рекомендация — отдельными карточками)

**P2-1. `redact_secrets` маскирует только токен, отделённый пробелом.**
Токенизация идёт по пробелам, поэтому `token:mfa.abc123def`, `x=mfa.abc123def` и токен внутри URL
(`...?t=<3-part token>`) доходят до preview без маски. Обратная сторона эвристики — ложное
срабатывание на `some.thing.else`. Контракт из ТЗ (mfa.*, 3-частный base64, `Bearer`) на
нормальных примерах выполняется и покрыт `r07`, но защита неполная. Рекомендация: искать паттерны
по всей строке (regex-free скан по `[A-Za-z0-9_\-]{n,}\.[...]`), а не по whitespace-словам;
добавить тест-кейсы из таблицы выше.

**P2-2. `pending_urls` не ограничен.** Если `NotificationClosed` не придёт (сервер уведомлений
упал, уведомление заменено через `replaces_id`), запись остаётся навсегда. Рекомендация: колпак
размера (например, 64) с вытеснением по LRU или TTL; `pending_urls_count()` уже есть, тест на
границу добавляется тривиально.

**P2-3. Дубли вариантов `NotificationEvent`.** Парсер сигналов (`notifications.rs:583-584`) порождает
только `Action`/`Closed`, а новые `ActionInvoked`/`NotificationClosed` конструируются исключительно
тестами. Функционально безвредно (`handle_event` покрывает оба набора), но это мёртвая часть
публичного API и риск расхождения в будущем. Рекомендация: либо переключить парсер на новые
варианты и пометить старые `#[deprecated]`, либо оставить один набор.

**P2-4. Устаревший модульный doc.** Строка 12 `notifications.rs` утверждает: «DND/группировка/
история — уровень приложения (P6-01/P3), не этого модуля». После PR-59 DND-политика живёт в этом
модуле (`NotificationRouter::set_dnd`). Док противоречит коду; правится одной строкой.

**P2-5. `cargo fmt --check` для изменённых файлов не проходит.** `cargo fmt --manifest-path
rust/Cargo.toml -p discord-platform --check` → 13 diff-блоков: 2 в `notifications.rs`
(`is_three_part_token`, `is_mfa_token` — перенос цепочек), 11 в `notifications_contract.rs`
(однострочные `assert!(matches!(...))`). Полный `--all --check` даёт 20 блоков, из них 7 — в
файлах, которые этот PR не трогал (`discord-core/src/gateway.rs`, `discord-core/tests/rest_retry.rs`,
уже в `unstable`), то есть базлайн `unstable` тоже красный по fmt. Классифицирую как P2 для PR-59
(дрейф rustfmt 1.99 + унаследованный фон), но `scripts/ci.sh:59` первым шагом гоняет именно
`cargo fmt --all -- --check` — CI покраснеет.

## 6. Процессные риски (вне кода PR-59, требуют решения координатора)

**6.1. Общий `CARGO_TARGET_DIR` даёт ложные red/green между рабочими деревьями.**
Первый прогон ровно команды из ТЗ с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target` упал:

```
error[E0432]: unresolved imports `discord_platform::notifications::Event`,
`IncomingMessageNotification`, `NotificationDispatchDecision`, `NotificationRouter`, `redact_secrets`
```

— то есть тест линковался с `libdiscord_platform-e6c27433efc85773.rlib`, собранным в 00:16 **другим
рабочим деревом** из старого `notifications.rs`. Причина: для path-пакетов cargo хэширует только
вид источника, а не абсолютный путь, поэтому одинаковые `name+version` в разных worktree дают один
и тот же unit hash; `.d`-файл хранит относительные пути, mtime-проверка считает артефакт свежим.
Тот же прогон на изолированном таргете — зелёный (34/34). Вывод: `REPORT.md`/`green.log` воркера
достоверны (я воспроизвёл цифры), но любые прогоны на общем таргете могут молча показать чужой
результат. Рекомендация: `CARGO_TARGET_DIR` на worktree (`$PWD/rust/target` или
`/tmp/cargo-target-<task>`) либо `cargo clean -p discord-platform` перед прогоном.

**6.2. fmt-базлайн `unstable` красный** (см. P2-5). Нужен отдельный форматирующий PR по
`cargo fmt --all`, иначе каждый PR будет упираться в первый шаг `ci.sh`.

## 7. Что осталось

- Merge в `unstable` делает координатор/интегратор после PM-одобрения; воркер и ревьюер коммитов не
  делают — `git status` после ревью содержит только два файла PR, md5 совпадают с §4 `REPORT.md`.
- P2-1…P2-5 предлагаю завести карточками в `waves/RW-40/G5/BACKLOG-*`; блокирующих замечаний нет.
- Полный `gate:worker`/batch-гейт не запускался: по ТЗ ревью ограничено targeted-прогоном
  `discord-platform` + clippy; полный гейт — за integration-лейном.
