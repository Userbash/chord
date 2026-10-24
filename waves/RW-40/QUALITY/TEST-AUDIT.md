# TEST-AUDIT — G4 QA: сила тестов, mutation, e2e, bug-tracking

Дата: 2026-10-24. Задача: `Q-AUDIT-G4`. Аудитор: субагент G4 (read-only). PM — DeepSeek.
Режим: **только чтение**. Код/тесты/доки не менялись, `git commit` не делался, live/сеть/окно
не запускались (CDP/Frida/Discord/AT-SPI не вызывались).

## 0. Снимок и метод

| Что | Значение |
|---|---|
| `HEAD` | `0239543` |
| Дерево | **грязное, в работе**: `rust/gui-spike/` целиком untracked, файлы правились 12:13–12:16 (G1-01 + Q-G2-P1 в полёте); `tests/unit/gui-spike-*.test.mjs` и `tests/support/` тоже untracked |
| `scripts/gate.mjs` | `d4207a1c817e0f415aa2c3aae4612578` |
| `tests/unit/gui-spike-features.test.mjs` | `03e95cc03adb87432a38085c8ed42cfe` (2487 строк) |
| `tests/unit/gui-spike-auth.test.mjs` | `8f45e984b44f14478f461bd826699076` (1425) |
| `tests/support/gui-dom-harness.mjs` | `12ea7059e2c4de7c64a0daa40711094a` (1397) |
| `src/cli/run.mjs` | `7f70fb57d492717ffdd75ce6e7d151e8` (350) |
| `rust/gui-spike/src/rest_failure.rs` | `bb74c43e5b23c3efd6bace2b7cf1c1fa` |

Метод: чтение `TEST-POLICY.md`, `QUALITY/{STRATEGY,BASELINE-QUALITY,G1-AUDIT,G2-AUDIT,BUGS}.md`,
всех `tests/**`, `scripts/gate.mjs`; прогон meta/unit/e2e/coverage; **mutation smoke на копии
репозитория** в `/tmp/opencode/g4-mut` (репозиторий не изменялся: мутация живёт только в копии,
после прогона файл восстанавливался).

Ограничения: Rust-мутации не гонялись (один `cargo` + время сборки; для Rust приведены
кандидаты с оракулом, без прогона). Выводы по «что тест НЕ ловит» для Rust — из чтения
тестов и продукта, отмечено отдельно.

## 1. Прогоны (evidence)

| Команда | Exit | Результат | Лог |
|---|---:|---|---|
| `node --test tests/meta/*.test.mjs` | 0 | 39 / pass 39 / fail 0 / skip 0 | `waves/RW-40/PM/qa-g4/node-test-meta.log` |
| `node --test tests/unit/*.test.mjs` | 0 | **384 / pass 384 / fail 0 / skip 0** | `waves/RW-40/PM/qa-g4/node-test-unit.log` |
| `node --test tests/e2e/*.test.mjs` | 0 | 4 / pass 0 / **skip 4** | `waves/RW-40/PM/qa-g4/e2e-skip.log` |
| `npm run test:coverage` | 0 | all files line 99.40 / branch 91.88 / funcs 97.03 | `waves/RW-40/PM/qa-g4/coverage.log` |
| mutation smoke (11 мутантов, копия) | — | **6 caught / 5 missed** | `waves/RW-40/PM/qa-g4/mutation-smoke.log` |
| зондирование e2e-поверхности CLI | 1/2 | `cdp eval|clicktext|list` → exit 1, `walk` → exit 2 | `waves/RW-40/PM/qa-g4/e2e-surface-probe.log` |

Baseline совпадает по meta/unit; e2e по-прежнему **4 skip из 4** (baseline Q-09 не сдвинулся).

## 2. Инвентаризация слоёв

