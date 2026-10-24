# QA-E2E-PLAN — 4 e2e, семь потоков M2 и минимальный T2/T3

Дата: 2026-10-24. Роль: **G4 QA**. Режим: **read-only** — браузер, Chrome, сеть, Discord
и live-сценарии не запускались; тесты, код и `package.json` не менялись. Изменён только
этот отчёт.

Вход: `tests/e2e/*`, `tests/smoke/*`, `scripts/gate.mjs`, `scripts/test-impact.mjs`,
`waves/RW-40/QUALITY/QA-STRATEGY.md`, `QA-AUDIT.md`, `BUGS.md`, `docs/rust-client/TEST-PLAN.md`,
`package.json`, `waves/RW-40/MASTER-PLAN.md` §6, `LOAD-BALANCE.md` §5.

Фактическая проверка (без браузера):

```text
$ env -u CHROME_BIN -u CHROME_PATH -u DISCORD_LAB_LIVE_CDP \
    node --test 'tests/e2e/**/*.test.mjs'
ℹ tests 4   ℹ pass 0   ℹ fail 0   ℹ skipped 4
```

`CHROME_BIN`, `CHROME_PATH`, `DISCORD_LAB_LIVE_CDP` не заданы; `chromium`, `chrome`,
`brave`, `edge` в `PATH` отсутствуют. `DISPLAY=:0` есть, но он не используется: e2e-набор
не стартовал ни одного процесса браузера.

**M2 здесь — milestone «M2 QA handoff»** (`MASTER-PLAN.md` §6: e2e/browser, QA-матрица,
P0/P1=0; live отдельно), а не исторический M2 «Frida-слой» из `TASKS.md`.

---

## 0. Краткий вывод

1. **4/4 e2e — skip.** Node показывает `✔` у describe-блока, но `pass 0 / skipped 4`;
   это не прохождение. `gate:release` в таком состоянии даёт `passed_with_skips` и exit 0.
2. **Код `cdp eval|clicktext|list|shot` и `walk fixture` в рабочем дереве есть**
   (`src/cli/run.mjs`, `src/core/cdp/client.mjs`). Утверждение старого `TEST-AUDIT.md`
   «кода подкоманд нет» устарело. Оставшийся блокер — браузер, CI-job и политика skip.
3. **Семь потоков M2 уже покрыты fixture-контрактами без браузера** (auth 46, rail 8,
   channel-switch 9, join 24, voice 33, features 98, включая reconnect). P1A — это не
   переписывание unit, а **browser E2E на реальном `ui/index.html` + fake IPC + CI**.
4. **Найден P1-блокер сценария 3 (search remote error):** в реальном приложении
   `search_remote` не вызывается вообще. `app.js:1200-1206` передаёт `guildId` из
   `.guild-item.is-selected` **в момент boot**, когда рейл ещё пуст (плитки строятся
   только после `ready()`); `search-overlay.js:244` фиксирует пустую строку, а
   `fetchRemote()` при `guildId === ""` сразу выходит (`:883`). DOM-шим маскирует это:
   `tests/support/gui-dom-harness.mjs:810-821` создаёт три статичные плитки, одну
   выбранную. Browser E2E обязан покраснеть до фикса G2; PM заводит P1-запись в
   `BUGS.md` (owner G2) и одну regression-проверку.
5. **Live T3 не заменяется fixture-результатом.** Fixture доказывает UI/wiring против
   стаба, но не реальный READY, отправку, invite TTL и голос.

---

## 1. Таблица текущих e2e (4 тест-кейса в 3 файлах)

