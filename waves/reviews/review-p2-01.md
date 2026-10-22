# Независимое ревью P2-01 — типизированный READY (RW-12)

**Ревьюер:** независимый субагент (не автор кода). Режим: только чтение исходников, тестов,
логов, ТЗ и артефактов; правок кода/тестов/доков нет. Записан только этот отчёт.
Живой Discord и сеть не трогал; `cargo` писал только в `target/`.

**Снимок:** рабочее дерево dirty (RW-10/RW-11 + RW-12), коммитов нет. Проверяемые md5
стабильны в течение ревью.

**Область:** `waves/RW-12/DESIGN-P2-01.md` (A1 §2–§3, §5), `waves/RW-12/TZ-P2-01.md`,
отчёты `w12a.md`, `w12b.md`, `w12c.md`, red/green-логи и red-копии тестов;
`rust/crates/discord-core/src/ready/*`, `tests/ready_*.rs`, `tests/support/ready_fixture.rs`.

## 1. Итоговый вердикт

**Не принято.** Блокирует **P1-1**: на реальном READY строковые поля, закодированные атомами
(`data_mode`, `session_type`, `sessions[].status`, `client_info.client`, `client_info.os`),
теряются (`None`), потому что `value::text` принимает только `EtfValue::Str`. Зелёные тесты
этого не видят: JSON→ETF адаптер превращает только `"nil"`/`"true"`/`"false"` в `Atom`, а
`"normal"`/`"full"`/`"invisible"`/`"desktop"`/`"linux"` оставляет `Str`.

P0 нет. P2-1…P2-3 не блокируют вердикт отдельно, но должны быть закрыты или явно отложены
координатором; P3-1…P3-3 — ниты/контрактные неоднозначности.

После исправления P1-1 и повторного red/green приёмка возможна; P2-2 требует решения по
замороженной сигнатуре `parse_user` (см. ниже).

## 2. Находки

