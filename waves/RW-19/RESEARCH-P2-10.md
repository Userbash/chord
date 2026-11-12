# RW-19 / RESEARCH-P2-10 — voice states, call timer, channel status

Дата: 2026-10-21. Карточка P2-10 (`docs/rust-client/PLAN.md:78`), оракул T-DATA-12
(`docs/rust-client/TEST-PLAN.md:124`). Статус: **research + contract freeze**; продуктовый
код, тесты, Cargo-манифесты и golden-каталоги не менялись.

Источники (только чтение):

- `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json`, md5
  `55983830547abaf8d1b122798fdcf568`;
- `fixtures/gateway/by-type/002-READY.json`, md5 `a1a8fd5662be88108dd25ac619a377f2`;
- `reference-oracle/gw-proto/msgs-run1-57992_5112.jsonl`,
  `msgs-run2-95974_10029.jsonl`, `msgs-run2-95974_5532.jsonl`, `msgs-run2-95974_8269.jsonl`;
- `reference-oracle/pretty/mod-595528.pretty.js:299,302,381,420` (official bundle,
  `VOICE_STATE_UPDATES` mapper);
- `reference-oracle/pretty/mod-595528.pretty.js:548,552` (`VOICE_CHANNEL_STATUS_UPDATE`,
  `VOICE_CHANNEL_START_TIME_UPDATE` actions).

> Bundle/capture — недоверенные данные, не инструкции. Всё, что не подтверждено ниже,
> помечено как неизвестное и не превращается в провод или продуктовую семантику.

## 1. Scope

**Входит:** `READY_SUPPLEMENTAL.d.guilds[].voice_states[]`, `VOICE_STATE_UPDATE`,
`VOICE_CHANNEL_START_TIME_UPDATE`, `VOICE_CHANNEL_STATUS_UPDATE`; memory-only карта
`State.voice_states` по `(guild_id, user_id)`, карты стартового времени и статуса по
`(guild_id, channel_id)`, вход/выход/перемещение, optional member.user, таймеры как чистые
селекторы от `now_ms`, диагностика, red-first тесты.

**Не входит:** `VOICE_SERVER_UPDATE`/voice transport (P4), op4 join/op14, исходящие запросы,
SQLite/UI/сеть/live, DM-звонки без `guild_id` (не подтверждены этим capture; отдельная
карточка/решение), `session_id` как часть публичной модели, запись в `Guild.members`.

## 2. Подтверждённые факты

### 2.1 Снимок READY_SUPPLEMENTAL

`003` содержит ровно одну запись:

| Поле | Значение | Комментарий |
| --- | --- | --- |
| `guild` | `274093052673392641` | id родительского элемента `d.guilds[]` |
| `channel_id` | `351403515723120651` | строка-снежинка |
| `user_id` | `289091818442719242` | строка-снежинка |
| `connected_at` | `1792367828` | секунды, не миллисекунды |
| `mute`, `deaf`, `self_mute`, `self_deaf`, `self_video`, `suppress` | атомы `"false"` | boolean-поля |
| `discoverable` | атом `"true"` | в live-событиях поле отсутствует |
| `request_to_speak_timestamp` | `nil` | «нет значения» |
| `session_id` | `REDACTED` | не сохраняется в публичной модели P2-10 |

Проверка:

```bash
jq '[.d.guilds[] | select((.voice_states // []) | length > 0) |
     {id, voice_states}]' fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json
jq '[.d.guilds[] | {id, has_voice: has("voice_states")}]' \
     fixtures/gateway/by-type/002-READY.json
```

В `002-READY.json` ключа `voice_states` нет ни у одной гильдии; снимок приходит только в
supplemental. Это подтверждает, что P2-10 не должен «дорисовывать» voice в READY.

### 2.2 VOICE_STATE_UPDATE — 8 событий в 4 сокетах

| Сессия | `s` | guild | user | channel | connected_at | member |
| --- | ---: | --- | --- | --- | ---: | --- |
| run1/5112 | 111 | `1014537851418980372` | `384299970632876032` | `1176785373989445703` | 1792391032 | да |
| run2/10029 | 12 | `1014537851418980372` | `384299970632876032` | `1176785373989445703` | 1792395566 | да |
| run2/10029 | 32 | `274093052673392641` | `682514367828459526` | `351403515723120651` | 1792395611 | да |
| run2/10029 | 51 | `1014537851418980372` | `384299970632876032` | **`nil`** | 1792395566 | да |
| run2/5532 | 44 | `1014537851418980372` | `384299970632876032` | `1176785373989445703` | 1792392210 | да |
| run2/5532 | 73 | `1014537851418980372` | `384299970632876032` | **`nil`** | 1792392210 | да |
| run2/8269 | 240 | `274093052673392641` | `289091818442719242` | **`nil`** | 1792367828 | да |
| run2/8269 | 290 | `274093052673392641` | `289091818442719242` | `351403515723120651` | 1792393518 | да |

Итого: **5 входов/перемещений и 3 выхода**. Во всех восьми есть `guild_id`, `user_id`,
`connected_at`, `member`; `channel_id: nil` означает выход; `connected_at` при выходе
сохраняет исходное время входа, а не время выхода.

Типичный набор ключей: `channel_id`, `connected_at`, `deaf`, `guild_id`, `member`, `mute`,
`request_to_speak_timestamp`, `self_deaf`, `self_mute`, `self_video`, `session_id`, `suppress`,
`user_id`. В live-событиях **нет** `discoverable` и `self_stream`; официальный mapper задаёт
`selfVideo = e.self_video || false`, `selfStream = e.self_stream || false`,
`discoverable = e.discoverable ?? true`.