| # | Тест / файл | Что проверяет | Почему skip сейчас | Oracle / fixture | CHROME_BIN / CI-путь |
|---|---|---|---|---|---|
| 1 | `cdp против headless-браузера: targets/eval/clicktext/list/shot` — `tests/e2e/cdp-browser.e2e.test.mjs` | Реальный CLI → CDP против локальной страницы: `targets`, `document.title`, `1+1`, клик «Мой аккаунт», текст `#panel`, 6 кнопок, PNG | `findBrowser()` не нашёл `CHROME_BIN`/`CHROME_PATH` и ни одного известного бинаря в `PATH`; вызов `t.skip` (строка 18) | `tests/fixtures/app/index.html`: title `Discord Lab Fixture`, ровно 6 `BUTTON`, панель «Аккаунт…», PNG signature `89 50 4e 47`. Ожидания заданы разметкой фикстуры, не выводом CLI | Нужен реальный Chromium/Chrome: локально `CHROME_BIN=/usr/bin/chromium`; в CI — job с установленным/pinned Chromium и обязательной проверкой `test -x "$CHROME_BIN"` |
| 2 | `walk fixture: обход пунктов, запрет, JSONL и скриншоты` — `tests/e2e/walk-fixture.e2e.test.mjs` | CLI `walk fixture`: 4 обычных пункта кликаются по порядку, «Покинуть сервер»/«Выйти» пропускаются, JSONL + 4 PNG, page-side `__clickLog`, одна ожидаемая console-ошибка | Тот же `findBrowser()` и `t.skip` (строка 20) | Та же `tests/fixtures/app/index.html`; ожидаемые `label`/`step` `[1..4]`, чёрный список, PNG magic, `window.__clickLog`; ошибка `Уведомления` детерминирована фикстурой | Тот же `CHROME_BIN`; обязателен отдельный `--user-data-dir` и последовательный запуск (Chrome не параллелить) |
| 3 | `live Discord: targets показывает страницы Discord` — `tests/e2e/live-discord.e2e.test.mjs` | Подключение к уже запущенному Discord CDP и наличие `type=page` target | `DISCORD_LAB_LIVE_CDP` не задан; skip через `{ skip: ... }` (строки 10-15) | Заранее запущенный Discord с `--remote-debugging-port=9223`; протокол CDP и строка `\tpage\t`. Это не fixture и не CI-сценарий | **Не `CHROME_BIN`.** Только отдельное согласованное T3-окно; endpoint из защищённой конфигурации, не из репозитория |
| 4 | `live Discord: eval 1+1 выполняется на живой странице` — `tests/e2e/live-discord.e2e.test.mjs` | `Runtime.evaluate` через CLI на живой странице, JSON `2` | Тот же `DISCORD_LAB_LIVE_CDP`; skip (строки 10-12, 21) | Тот же живой endpoint; oracle — JSON `2` из CDP runtime | Только T3-окно. В обычном CI и в browser-job не запускать |

Важно: кейсы 1–2 проверяют **CLI/CDP/walk harness** на мини-странице
`tests/fixtures/app/index.html`, а не семь продуктовых потоков M2 на реальном
`rust/gui-spike/ui/index.html`.

Правила статуса для этой таблицы:

- `skipped 4` — это **0 доказанных сценариев**, а не «e2e зелёный»;
- suite-level `✔` в Node означает лишь «файл выполнился без fail», а не pass вложенных
  skip-тестов;
- для live: **не задан endpoint → проверка не выполнена (unmet T3)**; задан, но
  соединение/контракт падает → **fail**, не skip.

---

## 2. Семь сценариев M2

Источник — `QA-STRATEGY.md` §4. Уровни ниже разделены явно:

- **fixture-contract (без браузера)** — Node + DOM-шим/fake transport; уже существует,
  это T0/T1, **не E2E**;
- **browser fixture** — настоящий headless Chromium, реальный `rust/gui-spike/ui/index.html`,
  `window.__TAURI__.core.invoke` подменён детерминированным fake; сети/live нет;
- **live T3** — только то, что нельзя доказать стабом; отдельное окно.

### 2.1 Сценарии, fixture и oracle