| ID | Severity | Файл:строка | Суть | Доказательство (кратко) | Предлагаемый фикс |
|---|---|---|---|---|---|
| **P1-1** | **P1** | `src/ready/value.rs:41-48`; `src/ready/snapshot.rs:260,307-318`; `src/ready/guild.rs:315,516-533`; `src/ready/social.rs:321-322,335` | Атом-строки не считаются текстом: `text()` принимает только `Str`; на реальном проводе `d.session_type="normal"`, `d.guilds[].data_mode="full"`, `sessions[0].status="invisible"`, `client_info.client="desktop"`, `client_info.os="linux"` — это `Atom`. Поля молча становятся `None`; часть попадает в `ignored`, часть (sessions) — без диагностики. | Прямой разбор сырого потока: `d.session_type: atom 'normal'`; `data_mode: atom 'full'` (7/7); `status: atom 'invisible'`; `client_info.client: atom 'desktop'`; `os: atom 'linux'`. Пробник: `data_mode=None session_type=None`, `status=None client=None os=None`; `ignored=["guilds[0].data_mode","d.session_type",…]`. | Ввести нормализатор текста, принимающий `Atom`, кроме зарезервированных `nil`/`true`/`false` (bool/nil обрабатываются раньше); использовать в `optional_text`/`text_field`/`opt_text`/`session.status`/`client_info`. Добавить синтетические тесты с `EtfValue::Atom` для всех 5 полей; при возможности расширить тестовый адаптер по белому списку атомов из `gateway-schema.md` §1.4. |
| **P2-1** | P2 | `src/ready/snapshot.rs:148-163` | `Ready::from_dispatch` при наличии `d`, но не-map, возвращает `WrongType{path:"d"}`, тогда как DESIGN §2.1 требует `NotReady` (Missing — только если `d` нет). | `from_dispatch(d=5): Err(WrongType { path: "d", expected: "map", found: "int" })`. | В `from_dispatch` проверить `value.get("d")`: нет → `Missing{"d"}`; не map → `NotReady`; затем `parse`. `Ready::parse` оставить фатальным `WrongType`. Добавить кейс в A14. |
| **P2-2** | P2 | `src/ready/social.rs:151-163,198-226,263-281,319-342,373-395` | Неверные типы необязательных полей вложенных объектов не пишутся в `ignored`: `parse_user`, `parse_private_channel`, `parse_relationship`, `parse_client_info`, `parse_session`, `parse_member` используют `and_then(...).unwrap_or(default)`/`filter_map`. Нарушение §2.3 «все поля вложенных объектов … неверный тип → дефолт + ignored». | Пробник с `private_channels[0].flags="oops"` и `merged_members[0][0].flags="oops"`: `diag=ReadyDiagnostics { unknown_top_level: [], malformed: [], duplicates: [], ignored: [] }`. Отчёт W12b §8.3 признаёт только `recipient_ids`/`roles`, но фактически молча теряются и скаляры. | Протянуть `&mut ReadyDiagnostics` в парсеры (нужно разрешение координатора на изменение замороженных сигнатур) либо проверять поля в коллекционных циклах; для каждого поля — `default + ignored`, для обязательных — `malformed`-skip. Добавить синтетические тесты на скаляры и элементы `roles`/`recipient_ids`. |
| **P2-3** | P2 | `src/ready/snapshot.rs:241-254`; `src/ready/guild.rs:173-188`; `src/ready/social.rs:405-452` | Привязка `merged_members[i]` к `guilds[i]` идёт по индексу **уже отфильтрованного** `Vec<Guild>`. Если гильдия пропущена (`malformed`) или схлопнута как дубликат, группы сдвигаются и участники попадают в чужую гильдию. | Синтетика: raw guilds `[bad, id2]`, groups `[user100, user200]` → `guilds=[(2,[100])]`, `malformed=["guilds[0]","merged_members:2 vs guilds:1"]`; группа 100 принадлежала пропущенной guild 0. | Сохранять raw-индекс гильдии (в `Guild` или параллельном `Vec<Option<Guild>>`) и прикреплять группу только при совпадении raw-индексов; для пропущенной гильдии группу пропускать/помечать. Аналогично определить поведение при дубликатах. Добавить тест с пропущенной guild + 2 группами. |
| **P3-1** | P3 | `src/ready/snapshot.rs:241-254` | Явный `merged_members: []` при непустых `guilds` не даёт `malformed`, потому что весь блок проверки под `if !merged_members.is_empty()`. §3.5 шаг 9 говорит «несовпадение длин → malformed»; §2.3 одновременно называет `[]` эквивалентом пустого `Vec` — нужна фиксация трактовки. | 2 гильдии + `merged_members=[]` → `diag=ReadyDiagnostics { … all empty … }`. | Различать отсутствие/`nil` и явный `[]` либо явно записать в контракте, что `[]` = отсутствие; добавить B7-вариант с пустым списком. |
| **P3-2** | P3 | `src/ready/guild.rs:207-209,218-219,229-230,529,549,569,608,625` | `ignored`-пути вложенных полей гильдий идут без префикса `d.` (`guilds[1].channels[0].position`), тогда как пример DESIGN §3.3 — `d.users`/`d.read_state.version`, а W12c пишет `d.read_state.partial`. Диагностики несогласованы между W12a/W12b/W12c. | A8 проверяет `ignored == ["guilds[1].channels[0].position"]`; C7/C8 — `d.read_state.entries`, `d.read_state.partial`. | Для всех `ignored` использовать путь от `d` (`d.guilds[1].channels[0].position`), обновить A8 и red/green-логи; `malformed`/`duplicates` оставить в форме провода. |
| **P3-3** | P3 | `src/ready.rs:17`; `src/ready/value.rs:30-149` | Модуль `value` и его помощники объявлены `pub`, хотя замороженные сигнатуры §3.4 — `pub(crate)`. Лишняя публичная поверхность API. | `ready.rs:17: pub mod value;`, `value.rs:31: pub fn get…`; DESIGN §3.4: `pub(crate) fn get…`. | Сделать `pub(crate) mod value` и/или `pub(crate) fn`; на тесты не влияет. |

## 3. Доказательства

### 3.1. P1-1 — сырой провод, а не JSON-фикстура

