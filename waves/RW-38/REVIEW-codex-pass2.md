# RW-38 — REV-1, второе ревью (pass2): проверка снятия P1 и TEST-POLICY

Дата: 2026-10-24. Ревьюер: **REV-1** (не автор кода). Код не менялся: только чтение,
прогоны и этот файл. Все воспроизведения — в `/tmp/opencode/`; в репозитории изменён только
этот отчёт, плюс live-прогоны штатно дописали `waves/window-runs.jsonl` (2 записи, см. §0).

Контекст байтов: `HEAD 0239543`, дерево dirty; `rust/gui-spike/` целиком untracked.
Текущие файлы **не совпадают** с `waves/RW-38/workers/W38f/md5-final.txt` (после W38f
волны W39c/W39d/… разложили `ui/app.js` на `ui/features/*`, `ui/components/*` и правили
пробу/smoke). Поэтому проверялись **и** сохранённые red/green-логи W38f, **и** текущее
поведение; все заявленные фиксы P1 в текущих байтах присутствуют и воспроизведены.

Окружение: `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-1`, AT-SPI-шина жива, `secret-tool` в PATH.
Один `cargo` за раз; фоновых сборок не запускал.

## 0. Что реально прогнано (команда → exit → артефакт)

| # | Команда | exit | Артефакт / итог |
|---|---|---|---|
| 1 | `python3 scripts/gui-window-probe.py --json /tmp/opencode/rev1-pass2-probe.json --runs-log /tmp/opencode/rev1-pass2-runs.jsonl --timeout 30` | 0 | `ok:true`, `pid 646889 == window.pid`, окно `0x1c00003` 1280×820, PNG 5037 цветов, `emoji_cells:20`, 38 тумблеров |
| 2 | «старый» экземпляр (pid 647314, окно `0x1800003`, `_NET_WM_PID=647314`, дефолтный заголовок) + проба №1 | 0 | проба взяла **своё** окно `0x1c00003` (pid 647444), запись в `/tmp/opencode/rev1-pass2-runs.jsonl`; после пробы чужое окно осталось единственным → проба его не трогала |
| 3 | `/tmp/opencode/rev1-pid-filter.py` (прямой оракул `atspi_app`/`x11_capture`) | 0 | `atspi_app(own)=('discord-gui-spike',662284)`, `atspi_app(wrong)=None`, `x11_capture(own)` → `0x1800003`, `x11_capture(wrong)` → `EnvFail: своё окно PID … не найдено` |
| 4 | `/tmp/opencode/rev1-atspi-dump.py` (своё окно, клик по «Эмодзи») | 0 | в listbox «Эмодзи» 20 узлов, **все** `list item` с именами эмодзи (`улыбка`, `хохот`, …); 10 `page tab` — **вне** listbox; имён категорий внутри listbox нет |
| 5 | `/tmp/opencode/rev1-probe-branches.sh`: stale / ранний exit / без snapshot / hang / без дисплея | 1 / 2 / 2 / 2 / 2 | stale → `FAIL: бинарь старше ui/*`; ранний exit → `SKIP: спайк завершился на шаге «AT-SPI»`; без snapshot → то же на «UI_SNAPSHOT»; hang → `window-ready не появился за 15 с`, сирот нет (`pgrep` пуст); без дисплея → `SKIP: нет DISPLAY…` |
| 6 | `node --test --test-concurrency=1 tests/smoke/gui-spike.smoke.test.mjs tests/smoke/gui-window.smoke.test.mjs` | 0 | **2/2 pass, 0 skip**, 14,4 с; процессов `discord-gui-spike` после прогона нет |
| 7 | `node --test tests/unit/gui-spike-dom.test.mjs` | 0 | **29/29 pass, 0 skip** |
| 8 | `env -u GUI_SPIKE_E2E cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test launch` | 0 | **1/1**, 9,16 с — live-запуск без opt-in (не ранний return) |
| 9 | `SPIKE_BIN_OVERRIDE=/tmp/opencode/fake-spike-early.sh … --test launch` | **101** | `окно прожило 250.4ms < 8s … преждевременный exit 0` |
| 10 | `DISCORD_LAB_KEYRING_LIVE=1 cargo test --offline -p discord-platform --test keyring_contract -- --ignored` | 0 | **12/12 pass, 0 ignored**, 0,72 с |
| 11 | `node --test tests/meta/test-policy.test.mjs` | 0 | **5/5 pass, 0 skip** |
| 12 | `npm run gate:worker` | 0 | `.gate/worker-2026-10-24T10-18-43-645Z.json`: 7 фаз, verdict `passed_with_skips`, smoke **22/22, 0 skip**, workspace **1116 (1104 pass, 12 ignored)**, 46,7 с |
| 13 | `/tmp/opencode/stale-demo` (демонстрация mtime-гарда) | 0 | правка `ui/features/emoji.js` при свежем бинаре **не видна** обоим stale-гардам (`glob("*")`/`readdirSync` не рекурсивны) |