| Слой | Файлов | Тестов | Что реально проверяет | Чем является оракул | Чего не ловит |
|---|---:|---:|---|---|---|
| **unit: gui-spike UI** (`tests/unit/gui-spike-{features,auth,dom,model,bench}`) | 5 | 213 `it` | поведение `ui/app.js` в DOM-шиме: экраны, клавиши, композер, реакции, поиск, настройки, auth-флоу; модель `PageStore`/высоты | состояние шима (`data-*`, `aria-*`, `invoke`-вызовы, `timers`, `rAF`) + `model.js`/`ROW_HEIGHTS` | source-scrape CSS (auth/dom), геометрия-цепочка CSS↔JS, race/stale, таймеры-утечки, resize/zoom |
| **unit: CLI/core** (15 файлов) | 15 | 171 `it` | `args/csv/calls/hook-io/inventory/paths/processes/nm/domains/cdp/client/walk-log/blacklist/profile/frida*` | реальные фикстуры (`tests/fixtures/**`, golden CSV, nm) + инжектируемые `fetch`/`WebSocket`/`Frida` | `src/cli/run.mjs` (диспетчер команд) в coverage **отсутствует** |
| **meta** | 8 | 39 | §8↔файлы, docs↔HELP, waves/db.jsonl, git-tracking фикстур, правило «не ослаблять тесты», CSS-каскад DP-01, Rust-план | текст документов/исходников (source-scrape), git | смысл кода: регексп совпадёт при мёртвой ветке; счётчики «≥100» набиваются |
| **smoke** | 8 | 22 | настоящий CLI (`spawn`): help/version/коды, summarize (golden CSV md5), pids/inventory (Frida), cxxfilt (бинарь), live GUI + AT-SPI проба | exit code + stdout + файлы + AT-SPI отчёт окна | всё, что за `skip`; ветки «не тот env» (принимается `либо/либо`) |
| **e2e** | 3 | 4 | CDP против headless-браузера на fixture + live Discord | реальный браузер/реальная страница | **сейчас не проверяет ничего (4/4 skip)** |
| **contract (Rust)** | 12 в `gui-spike/tests` + crate-тесты | 164 `#[test]` в gui-spike (crate core 574, workspace 1112 по baseline) | типизированные ошибки REST/WS, декодеры, store/keyring, i18n, ETW, geometry-оракул | fake-transport + реальные фикстуры + `main.rs` текстом (9 тестов `include_str!`) | keyring/таймаут/конкурентность (нет fake-clock), 403/404 — закрыто в этом же дереве (G1-01), Tauri-регистрация — только по тексту |

Классификация по способу проверки (оценка по чтению):

- **поведенческие** (состояние/побочные эффекты): ~90% unit UI, ~95% CLI-unit, 8/12 Rust contract
  файлов, smoke `summarize/cli/pids/inventory`.
- **статические, с независимым оракулом** (CSS-каскад по SPEC, golden md5, nm/CSV, §8):
  `meta/design-breakpoints`, smoke `summarize`, unit `domains/nm/calls`.
- **source-scrape / присутствие строки** (единственный оракул — подстрока в файле): 9 Rust-тестов
  `include_str!("../src/main.rs")`, `meta/{docs-sync,rust-client-sync,test-policy,waves-sync,plan-sync}`,
  `unit/gui-spike-auth` (CSS 2 теста), `unit/gui-spike-dom` (7 мест), `meta/design-breakpoints`
  (последний `it` — `screens.js`/`actions.js` регекспом).
- **текст вместо состояния**: `features:208` (empty-state), `features:2382` (позиция вместо геометрии).

## 3. Десять самых слабых тестов

Формат: `ID | file:line | что НЕ ловит | P | минимальный тест | ожидаемый red`.

**QA4-01** | `tests/unit/gui-spike-features.test.mjs:200-208` | empty-state QuickSwitcher проверяется
только `textContent`; инверсия `hidden` не видна → **живой баг G2-03 зелёный** | **P1** |
при 8 опциях `empty.hidden !== null`, при 0 — `hidden === null` (и `hint` не тронут) | сейчас:
в коде `if (visible.length > 0) removeAttribute("hidden")` → тест обязан падать на 8 опциях.

**QA4-02** | `tests/unit/gui-spike-features.test.mjs:1113-1136` | confirm-удаление проверено только
на одной строке; «подтверждение липнет к следующей строке» (G2-11) не ловится; мутация M6 missed |
**P1** | A→confirm→`mouseover` B → label «Удалить сообщение», один клик **не** шлёт `delete_message`;
повторный confirm A — ровно один вызов | сейчас: `deletePending` не сбрасывается на `close()`/смене
строки → тест падает на «B с первого клика удаляется».

**QA4-03** | `tests/unit/gui-spike-auth.test.mjs:377-403` | раскладка auth-карточки проверяется
регекспом по тексту `auth.css` (480/784/416/240, `@media ≤830`); ни каскада, ни переопределения,
ни zoom/DPR | P2 | собрать каскад как в `meta/design-breakpoints` (или геометрия в шиме):
`width(card)` при `qr=true` на ширинах 1200/830/700, `qr.display` на 830/831 | сейчас: добавить
после проверяемого правила `.auth-card{width:100%}` — тест остаётся зелёным (мутация «перекрыть
позже» не ловится).

