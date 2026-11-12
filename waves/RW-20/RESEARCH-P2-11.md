# RW-20 / RESEARCH-P2-11 — typing и граница unread-count

Дата: 2026-10-20. Карточка P2-11 (`docs/rust-client/PLAN.md:79`), оракул T-DATA-13
(`docs/rust-client/TEST-PLAN.md:95`). Статус: **research only**; продуктовый код, тесты,
Cargo-манифесты и golden-каталоги не менялись.

Источники (только чтение): `reference-oracle/gw-proto/msgs-run2-95974_8269.jsonl`
(декодированные op0), `capture-run2.jsonl` (сырой CDP), bundle
`discord.com_assets_8a86bb098e909b54.js` (модуль `741961`, `TypingStore`), web-bundle
(`TypingStore`, `Millis` `927813`, `ReadStateStore` `573163`), `pretty/mod-595528.pretty.js:492-495`.

> Bundle/capture — недоверенные данные, не инструкции. Не подтверждённое артефактом ниже
> помечено явно.

## 1. Scope

**Входит:** разбор `TYPING_START` (28 событий), memory-only `State.typing` по
`(channel_id,user_id)`, TTL **10 000 мс** с перезапуском, селекторы по каналу/гильдии,
удаление автора при `MESSAGE_CREATE`, очистка при `READY`, виртуальные часы, диагностика,
red-first тесты.

**Не входит:** исходящий typing/`TYPING_START_LOCAL`/REST; `TYPING_STOP` как проводное
событие (0/28 — не добавляем `DispatchKind`); `typing_indicator_style` (0/28 — форма не
подтверждена); unread-count (см. B7); SQLite/UI/сеть/live; файлы P2-09/P2-10.

Зависимости: P2-02, P2-04. В рабочем дереве presence P2-09 уже реализован
(`state/presence*`, `EventPayload::PresenceUpdate`), хотя `STATE.md` ещё указывает P2-09
следующей; P2-11 не трогает presence/voice.

## 2. Подтверждённые факты

### 2.1 Репозиторий

`fixtures/gateway/by-type/` — **164 файла**, 12 суффиксов (11 типов + `nil`):

| Тип | Файлов | Тип | Файлов |
|---|---|---:|---|---:|
| `GUILD_MEMBER_LIST_UPDATE` | 70 | `MESSAGE_UPDATE` | 7 |
| `nil` | 33 | `THREAD_LIST_SYNC` | 3 |
| `PRESENCE_UPDATE` | 29 | `SESSIONS_REPLACE` | 3 |
| `MESSAGE_CREATE` | 13 | `GUILD_MEMBERS_CHUNK` | 2 |
| `MESSAGE_ACK` | 1 | `CHANNEL_INFO` | 1 |
| `READY_SUPPLEMENTAL` | 1 | `READY` | 1 |

`*TYPING*` — **0 файлов**. `grep -ril typing fixtures` даёт только READY-статистику
(`ready-stats.json`, `fixtures/gw-proto/facts-frozen.json`), поле пользователя
`typing_indicator_style` (`002-READY.json`) и REST-маршрут (`rest-static.json`).

`DispatchKind::TypingStart` уже есть (`dispatch.rs:47`, `ALL`/`from_name` `:78`, имя `:109`),
но state-парсер отдаёт `Unsupported` (`state/types.rs:524`) — P2-11 меняет только state-слой.

Существующие тесты **закрепляют** `TYPING_START` как Unsupported: `tests/state_types.rs:262-267`
и `tests/state_reducer.rs:278-283`. Их надо переключить на `MESSAGE_POLL_VOTE_ADD` (есть в
`ALL`, парсера нет). Пустой `d` в позитивном тесте `TYPING_START` нельзя: будет `Missing`.

Часов в `state` нет: `apply`/`apply_dispatch`/`apply_frame` не принимают `now_ms`. Прецедент —
`gateway.rs:5,167` (`now_ms: u64` от вызывающего, без `std::time`).

### 2.2 Capture: 28 START, 0 STOP

`msgs-run2-95974_8269.jsonl`, payload в `term.d`:

| Параметр | Значение |
|---|---|
| START / STOP | **28 / 0** |
| `idx` / `s` | 117…552 / 192…579 |
| ключи `d` | ровно `{guild_id, channel_id, member, timestamp, user_id}` 28/28 |
| типы | guild/channel/user — string 28/28; timestamp — int 28/28 |
| гильдия | `942355007271997450` 28/28 |
| каналы | `1434201417467433001` ×27; `942355562958573628` ×1 |
| пользователи | `367288240207822858` ×27; `1301880740245405707` ×1 |
| дубликат пары | `(1434201417467433001, 367288240207822858)` ×27 |
| timestamp | 1 792 392 992…1 792 394 710 (секунды) |
| `member` | map 28/28; `member.user` map 28/28; `roles` list длины 1 28/28 |
| `member.nick` | атом `"nil"` 28/28 |
| `user_id == member.user.id` | 28/28 |
| `typing_indicator_style` | отсутствует 28/28 |
| размер | 711…764 Б; сумма 19 961 Б |
| self `384299970632876032` | 0 |

Первый sample (idx 117): `channel=942355562958573628`, `user=1301880740245405707`,
`guild=942355007271997450`, `timestamp=1792392992`.

Повторы одной пары идут через **8–13 с**, затем паузы 108/119/327/950 с: клиент повторяет
START до истечения 10 с, а после паузы индикатор успевает истечь. Значит повторный START —
**перезапуск TTL**, не дубликат.

Оракул (read-only):

```bash
python3 - <<'PY'
import json, collections
p='reference-oracle/gw-proto/msgs-run2-95974_8269.jsonl'
t=[json.loads(l)['term']['d'] for l in open(p) if json.loads(l).get('t')=='TYPING_START']
print(len(t), collections.Counter(x['channel_id'] for x in t),
      collections.Counter(x['user_id'] for x in t),
      min(x['timestamp'] for x in t), max(x['timestamp'] for x in t))
PY
```

T-DATA-13 указывает `capture-run2.jsonl`, но в сыром CDP-логе литералов `TYPING_START` нет
(кадры бинарные). Оракул — декодированный `msgs-run2-95974_8269.jsonl`; это надо внести в ТЗ.

### 2.3 Официальный `TypingStore`

Модуль `741961` (дубль в web-bundle у байта 9 377 747). Подтверждено:

- `h = 10 * Millis.SECOND` = **10 000 мс**; `Millis.SECOND=1e3` (модуль `927813`).
- START: `clearTimeout` старого таймера, новый на 10 с; по таймеру dispatch
  **синтетического** `{type:"TYPING_STOP", channelId, userId, guildId}`; карты
  `channel→user→timeout` и `guild→channel→user→timeout`; повтор перезапускает таймер.
- STOP reducer удаляет запись/таймер/guild-карту; `false`, если записи не было; чистит
  per-user `customTypingIndicatorConfig`, если user больше нигде не печатает.
- LOCAL: self, rate-limit `0.8*TTL`=8 000 мс, при >5 печатающих задержка до
  `1.5*TTL`=1 500 мс, затем REST. В P2-11 не входит.
- `MESSAGE_CREATE` удаляет typing автора; optimistic дополнительно сбрасывает local send.
- `CONNECTION_OPEN`/`OVERLAY_INITIALIZE` очищают карты (READY в нашей модели и так даёт
  новый `State`).
- Селекторы: `getTypingUsers(channel)` (frozen `{}`), `getTypingUsersByGuild`,
  `isTyping`, `getCustomTypingIndicatorConfig`. `getTypingUsers` **включает self**;
  потребители (typeahead/avatar/preview) фильтруют self сами.
- Mapper (`mod-595528.pretty.js:492-495`): `member` и `guild_id` optional;
  `member.user` upsert-ится в user/member; `typing_indicator_style` → custom config.

### 2.4 Unread-count: отдельная семантика

`ReadStateStore` (модуль `573163`): `rebuildChannelState` считает unread по загруженным
сообщениям с timestamp > ack, `estimated=true` при неполной истории; `MESSAGE_CREATE`
self-сообщения авто-ack-ает, старый/дублирующийся id не считает, открытый/фокусный канал
авто-ack-ает, иначе `unreadCount++`; mentions — отдельно; `ack` сбрасывает оба счётчика;
`canBeUnread`/`canTrackUnreads` требуют thread/прав/guild opt-in/recent visits.

В READY 157 read-state записей имеют только `badge_count, flags, id, last_acked_id,
last_message_id, last_pin_timestamp, last_viewed, mention_count, read_state_type` —
**`unread_count` нет**. P2-07 поэтому его не выдумал (`waves/RW-17/RESEARCH-P2-07.md:164-165`).
Без UI-фокуса, прав и полноты истории точный счётчик в ядре невозможен.

## 3. Blockers / решения координатора