| # | Сценарий M2 | Fixture / IPC-команды | Oracle и browser-assert | Live T3? | Существующее покрытие без браузера |
|---|---|---|---|---|---|
| 1 | Login → READY → rail | `auth_status` → `{status:"no-session", stored:false}`; `auth_login_token` → `{status:"ready", user}`; `ready` → guilds из `fixtures/gateway/by-type/002-READY.json`; `channels({guildId})` | Гейт `[data-state=ready]` скрыт/`aria-hidden`, `.shell` открыт; рейл: 7 `button.guild-item[data-guild-id]` в порядке READY, первая выбрана, имя из `properties.name`; `auth-token.value === ""`; `ready()` вызван только после auth. Oracle — READY-файл (7 гильдий), не DOM-шим | Да, реальный `auth_status/login` (T3) | `gui-spike-auth.test.mjs` (46), `guild-rail.test.mjs` (8) |
| 2 | Открыть канал → send/edit/delete/reply | `ready`; `channels`; `messages_page` с `1163408591315812392`; `send_message`, `reply_message`, `edit_message`, `delete_message`; `read_state`, `ack_message` | После выбора гильдии `1014537851418980372` дерево содержит канал; `messages_page` рендерит сообщения из `*-MESSAGE_CREATE.json`; send/reply/edit/delete получают **реальный** `channelId` и `messageId` (не `"news"`/index); цитата reply видна; ошибка send видима, а не проглочена. Oracle — READY + MESSAGE_* + журнал fake invoke | Да, реальная отправка (P0-02) — только в разрешённом окне | `channel-switch.test.mjs` (9) |
| 3 | Search remote error → no fallback | `search_remote` reject `{kind:"unauthorized"}` (и таблица `forbidden/notFound/client/server/rateLimited/storage`); счётчик вызовов | `.search-status[data-state=error]` с текстом; `matchesCount === 0`; **ноль** локальных результатов и ноль повторных fallback-запросов; журнал: ровно один `search_remote` с выбранным `guildId` | **Нет** (ошибку нельзя стабильно и безопасно инжектировать в live) | `gui-spike-features.test.mjs` (набор search error) |
| 4 | Join invite → preview → accept → rail | `resolve_invite({code})` → preview; `accept_invite({code})` → `{guildId}`; `join_guild({guildId})` | Модалка показывает preview (имя/участники/срок); после accept — success-текст и новая плитка в рейле; журнал вызовов ровно один на шаг; ошибки `[S]`-контракта видимы и retry не дублирует accept | Да, реальный код/TTL/`USER_BANNED` (G-03-LIVE) | `join-server.test.mjs` (24) |
| 5 | Settings → density/theme → reload persistence | `settings_load` → `{version:1, toggles, density:"cozy", theme:"dark", locale:"ru"}`; `settings_save` → сохраняет payload в fake-store, переживающий reload | Клик «compact» и «light» меняет `html[data-density]`/`html[data-theme]` и `aria-pressed`; после `Page.reload` fake `settings_load` возвращает сохранённое, атрибуты и выбранные опции восстановлены; `settings_save` не содержит чужих ключей/секретов. Для density oracle — `tokens.css:361` (`--message-gutter:80px`), для theme — только атрибут: light-CSS в спайке пока нет | **Нет**; реальный файл настроек проверяется Tauri smoke/unit | `gui-spike-dom/features`, `design-breakpoints` |
| 6 | Reconnect/offline banner → retry | `net_status` → последовательность `ready → reconnecting → offline`; `net_connect` → успех/401 | Баннер `.net-status` видим с текстом «Переподключение…»/«Нет соединения»; после успешного retry и `net_status=ready` баннер скрыт; 401 → «Сессия истекла. Войдите снова.»; нет retry-loop, ровно один `net_connect` на клик | Опционально: контролируемый разрыв сети в окне; для M2 не обязателен | `gui-spike-features.test.mjs` § L-01 |
| 7 | Voice panel disabled/idle | fake `voice.store.set({state:"idle"})`; `service:null` в сборке; журнал IPC/audio | Начальное `data-state=disabled`, join с видимой причиной; после `store.set("idle")` — idle-текст, join доступен, участники пусты; **ноль** `send_typing`/network/audio вызовов. Реальный `getUserMedia` не вызывается | Да, только реальное подключение/аудио — отдельное T3, вне M2 fixture acceptance | `voice-ui.test.mjs` (33) |

### 2.2 Ответ на «fixture-only vs браузер vs live»

- **Fixture-only без браузера:** все 7 уже можно проверять и проверяются на DOM-шиме.
  Это дешёвый T1-уровень, но он **не** доказывает реальный DOM/CSS, порядок загрузки
  модулей, `reload`, поведение браузера и не заменяет browser E2E.
- **Browser fixture без live:** все 7 (включая 3, 5, 6). Один и тот же fake transport,
  но страница — настоящий `ui/index.html`. Это минимальный состав M2 acceptance.
- **Live T3:** 1 (реальный вход/READY), 2 (реальная отправка/правка/удаление/reply),
  4 (реальный invite/TTL), 7 (реальное аудио/устройства). Для 3 и 5 live не нужен;
  6 — только если PM включит контролируемый разрыв в окно.

**M2 нельзя принять**, если семь browser-сценариев не существуют или скипаются; зелёные
unit/DOM и `gate:release` с `passed_with_skips` для этого недостаточны.

---

## 3. Минимальный T2/T3 набор и команды

### 3.1 Предусловия

- Node ≥ 24 (`package.json.engines`), `npm ci` уже выполнен.
- Реальный Chromium/Chrome доступен по **проверенному** пути; `CHROME_BIN` указывает на
  исполняемый файл. Не устанавливать браузер динамически внутри теста.