Независимый разбор (временный ETF-walker, без записи в репозиторий) потока
`/home/sanya/re-deep/gw-proto/zstd-frames-95974_1389.hexl`:

```text
decompressed 824816 B, 137 terms, READY found
d.session_type: tag=atom val='normal'
d.guilds[0].data_mode: tag=atom val='full'
d.guilds[1].data_mode: tag=atom val='full'   (и так все 7)
d.sessions[0].status: tag=atom val='invisible'
d.sessions[0].client_info.client: tag=atom val='desktop'
d.sessions[0].client_info.os: tag=atom val='linux'
```

Это согласуется с `docs/re/gateway-schema.md:64,76`: ASCII-строки кодируются `0x64 atom`;
в словаре атомов есть `full`×7, `normal`×1, `invisible`×2, `desktop`, `linux`.
`session_id` и `user.username` в READY — `0x6d binary`, поэтому они не затронуты.

Пробник на path-зависимостях (`/tmp/opencode/ready-probe`, офлайн, без записи в репо):

```text
$ cargo run --offline -q
atom strings: ok
  data_mode=None session_type=None country=None subdivision=None
  session status=None client=None os=None
  ignored=["guilds[0].data_mode", "d.session_type", "d.country_code", "d.subdivision_code"]
```

Примечание: в пробнике `country_code`/`subdivision_code` тоже сделаны атомами для
демонстрации; в реальном READY они `0x6d binary` и парсером принимаются. Реальная потеря —
**11 значений**: 7×`data_mode` + `session_type` + `status` + `client` + `os`.

Тест B5 (`ready_social.rs`) проверяет `status=invisible`, `client=desktop`, `os=linux`, но
благодаря адаптеру получает `Str` и зеленеет. На реальном проводе B5-контракт не выполняется.

### 3.2. P2-1 — конверт `from_dispatch`

```text
$ cargo run --offline -q   # тот же пробник
from_dispatch(d=5): Err(WrongType { path: "d", expected: "map", found: "int" })
```

DESIGN §2.1 (строки 80–82): `d` — map, иначе `NotReady`; `Missing { path:"d" }` — только
если `d` нет. A14 не покрывает этот случай.

### 3.3. P2-2 — молчаливые дефолты в `social.rs`

```text
nested wrong types: diag=ReadyDiagnostics {
  unknown_top_level: [], malformed: [], duplicates: [], ignored: []
}
```

Синтетика: `private_channels[0].flags = Str("oops")`,
`merged_members[0][0].flags = Str("oops")`. Оба поля заменились на `0`, диагностик нет.
Аналогично ведут себя `channel_type`, `recipient_ids`, `status`, `client_info`, `bot`,
`roles` и др. — см. перечисленные строки в таблице.

### 3.4. P2-3 — сдвиг `merged_members` после `malformed`-гильдии

```text
skipped guild alignment: guilds=[(2, [100])]
  malformed=["guilds[0]", "merged_members:2 vs guilds:1"]
```

Сырые гильдии: `[bad-id, id=2]`, группы: `[[user100], [user200]]`. Гильдия 0 пропущена,
после чего группа 0 (user100) прикреплена к единственной оставшейся гильдии 2. Это неверная
привязка, а не только диагностика.

### 3.5. P3-1 — явный пустой `merged_members`

```text
empty merged_members: diag=ReadyDiagnostics {
  unknown_top_level: [], malformed: [], duplicates: [], ignored: []
}
```

2 гильдии + `merged_members=[]`; `malformed` пуст из-за guard `if !merged_members.is_empty()`.

### 3.6. P3-2 — пути `ignored`

```text
A8: ignored = ["guilds[1].channels[0].position"]        # W12a
C7: ignored = ["d.read_state.entries", "d.read_state.version", "d.user_guild_settings"]
C8: ignored = ["d.read_state.partial", "d.user_guild_settings.partial"]
DESIGN §3.3: ignored = ["d.users", "d.read_state.version", …]
```

### 3.7. P3-3 — публичность `value`

```text
ready.rs:17: pub mod value;
value.rs:31: pub fn get…
DESIGN §3.4: pub(crate) fn get…
```