| ID | Вопрос | Предложение |
|---|---|---|
| B1 | Нет TYPING-фикстуры; оракул только read-only | добавить `tests/data/typing-run2.jsonl` (28 строк, только id/timestamp) с разрешения; `fixtures/by-type` — вариант с PII/манифестом |
| B2 | STOP в проводе 0/28 | не добавлять `DispatchKind`; `remove_typing` — внутренний/UI |
| B3 | DM-форма не подтверждена (все 28 — guild) | `guild_id`/`member` optional; не требовать guild |
| B4 | `typing_indicator_style` 0/28 | не моделировать; при наличии — `ignored` |
| B5 | В state нет часов | `Event.received_at_ms` + `from_frame_at`; `apply_at` отклонён |
| B6 | Official upsert-ит member | в P2-11 только `member.user` → `State.users`; `Guild.members` не трогать |
| B7 | Карточка требует unread, P2-07 отложил | P2-11 = typing; unread — отдельное решение; иначе только `observed_unread` + `estimated` |
| B8 | Нет cap; TTL не чистит map | lazy-фильтр + явный `prune_typing`; cap не изобретать |
| B9 | Тесты закрепили TYPING как Unsupported | переключить на `MESSAGE_POLL_VOTE_ADD`; позитив — валидный `d` |
| B10 | P2-09/P2-10 не приняты, общие `types.rs`/`reducer.rs` | старт после фиксации P2-09; presence/voice не трогать |
| B11 | T-DATA-13 указывает не тот файл | в ТЗ — `msgs-run2-95974_8269.jsonl` + команда |

Не подтверждено: `typing_indicator_style` в DM; START с `member=null`; START для user вне
`State.users`; `guild_id`, не совпадающий с каналом. Synthetic-тесты это покрывают, но
официальной семантикой не объявляется.

## 4. Предлагаемый контракт

### 4.1 Время и типы

```rust
pub const TYPING_TTL_MS: u64 = 10_000;
pub type TypingKey = (u64, u64); // (channel_id, user_id)

pub struct TypingStart {
    pub channel_id: u64,
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub wire_timestamp: Option<i64>, // секунды; только диагностика, не TTL
    pub member_user: Option<User>,   // member.user
}

pub struct TypingEntry {
    pub channel_id: u64,
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub wire_timestamp: Option<i64>,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}
impl TypingEntry {
    pub fn is_active(&self, now_ms: u64) -> bool;   // now_ms < expires_at_ms
    pub fn remaining_ms(&self, now_ms: u64) -> u64; // saturating_sub
}
```

- `Event` получает `received_at_ms: Option<u64>`; `Event::from_frame_at(frame,now_ms)`,
  `Event::with_received_at(now)`, `apply_frame_at(state,frame,now)`.
- `TYPING_START` без часов: `ignored "typing.received_at_ms"`, запись не создаётся.
  `SystemTime`/wire-timestamp для TTL не используем: official стартует TTL в момент dispatch.
- `expires_at_ms = now_ms.saturating_add(TYPING_TTL_MS)`.
- `State.typing: BTreeMap<TypingKey,TypingEntry>`; `from_ready` — пусто; `Debug` — count.
- `EventPayload::TypingStart(TypingStart)`; root re-export типов и константы.

### 4.2 Разбор

- `d` не map → `WrongType { path:"typing", expected:"map" }`.
- `channel_id`/`user_id` обязательны → `Missing("typing.<field>")` / `InvalidId`.
- `guild_id` optional: id → `Some`; иначе `ignored "typing.guild_id"`, `None`.
- `timestamp` int/десятичная строка → `Some`; иначе `ignored "typing.timestamp"`.
- `member`: нет/null → `None`; не map → `ignored "typing.member"`;
  `member.user` нет/не map/`parse_user` fail → `malformed "typing.member.user"`;
  `member.user.id != user_id` → inconsistency
  `typing:<channel>:<user>:member-user:<id>`, `member_user=None` (не отравлять users);
  иначе `Some(User)`. `nick`/`roles`/`avatar` участника в P2-11 не сохраняются (B6).
- `typing_indicator_style` есть → `ignored "typing.typing_indicator_style"`; нет — молча.
- Неизвестные ключи игнорируются как в P2-02.

### 4.3 Reducer и селекторы

- START: `upsert_user(member_user)`, затем `typing.insert(key, entry)` — повтор перезапускает
  TTL, `duplicates` не пишется; `Effects` пусты, исходящего typing нет.
- `MESSAGE_CREATE`: после вставки сообщения удалить `(channel_id, author.id)`; `optimistic`
  в модели нет — удаляем всегда.
