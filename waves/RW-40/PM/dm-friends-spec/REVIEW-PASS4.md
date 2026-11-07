# G3-DM-FRIENDS-SPEC — независимое ревью, Pass 4

**Вердикт: PASS. P0 — 0, P1 — 0, P2 — 2.**

P1 Pass 3 (`normalizeSettings` + `homeMode` против замороженного оракула
`gui-spike-features`) закрыт вариантом A, все три P2 Pass 3 исправлены по
существу. Остались две редакционные неточности: устаревшая строка статуса и
одно предложение, которое буквально противоречит boot-правилу и оракулу того же
документа. Ни одна не меняет реализуемый контракт, поэтому приёмку не блокируют,
но до заморозки их надо поправить.

Ревью read-only. Прочитаны `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (2207 строк,
ветка `rw40/dm-friends-spec-pass2`, HEAD `4e24048`),
`waves/RW-40/PM/dm-friends-spec/REVIEW-PASS3.md`,
`waves/RW-40/PM/dm-friends-spec/REPORT.md`,
`waves/RW-40/PM/dm-friends-spec/validate-pass4.mjs` (+ `.log`),
`rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/app.js`,
`rust/gui-spike/src/settings_store.rs`,
`rust/crates/discord-core/src/state/{types,presence}.rs`,
`rust/gui-spike/ui/tokens.css`,
`tests/unit/gui-spike-features.test.mjs`, `tests/unit/guild-rail.test.mjs`,
`rust/gui-spike/tests/settings_contract.rs`. Спецификация, код, тесты и
golden-каталоги не изменялись; записан только этот отчёт.

## Сводка по P1 и P2 Pass 3

| # Pass 3 | Результат | Доказательство в Pass 4 |
|---|---|---|
| P1 (`normalizeSettings` всегда добавлял `homeMode`) | **Закрыт, вариант A** | §0.2 п.4 (`:54`): `...(typeof source.homeMode === "boolean" ? { homeMode: source.homeMode } : {})`; «не булево значение и отсутствие ключа **не добавляют** `homeMode`»; `T-N107-HOME-NAV-01` п.6 (`:2194`) требует ровно пять ключей на пути диалога и `lastGuildId: "@me"` при активном Home |
| P2.1 `kind` вместо `activity_type` | **Закрыт** | §4.1.1 (`:2085`): `activity.activity_type == ActivityType::Streaming`, поле `pub activity_type: ActivityType` (`presence.rs:201` — совпало), фраза «Поля `kind` в структуре `Activity` нет»; в спеке `kind == ActivityType` — 0 вхождений |
| P2.2 укороченный `aria-label` `#friends-tab-pending` | **Закрыт** | §2.3 (`:607`): `aria-label="Ожидание, 2 входящих запроса в друзья"` — полная формула §2.2 п.1; короткой версии в документе 0 |
| P2.3 ссылка `tokens.css:101` | **Закрыт** | §4.1.1 (`:2085`): `tokens.css:113`; в `tokens.css` `--status-streaming: #593695` действительно на 113, на 101 — `--danger` |

## Проверка P1 по независимому оракулу

Проверял не текст отчёта, а реальные файлы:

- `normalizeSettings` (`features/settings.js:199-221`) сейчас пропускает
  `lastGuildId` только при `^\d{1,20}$`; расширение до `"@me"` сохраняет приём
  «ключа нет в результате» — `...(lastGuildId !== undefined ? { lastGuildId } : {})`,
  то есть предписанный Pass 4 спред действительно повторяет существующий стиль.
- `collectSettings` (`:375-391`) собирает `version`, `toggles`, `density`,
  `theme`, `locale` и добавляет `lastGuildId` только при непустом
  `getEffectiveLastGuildId()`. Ключа `homeMode` там нет — значит вариант A не
  требует правки замороженного теста.
- `tests/unit/gui-spike-features.test.mjs`: `DEFAULTS` (`:2387`) без `homeMode`,
  `assert.deepEqual(Object.keys(payload).sort(), ["density", "locale", "theme",
  "toggles", "version"])` (`:2451-2455`), `homeMode` в файле — 0 вхождений.
  Прогон `node --test tests/unit/gui-spike-features.test.mjs
  tests/unit/guild-rail.test.mjs` → **116 passed, 0 failed**: конфликт P1 снят
  на уровне контракта, который эти тесты проверяют.
- Заявка «Home не теряется» опирается на реальную проводку: `app.js:1497`
  передаёт `getLastGuildId: () => preferredGuildId`, `preferredGuildId`
  выставляется в `onChoose`/`rememberGuild` (`:1299-1332`) и при
  `settings_load` (`:1236-1237`); §2.1.1 п.5 задаёт явные `homeMode: true/false`
  при пользовательском выборе, а `@me` в `getEffectiveLastGuildId` покрыт
  списком пяти фильтров §0.2 п.4. Расхождения с кодом не нашёл.
- Rust-сторона совместима: `UiSettings` — `#[serde(deny_unknown_fields)]`
  (`settings_store.rs:144`), `normalize_last_guild_id` — `:281-283`,
  `validate` проверяет `lastGuildId` через `is_snowflake` (`:202-211`),
  `save` кладёт `normalize_last_guild_id(...)` в `..settings.clone()`
  (`:518-524`), то есть аддитивное `home_mode` не теряет остальных полей.
  `"@me"` в `rust/gui-spike/tests/settings_contract.rs` нет, существующие
  невалидные значения его не содержат — расширение фильтров тесты не ломает.

