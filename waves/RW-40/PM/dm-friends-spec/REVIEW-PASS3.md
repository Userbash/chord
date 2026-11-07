# G3-DM-FRIENDS-SPEC — независимое ревью, Pass 3

**Вердикт: RETURN (P1).** Все 11 P2 и замысел P1 из
`waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW-PASS2.md` закрыты по существу: Home
получил схему персистентности (поле `homeMode` + приём `"@me"` пятью фильтрами),
`state.presences` описан как `BTreeMap<PresenceKey, Presence>` с ключом
`(user_id, PresenceScope::Private)`, таблица контраста §3.3 пересчитана (все 18
значений сошлись с формулой WCAG), snowflake взяты из fixture, `aria-label`
сведены к одной формуле, `#friends-view` и `#message-request-banner` монтируются
с `hidden`, roving `tabindex` при Home задан, `components/dm-friends.css`
подключён, hex из CSS-комментария убран, баннер получил разметку и стили,
`CHANNEL_RECIPIENT_REMOVE` помечен `[A]`.

Приёмку блокирует одно новое место: нормативное правило `normalizeSettings`
(§0.2 п.4) добавляет ключ `homeMode` в **каждый** payload `settings_save` из
диалога настроек, а замороженный оракул `tests/unit/gui-spike-features.test.mjs`
требует там ровно пять ключей и объявлен этой же спецификацией замороженным
(`[F]`). Плюс три P2 в переписанных местах (§4.1.1 и §2.3).

Ревью read-only. Прочитаны `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (2207 строк;
файла `docs/specs/DM-FRIENDS-SPEC.md` в ветке нет — фактический путь указан выше),
`waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW-PASS2.md`,
`rust/gui-spike/src/settings_store.rs`, `rust/gui-spike/ui/app.js`,
`rust/gui-spike/ui/features/{settings,notifications}.js`,
`rust/gui-spike/ui/{tokens.css,style.css,index.html}`,
`rust/crates/discord-core/src/state/{types,presence}.rs`,
`rust/crates/discord-core/src/ready/social.rs`,
`rust/crates/discord-core/src/dispatch.rs`,
`rust/gui-spike/tests/settings_contract.rs`,
`tests/unit/{guild-rail,gui-spike-features}.test.mjs`,
`fixtures/gateway/by-type/002-READY.json`. Изменён только этот файл.
HEAD `4e24048`, ветка `rw40/dm-friends-spec-pass2`.

Классификация: **P0 — 0, P1 — 1, P2 — 3.**

## Сводка по 1 P1 и 11 P2 Pass 2

| # Pass 2 | Результат | Доказательство в Pass 3 |
|---|---|---|
| P1. Персистентность Home | **Закрыт по существу, конфликт с `[F]`** | §0.2 п.4 (`:53-57`): `home_mode: Option<bool>` + приём `"@me"` в `normalize_last_guild_id`, `validate`, `readLastGuildId`, `normalizeSettings`, `getEffectiveLastGuildId`; §2.1.1 п.3 (`:248-253`) задаёт boot-порядок; `T-N107-HOME-NAV-01` п.4–5 (`:2194`) требует round-trip save → load → boot. Конфликт — P1 ниже |
| P2.1 Тип/ключ `state.presences` | **Закрыт по существу, новые ошибки в §4.1.1** | §4.1.1 (`:2070-2085`): `BTreeMap<PresenceKey, Presence>` (`types.rs:118`), `PresenceKey = (u64, PresenceScope)` (`presence.rs:264`), чтение `get(&(user_id, PresenceScope::Private))`; ссылки `is_online` `65-69`, `ActivityType` `78-82`, `:107`, `Presence` `268-278` совпали. Ошибки `kind`/`tokens.css:101` — P2.1 ниже |
| P2.2 Арифметика §3.3 | PASS | Независимый пересчёт всех 18 значений: 5.58, 9.66, 4.61, 6.38, 8.12, 4.85, 6.02, 4.89, 4.54, 5.96, 6.43, 6.82, 13.45, 12.77, 13.28, 7.65, 8.57, 7.63 — все совпали с таблицей (`:2042-2058`); минимум 4.54 отмечен |
| P2.3 Snowflake из fixture | PASS | `guilds[0] = 274093052673392641`, `guilds[6] = 1340572775974305803` (`002-READY.json`); §5.2 п.1 (`:2161-2163`) берёт id из `d.guilds`, литералов нет |
| P2.4 Единая формула `aria-label`, ссылка §2.1.4 | **Закрыт частично** | §2.1.2 п.3 (`:313-317`) и §5.1 (`:2141,2143,2146`) дают одну формулу; ссылок на §2.1.4 в документе 0. Остался рассинхрон в снимке `#friends-tab-pending` — P2.2 ниже |
| P2.5 Начальный `hidden` `#friends-view` | PASS | §2.3 (`:551`) + разметка (`:561-562`); `#message-request-banner` — `hidden` + `aria-hidden="true"` (`:510,518-519`); оракул `T-N107-A11Y-01` п.6 |
| P2.6 Roving `tabindex` при Home | PASS | §2.1.2 п.1 (`:298`): `tabindex="0"` у активной до перехода плитки, иначе у первой; §5.2 п.1 (`:2164`); `T-N107-A11Y-01` п.8 |
| P2.7 Владение монтажом Home-узлов | PASS | §0.2 п.3 (`:52`) перечисляет `#dm-quick-search-btn`, `.dm-nav-list`, `#dm-nav-requests-item`; §2.1.1 п.2 (`:241`) и §2.2 (`:323-326`) задают идемпотентный монтаж |
| P2.8 `<link>` на `components/dm-friends.css` | PASS | §0.2 п.5 (`:57`); оракул `T-N107-TOKENS-01` п.3 (`:2207`) требует наличие `<link>` в `index.html` |
| P2.9 Hex в комментарии CSS | PASS | В блоке §3.2 (842 строки) — 0 `#hex`/`rgb()`/`hsl()` (проверено скриптом), запрет расширен на комментарии (§0.2 п.1 `:36`), оракул п.2 явно включает комментарии |
| P2.10 `#message-request-banner` | PASS | §2.2.1 (`:508-545`): DOM с `role="note"`, `aria-labelledby`, две кнопки; CSS-блок 9 (`:2005-2033`); правила показа/ARIA/действий |
| P2.11 `[A]` в `CHANNEL_RECIPIENT_REMOVE` | PASS | §1.5.2 (`:222`) явно помечает правило выхода `@me` как `[A]` и отделяет его от `[D] @569942` |

