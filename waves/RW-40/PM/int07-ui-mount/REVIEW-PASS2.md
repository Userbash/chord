# Ревью Pass 2: INT-07-UI-APP-MOUNT — монтаж `voiceDevices` и `notifications` в `app.js`

- **Ветка / HEAD:** `rw40/int07-ui-app-mount` / `4e24048` (правки Pass 2 — незакоммиченное рабочее дерево).
- **Объект ревью:** `REPORT.md` (fix round), диф `rust/gui-spike/ui/app.js`,
  `rust/gui-spike/ui/index.html`, `tests/unit/gui-spike-features.test.mjs`, артефакты
  `waves/RW-40/PM/int07-ui-mount/`.
- **Основание:** `waves/RW-40/PM/int07-ui-mount/REVIEW.md` (RETURN, 2 × P1, 8 × P2).
- **Ревьюер:** DeepSeek Review 32B (независимая проверка, автору код не принадлежит).
- **Дата:** 2026-10-30.
- **Вердикт:** **RETURN** — 3 × P1, 8 × P2.

Коротко: оба P1 из первого ревью закрыты по существу, P2-3/P2-5/P2-6/P2-8 закрыты,
P2-1 закрыт для достижимых путей. Но исправление P2-2 смонтировало панель голоса на шесть
IPC-команд, которых нет в Rust (тот же класс дефекта, за который был заблокирован
P1-2), закрытие P1-2 держится только на незакоммиченной дельте чужой карточки, а
две проверки, лежащие в HEAD, удалены из теста без процедуры `TEST-POLICY.md` §5 и без
восстановления блока, задокументированного в `g05-rest/REPORT.md` §5.

---

## 1. Что проверено

### 1.1. MD5 против `REPORT.md` §5

| Файл | В отчёте | Измерено | Итог |
|---|---|---|---|
| `rust/gui-spike/ui/app.js` | `a9a00508e608c16879cced90790a1b91` | `a9a00508e608c16879cced90790a1b91` | совпадает |
| `rust/gui-spike/ui/index.html` | `bae962dd0033f531aefbbb57e2b52521` | `bae962dd0033f531aefbbb57e2b52521` | совпадает |
| `tests/unit/gui-spike-features.test.mjs` | `c1015c9245065e651a848cc8f3f07478` | `c1015c9245065e651a848cc8f3f07478` | совпадает |
| `green.log` | `9469bb92caaccfddc860c37a725ceecd` | `9469bb92caaccfddc860c37a725ceecd` | совпадает |
| `mutations.log` | `5cb7ef30f533c5de13e5d7e68c30ae94` | `5cb7ef30f533c5de13e5d7e68c30ae94` | совпадает |
| `red.log` | `de60d8d22d00135eb2f167d658d31f72` | `de60d8d22d00135eb2f167d658d31f72` | файл от прошлого раунда (см. P2-4) |

### 1.2. Прогоны (повторены ревьюером)

```bash
node --test tests/unit/gui-spike-features.test.mjs \
           tests/unit/notifications-ui.test.mjs \
           tests/unit/voice-devices-settings.test.mjs \
           tests/meta/ipc-path.test.mjs
# tests 150 | pass 150 | fail 0 | exit 0
# (features 107, ipc-path 12, notifications-ui + voice-devices — остальное)

node --test tests/unit/gui-spike-dom.test.mjs
# tests 38 | pass 38 | fail 0 | exit 0    (VW-01 не менялся и зелёный)

node --test tests/unit/server-menu.test.mjs
# tests 28 | pass 28 | fail 0 | exit 0    (D7/429 закрыт в наборе карточки g05-rest)

npm run typecheck
# tsc -p tsconfig.json && tsc -p tsconfig.agent.json → exit 0
```

Логи ревьюера: `review-pass2-tests.log` (`034d4e76b120b83945eb6679ddc75893`),
`review-pass2-typecheck.log` (`204fe02af43c0d832db32fcb94821690`).

### 1.3. Мутации

`mutations.log` прочитан целиком, 10/10 `KILLED`, после каждого restore `match=True`,
финальный `FINAL_MD5=a9a005...`. Мутант `MUT-6-p1-aria-disabled-guard-removed` воспроизведён
независимо: замена двух guard'ов на `if (false) return;` даёт MD5
`8d268973c4400caadcec2f288a73cf53` (совпадает с логом), падение
`disabled пункт открыл диалог`, после restore MD5 снова `a9a00508e608c16879cced90790a1b91`.

### 1.4. Функциональная проба