Примечание: live-прогоны №6 и №12 штатно дописали в `waves/window-runs.jsonl` записи
`15:15:18` (pid 650340) и `15:18:41` (pid 703966) — обе с PID/окном/PNG/snapshot; это
поведение тестов, не ручная правка.

## 1. P1 pass1 → статус

| P1 pass1 | Проверка (чем) | Статус |
|---|---|---|
| **P1-1** проба брала первое «discord»-приложение/окно без сверки PID → ложный pass на чужом/старом окне | код `scripts/gui-window-probe.py:129-150` (AT-SPI только `get_process_id()==pid`, 0 → `None` → `EnvFail`, ≥2 → `Fail`), `:300-306` (X11: уникальный `SPIKE_WINDOW_TITLE` **и** `_NET_WM_PID==pid`, 0 → `EnvFail`, ≥2 → `Fail`); `main.rs:682-686` реально ставит заголовок; п.2, 3, 5 | **CLOSED** |
| **P1-2** `role="grid"` без row → в AT-SPI пустая таблица; тесты не видели | `ui/index.html:150` `role=listbox` + `aria-label="Эмодзи"`; `ui/features/emoji.js:397-403` ячейки `role=option` + непустой `aria-label`; `gui-window-probe.py:421-430` ячейки ищутся **внутри своего** listbox, порог 10 именованных; `tests/unit/gui-spike-dom.test.mjs:494-517` проверяет каждую ячейку; п.4 (20 реальных `list item` с именами эмодзи, 0 категорий внутри) | **CLOSED** |

Дополнительно (P2 pass1, влияющие на ложный pass):
**P2-1** (тихий skip на дисплее) — закрыт: нет бинаря/`status===2` на дисплее → fail
(`gui-spike.smoke:28-35`, `gui-window.smoke:30-35`), skip только без дисплея или явный opt-out.
**P2-3** (жизнь окна) — закрыт: `launch.rs:133-139`, `gui-spike.smoke:64-70`,
`gui-window-probe.py:153-157`; ранний exit 0 → красный (п.9), смерть в пробе → exit 2 (п.5).

## 2. Находки

### P0

Нет.

### P1

Нет.

### P2

#### P2-1. Ветки привязки к PID не покрыты автотестом — откат к «первому совпадению» гейт не поймает

- `scripts/gui-window-probe.py:129-150` (`atspi_app`), `:300-306` (X11-фильтр): ветки 0 и ≥2
  проверены только моим ad-hoc оракулом (§0 п.3); в `tests/` вызова `atspi_app`/`x11_capture` нет.
- При одном живом экземпляре smoke проходит и при регрессе «снова первое приложение»;
  сценарий pass1 (старый экземпляр + свежий) в гейте не воспроизводится.
- **Исправление:** вынести чистые функции отбора (`select_own_app(apps, pid)`,
  `select_own_windows(found, pid)`) и покрыть в `tests/unit/gui-window-probe.test.mjs`:
  0 → EnvFail, 1 → ok, 2 → Fail; X11 — чужой PID отвергается.

#### P2-2. Stale-guard не рекурсивный: правка `ui/features/*` или `ui/components/*` без пересборки даёт ложный pass

- `scripts/gui-window-probe.py:83-86` — `(SPIKE / "ui").glob("*")` берёт только верхний уровень;
  `tests/smoke/gui-spike.smoke.test.mjs:16-18` — `readdirSync(uiDir)` + `statSync` по элементам.
  mtime каталога `ui/features` не меняется при правке существующего файла (п.13: правка
  `emoji.js` не видна, бинарь считается свежим).
- Именно в `ui/features/emoji.js` живёт фикс P1-2; после W39c UI разложен на подкаталоги,
  поэтому дыра стала реальной. Сейчас дерево консистентно (`emoji.js` 15:07:19 < бинарь
  15:07:39), но гард это не доказывает.