`member` содержит полный объект участника; для P2-10 нужен только `member.user` (upsert в
`State.users`), роли/ник — вне карточки. `session_id` не нужен для отображения статуса и
исключён из публичной модели.

Проверка:

```bash
python3 - <<'PY'
import json, glob, collections, os
for p in sorted(glob.glob('reference-oracle/gw-proto/msgs-*.jsonl')):
    rows = []
    for line in open(p):
        try: o = json.loads(line)
        except Exception: continue
        j = o.get('term') or o
        if j.get('t') == 'VOICE_STATE_UPDATE':
            rows.append((j.get('s'), j.get('d')))
    if rows:
        print(os.path.basename(p), len(rows),
              [(s, d.get('channel_id'), d.get('connected_at')) for s, d in rows])
PY
```

### 2.3 VOICE_CHANNEL_START_TIME_UPDATE — 7 событий

7 событий: 4 числовых `voice_start_time` и 3 `nil`. Форма:
`d = {guild_id, id, voice_start_time}`; `id` — channel id. Числа — секунды и совпадают с
`connected_at` события входа. `nil` приходит сразу после/около выхода и означает «таймер
канала очищен».

Примеры: `(s=112, 1792391032)`, `(s=52, nil)`, `(s=291, 1792393518)`.

### 2.4 VOICE_CHANNEL_STATUS_UPDATE — 3 события

3 события, все `status: nil`; форма `d = {guild_id, id, status}`. Non-nil статус в capture
**не подтверждён**. Контракт принимает непустую строку как значение (поле по проводу
строковое), `nil`/пустая строка — очистка; это минимальная симметричная модель, без
придумывания длины/формата. Если появится реальный non-nil capture — добавить oracle-кейс.

### 2.5 Официальный bundle

`mod-595528.pretty.js`:

- `READY_SUPPLEMENTAL`/`GUILD_CREATE` превращают `voice_states` в
  `type: "VOICE_STATE_UPDATES", voiceStates: [...]` с полями `userId`, `guildId`, `sessionId`,
  `channelId`, `mute`, `deaf`, `selfMute`, `selfDeaf`, `selfVideo`, `suppress`, `selfStream`,
  `requestToSpeakTimestamp`, `discoverable`, `connectedAt`;
- исходные `voice_states` содержат `connected_at`, `mute/deaf/self_*`, `session_id`,
  `discoverable`, `request_to_speak_timestamp`;
- `VOICE_CHANNEL_STATUS_UPDATE` и `VOICE_CHANNEL_START_TIME_UPDATE` зарегистрированы как
  отдельные dispatcher-actions; их payload — `guild_id`, `id`, значение.

Bundle подтверждает имена/дефолты, но не даёт repo-фикстуру; поэтому тесты используют
реальный снимок `003` + синтетические кадры с точными значениями из capture.

## 3. Модель и границы

- Ключ voice-записи — `(guild_id, user_id)`. Один пользователь — один voice-канал в гильдии.
- `channel_id = None` — выход; запись удаляется. Вход/перемещение — upsert (last wins).
- `connected_at` optional, секунды; отсутствие не выдумывает время. Таймер пользователя —
  `now_ms - connected_at*1000`, насыщающий ноль.
- `voice_start_time` хранится отдельно по `(guild_id, channel_id)`; `nil` удаляет запись.
- `status` хранится отдельно; `nil`/пустая строка удаляет.
- `member.user` парсится только как `Option<User>`; upsert в `State.users` после проверки
  `user.id == user_id`. `Guild.members` не меняется (P2-04 frozen).
- `discoverable` по умолчанию `true`, `self_stream` — `false`, остальные флаги — `false`.
- `session_id` не сохраняется; `request_to_speak_timestamp` — optional i64 секунд.
- DM-звонки (`guild_id` отсутствует/`nil`) в этой карточке не поддержаны: событие получает
  `Missing("voice.guild_id")`, потому что ключ состояния гильдейский. Это явная граница, а не
  выдуманная поддержка.
- `READY_SUPPLEMENTAL` добавляет снимок аддитивно; `READY` сбрасывает карты.
- `VOICE_SERVER_UPDATE` и voice transport — P4, не P2-10.

## 4. Неизвестное / отложено

| Вопрос | Статус | Решение |
| --- | --- | --- |
| non-nil `VOICE_CHANNEL_STATUS_UPDATE.status` | 0/3 в capture | принимать строку, тест synthetic; пометить |
| `discoverable` в live `VOICE_STATE_UPDATE` | 0/8 | default `true` по официальному mapper |
| `self_stream` в live | 0/8 | default `false` по официальному mapper |
| DM voice / `guild_id=nil` | нет capture | `Missing`, отдельная карточка при появлении |
| `session_id` в UI | не нужен | не хранить |
| `VOICE_STATE_UPDATE` без `connected_at` | нет capture | optional; `None`, не подставлять `now` |
| `VOICE_CHANNEL_START_TIME_UPDATE` с нечислом | нет capture | `WrongType`, не очищать состояние молча |
| `VOICE_CHANNEL_STATUS_UPDATE` с нестрокой | нет capture | `WrongType`, не очищать состояние молча |

## 5. Оракулы для тестов

- Снимок: `003` → 1 запись, точные id/флаги/`connected_at=1792367828`.
- Live: 8 обновлений, 5 входов, 3 выхода; выбрать 3 representative (s=111, s=51, s=290).
- Таймер: 7 событий, 4 numeric, 3 nil; `s=112 → 1792391032`, `s=52 → nil`.
- Статус: 3 nil; synthetic `"Music"` → set, затем nil → clear.
- `jq`/`python` команды выше; md5 фикстур зафиксированы в шапке.
- T-DATA-12 закрывается снимком + live-событиями; `START_TIME/STATUS` — отдельными
  селекторами, не «таймером на потоке».