**QA4-04** | `rust/gui-spike/tests/guilds_contract.rs:332-345` (`tauri_commands_take_no_token_and_read_backend`)
| «один helper `token_or`» доказывается подстроками `"async fn token_or<"`, `"backend.token()"`,
`body.contains("token_or::<")`; helper может быть мёртвым/сломанным, а все команды — ходить мимо |
**P1** (это evidence закрытия G1-02) | поведенческий тест: `AuthBackend` с fake `TokenStore`,
возвращающим `Err("Secret Service недоступен")`; вызвать `join_guild`-команду → ошибка домена
`storage` (или явно задокументированный текущий `Network`), текст причины не потерян | сейчас:
`token_or` не вызывается внутри `join_guild` → тест краснеет.

**QA4-05** | `tests/meta/design-breakpoints.test.mjs:335-372` | последний `it` — регексп по
`screens.js`/`actions.js`: `assert.match(screensJs, /setAriaHidden\(node, !active\)/)`,
`panel.style.display = expanded ? "none" : ""`. Ловит переименование, но не инверсию логики
(строка на месте, поведение сломано) | P2 | поведенческий дубль: в шиме переключить 4 экрана,
проверить `aria-hidden` на `.screen` и **отсутствие** `aria-hidden`/`hidden` на панелях; для
inline-display — `clickRail` + `style.display` в `""`/`none` | сейчас: заменить в `screens.js`
`setAriaHidden(node, !active)` на `setAriaHidden(node, active)` → meta-тест зелёный, поведенческий — красный.

**QA4-06** | `tests/meta/plan-sync.test.mjs:118-138` | «токены §8 встречаются в тесте» —
`text.includes(token)`: имя функции в **комментарии** удовлетворяет проверку; тест-файл может не
импортировать модуль | P2 | стрипать комментарии перед поиском; требовать
`from "<модуль>"` в импортах для каждого токена-идентификатора | сейчас: перенести токен
`pickNextItem` в комментарий теста → проверка зелёная (токен не покрыт).

**QA4-07** | `tests/meta/rust-client-sync.test.mjs:116,128,180` | целостность Rust-плана —
`ids.length >= MIN_FEATURES|MIN_TESTS|MIN_CARDS` + уникальность: набивается «пустыми» строками
(`F-999 \| todo \| —`) | P3 | каждая строка должна иметь непустые обязательные колонки (owner/статус/
тест-ID из `TEST-PLAN`) и ссылку на существующий `T-*` | сейчас: добавить 5 строк-заглушек в
`FEATURES.md` → счётчик растёт, тест зелёный.

**QA4-08** | `tests/unit/gui-spike-features.test.mjs:2360-2422` (G2-02) | заявлено «оверлей не выходит
за `.content`», а проверяется `content.style.position === "relative"` (деталь реализации), не
геометрия; при этом Q-11 отмечает, что `position:relative` ставится inline из JS | P2 | в шиме
сравнить прямоугольники: `rect(.error-state)` ⊆ `rect(.content)`; и это же — после `app.triggerResize()`
| сейчас: убрать `position:relative` и задать `inset:0` на `.error-state` (накрывает viewport) →
тест падает только по inline-style, а не по перекрытию; заменить inline на CSS-класс → тест
падает ложно (ложный красный на рефакторинг).

**QA4-09** | `tests/smoke/inventory.smoke.test.mjs:11-14` | `assert.match(stderr, /Frida недоступна|пропущено/i)`
принимает два разных исхода (среда недоступна **или** цель пропущена); регресс «любая ошибка → 2»
пройдёт | P3 | развести: при доступной Frida ждать `пропущено`, при недоступной — `Frida недоступна`
(ветку выбирать по отдельному `pids`-пробнику) | сейчас: вернуть код 2 с текстом «пропущено» на
ошибку аргументов (не тот класс) → тест зелёный.

**QA4-10** | `tests/meta/docs-sync.test.mjs:35-42` | README обязан содержать `` `команда` `` —
достаточно одного упоминания в любом контексте (даже «не используйте `` `walk` ``»); синтаксис/коды
выхода не сверяются | P3 | для каждой команды HELP: строка README с `discord-lab <cmd>` и кодом
выхода; сверять с `smoke/cli` | сейчас: заменить пример README на «`` `walk` `` пока нет» → тест зелёный.