## 4. Что проверено

- Контракт A1 §2.1–§2.4, §3.3–§3.6, §5.1–§5.3; ТЗ P2-01 P-1…P-6.
- Независимый jq-оракул по `fixtures/gateway/by-type/002-READY.json`
  (md5 `a1a8fd5662be88108dd25ac619a377f2`): `top=40, guilds=7, channels=272, roles=132,
  emojis=363, stickers=51, private=17, users=146, rels=60, read_state=157/128949/false,
  UGS=58/734/false, overrides=1328, sessions=1, merged=7×1, user_keys=32`; крупная гильдия
  83/34/46/6, `member_count=24228`, `premium=5`, `version=1792391252875`; прочие числа
  (no-bot 41, display_name 44+5 nil, relationships 1:18/2:42, override no-flags 75,
  mute obj 38, окна -1/3600/86400 и т.д.) — совпали с тестами и отчётами.
- Точечные значения тестов (private channel 696738350706458685, nil message 1346114403149549638,
  relationship/session/merged_members[5]/read_state[0], DM UGS version 666/3 overrides,
  channel/role/emoji/sticker детали) — независимо сверены с фикстурой.
- `cd rust && cargo test --offline -p discord-core` — зелёный; `cargo test --offline
  --workspace` — зелёный. Целевые наборы: `ready_core` 14, `ready_social` 13,
  `ready_read_state` 11, `ready_fixture` 3, legacy `ready_data` 9.
- `cargo fmt --check` → exit 0; `cargo clippy --offline -p discord-core --all-targets
  -- -D warnings` → exit 0.
- Паники на битых входах: проверены non-map `read_state.entries`, `channel_overrides`,
  `private_channels`, `merged_members`; паник нет, пути уходят в `malformed`.
- Debug: `Ready`/`Session` не печатают `session_id`, `user_settings_proto`, `raw`; `User`
  не содержит `email`/`phone`; `raw()` задокументирован. A13/B5 зелёные.
- `raw` сохраняет полный `d`; top-level unknown/reporting и last-wins dedup — по контракту.
- Границы файлов: worker-файлы изменены в 13:41–13:44; общие `ready.rs`/`error.rs`/`value.rs`
  — в 13:12–13:15, `tests/support/*` — 13:21; `Cargo.toml`/`Cargo.lock` — 11:39–11:40
  (изменения RW-10, не P2-01). Пересечений по mtime/отчётам/md5 не видно.
- Red-first: red-копии побайтно равны финальным тестам (`diff` пуст):
  W12a `7d22f52eaec229789ffa36300fe4e305`, W12b `765a2ba909596750c2f95e3ba4c5e9b2`,
  W12c `4411a18c1fe7b05a5cc5970b9c563c8f`. Red-логи: W12a 4 passed/10 failed,
  W12b 1/12, W12c 1/10, все `# EXIT=101`. Green-логи — `0 failed`, workspace — `EXIT=0`.
  Исправление оракула B1 (44 без ключа + 5 `nil` = 49 `None`) — законная сверка с jq,
  а не подгонка под код; повторный red против замороженной заглушки описан в отчёте.
  W12c rustfmt-only расхождение red-копии задокументировано.

## 5. Что не проверялось / вне scope

- Живой Discord, attach/walk, сеть, реальный gateway-run — запрещены и не выполнялись.
- Интеграция `Ready` в `GatewayPipe`/P2-02, редьюсер, SQLite, REST, UI, protobuf,
  presence/voice, `READY_SUPPLEMENTAL` — вне P2-01.
- Legacy `ReadySummary` (RW-05): его `Debug` по-прежнему печатает `session_id`; это
  зафиксированный риск #15 DESIGN и вне P2-01, в находки не включён. `Ready`/`Session`
  P2-01 в порядке; `resume_gateway_url` в реальном захвате — обычный URL без токена.
- Границы проверены по mtime/отчётам/md5, а не по git-истории: коммитов нет, дерево dirty.
- Содержимое `golden`/`logs/`/`functions/`/`evidence/` не изменялось; сеть не использовалась.