- Пункт `#server-menu-notifications` до `rail.load()` — `aria-disabled="true"`,
  `tabindex="-1"`, `data-static-disabled="true"`, `aria-describedby="menu-reason-not-implemented"`;
  после `rail.load()` все атрибуты заглушки сняты, `data-action="guild-notifications"`,
  `tabindex="0"`; повторный `openServerMenu()` (он вызывает `refreshMenuState()`) не
  возвращает disabled — потому что `disabledReason()` (`actions.js:453-460`) читает
  `data-static-disabled`, а его снимают.
- Колокол до первого канала — `aria-disabled="true"` (VW-01 цел), после
  `chat.openChannel({id})` — `aria-disabled`/`title` сняты, `aria-haspopup="menu"`,
  `aria-controls="channel-notif-menu"`.
- Клик и `Enter`/`Space` по disabled-контролам не открывают ни диалог, ни меню, не шлют IPC,
  не дают `ACTION_FAILED` (проверено тестами и вручную).
- Клик по доступному пункту закрывает `#server-menu`, сбрасывает `aria-expanded` и открывает
  `#guild-notifications-dialog` с `guildId=777` и каналами из `tree.channels`.
- `contextmenu` по каналу при закрытой модалке её не открывает и не уводит фокус в скрытый
  контрол; `Escape` в модалке возвращает фокус на `#server-menu-notifications`.

### 1.5. Что не проверялось

Полный `npm run gate:worker`, `cargo`-воркспейс и живое окно Tauri — по ТЗ карточки это
batch-гейт и Rust-лейн; ревью их не запускало (окно не согласовано).

---

## 2. P1 — блокирует приёмку

### P1-1. Панель голоса сохраняет через шесть IPC-команд, которых нет в Rust

**Где:** `app.js:1586-1604`, `features/voice-devices-settings.js:530,576,588,653,704,714`,
`rust/gui-spike/src/main.rs`.

**Что происходит.** `createVoiceDevicesSettings` получает `service` с вызовами
`set_input_device`, `set_output_device`, `set_input_volume`, `set_output_volume`,
`set_input_mode`, `set_mute_state`. В `main.rs` нет ни одной из этих команд:

```bash
$ grep -c "set_input_device\|set_output_device\|set_input_volume\|set_output_volume\|set_input_mode\|set_mute_state" rust/gui-spike/src/main.rs
0
$ grep -in "voice" rust/gui-spike/src/main.rs | wc -l
0
```

Ни `#[tauri::command]`, ни записи в `generate_handler!` (`main.rs:1167-1219`). В живом окне
первое же переключение устройства, громкости, режима или mute даст
`Не удалось сохранить настройку: …` в `#voice-devices-notice` — контроллер ошибку не глотает
(`voice-devices-settings.js:390-405`), поэтому пользователь увидит alert на каждое действие.

**Почему блокирует.** Это дословно тот же дефект, за который первое ревью заблокировало P1-2
(«монтаж пути, который гарантированно падает при любом сохранении»). Первый раунд закрыл его
для уведомлений; второй раунд внёс его для голоса. Рекомендация прежнего ревью по P2-2
допускала два варианта — «`service` с `invoke(...)` **и реальным списком**» либо «пометить
секцию как предварительную». Не выполнено ни то, ни другое: список устройств остался
синтетическим (`DEFAULT_VOICE_DEVICES`, `app.js:39-58`), команды не зарегистрированы,
честной пометки «не сохраняется» в UI нет. Отчёт признаёт это в §6.1 и всё равно оставляет
секцию рабочей на вид.

**Что нужно.** Либо зарегистрировать шесть команд в Rust (V-07 backend, `main.rs` — вне
матрицы карточки), либо до этого держать секцию в явно заглушечном состоянии: `aria-disabled`
/ видимая пометка «сохранение появится после подключения голосового backend'а» вместо
интерактивных контролов с alert'ом на каждое изменение. Решение — за Team Lead/PM, потому что
первый вариант выходит за файловую матрицу.

### P1-2. Закрытие P1-2 первого ревью держится на незакоммиченной дельте чужой карточки

**Где:** `main.rs:863-902`, `:1201-1202`, `tests/meta/ipc-path.test.mjs`.

**Что происходит.** В рабочем дереве команды `update_guild_notification_settings` и
`update_channel_notification_override` зарегистрированы — ревью это подтверждает, `ipc-path`
(12/12) видит совпадение `#[tauri::command]` и `generate_handler!`. Но в HEAD их нет:

```bash
$ git show HEAD:rust/gui-spike/src/main.rs | grep -c update_guild_notification_settings
0
```

Дельта `main.rs` в рабочем дереве — `+842` строки, это работа карточки N-05 backend, а не
INT-07 (`REPORT.md` прямо пишет: «в моих файлах правок не требовалось»). Значит, при мерже
только INT-07 (или при сборке из HEAD) модалка уведомлений снова уедет в
«команда не найдена».

Проверяющего теста на этот класс дефекта нет и сейчас: `tests/meta/ipc-path.test.mjs`
сверяет Rust ↔ `generate_handler!`, но **не** сверяет имена `invoke("…")` из `rust/gui-spike/ui`
с зарегистрированными командами. Именно поэтому P1-1 прошёл гейт, а исходный P1-2 был
возможен.

**Что нужно.** (1) Зафиксировать, что дельта `main.rs` уходит в `unstable` тем же
integration-мержем, что и INT-07 (или закоммитить её в этой ветке). (2) Добавить мета-тест
«каждое имя `invoke("…")` в `rust/gui-spike/ui` есть в `generate_handler!`» — это durable
guard, который поймал бы и P1-1.

### P1-3. Из HEAD удалены две проверки без процедуры `TEST-POLICY.md` §5 и без восстановления блока `g05-rest`

**Где:** `tests/unit/gui-spike-features.test.mjs` (рабочее дерево, строка 3086;
HEAD — строки 3087-3088), `waves/RW-40/PM/g05-rest/REPORT.md` §5, `TEST-POLICY.md` §2.1/§5.

**Что происходит.** Pass 2 удалил из теста:

```js
assert.equal(byId(app, "leave-confirm").getAttribute("disabled"), null, "кнопка осталась disabled после 429");
assert.equal(app.callsOf("leave_guild").length, 2, "повтор после ошибки не ушёл");
```

`REPORT.md` §1 (P2-7) называет их «чужой хунк `g05-rest`» и снимает. Факты:

1. Строки **есть в HEAD** (`git show HEAD:tests/unit/gui-spike-features.test.mjs`,
   3087-3088) — это не незакоммиченный чужой диф, а часть integration-коммита `4e24048`.
2. `waves/RW-40/PM/g05-rest/REPORT.md` §5 документирует не удаление, а **замену** этих двух
   ассертов блоком из пяти проверок (`disabled !== null` во время отсчёта, три
   `flushTimers()`, ручной повтор `calls === 3`) по основанию G05-REST-SPEC §3.9/D7.
   Этого блока в файле нет (`grep "disabled !== null" tests/unit/gui-spike-features.test.mjs`
   пусто).
3. Удалённые ассерты **падают** на текущем коде: рабочее дерево содержит реализацию
   cooldown'а (`actions.js:611-631`, `:346`), при 429 кнопка остаётся disabled и повтор не
   уходит. Проверено ревьюером: временное возвращение двух строк в копию теста даёт
   `AssertionError: кнопка осталась disabled после 429`, `'' !== null`.
4. Процедуры §5 нет: ни таблицы `old → new → основание`, ни записи в `DECISIONS.md`, ни
   письменного waiver'а.

**Почему блокирует.** `TEST-POLICY.md` §2.1 («удалять/комментировать `assert`») формально
относит это к P0; смягчает только то, что поведение D7 покрыто отдельным набором
`tests/unit/server-menu.test.mjs` (28/28 pass), то есть слепого пятна в поведении нет.
Но состояние доказательств противоречиво: HEAD-версия теста красная на текущем коде, а
Pass 2 сделал её зелёной удалением, а не восстановлением задокументированного блока.

**Что нужно.** Владельцем `g05-rest`/integration-лейном — восстановить блок из §5
(`disabled !== null` → `flushTimers()` → `disabled === null` → ручной клик → `calls === 3`)
и записать изменение в `DECISIONS.md`. INT-07 в этом файле должен оставить только свой
блок `INT-07-UI-APP-MOUNT`; удаление чужих проверок — не его работа и не «возврат матрицы».

---

## 3. P2 — не блокирует, но нужно поправить

### P2-1. `setPttKey` не передан в `service` — PTT-клавиша теряется молча