- **Исправление:** рекурсивный обход `ui/**` (или sha256 манифеста встроенных ассетов),
  в обоих местах; тест-кейс «изменён только `ui/features/*.js` → stale».

#### P2-3. `METRICS.md` смешивает две эпохи гейта; цитируемый отчёт не содержит заявленных фаз

- `waves/RW-38/METRICS.md:3-5` — источником указан `.gate/worker-2026-10-23T15-26-34-287Z.json`,
  но там **4 фазы** (typecheck/meta/unit/smoke), verdict `passed`, **0 skip** — нет
  `ui-tsc`/`spike-rust`/`workspace-rust`, о которых говорит строка 57.
- `:57` — «`passed_with_skips`: meta 29/29, unit 260/260, spike-rust 20/20 … 0 skip» —
  внутреннее противоречие (passed_with_skips и 0 skip), а актуальные значения другие:
  meta **65/65**, unit **651/651**, smoke 22/22; `spike-rust` в отчёте гейта `counts:null`.
- `:39` — «ячейки эмодзи 120»: на текущих байтах 20 (подтверждено п.1, 4, 12 и `window-runs`);
  120 было на байтах W38f (`workers/W38f/green-p1-1-foreign.log`).
- `:58` — workspace 1115/1103/12 соответствует гейту `10-10-42`, а не цитируемому.
- **Исправление:** указать последний фактический отчёт гейта (id), привести его счётчики
  as-is, пометить числа W38f как исторические; 120 → «20 сейчас (120 на W38f)».

#### P2-4. `REPORT.md`: md5-блок не соответствует ни W38f, ни текущим байтам; два факта неверны

- `waves/RW-38/REPORT.md:63-81` — md5 `gui-window-probe.py 578ba235…` (W38f `ef633a2e…`,
  сейчас `f62b8b52…`), `gate.mjs 98a54472…` (сейчас `c8a4875e…`), smoke `9d663736…`
  (сейчас `8678287b…`) и т.д. Ни один хеш блока не совпадает ни с `workers/W38f/md5-final.txt`,
  ни с текущим деревом; в §4a новых md5 нет.
- `:92` — «проба теперь требует **120** ячеек» — неверно: `MIN_NAMED_CELLS = 10`
  (`gui-window-probe.py:53`), фактически 20.
- `:99` — «gate **passed, 0 skip**» — фактически `passed_with_skips` с 12 ignored
  (`keyring_contract`); это же подтверждает `METRICS.md:57`.
- **Исправление:** заменить md5-блок на `md5-final.txt` W38f + примечание «позже изменены
  W39*»; исправить 120 → порог 10/факт 20; исправить вердикт гейта.

#### P2-5. Мутационное доказательство не перепрогнано на финальных байтах; для P1-2 мутанта нет

- `waves/RW-38/mutations-gui.log:1` — вход `app.js md5=579570f9…`; W38f — `c056c93a…`,
  текущий — `78039af3…`. Лог не соответствует сдававшимся байтам, md5 в шапке это фиксирует,
  но перепрогона нет.
- Только G1 (memberRows) и G2 (38→37); AT-SPI-инвариант P1-2 (listbox/option, имена ячеек)
  и привязка к PID мутациями не подтверждены. TEST-POLICY §3.6 требует caught-мутанта для
  нового инварианта.
- `waves/RW-38/run-mutations-gui.py:22` мутирует **рабочий** `ui/app.js` без trap на
  SIGINT/SIGTERM — обрыв оставит дерево мутированным.
- **Исправление:** перепрогон на замороженных байтах с md5 в шапке каждого лога; мутанты
  «убрать `role=listbox`», «убрать `aria-label` у ячейки», «проба берёт первое приложение»;
  сборка копии в `/tmp` + trap с restore/rebuild.

#### P2-6. `launch.rs` на headless «проходит» без проверки, и этот пропуск не виден в гейте

- `rust/gui-spike/tests/launch.rs:100-103` — при отсутствии дисплея `eprintln + return`;
  cargo считает тест **passed** (`0 ignored`), а `spike-rust` в гейте имеет `counts:null`
  (`scripts/gate.mjs:149-153`). На дисплее live-проверка реально идёт (п.8), но на headless
  live-слой исчезает без следа в отчёте — остаток pass1 P2-4 и нарушение духа TEST-POLICY §9
  («skip только с причиной»).