- `READY`: новый `State` с пустым `typing`; `RESUMED` не чистит.
- `remove_typing(channel,user)->bool`, `clear_typing()`, `prune_typing(now)->usize`
  (удаляет `!is_active`, вызывается явно).
- Ошибка разбора — `malformed "event:..."`, `last_seq` не откатывается, typing не меняется.

| Селектор | Поведение |
|---|---|
| `typing_entry(ch,user,now)` | `None`, если нет/истёк |
| `is_typing(ch,user,now)` | `now < expires` |
| `typing_users(ch,now)` / `typing_user_ids` | активные, порядок `user_id` |
| `typing_users_by_guild(g,now)` | `guild_id==Some(g)`, порядок `(channel,user)` |
| `typing_channel_count(now)` / `typing_entry_count(now)` | активные каналы/записи |
| `typing_remaining_ms(ch,user,now)` | `None` если нет/истёк, иначе остаток |

`typing_users` **включает self** (как official); self-фильтр — UI. Пользователь может
отсутствовать в `State.users` — запись всё равно возвращается.

### 4.4 Файлы (предложение)

- **W20a**: `src/state/typing.rs`, `types.rs`, `mod.rs`, `lib.rs`, `tests/typing_types.rs`,
  `tests/data/typing-run2.jsonl` (28 строк, минимизированные).
- **W20b**: `src/state/typing_reducer.rs`, `reducer.rs`, `tests/typing_reducer.rs`, правки
  `tests/state_types.rs`/`state_reducer.rs`.
- **W20c**: `src/state/typing_selectors.rs`, `tests/typing_selectors.rs`.
- `mod.rs`/`lib.rs` — только W20a; W20b/c не редактируют экспорты.

### 4.5 Unread — только при отдельном решении (B7)

Не называть официальным `unreadCount`. Если включать: `observed_unread` (count, ack_id,
`estimated`) ; self/старый/дубликат не считаем, remote — `+1`, `estimated=true`;
`MESSAGE_ACK`/`READY` сбрасывают; UI ack-ает видимый канал. Это меньше official
(`canBeUnread`/`canTrackUnreads`/focus отсутствуют), но честно помечено.

## 5. Матрица тестов

Core — **26** (t/r/s), опционально +4 unread (u01–u04) = максимум 30. Новые файлы сначала
compile-red; для behavior-red — парсер без reducer.

### 5.1 `tests/typing_types.rs`

| ID | Сценарий | Оракул |
|---|---|---|
| t01 | Реальный idx 117 | channel/user/guild/timestamp из capture; `member_user.id==user_id`; diagnostics пуст |
| t02 | Все 28 строк `tests/data/typing-run2.jsonl` | 28 событий; key-set 5; 28 guild; каналы 27/1; users 27/1; дубликат 27; ts min/max; style 0; STOP 0 |
| t03 | Synthetic полный `member.user` | username/global_name/bot/public_flags разобраны; diagnostics пуст |
| t04 | Минимальный `d` (только channel/user) | guild/timestamp/member_user = None; diagnostics пуст (DM-форма) |
| t05 | Нет/битые id; `d` не map | Missing/InvalidId/WrongType; без паники |
| t06 | Optional неверных типов (guild list, timestamp bool, member string, member.user string) | `ignored`/`malformed` по путям; событие Ok, обязательные поля целы |
| t07 | Нет `member.user`; затем id mismatch | `malformed "typing.member.user"`; inconsistency, `member_user=None` |
| t08 | style есть/нет; маршрут/часы | есть → `ignored`; нет → пусто; `TypingStart` не Unsupported; `from_frame_at` Some, `from_frame` None |

### 5.2 `tests/typing_reducer.rs`

| ID | Сценарий | Оракул |
|---|---|---|
| r01 | READY + START `(200,300)` at 1 000 000 | entry; started 1 000 000, expires 1 010 000; len 1; effects/diag пусты |
| r02 | Реальный idx 117 after READY at 2 000 000 | entry; user upsert в `State.users`; `Guild.members` не изменён; effects пуст |
| r03 | Повтор пары через 8 000 мс | len 1; expiry = new+10 000; `duplicates` пуст; active на new+9 999, не active на new+10 000 |
| r04 | 28 реальных START, `now=timestamp*1000` | map len 2 (без prune); активна 1 (последняя пара); `duplicates` пуст |
| r05 | Два entry; MESSAGE_CREATE автора первого | первый удалён, второй остался; сообщение в state; effects пуст |
| r06 | START, затем READY | typing пуст; last_seq сброшен; effects пуст |
| r07 | START через `from_frame` (без часов) | entry нет; `ignored` содержит `typing.received_at_ms`; last_seq применён |
| r08 | START s=2, повтор s=2, затем s=3 | stale не меняет expiry; s=3 перезапускает; `stale_sequences` содержит 2 |
| r09 | Битый START (нет channel) через `apply_frame_at` | `malformed` содержит `event:`/`typing.channel_id`; typing пуст; last_seq не откатился |