`app.js:1589-1603` перечисляет шесть методов, но контроллер вызывает ещё и
`invoke(service.setPttKey, pttKey)` (`features/voice-devices-settings.js:677`). В `app.js`
`setPttKey` отсутствует, а `invoke()` контроллера при `typeof fn !== "function"` просто
`return` (`:390-392`) — без уведомления. Итог: назначение/сброс PTT-клавиши не сохраняется
и не сообщает об этом, хотя `REPORT.md` §1 (P2-2) заявляет «ошибки IPC не глотаются», а
первое ревью в P2-2 перечисляло PTT явно. Тест `voice-devices-settings.test.mjs` этого не
ловит: он подаёт фейковый `service` с полным набором методов.

### P2-2. `P2-4` первого ревью не исправлен

Модалка по-прежнему показывает литеральные дефолты как факт: `syncNotificationsWithGuild()`
передаёт `guildEntry: storedEntry ?? { guild_id }` (`app.js:1708`), а дефолты строит
`features/notifications.js`. Отчёт честно фиксирует это в §6.2 и отдаёт N-05 backend.
Принимается как задокументированный остаток, но карточка не закрыта по этому пункту.

### P2-3. Изменения контракта не записаны в `DECISIONS.md`

`SERVER-MENU.md:346` описывает пункт 4 как `disabled` с `aria-haspopup="menu"`. Pass 2
включает его в рантайме и ставит `aria-haspopup="dialog"` (`app.js:1641-1653`). Колокол
`[data-action="inbox"]` после первого канала теряет `aria-disabled` и `title` — это меняет
VW-01-affordance, хотя сами VW-01-тесты остаются зелёными (проверяют старт).
`TEST-POLICY.md` §5 требует для такого изменения запись в `DECISIONS.md` с ID, датой и
риском; ни в `waves/RW-40/PM/`, ни в `waves/RW-39/PM/DECISIONS.md` записи об INT-07 нет.
Содержательно решение разумное (иначе фича недостижима), не хватает только оформления —
это делает Team Lead, а не автор.

### P2-4. У fix-раунда нет RED-артефакта

`REPORT.md` §2 приводит `tests 107 | pass 105 | fail 2 | exit 1`, но файла с этим прогоном
нет: `red.log` — от прошлого раунда (102 теста, `de60d8d2…`). По `TEST-POLICY.md` §4
доказательство — `команда → exit code → артефакт`; новый RED должен быть отдельным файлом
(«логи не переписываются после факта; повтор — новый файл»). Строки в отчёте без лога —
ровно то, за что первое ревью снизило оценку `green.log` (P2-7).

### P2-5. Видимость панели привязана только к клику по `#settings-nav`

`setVoicePanelVisibility()` вызывается на boot и на `click` пунктов навигации
(`app.js:1626-1636`). Любой другой путь смены `#settings[data-active-pane]` (программный
`settings.showPane(...)`, deep-link из другого экрана) оставит панель голоса видимой на
чужой вкладке. Проверено пробой: навигационные клики работают (тест Pass 2 корректен),
но синхронизация не является инвариантом разметки. Надёжнее — `MutationObserver` на
`data-active-pane` или хук в `showPane`, а не список слушателей.

Отдельно (не дефект этой карточки, но рядом): ссылка «Уведомления» внутри voice-панели
(`features/settings.js:579`) вызывает `showPane("notifications")`, а `PANE_BY_SECTION`
(`:252-256`) хранит русские ключи — переход не работает. Проба подтвердила: после клика
`data-active-pane` остаётся `voice`. Файл чужой, к INT-07 отношения не имеет, но
«синхронизация по вкладкам» держится на единственном живом пути.

### P2-6. MD5 в отчёте не изолирует карточку

`app.js` в этом рабочем дереве — общий файл integration-лейна: диф `+508` строк содержит
дельты рейла, invites, dispatch, emoji и др. Собственная дельта INT-07 — только
`app.js:1558-1791` и две строки в `return` (`:1850-1851`). MD5 файла (`a9a005…`) поэтому
не воспроизводится, как только другая карточка перезапишет `app.js`; сравнивать его между
раундами можно только при неизменном дереве. Для доказательства лучше указывать диапазоны
строк и/или `git diff`-хунк, а не хэш общего файла.

### P2-7. Нет мета-проверки «UI `invoke` → `generate_handler!`»

См. P1-2. Пока такого теста нет, любой монтаж контроллера с `service`/`invoke` может
уехать в `unstable` нерабочим, и ни один гейт этого не заметит. Кандидат:
`tests/meta/ipc-path.test.mjs` (там уже есть парсер `#[tauri::command]` и `generate_handler!`).

### P2-8. Список устройств остался синтетическим