- **Исправление:** `#[ignore = "live: нужен DISPLAY/WAYLAND"]` + запуск `--ignored`
  в live-фазе, либо фиксировать в note/extra гейта `launch: skipped (headless)` и считать
  это skip, а не pass.

#### P2-7. У фазы `smoke` нет `failOnSkips`: live-слой можно выключить переменной окружения, гейт останется зелёным

- `scripts/gate.mjs:162` — фаза `smoke` без `failOnSkips`; при экспортированном
  `GUI_SPIKE_E2E=0` (или `GUI_WINDOW_PROBE_OPTIONAL=1`) на машине с дисплеем оба live-теста
  уйдут в skip, а гейт вернёт `passed_with_skips`/exit 0 (warning только в отчёте).
  Сейчас на дисплее skip 0 (п.6, 12), но защита — только соглашение.
- **Исправление:** `failOnSkips: true` для `smoke` при наличии дисплея (или gate явно
  выставляет `GUI_SPIKE_E2E=1`, а opt-out допустим только письменным waiver по TEST-POLICY §6).

#### P2-8. `window-runs.jsonl` пишется только на успехе и без части AT-SPI-сводки

- `scripts/gui-window-probe.py:536-556` — запись добавляется после всех проверок; при
  `Fail`/`EnvFail` в JSON есть `pid`/`log`, но в `window-runs.jsonl` следа нет.
  В записи нет `window.pid`, `rail`, `chat_items`, `member_items` (только `emoji_cells`/
  `settings_toggles` и snapshot).
- TEST-POLICY §4: live-проверка оформляется PID + окно + PNG + AT-SPI-снимок + запись в
  `window-runs.jsonl`; для отрицательного результата это сейчас не выполнено.
- **Исправление:** писать запись в `finally` для всех исходов (`ok`/`skip`/`fail`, с причиной),
  добавить `window.pid` и счётчики рейла/чата/участников.

### P3

1. `scripts/gui-window-probe.py:52` — `CELL_ROLES` включает `push button`/`table cell`;
   регресс «ячейки стали кнопками» live-проверку пройдёт (DOM-тест строже, требует `option`).
2. `scripts/gui-window-probe.py:182-195` — `find_all` глотает исключения и ограничен depth 24:
   частичное дерево может дать и ложный fail, и «достаточный» счётчик без диагностики.
3. `tests/unit/gui-spike-bench.test.mjs:188,231` — `describe(..., { skip: platform!=="linux" })`
   без текстовой причины; TEST-POLICY §9 требует причину (на Linux не срабатывает).
4. Порог ячеек: `MIN_NAMED_CELLS=10` и `gui-spike-dom.test.mjs:503` `cells.length >= 10` —
   потеря половины сетки (20 → 10) не ловится; сравнивать с фактическим числом отрендеренных
   ячеек (или с ожиданием категории).
5. `scripts/gate.mjs:144-163` vs TEST-POLICY §9 — worker-гейт штатно даёт `passed_with_skips`
   из-за 12 `keyring_contract` ignored; исключение не описано в политике (live 12/12 проверено,
   release-фаза `keyring-live` с `failOnSkips` есть). Зафиксировать исключение в PROCESS/TEST-POLICY.
6. `waves/RW-38/workers/W38f/red-p1-1-foreign-atspi.log` снят на промежуточной сборке
   (`AT-SPI PID … != запущенного …`); в финальном коде такого сообщения нет. Red-first для
   P1-1 на финальных байтах невоспроизводим, закрывает green + мои прогоны.
7. `rust/gui-spike/src/main.rs:690-694` — тест-хук `SPIKE_AUTH_DISABLED` (обходит экран входа)
   не ограничен `debug_assertions`; проба им пользуется. Вне scope RW-38, но стоит
   `cfg(debug_assertions)`/префикс `TEST_`.
8. `tests/meta/test-policy.test.mjs:68-73` — проверяет только наличие строк фаз
   (`typecheck/meta/unit/smoke/e2e/coverage`); удаление `ui-tsc` мета-тест не поймает
   (структурно проверены только `workspace-rust`/`spike-rust`, `:75-100`).

## 3. Аудит TEST-POLICY