- Один browser-job на runner; никаких параллельных `gate:release`.
- Live-переменные (`DISCORD_LAB_LIVE_CDP`, токены) в T2 **не заданы**.

### 3.2 T2 — release / M2 acceptance (fixture, без live)

После реализации harness и gate-патча (§4):

```sh
# 0. Preflight: отсутствие браузера — infra fail, не skip
test -x "$CHROME_BIN" || { echo "FAIL: CHROME_BIN не исполняется"; exit 1; }
node --version

# 1. Минимальный browser-набор M2: 2 старых fixture-теста + 7 сценариев, строго последовательно
CHROME_BIN=/usr/bin/chromium node --test --test-concurrency=1 \
  tests/e2e/cdp-browser.e2e.test.mjs \
  tests/e2e/walk-fixture.e2e.test.mjs \
  tests/e2e/m2-*.e2e.test.mjs

# 2. Полный release-гейт (typecheck, ui-tsc, meta, unit, rust, smoke, e2e, coverage,
#    keyring-live с failOnSkips, golden)
CHROME_BIN=/usr/bin/chromium npm run gate:release

# 3. Машинная проверка, что e2e реально прошёл, а не скипнулся
jq -e '
  .phases[] | select(.name == "e2e") |
  (.ok == true) and (.counts.fail == 0) and (.counts.skipped == 0)
' .gate/release-*.json
```

Текущее состояние (до реализации): шаг 1 даст `pass 0 / skipped 4`, шаг 3 — false.
Это **не** M2 pass.

T2 обязательные фазы (из `scripts/gate.mjs`): `typecheck`, `ui-tsc`, `meta`, `unit`,
`spike-rust`, `workspace-rust`, `smoke`, `e2e`, `coverage`, `keyring-live`, `golden`.
`keyring-live` требует Secret Service/`secret-tool`; без него T2 неполон.

### 3.3 T3 — live, только согласованное окно

```sh
# Preflight: Discord уже запущен с --remote-debugging-port=9223
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 \
  node bin/discord-lab.mjs cdp targets

# Минимальные live-проверки CDP (2 кейса)
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 \
  node --test tests/e2e/live-discord.e2e.test.mjs

# Release + live keyring/qualification
DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 \
DISCORD_LAB_KEYRING_LIVE=1 \
  npm run gate:qualify
```

Плюс ручной чек-лист T3 (каждый пункт — evidence: команда, время, результат, без токена):
READY/рейл; send/edit/delete/reply в тестовом канале; resolve/accept одноразового invite;
reconnect; voice join/leave (если в scope M2); keyring lock/unlock. Владелец окна —
PM; таймбокс и пред/пост-проверка обязательны.

### 3.4 Что нельзя считать pass

- `ℹ tests 4 / pass 0 / skipped 4` и `✔` у describe-блока;
- вердикт гейта `passed_with_skips` и exit code 0;
- отсутствие браузера в CI (это **infra fail**, а не «нет среды»);
- `CHROME_BIN` задан, но путь не исполняется/бинарь не стартует;
- live endpoint не задан (unmet T3), а также live endpoint задан, но недоступен
  (это **fail**, не skip);
- один pass после одного fail (flaky/inconclusive; нужны оба прогона);
- зелёные unit/DOM-тесты вместо browser E2E;
- `skip` в обязательной фазе T2, даже если остальные фазы зелёные.

### 3.5 Что должен изменить gate (владелец — PM/TL, не этот воркер)

- e2e-фаза: `failOnSkips: true` для fixture-набора; live-файл исключить из неё.
- live-фаза: только `qualify`, с `failOnSkips: true` и явным env; без endpoint — unmet.
- при retry сохранять **оба** результата (сейчас в `.gate/*.json` остаётся только
  последняя попытка и note «первый прогон: fail»).
- `--test-concurrency=1` для e2e и один browser-job на runner.

---

## 4. Разделение работ