## P1 — блокирует приёмку

### 1. `normalizeSettings` с `homeMode` ломает замороженный оракул `gui-spike-features`

§0.2 п.4 (`:54`) предписывает: «Дополнительно `normalizeSettings` пробрасывает
`homeMode` как boolean (не булево значение → `false`, **отсутствие поля →
`false`**)». При буквальном чтении нормализованный объект всегда содержит
`homeMode`, в том числе когда в источнике поля нет.

Путь сохранения диалога настроек: `collectSettings()`
(`features/settings.js:385-393`) собирает объект без `homeMode` и вызывает
`normalizeSettings(...)`; результат уходит в `settings_save`. Замороженный тест
`tests/unit/gui-spike-features.test.mjs` на этом пути проверяет точный набор
ключей:

- `DEFAULTS` без `homeMode` — `:2387`;
- `assert.deepEqual(Object.keys(payload).sort(), ["density", "locale", "theme", "toggles", "version"])` — `:2451-2455`.

После реализации по букве спеки payload получит шестой ключ `homeMode`, и тест
упадёт с сообщением «в файл уходят лишние поля». Спецификация сама объявляет
`tests/unit/gui-spike-*.test.mjs` замороженными (`[F]`, `:30`) и нигде не
оговаривает правку этого оракула. `T-N107-HOME-NAV-01` (`:2194`) проверяет
только сохранение с рейла, где `homeMode` передаётся явно, поэтому конфликт
новым оракулом не ловится.

Это не косметика: либо реализация нарушает спеку, либо gate становится красным
на чужом замороженном тесте. Достаточно одного уточнения:

- **A (предпочтительно).** Определить `normalizeSettings` как «`homeMode`
  попадает в результат только если в источнике есть boolean; при отсутствии
  ключ не добавляется». Режим Home при этом не теряется: в файле остаётся
  `lastGuildId: "@me"`, и boot-правило §2.1.1 п.3 (OR) его восстанавливает.
  Добавить в `T-N107-HOME-NAV-01` проверку сохранения из диалога настроек.
- **B.** Явно объявить правку оракула `gui-spike-features` (добавить `homeMode`
  в белый список) и записать это как изменение теста в `[F]`.

Пока в спеке нет ни A, ни B, P1 Pass 2 нельзя считать закрытым без нарушения
`[F]`.

## P2 — не блокируют вердикт, но должны быть закрыты до заморозки

### 1. §4.1.1: поле `Activity` называется `activity_type`, а не `kind`