## Механические проверки

- `node waves/RW-40/PM/dm-friends-spec/validate-pass4.mjs "$PWD"` (в ветке
  `rw40/dm-friends-spec-pass2`) → **17/17 PASS**, exit 0. Записанный
  `validate-pass4.log` совпадает с прогоном построчно.
- §3.2: 96 уникальных `var(--...)`, все объявлены в `tokens.css`, §3.1
  совпадает с §3.2 (0 расхождений), 0 сырых `#hex`/`rgb()`/`hsl()` включая
  комментарии, 0 `!important`, 0 `:has()`.
- §4.1.1 сверен с исходниками: `state.presences: BTreeMap<PresenceKey, Presence>`
  — `types.rs:118`; `pub type PresenceKey = (u64, PresenceScope)` —
  `presence.rs:264`; `is_online` — `:65-69`; `ActivityType` — `:78-82`;
  разбор `type` — `:107`; `Activity::activity_type` — `:201`; `Presence` и
  `activities` — `:268-278`. Все ссылки в допустимом диапазоне.
- Токен `--status-streaming` — `tokens.css:113`; `--danger` — 101. Ссылка
  исправлена именно туда.
- md5 совпадают с отчётом Pass 4: спецификация
  `71bd2b28986754461ddfe790220b5b6d`, `validate-pass4.mjs`
  `b9c90062bf46fb78d670417722056bb9`, `validate-pass4.log`
  `c69c58f6bd863d13a0c1c676ea50af11`.

## P2 — не блокируют вердикт, поправить до заморозки

### 1. Строка статуса осталась от Pass 3

`:3`: «Статус: **дизайн-контракт, Pass 3** … закрыты все 7 P1 и 12 P2 из
`DM-FRIENDS-SPEC-REVIEW.md`, а также 1 P1 и 11 P2 из
`DM-FRIENDS-SPEC-REVIEW-PASS2.md`». После Pass 4 документ не упоминает ни
`REVIEW-PASS3.md`, ни свой текущий статус. Читатель не может по шапке понять,
какой пасс перед ним и что P1 Pass 3 закрыт. Нужна строка вида «Pass 4 … 1 P1 и
3 P2 из `REVIEW-PASS3.md` закрыты».

### 2. §0.2 п.4: «`homeMode` отсутствует в файле … → режим сервера» противоречит boot-правилу

Там же, `:54`, через одно предложение после объяснения варианта A: «`homeMode`
отсутствует в файле или равен `false` → режим сервера (в Rust `None` трактуется
как `false`)». Буквально это означает, что файл без `homeMode` всегда даёт
режим сервера, хотя:

- §2.1.1 п.3 (`:249`) задаёт дизъюнкцию `homeMode === true` **или**
  `lastGuildId === "@me"`;
- сам Pass 4 объясняет сохранение Home именно отсутствием `homeMode` в payload
  диалога и восстановлением по `lastGuildId: "@me"`;
- `T-N107-HOME-NAV-01` п.6 (`:2194`) требует, чтобы после такого сохранения Home
  восстанавливался.

Реализация по букве этого предложения сломает оракул п.6. Формулировку стоит
сузить: «отсутствие `homeMode` или `false` не выбирает Home само по себе; режим
определяется дизъюнкцией с `lastGuildId`». Это уточнение текста, а не смена
контракта: три других места документа уже задают правильную семантику.

## Замечания к валидатору (на вердикт не влияют)

- Проверка «P1: замороженный оракул ждёт ровно 5 ключей»
  (`validate-pass4.mjs:21,25`) ищет литерал
  `["density", "locale", "theme", "toggles", "version"]`, а он в тесте
  встречается только в `DEFAULTS` (`:2387`); сама проверка набора ключей
  (`:2451-2455`) написана в несколько строк и этим шаблоном не покрыта. Вывод
  верный — я проверил `assert.deepEqual` вручную и прогнал тест, — но механизм
  проверяет не то утверждение, которое называет.
- `P1: T-N107-HOME-NAV-01 п.6` — регулярка `T-N107-HOME-NAV-01[\s\S]{0,4000}?Сохранение из диалога настроек`
  сработает и на упоминание фразы без требования к содержанию пункта. Для
  регрессии полезно проверять конкретные строки (`ровно пять ключей`,
  `lastGuildId: "@me"`).
- Остальные 15 проверок опираются на исходники (`presence.rs`, `settings.js`,
  `tokens.css`, замороженный тест), а не на текст спеки — это правильный оракул.

## Что осталось

1. Поправить строку статуса (`:3`) и формулировку в §0.2 п.4 (`:54`).
2. Опционально усилить два слабых чека валидатора.
3. Контраст §3.3, токены §3.1 и арифметику заново не пересчитывал: Pass 4 их
   не менял, а валидатор подтверждает только совпадение наборов токенов.
4. `docs/specs/DM-FRIENDS-SPEC.md` в ветке по-прежнему отсутствует; отчёт Pass 4
   это фиксирует, второго места истины нет — считаю правильным.