| Владелец | Что делает | Файлы / артефакты | Граница / приёмка |
|---|---|---|---|
| **G4 — test harness/QA** | Browser-harness: запуск реального `ui/index.html`, инъекция fake `__TAURI__`, ожидания по предикатам, журнал IPC, изоляция профиля, cleanup, таймауты; 7 e2e-файлов (один test id = один сценарий); мета-инвариант «7 сценариев существуют и не скипаются»; red-first лог для search-бага | `tests/helpers/ui-fixture.mjs`, `tests/helpers/browser.mjs` (timeout/cleanup), `tests/e2e/m2-login-ready-rail.e2e.test.mjs`, `m2-messages.e2e.test.mjs`, `m2-search-error.e2e.test.mjs`, `m2-invite.e2e.test.mjs`, `m2-settings-reload.e2e.test.mjs`, `m2-reconnect.e2e.test.mjs`, `m2-voice-idle.e2e.test.mjs`, `tests/meta/e2e-required.test.mjs` | Не менять продуктовый контракт ради теста; не подменять oracle скриншотом; gate — только согласованный integration-патч |
| **G5 / CI browser** | Поддерживаемый pinned Chromium в runner; `CHROME_BIN`; node 24; `npm ci`; последовательный browser-job; preflight «нет браузера → fail»; артефакты логов/скриншотов; очистка профиля; **без live** | `.github/workflows/ci.yml` — новый job `qa-browser`; CI-образ/runner; `.gate` в artifacts | Не запускать Discord/live; не делать браузер необязательным. Сейчас CI (`ci.yml` → `scripts/ci.sh --online --skip-release`) вообще не запускает Node-тесты — это надо явно добавить |
| **G1/G2 — fixture/product contract** | Детерминированный fake transport из уже курированных фикстур: `ready` (7 гильдий), `channels`, `messages_page` (`*-MESSAGE_CREATE/UPDATE`), `resolve/accept/join`, `settings_load/save`, `net_status/connect`, `process_metrics`, `spike_config`, `report_*`; журнал вызовов; ошибки `SearchError`; persist настроек через reload; **фикс boot-порядка search `guildId`** | `tests/fixtures/ui/*.json` + `tests/fixtures/ui/fake-tauri.mjs` + `MANIFEST.json` (provenance/md5); продуктовая правка `ui/app.js`/`search-overlay.js` для сценария 3 + unit-регрессия | Fixture не содержит токенов/секретов и не ходит в сеть; ожидаемые значения — из `fixtures/gateway/by-type/*` и контракта, **не** из кода под тестом; фикс search — red-first, отдельная карточка G2 |
| **G4b — secrets** | Инвентарь T3-секретов (Discord token, keyring/Secret Service, одноразовый invite); ephemeral-инъекция только в T3-job; redaction/скан логов и артефактов; запрет токенов в fixtures; отдельная keyring-live фаза; проверка symlink-fixture (PLAT-11b) | T3 env/secret store, `.gitignore` для артефактов, redaction-тест/чек-лист, отчёт по `DISCORD_LAB_KEYRING_LIVE` | Никаких реальных пользовательских токенов в CI-артефактах, fixtures, скриншотах и `cdp eval`-выводе; T2 не требует секретов вовсе |

Зависимости: G1/G2 фиксируют fixture и oracle → G4 пишет harness/сценарии → G5 даёт
браузерный runner → PM применяет gate-патч → T2 → отдельно T3. Фикс search (G2) можно
делать параллельно с harness, но browser-тест 3 должен быть red-first.

---

## 5. Риски