Прочие (вне топ-10, тот же класс): `unit/gui-spike-dom.test.mjs:769-772` (регексп `@import`),
`unit/gui-spike-auth.test.mjs:294-311` (id есть «в html **или** в auth.js» — мёртвая строка
удовлетворяет), `unit/gui-spike-bench.test.mjs:212,491` (round-trip `deepEqual(JSON.parse(file), parsed)` —
тавтология сериализации), `unit/gui-spike-auth.test.mjs:1401` (отсутствие секрета проверяется по
тексту DOM/логов — ок, но это поиск строки, а не состояние).

## 4. Mutation-кандидаты (измерено, 6 caught / 5 missed)

Прогон на копии `/tmp/opencode/g4-mut` (репозиторий не тронут), лог
`waves/RW-40/PM/qa-g4/mutation-smoke.log`. Baseline копии: frontend 170 pass.

Контекст: в проекте уже есть реестр мутаций — `docs/rust-client/MUTATION-LEDGER.md` (281 мутант,
278 KILLED, 99.6% по волнам B01/RW-17…RW-35). Он покрывает **Rust-крейты**; для JS/UI-слоя
(`rust/gui-spike/ui/**`, `src/**`) реестра и регулярного прогона нет — этот раздел его
первый срез (11 мутантов, 6 caught / 5 missed). Предложение — не новый файл, а секция
«JS/UI» в том же ledger с теми же правилами счёта (KILLED только по ожидаемому тесту,
restore + md5).

| # | Мутация (файл:строка) | Ожидание | Факт | Вывод |
|---|---|---|---|---|
| M1 | `features/quick-switcher.js:100` empty-state всегда виден | падение QA4-01 | **MISSED** (fail 0) | red-first дыра G2-03 |
| M2 | `features/composer.js:133` reply сбрасывается любой клавишей | падение G2-01 | **CAUGHT** (fail 1) | регрессия G2-01 защищена |
| M3 | `features/chat.js:189` text inset `-2` → `-8` | падение геометрии | **MISSED** | JS↔CSS геометрия строки не пинится (DP-02-класс) |
| M4 | `features/search-overlay.js:875` снять stale-seq guard | падение race-теста | **MISSED** | гонка ответов поиска не покрыта |
| M5 | `features/actions.js:125-126` `aria-expanded` всегда `"true"` | падение AT-контракта | **MISSED** | G2-06 не покрыт |
| M6 | `features/message-toolbar.js:244` не очищать `deletePending` | падение destructive-теста | **MISSED** | G2-11 не покрыт |
| M7 | `ui/model.js:229` `ROW_HEIGHTS.lineExtra` 22 → 18 | падение оракула высот | **CAUGHT** (fail 3) | геометрия модели защищена |
| M8 | `core/cdp/client.mjs:156` убрать guard `closed` | падение «закрытый клиент» | **CAUGHT** (fail 1) | защищено |
| M9 | `core/cdp/client.mjs:65` `pickTarget` без `type === "page"` | падение «страниц нет → undefined» | **CAUGHT** (fail 1) | защищено |
| M10 | `core/ui/walk-log.mjs:96` `shouldBail` на любой ошибке | падение «обычная ошибка не прерывает» | **CAUGHT** (fail 1) | защищено |
| M11 | `core/ui/walk-log.mjs:77` `consoleAPICalled` без `type=error` | падение счётчика ошибок | **CAUGHT** (fail 1) | защищено |

Rust-кандидаты (не прогонялись, нужен оракул для red-first; формат тот же):

| # | Мутация | Какой тест обязан упасть |
|---|---|---|
| R1 | `rest_failure.rs:73` `403 => Some(Forbidden)` → `None` | `search_contract::maps_other_http_errors_to_typed_errors_without_body_leak`, `messages_contract:73`, `reactions_contract:97` |
| R2 | `rest_failure.rs:98` `Client<429>` → `RateLimited{retry_after_ms:0}` заменить на `ClientOther{429}` | сейчас **нет** теста на 429-из-Client в messages/reactions → дыра (добавить) |
| R3 | `net.rs:264` `Stopped(_) => Closed(4013)` → `Closed(4000)` | `net_contract.rs` пинает 4013 — упадёт (**characterization**, не «правильное» поведение; G1-12) |
| R4 | `data.rs:1489` `ensure_day` — пропускать `2026-02-31` (уже так) | теста нет → red-first дыра G1-13 |
| R5 | `etf.rs:260` `panic!` на глубине оставить, но вызвать из `ws` c `u64::MAX` | `ws_transport.rs` границ не проверяет (G1-10) → дыра |