### 5.3 `tests/typing_selectors.rs`

| ID | Сценарий | Оракул |
|---|---|---|
| s01 | Пустое состояние | пусто/false/0/None; без паники |
| s02 | Entry 1 000 000; 1 009 999 / 1 010 000 / 1 010 001 | active только до expiry; на 1 010 000 `is_typing=false`, `typing_entry=None` |
| s03 | Два user в канале (300, затем 200) | users/ids — `[200,300]`; entry_count=2 |
| s04 | 3 канала, 2 гильдии + private | guild 100 → `[(10,1),(10,2)]`; guild 200 → `[(11,1)]`; None не попадает; channel_count=3, entries=4 |
| s05 | remaining at start/+4 000/expiry | 10 000 / 6 000 / None; без underflow |
| s06 | prune: одна истёкшая + одна активная | вернул 1; истёкшая удалена, активная осталась; повторно 0; diag не меняется |
| s07 | 28 реальных событий, `now=timestamp*1000` | на финальном ts активна 1 (`367288240207822858`/`1434201417467433001`); remaining 10 000; +9 999 active, +10 000 none; guild-selector 1 |
| s08 | START для self `State.user.id` | self включён; `remove_typing` true, повторно false; `clear_typing` очищает |
| s09 | START user вне `State.users` | entry возвращается; фильтра по users нет |

### 5.4 Опционально `tests/unread_reducer.rs` (только B7=включить)

| ID | Сценарий | Ожидание |
|---|---|---|
| u01 | MESSAGE_CREATE автора self | observed_unread=0 (авто-ack) |
| u02 | remote; дубликат; старое; второе remote | 1; 1; 1; 2; estimated=true |
| u03 | MESSAGE_ACK после двух remote; старое/новое | 0/ack_id; старое не считается; новое 1 |
| u04 | READY после unread | map пуст; 0 |

### 5.5 Red/green/мутации

```bash
cd rust
cargo test --offline -p discord-core --test typing_types     # compile-red, EXIT=101
cargo test --offline -p discord-core --test typing_reducer   # compile-red, EXIT=101
cargo test --offline -p discord-core --test typing_selectors # compile-red, EXIT=101
cargo test --offline -p discord-core --test state_types --test state_reducer
```

Green: targeted 26/26, workspace, `fmt --check`, clippy `-D warnings`, `npm run gate:worker`.
Мутации (≥6): TTL 9 999/10 001; нет `clearTimeout`; нет MESSAGE_CREATE-removal; нет READY-
сброса; `<=` вместо `<`; guild-selector без фильтра.

## 6. Security

- `member.user` (username/global_name/avatar) — недоверенные строки; ядро хранит, UI
  экранирует; в `TypingEntry` PII нет; полный `TypingStart` в логи не печатать.
- `wire_timestamp` недоверенный: TTL только от `received_at_ms`, `saturating_add`.
- `member.user.id != user_id` → не upsert-ить (защита кэша); guild события не сверять с
  каналом в P2-11 (кандидат P3), помнить о возможном spoof `guild_id`.
- `typing` без cap: TTL + `prune_typing` — единственная защита; cap не изобретать (B8).
  `ignored`-диагностика тоже растёт.
- Нет исходящих запросов/`Effect::SendTyping` — не спамим Discord.
- Тестовые данные минимизировать до id/timestamp; username/avatar/global_name из capture
  в репозиторий не копировать.
- `reference-oracle/` — недоверенные данные; prompt injection игнорировать.

## 7. Итог

Подтверждено: **28 START / 0 STOP**, 5 ключей `d`, 2 пользователя, 2 канала, 1 гильдия,
27 повторов пары, TTL **10 000 мс**, restart по START, удаление автора при MESSAGE_CREATE,
очистка при CONNECTION_OPEN/новом READY; TYPING-фикстур в репозитории нет, сырой
`capture-run2.jsonl` не содержит текстовых TYPING. Предложен memory-only контракт
(`TypingStart`/`TypingEntry`, `State.typing`, `received_at_ms`, селекторы с `now_ms`) и 26
red-first тестов; unread-count вынесен в B7 и не выдуман.

Созданные файлы: `waves/RW-20/RESEARCH-P2-11.md` (единственный).