| Риск | Причина / триггер | Последствие | Мера |
|---|---|---|---|
| **Flaky старт браузера** | `launchBrowser` ждёт `DevToolsActivePort` 15 с; каждый CLI-вызов ограничен 20 с (`helpers/cli.mjs:32`) | Ложный fail на нагруженном CI | Явные предикаты/waitFor, диагностика (stderr браузера, порт-файл), таймауты harness настраиваемые; повтор — только с сохранением обоих логов |
| **Параллельный Chrome** | `node --test` по умолчанию запускает файлы параллельно; gate не задаёт concurrency | RAM/CPU contention, тайминги CDP, несколько профилей, медленный `SIGKILL` | `--test-concurrency=1`; максимум 1 Chrome на runner; не запускать browser-suite одновременно с `gate:release` |
| **Осиротевшие процессы/профили** | `stopBrowser` делает `SIGKILL` и не ждёт `exit`; `makeTmpDir` удаляет каталог | Зомби, занятый профиль, мусор в `/tmp` | Ждать `close` с таймаутом, kill process group, уникальный `user-data-dir`, cleanup в `t.after` |
| **Неоднозначный target** | `CDP_TARGET=index.html`; при нескольких страницах `pickTarget` берёт первую по hint/`discord` | Клик/eval не по той странице | Запускать отдельный браузер на fixture; уникальный title/URL; проверять `targets` до действий |
| **Fidelity фикстур** | DOM-шим содержит статичные `guild-item` (`gui-dom-harness.mjs:810-821`), которых нет в `index.html`; boot-порядок отличается | Unit зелёный, реальный UI сломан (search `guildId`) | Browser E2E только на реальном `index.html`; мета-тест на соответствие шима и HTML; red-first на найденный баг |
| **Потеря flake-evidence** | gate `retries:1` хранит только последнюю попытку; первый output не пишется | «Зелёный» скрывает нестабильность | Патч gate: сохранять оба прогона/логи; первый fail помечать flaky |
| **Ложный pass из-за skip** | `passed_with_skips`, exit 0; suite `✔` при `pass 0` | M2 принят без проверок | `failOnSkips` для e2e; jq-проверка `skipped==0`; отдельный live-статус |
| **Live-риски** | Аккаунт/сессия, 429, TTL invite, смена target, устройства/PipeWire, токен в логах | Нестабильность и утечка секретов | Только T3-окно с владельцем; preflight; read-only где возможно; redaction; без CI |
| **Браузерная версия и `file://`** | Разные Chromium/Chrome, storage на `file://`, `--headless=new` | Разное поведение reload/persistence | Pinned-версия в CI; fake-store переживает reload (`window.name`/`sessionStorage`) с явной проверкой доступности; фиксировать версию в отчёте |
| **Лимит воркеров** | Несколько тяжёлых запусков одновременно | Блокировка target-dir/CPU, flaky | По `AGENTS.md`: максимум 2 фоновых запуска, 1 `cargo`, 1 browser-suite; следующий — после завершения предыдущего. Browser E2E — только G4/G5, не каждый воркер |

---

## 6. Порядок P1A (предложение)

1. **G1/G2:** собрать `tests/fixtures/ui/*` из уже существующих
   `fixtures/gateway/by-type/*` (+ `MANIFEST.json`), написать fake `__TAURI__` с журналом
   и persist; отдельной карточкой — red-first фикс search `guildId`.
2. **G4:** harness + сценарий 1 (login→ready→rail) на локальном Chromium; зафиксировать
   red/green; затем сценарии 2, 4, 6, 7; 3 — после/вместе с фиксом G2; 5 — reload.
3. **G5:** CI-job с pinned Chromium и preflight; запускать только fixture-набор.
4. **PM/TL:** gate-патч (`failOnSkips`, split live, сохранять retry-логи); затем T2.
5. **T3:** отдельное окно; live-отчёт и evidence; не смешивать с M2 fixture acceptance.

**Definition of done P1A:** 7 сценариев имеют стабильные test id; browser-набор даёт
`fail 0 / skipped 0`; CI-job краснеет при отсутствии браузера; gate не может дать
`passed_with_skips` за M2; search-баг закрыт red-first; live явно помечен как отдельный
невыполненный/выполненный контур.

---

## 7. Основание и evidence

- `QA-STRATEGY.md` §2–§4, §8 — уровни T0–T3, семь потоков, `skip ≠ pass`, retry 1.
- `QA-AUDIT.md` §2, §4 — QA-D6 (4/4 e2e skip), P1 browser CI, сценарное покрытие.
- `BUGS.md` Q-09 — e2e skip / отсутствующий fixture-прогон, owner G4; QA4-01a.
- `TEST-AUDIT.md` §5 — 4/4 skip и минимальный путь включения; замечание «кода cdp
  подкоманд нет» **устарело** для текущего рабочего дерева.
- `docs/rust-client/TEST-PLAN.md` §1, §1.2, §5 — уровни, независимый oracle, live вне CI.
- `MASTER-PLAN.md` §6 — M2 QA handoff: e2e/browser, P0/P1=0, live отдельно.
- `package.json` — `test:e2e`, `gate:release`, `gate:qualify`; отдельных fixture/live
  скриптов пока нет.
- `scripts/gate.mjs` — e2e `retries:1`, без `failOnSkips`; `passed_with_skips` → exit 0.
- `.github/workflows/ci.yml` — только Rust-контур, Node/browser job отсутствует.
- `tests/helpers/browser.mjs` — `findBrowser`, 15 с ожидания, `SIGKILL`.
- `tests/helpers/cli.mjs` — 20 с на CLI-вызов.
- `tests/support/gui-dom-harness.mjs:810-821` — статичные плитки гильдий в шиме.
- `rust/gui-spike/ui/app.js:1200-1206`, `features/search-overlay.js:117-123,244,883` —
  boot-фиксация `guildId` и ранный выход `fetchRemote`.