`:2085`: «есть хотя бы одна активность с `kind == ActivityType::Streaming`».
В `rust/crates/discord-core/src/state/presence.rs:201-203` поле объявлено как
`pub activity_type: ActivityType`; поля `kind` в структуре нет. Код по букве
спеки не скомпилируется.

### 2. §2.3: `aria-label` у `#friends-tab-pending` расходится с формулой

В снимке разметки (`:607`) стоит `aria-label="Ожидание, 2 входящих запроса"`,
а §2.2 п.1 (`:485`) и §5.1 (`:2146`) требуют
`"Ожидание, 2 входящих запроса в друзья"`. Это тот же класс, что P2.4 Pass 2
(«одна строка на элемент»): G4 не сможет зафиксировать точное доступное имя.
Нужно привести снимок к формуле.

### 3. §4.1.1: неверная ссылка на токен `--status-streaming`

`:2085` указывает `tokens.css:101`, но на 101 объявлен `--danger: #da3e44`;
`--status-streaming: #593695` — на `tokens.css:113`. Ссылка на несуществующее
место в источнике токенов.

## Механические проверки (независимо от текста)

- **Токены.** В блоке §3.2 — 96 уникальных `var(--...)` (340 вхождений), все 96
  объявлены в `tokens.css`; таблица §3.1 содержит ровно те же 96 имён, расхождений
  с §3.2 — 0.
- **Запреты.** В CSS-блоке §3.2 — 0 сырых `#hex`/`rgb()`/`hsl()`, 0 `!important`,
  0 `:has()`.
- **Контраст.** Все 18 значений §3.3 пересчитаны скриптом по WCAG 2.x
  (`(L1+0.05)/(L2+0.05)`) и совпали с точностью до 0.01; минимум — `--text-faint`
  на `--bg-chat` 4.54:1, CTA 5.58:1.
- **READY.** `002-READY.json`: 7 гильдий, 17 `private_channels` (все `type=1`,
  `recipient_ids.length=1`), 60 связей (18 типа 1, 42 типа 2), 146 пользователей,
  `id == user_id` у 60/60 — числа §1.1.3/§1.1.4/§T-N107-READY-01 совпадают.
- **Замороженные тесты рейла.** `tests/unit/guild-rail.test.mjs:223-248`
  (`Home` → `GUILDS[0]`, `End` → последняя, `ArrowUp` с первой → последняя)
  не меняется: `#home-rail-btn` — не `.guild-item` и не `[data-guild]`; семантика
  §5.2 п.1 совпадает с тестом.
- **Rust-контракт `"@me"`.** Существующие тесты
  `rust/gui-spike/tests/settings_contract.rs` не содержат `"@me"` в списках
  невалидных значений (`:594-660`, `:673-690`), поэтому расширение
  `normalize_last_guild_id`/`validate` их не ломает; аддитивное
  `home_mode: Option<bool>` совместимо с `..UiSettings::default()` и
  `skip_serializing_if` (файл без поля читается).

## Замечания (не влияют на вердикт)

- `features/settings.js:275-278` (`setLastGuildId`) — ещё один digits-only гейт,
  не входящий в перечисленные «пять фильтров». В `app.js` он безвреден (значение
  берётся из `getLastGuildId`), но при реализации стоит проверить, что кэш
  консоли не остаётся с прежней числовой гильдией.
- `T-N107-HOME-NAV-01` п.4 требует «Home до `ready()` без мигания». Это
  достижимо только когда `localStorage`/`sessionStorage` доступны: `settings_load`
  асинхронен и приходит после `ready()`. Для файла без хранилища остаётся
  короткий кадр гильдейского чата — стоит оговорить в оракуле.
- `#home-rail-btn` и `.rail-btn[data-screen="chat"]` одновременно могут нести
  `aria-current="page"` внутри одного `#guild-rail[role="navigation"]`; для
  скринридера это два «текущих» пункта. Не блокирует, но лучше развести
  (`aria-current="true"` у кнопки Home, как у плиток).

## Что проверить в Pass 4

1. Уточнить `normalizeSettings`/`homeMode` так, чтобы `gui-spike-features`
   остался зелёным (вариант A), либо явно объявить правку оракула (вариант B).
2. Заменить `kind` на `activity_type` и `tokens.css:101` на `:113`.
3. Привести `aria-label` `#friends-tab-pending` в снимке §2.3 к формуле §2.2 п.1.

Проверка выполнена чтением назначенных исходников и fixture; спецификация, код,
тесты и golden-каталоги не изменялись. Единственный записанный файл — этот отчёт.