| Пункт | Проверка | Итог |
|---|---|---|
| §2.3 нет `.only`/`todo`/пустых assert | `tests/meta/test-policy.test.mjs` — 5/5; ручной `rg` по `tests/**/*.mjs` — совпадений нет | ✅ |
| §2.3/§9 skip только с причиной | live-скипы только `!hasDisplay` с текстом; `t.skip` с причинами; один boolean-skip без причины (P3-3) | ✅/⚠️ |
| §9 на дисплее live не skip | smoke 2/2 skip 0; `launch` без opt-in 1/1 (9,16 с); gate smoke 22/22 skip 0 | ✅ |
| §8 gate: `ui-tsc`, `spike-rust`, `workspace-rust` | `scripts/gate.mjs:144-163`; мой прогон — 7 фаз | ✅ |
| §8 `workspace-rust` — весь workspace, не gui-spike | `cargo test --offline --workspace`, cwd `rust`; `cargo metadata` — 13 crates, `gui-spike` не входит; meta-тест `:75-100` это фиксирует | ✅ |
| 12 `keyring_contract` ignored с причиной и live | 12 `#[ignore = "live-keyring: …"]`; `DISCORD_LAB_KEYRING_LIVE=1 … --ignored` → 12/12 pass, 0 ignored | ✅ |
| §3.1 RED до кода | red/green-логи W38f есть; red P1-1 — на промежуточной сборке (P3-6); red P1-2 (`red-p1-2-dom.log`) и green есть | ✅/⚠️ |
| §3.6 мутация нового инварианта | только G1/G2, вход `app.js` не сдававшийся; мутанта P1-2/PID нет | ❌ → P2-5 |
| §4 evidence (`команда → exit → артефакт`) | в REPORT таблицы без exit-кодов; `METRICS` цитирует не тот gate; `window-runs` только успехи | ⚠️ → P2-3/4/8 |
| §5 изменение тестов не автором | тесты untracked, `git diff` вакуумный; проверял содержимое: ослаблений относительно W38f-логов не видно (DOM 20→29 кейсов, smoke/launch только ужесточены) | ✅ (с оговоркой) |

## 4. Gate evidence

| Отчёт | Фазы | Вердикт | Skip |
|---|---|---|---|
| `.gate/worker-2026-10-24T10-10-42-282Z.json` (последний до ревью) | typecheck, ui-tsc, meta, unit, spike-rust, workspace-rust, smoke | `passed_with_skips` | 12 (`workspace-rust`/keyring), smoke 22/22 skip 0, unit 651, meta 65 |
| `.gate/worker-2026-10-24T10-18-43-645Z.json` (**мой прогон**) | те же 7 фаз | `passed_with_skips` | 12 (`workspace-rust`/keyring), smoke 22/22 skip 0, unit 651, meta 65, workspace 1116 (1104 pass + 12 ignored) |
| `.gate/worker-2026-10-23T15-26-34-287Z.json` (**цитируется в METRICS**) | typecheck, meta, unit, smoke | `passed` | 0; meta 29, unit 260; `ui-tsc`/`spike-rust`/`workspace-rust` **отсутствуют** |
| `waves/RW-38/workers/W38f/gate-worker-final.log` (финал W38f) | typecheck, ui-tsc, meta, unit, spike-rust, smoke | `passed` | 0; `workspace-rust` ещё нет |

Вывод: актуальный гейт содержит все требуемые фазы и даёт 0 display-skip; единственные
12 skip — `keyring_contract` с причиной в `#[ignore]`, live-прогон 12/12 pass. `METRICS.md`
ссылается на отчёт, который этим требованиям не отвечает, поэтому его строка §3 не является
доказательством (P2-3). `REPORT.md:99` («passed, 0 skip») противоречит фактическому
`passed_with_skips` (P2-4).

## 5. Вердикт

**PASS** — P0 = 0, **P1 = 0** (P1-1 и P1-2 закрыты и воспроизведены на текущих байтах).
P2 = 8, P3 = 8. Блокирующих для приёмки нет; до выпуска желательно закрыть P2-2 (ложный
pass на устаревшем встроенном UI), P2-3/P2-4 (evidence: METRICS/REPORT не соответствуют
фактам) и P2-5 (мутации на финальных байтах). P2-1/P2-6/P2-7/P2-8 — карточки следующей волны.

Ограничения ревью: live-прогоны шли на текущем (dirty) дереве, а не на замороженных байтах
W38f; P1-1/P1-2 проверены и логикой, и поведением, но дифференциального диффа W38f↔current
нет (файлы untracked). Гонки с другими волнами возможны: за время ревью `workspace-rust`
вырос с 1115 до 1116 тестов.