`DEFAULT_VOICE_DEVICES` — две записи `default-input` / `default-output` (`app.js:39-58`).
Если секция останется интерактивной после регистрации команд (P1-1), список должен
приходить из `voice_devices`/`list_audio_devices`, иначе пользователь выбирает из двух
выдуманных устройств. Если команды не появятся — секция обязана быть помечена как
предварительная (см. P1-1).

---

## 4. Закрытие замечаний `REVIEW.md`

| Пункт | Статус | Комментарий |
|---|---|---|
| **P1-1** disabled-контролы выполняют действия | **закрыт** | Строгий guard `aria-disabled` в клике и клавишах; пункт включается после `rail.load()` (снят `data-static-disabled`, поэтому `refreshMenuState()` его не гасит), колокол — после первого канала. VW-01 (`gui-spike-dom`, 38/38) не тронут. Оформление изменения контракта — P2-3. |
| **P1-2** IPC-команды уведомлений не зарегистрированы | **закрыт в дереве, не в HEAD** | `main.rs:863-902`, `:1201-1202` есть, `ipc-path` 12/12. Но дельта чужая и незакоммиченная — см. P1-2 ревью. |
| **P2-1** панель не на той вкладке | **закрыт для достижимых путей** | Видимость привязана к `data-active-pane="voice"`, `role="region"`, `hidden` по умолчанию; сиблинг `#settings` сохранён осознанно (инвариант 38). Хрупкость — P2-5. |
| **P2-2** `voiceDevices` без `service` | **не закрыт** | `service` передан, но 6 команд не зарегистрированы (P1-1), `setPttKey` отсутствует (P2-1), список устройств синтетический (P2-8). |
| **P2-3** классы `.notif-toggle` без стилей | **закрыт** | Правила `.notif-toggle` / `.notif-toggle-input` добавлены в `index.html:25-64`, инвариант 38 в `#settings` цел (6/6 в модалке, 0 `.toggle`). Дублирование CSS в HTML — приемлемый компромисс в границах матрицы. |
| **P2-4** дефолты как факт | **не закрыт, задокументирован** | См. P2-2 ревью; владелец N-05 backend. |
| **P2-5** меню под модалкой; open→close | **закрыт** | Меню закрывается до открытия диалога, `aria-expanded="false"`; путь open→close удалён, `openChannelNotifications` модалку не переоткрывает; тест на `contextmenu` при закрытой модалке и на фокус есть. |
| **P2-6** магические fallback'и | **закрыт** | `id = rail.selectedId \|\| selectedGuildId() \|\| ""`, пустой id → статус «Сначала выберите сервер.» без IPC; колокол берёт только `openable`-тип (`CHANNEL_KINDS`), иначе видимая причина. Тест на пустой рейл есть. |
| **P2-7** доказательства | **частично** | `green.log` перезаписан и честен (107/38/17 + 2 `tsc`, 299 строк). Но: чужой хунк не «снят», а удалён из HEAD без §5 — P1-3; RED-артефакта fix-раунда нет — P2-4. |
| **P2-8** пробелы в тестах и мутациях | **закрыт** | Добавлены проверки ARIA входных контролов, классов тумблеров, закрытия меню, пустого рейла, `contextmenu`/`Escape`/фокуса; 10/10 мутаций, `MUT-6` воспроизведён ревьюером. |

---

## 5. Что нужно для PASS

1. **P1-1:** зарегистрировать `set_input_device`, `set_output_device`, `set_input_volume`,
   `set_output_volume`, `set_input_mode`, `set_mute_state` (V-07 backend) **или** перевести
   секцию в явно заглушечное состояние без интерактивных контролов.
2. **P1-2:** подтвердить, что дельта `main.rs` (команды уведомлений) уходит в `unstable`
   тем же мержем; добавить мета-тест «UI `invoke` ⊆ `generate_handler!`».
3. **P1-3:** восстановить в `gui-spike-features.test.mjs` блок 429 из `g05-rest/REPORT.md` §5
   (или вернуть файл к состоянию, где чужих правок нет), оформить `DECISIONS.md`.
4. **P2-1:** добавить `setPttKey` в `service` (`invoke("set_ptt_key", { key })` или осознанно
   задокументировать, что PTT не сохраняется).
5. **P2-3, P2-4:** запись в `DECISIONS.md` об изменении disabled-контракта пункта 4 и
   колокола; отдельный лог RED fix-раунда.

После этого достаточно повторить прогон четырёх наборов из §1.2, `npm run typecheck`,
и перезаписать `green.log` новым файлом (не правкой старого).