## 5. E2E: 4 skip — причины и что не покрыто

`node --test tests/e2e/*.test.mjs` → 4 теста / 4 skip, exit 0 (`e2e-skip.log`).

| Тест | Причина skip (фактическая) | Fixture без live возможен? |
|---|---|---|
| `cdp-browser.e2e.test.mjs:16` targets/eval/clicktext/list/shot | нет браузера в PATH и `CHROME_BIN` не задан | **да** (file:// + headless Chromium, локальный CDP) |
| `walk-fixture.e2e.test.mjs:18` обход пунктов + JSONL + PNG | то же | **да** |
| `live-discord.e2e.test.mjs:15` targets | `DISCORD_LAB_LIVE_CDP` не задан | нет (нужен запущенный Discord) |
| `live-discord.e2e.test.mjs:21` eval 1+1 | то же | нет |

**Главное (P1): два «fixture»-теста не просто пропущены — они сейчас были бы КРАСНЫМИ.**
`src/cli/run.mjs` реализует только `cdp targets`; `walk`/`domains`/`static` — заглушки
(«появится в M3/M4»). Проверено фактически, без сети (`e2e-surface-probe.log`):

```
node bin/discord-lab.mjs cdp clicktext "Мой аккаунт" → "Неизвестная команда cdp: clicktext", exit 1
node bin/discord-lab.mjs cdp eval "1+1"              → "Неизвестная команда cdp: eval",      exit 1
node bin/discord-lab.mjs cdp list button             → "Неизвестная команда cdp: list",      exit 1
node bin/discord-lab.mjs walk fixture ...            → "Команда walk появится в M3.",        exit 2
```

Следствие: `gate:release` → `passed_with_skips`, но `skip` здесь маскирует **не «нет среды», а
нереализованную функцию**. TEST-POLICY §10 («e2e всё равно skip, примем так») прямо называет это
недопустимым решением. Это не дефект теста — дефект учёта: D1 в `TASKS.md` сформулирован как
«e2e fixture (без live) включить», будто нужен только браузер.

Что не покрыто, пока e2e спит: реальный WS-хендшейк CDP и `Runtime.evaluate` по сети; `clicktext`
на настоящей геометрии; `list` по реальному DOM; `shot` (PNG, не «магии»); журнал `walk`
(JSONL+скриншоты) на реальном браузере; blacklist-пропуски на живом UI.

Минимальный путь включения (без live): `CHROME_BIN=<chromium|chrome|brave>` +
реализовать `cdp eval|clicktext|list|shot` и `walk` (или пометить e2e как `red-first под D1` с
отдельным статусом, а не `skip`). Кода в `src/cli/run.mjs` для этих подкоманд **нет** — это
отдельная карточка, не «настройка среды».

## 6. Bug-tracking: какие findings обязаны стать regression-тестами

Статус сверен с `QUALITY/BUGS.md`, `waves/RW-39/PM/DECISIONS.md` (D-80…D-83) и текущим деревом.
Важно: G1-01 **уже реализован в дереве** (`rest_failure.rs`, `SearchError::Forbidden/NotFound/Server`),
тесты messages/reactions/search обновлены — G1-AUDIT в этой части устарел.

| Приоритет | Finding | Статус | Нужен тест (оракул) | Red сейчас? |
|---|---|---|---|---|
| 1 | **QA4-04 / G1-02** keyring→Network | «closed (D-82)», но evidence — source-scrape | fake `TokenStore` → `Err("Secret Service недоступен")`: команда отдаёт домен-специфичную ошибку, причина не потеряна | да (kind = Network) |
| 2 | **G2-03** quick-switcher empty (QA4-01) | open | `hidden`-атрибут при 8 и 0 опциях | да (инверсия в коде) |
| 3 | **G2-11** sticky delete-confirm (QA4-02) | open | confirm A → hover B → один клик не удаляет B | да (label/pending липнут) |
| 4 | **G2-06** `aria-expanded` эмодзи | open | open→close/Esc → `"false"` | да (остаётся `"true"`) |
| 5 | **G1-04** keyring `recv()` без таймаута | open | `ThreadedStore` + `BlockingStore` → `Err` за ≤50 мс, текст «не ответил» | да (висит) |
| 6 | **G1-03** mutex через `/users/@me` | open | fake validator `sleep(200ms)`; `token()` из второго потока < 100 мс | да (сериализация) |
| 7 | **G1-13** `ensure_day` форма без календаря | open | `2026-02-31`, `2026-13-01` → `InvalidArg`; `2026-02-28` → Ok | да (проходит) |
| 8 | **G2-07** поиск без debounce | open | 5 `input` подряд → ровно 1 `search_remote` после 250 мс | да (5 вызовов) |
| 9 | **G2-09** таймеры auth перезаписываются | open | после `ready`/`no-session` `env.timers.size` не растёт; повторный 429 не плодит тики | да |
| 10 | **G2-12** `createElement("svg")` без namespace | open | `svg.namespaceURI === "http://www.w3.org/2000/svg"` (нужен аддитивный harness: сохранять namespace в `createElementNS`) | да (HTML-namespace) |
| 11 | **QA4-06 / plan-sync** токен в комментарии | open | strip-комментарии + импорт модуля | — (процессный) |
| 12 | **G2-13** embed-ссылка навигирует | open | клик по `.embed-title` не меняет `location`/не убирает `.shell` | да |
| 13 | **G2-14** двойной `renderMemberPanel` | open | два рендера → один листенер, ровно один `aria-current` | да |
| 14 | **G1-05** `reply_thread.rs` не в `lib.rs`, 5 `.bak*` | open | meta: `git ls-files rust \| grep -E '\.(bak\|backup)'` = пусто; модуль либо в сборке, либо удалён карточкой | да |
| 15 | **G1-10** `etf::encode` panic-API | open | heartbeat с `u64::MAX`/макс. `reasons` не паникует | да |
| 16 | **G1-12** `StopReason::ReconnectLimit → Closed(4013)` | open | таблица `StopReason → GatewayEvent` для 3 вариантов (после решения PM о смене кода) | characterization (пин 4013) |
| 17 | **G2-05** `applyFilter` игнорирует поля | open | `applyFilter({from})` меняет выдачу и контрол; `{pinned:true}` фильтрует | да |
| 18 | **Q-11** `.content{position:relative}` inline | open | геометрия (QA4-08) вместо inline-style | частично |

Уже защищено и не требует новой карточки: G2-01 (`features:2323`), G2-02 (`features:2360`),
G2-04 (`features:2423`), G1-01 (`{search,messages,reactions}_contract` типизированные таблицы),
Q-01 (регистрация ровно одного `join_guild` — но только текстом, см. QA4-04).

Правило для `BUGS.md`: пока у строки в колонке «Тест» стоит `features tests`/`targeted`/`meta/none`
(Q-02/Q-03/Q-04), это не тест-регрессия, а ссылка на файл. Предлагаю колонку «Тест» считать
валидной только при формате `<файл>:<строка или имя it/test>`.

## 7. Split-план тест-монолитов (без изменения assert)

Правила: только перемещение `it`-блоков и хелперов; набор `it`-имён до/после сравнивается
meta-тестом (см. §8, G-4); диф ревьюит не автор (TEST-POLICY §5).

### 7.1 `tests/unit/gui-spike-features.test.mjs` (2487) → 9 файлов + helper

| Новый файл | Диапазон (текущие строки) | `it` |
|---|---|---:|
| `tests/unit/gui-spike-nav.test.mjs` | 152-816 (QuickSwitcher, экраны, эмодзи, настройки-навигация, клавиши, консоль) | 17 |
| `tests/unit/gui-spike-chat.test.mjs` | 817-1200 (строки/extras/hover/edit/delete/reply) | 10 |
| `tests/unit/gui-spike-reactions.test.mjs` | 1200-1277 | 4 |
| `tests/unit/gui-spike-composer.test.mjs` | 1277-1437 | 5 |
| `tests/unit/gui-spike-members.test.mjs` | 1437-1566 | 3 |
| `tests/unit/gui-spike-search-local.test.mjs` | 1566-1770 | 6 |
| `tests/unit/gui-spike-search-remote.test.mjs` | 1770-2076 | 9 |
| `tests/unit/gui-spike-settings.test.mjs` | 2076-2310 | 7 |
| `tests/unit/gui-spike-g2.test.mjs` | 2310-2487 (G2-01/02/04) | 3 |
| `tests/support/gui-app-helpers.mjs` (новый) | `byId/all/firstNode/chatRow/scrollChat/withApp/api/remoteSearch/loadApp`-обёртки | — |

### 7.2 `tests/unit/gui-spike-auth.test.mjs` (1425) → 5 файлов + helper

| Новый файл | Строки | `it` |
|---|---|---:|
| `gui-spike-auth-markup.test.mjs` | 272-451 (разметка, id, CSS, QR) | 6 |
| `gui-spike-auth-login.test.mjs` | 452-522 | 3 |
| `gui-spike-auth-captcha.test.mjs` | 522-671 | 3 |
| `gui-spike-auth-mfa.test.mjs` | 671-928 | 11 |
| `gui-spike-auth-token-restore.test.mjs` | 1067-1351 | 8 |
| `gui-spike-auth-errors.test.mjs` | 928-1067 + 1351-1425 | 10 |
| `tests/support/gui-auth-helpers.mjs` | `loadAuthApp/domDump/assertSecretGone/type`, патч `focus` один раз на файл | — |

Порядок безопасности: сначала вынести хелперы (файл-монолит зелёный), затем переносить блоки
по одному; после каждого шага — `node --test tests/unit/gui-spike-*.test.mjs` (170 pass) и
сверка числа `it` (213 суммарно).

### 7.3 `tests/support/gui-dom-harness.mjs` (1397) → 4 модуля + фасад

| Новый модуль | Что уходит | Публичный контракт |
|---|---|---|
| `shim-dom.mjs` | `StyleDecl/ClassList/Dataset/ShimElement/HTMLInputElement/HTMLButtonElement/ShimDocument` | `export *` |
| `shim-env.mjs` | глобалы, таймеры, rAF, `ResizeObserverShim`, IPC-заглушки | `export *` |
| `fixture-dom.mjs` | `buildDom` (статический `index.html`-каркас) | `export *` |
| `gui-dom-harness.mjs` | `GuiDom/loadApp` + `createElementNS` (аддитивно: сохранять namespace — нужно для G2-12) | прежние имена, `export *` из трёх модулей |

Ограничение: harness — общий шим, менять **только аддитивно**; после сплита обязательны
`unit` 384/0 и `auth` секрет-тесты.

## 8. Гейт: что добавить (и что сейчас слепо)

### 8.1 Дыры текущего `gate:worker`/`gate:release` (факты)

1. **Rust-ядро не в гейте.** Фазы: typecheck, ui-tsc, meta, unit, `spike-rust`
   (`cargo test --manifest-path gui-spike/Cargo.toml`), smoke; в release ещё e2e/coverage/golden.
   `cargo test --workspace` (core 574 / workspace 1112 по baseline) **не запускается** ни в одном
   скоупе — зелёный `gate:release` ничего не говорит о `discord-core`. PROCESS §10 требует
   workspace на batch gate, но это ручная дисциплина, а не проверка.
2. **Coverage слеп к CLI.** Отчёт покрывает 14 файлов `src/**` и **не содержит
   `src/cli/run.mjs`** (350 строк, ~25% `src`): файл не импортируется unit-тестами, поэтому
   Node не включает его в отчёт, а порог 90% проходит. Все подкоманды `cdp/walk/domains/static/hook`
   фактически вне покрытия.
3. **e2e skip = pass.** Вердикт `passed_with_skips`, exit 0, предупреждение только текстом
   (`gate.mjs:362`). На текущем дереве это скрывает 2 теста, которые были бы красными (§5).
4. **Golden-манифест не проверяется на полноту.** `goldenPhase` делает `md5sum -c` по старому
   манифесту (18 окт): файлы, добавленные в `logs/functions/evidence` позже, в сверку не попадают;
   отсутствие манифеста вообще возвращает `ok: true` c note.
5. **`test-policy` мета-тест не видит `skip`.** `TEST-POLICY §2.3` запрещает `skip` без причины, но
   `tests/meta/test-policy.test.mjs` сканирует только `.only/.todo/assert.ok(true)/--no-verify`.
   `t.skip(...)`/`{ skip: ... }` можно добавлять молча.
6. **`test-policy` проверяет гейт по подстроке.** `gate.includes('"e2e"')` — фаза может быть
   отключена передачей пустого списка, а проверка останется зелёной.

### 8.2 Предлагаемые проверки (по возрастанию цены)

| # | Проверка | Куда | Что ловит |
|---|---|---|---|
| G-1 | `rust-fmt` (`cargo fmt --check`), `rust-clippy` (`-D warnings`), `rust-workspace` (`cargo test --offline --workspace`) | `commonPhases` (clippy/fmt — release) | ядро и линт вне гейта сейчас |
| G-2 | `e2e-required`: в `--scope release` `skipped > 0` в фазе e2e → **failed** (или явный `--allow-e2e-skip "<причина>"` с записью в отчёт) | `gate.mjs` | «skip ≠ pass» из TEST-POLICY §3.1 |
| G-3 | `e2e-fixture`: если `CHROME_BIN` задан, но бинарь не исполняется — fail, не skip; `E2E_REQUIRE=1` превращает «нет браузера» в fail | `e2e/*` + gate | «нет среды» vs «нет кода» |
| G-4 | `test-inventory`: meta-тест сверяет список `it`-имён по слоям с эталоном (снимок), падает на удаление/переименование без записи в `DECISIONS.md` | `tests/meta/test-inventory.test.mjs` | тихое удаление тестов (п.2 §2 TEST-POLICY) |
| G-5 | `mutation-smoke`: скрипт `scripts/mutation-smoke.mjs` с фиксированным списком мутантов (M1…M11 из §4) и эталоном caught/missed; запускать в release (~10 с), результат — секция «JS/UI» в `docs/rust-client/MUTATION-LEDGER.md` по тем же правилам (KILLED/eq/inconclusive, restore+md5) | gate release | «тест есть, регресс не ловит»; сейчас даёт 5 missed — эталон фиксирует долг и требует закрывать |
| G-6 | `source-scrape budget`: meta-тест запрещает `include_str!`/`readFileSync(ui/**)` как **единственный** assert в `it`/`test` (явный allowlist с причиной) | `tests/meta/test-policy.test.mjs` | QA4-03/04/05/07/10-класс |
| G-7 | `coverage-cli`: добавить `tests/unit/cli-run.test.mjs`, который импортирует `run.mjs` и проверяет диспетчеризацию на заглушках (help/version/неизвестная команда/`cdp` без подкоманды) — файл появится в coverage; плюс `--test-coverage-branches=90` уже есть | unit + coverage | 25% `src` вне метрики |
| G-8 | `golden-freshness`: файл в `logs/functions/evidence` новее `.gate/golden.manifest` → failed | gate release | «golden сверен не полностью» |
| G-9 | `plan-sync` strip-комментарии перед поиском токенов; требования к импорту | `tests/meta/plan-sync.test.mjs` | QA4-06 |
| G-10 | `test-policy` сканирует `it.skip`/`{ skip:` и требует непустую причину-строку; проверяет, что фазы гейта берутся из массива, а не из подстроки | `tests/meta/test-policy.test.mjs` | п.5–6 §8.1 |

Порядок: G-1, G-2, G-7 — обязательны сразу (закрывают слепые зоны без роста цены);
G-4/G-5/G-6 — после сплита тестов (§7), чтобы дифы были читаемыми.

## 9. Что не делалось (границы аудита)

- Не запускались: CDP-клиент против живой страницы, Frida, Discord, AT-SPI/X11-проба (`DISPLAY=:0`
  есть — поэтому smoke/live не гонялись намеренно), `gate:worker/release` целиком.
- Rust-мутации не выполнялись (только кандидаты R1–R5); cargo-прогоны не дублировались.
- Код, тесты, фикстуры, `TEST-POLICY.md`, `gate.mjs`, `QUALITY/*` не изменялись.
- `logs/`, `functions/`, `evidence/` не трогались.
- Дерево грязное и менялось во время аудита (§0) — md5 в шапке относятся к снимку ~12:1x;
  если G1 продолжит правки `gui-spike`, часть статусов в §6 сдвинется.

## 10. Артефакты

| Артефакт | md5 |
|---|---|
| `waves/RW-40/QUALITY/TEST-AUDIT.md` (этот файл) | см. ответ PM |
| `waves/RW-40/PM/qa-g4/node-test-meta.log` | 39/0 |
| `waves/RW-40/PM/qa-g4/node-test-unit.log` | 384/0 |
| `waves/RW-40/PM/qa-g4/e2e-skip.log` | 4 skip |
| `waves/RW-40/PM/qa-g4/coverage.log` | 99.40/91.88/97.03, `run.mjs` отсутствует |
| `waves/RW-40/PM/qa-g4/mutation-smoke.log` | 6 caught / 5 missed |
| `waves/RW-40/PM/qa-g4/e2e-surface-probe.log` | e2e-поверхность CLI = заглушки |
| `/tmp/opencode/g4-mut/` (копия для мутаций), `/tmp/opencode/g4-mut*.mjs` | вне репозитория |
