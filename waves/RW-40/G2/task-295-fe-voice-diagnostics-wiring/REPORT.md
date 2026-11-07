# REPORT — task-295 (`FE-INT-16` / `RE-22-04`, G2 Frontend): подключение `voice-diagnostics.js` к `voice.js`

- **Ветка:** `feature/task-295-fe-voice-diagnostics-wiring` (worktree `/home/sanya/orca/workspaces/project/feature-task-295-fe-voice-diagnostics-wiring`, база `unstable` = `648ba83`)
- **Волна / отдел:** `RW-40` / `G2 Frontend (WebView UI & DOM/A11y)`, уровень `L1`, приоритет `P1`
- **Модель:** `opencode --model deepseek/deepseek-v4.1-flash`
- **Дата:** 2026-10-31
- **Коммитов нет:** `git commit` / `git push` не выполнялись.

## 1. Сделано

| Файл | Что изменилось |
|---|---|
| `rust/gui-spike/ui/features/voice.js` | `import`/реэкспорт `evaluatePingQuality` и `createVoiceDiagnosticsController` из `./voice-diagnostics.js`; `createVoicePanel` принимает необязательный `options.diagnosticsController` и по клику на `#voice-ping` в состоянии `connected` вызывает `togglePopout(pingEl)` (fallback `openPopout()`, затем `toggle()`); новый экспорт `mountVoicePanel(root, store, service = null, options = {})` с `options.enableDiagnosticsPopout` и `options.diagnosticsOptions` |
| `tests/unit/voice-ui.test.mjs` | +3 теста в новом сьюте «интеграция voice-diagnostics (RE-22-04)»: реэкспорт и пороги оракула, клик в `connected` → `togglePopout(anchor)`, `openPopout()` + автосозданный контроллер |

Обратная совместимость: `pingQuality(ping)` не тронута (`<= 120` → `fine`, `<= 250` → `average`, `> 250` → `bad`), `data-quality` по-прежнему считается только ею. Без `diagnosticsController` и без `enableDiagnosticsPopout` поведение панели прежнее — клик по `#voice-ping` ничего не делает, как и до задачи.

**Отклонение от буквы ТЗ (важно).** Функции `mountVoicePanel` в репозитории не было: `git grep mountVoicePanel` по всем локальным и удалённым веткам дал 0 совпадений, в `voice.js` на `unstable` её тоже нет. Пункт ТЗ «сохрани все существующие проверки `mountVoicePanel`» применить было не к чему, поэтому функция создана как тонкая обёртка: находит `#voice-panel` внутри `root` (или создаёт `<section id="voice-panel" class="voice-panel">`), резолвит контроллер диагностики и делегирует в `createVoicePanel`. Сигнатура и поведение — ровно по §2 ТЗ.

## 2. md5sum (финальные байты)

```
ce826be256b0b759ae265503a9d0fd3a  rust/gui-spike/ui/features/voice.js   (было 652f974268db0f7ddee22304fd2a692c)
557db0d6401fa917eb3bcf284de2aa0a  tests/unit/voice-ui.test.mjs           (было 1ab81fe664930a75173ed26cfb5715fd)
```

`git show HEAD:...voice.js` даёт `652f974268db0f7ddee22304fd2a692c` — совпадает с `[F]`-ссылкой в `VOICE-DIAGNOSTICS-SPEC.md` §0.2, то есть база была нетронутой.

## 3. Strict TDD (red → green)

**Red** — `red.log`: тесты добавлены и запущены до реализации, `voice.js` ещё не экспортирует новые имена:

```
$ node --test tests/unit/voice-ui.test.mjs
SyntaxError: The requested module '../../rust/gui-spike/ui/features/voice.js'
does not provide an export named 'createVoiceDiagnosticsController'
ℹ pass 0  ℹ fail 1   EXIT=1
```

**Green** — `green.log`:

| Проверка | Команда | Результат |
|---|---|---|
| targeted UI | `node --test tests/unit/voice-ui.test.mjs` | `47/47 pass` (44 прежних + 3 новых), exit 0 |
| targeted diagnostics | `node --test tests/unit/voice-diagnostics.test.mjs` | `67/67 pass`, exit 0 |
| types (корень + agent) | `npm run typecheck` | 0 errors, exit 0 |

## 4. Оракул контракта

Независимые источники, а не текущая реализация:

| Утверждение | Оракул |
|---|---|
| `evaluatePingQuality(250, 0) === "fine"`, `(251, 0) === "average"`, `(501, 0) === "bad"`, `(100, 10.1) === "bad"` | `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`, модуль `613681` @ `2469479` (`RTCConnection.quality`: `> 500` / `> 10%` → bad, `> 250` / `> 5%` → average) |
| `pingQuality(120) === "fine"`, `(250) === "average"`, `(251) === "bad"` | `waves/RW-40/DESIGN/voice-spec.json` → `VG-06`, `panel.ping` (наши пороги 120/250) |
| Клик по `#voice-ping` открывает попап; в остальных состояниях — нет | `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md` §3.1 (триггер `#voice-ping`) и §1.1 таблица состояний `unknown` вне `RTC_CONNECTED` |
| Якорь попапа — сама кнопка (для позиционирования над триггером) | там же, §3.1 «Размещение: над триггером, выравнивание по левому краю кнопки» |

## 5. gate:worker

`npm run gate:worker` выполнен (воркер работает один, вне batch): отчёт `.gate/worker-2026-10-31T07-24-11-050Z.json`, **вердикт `failed`**. Фазы: `typecheck ok`, `ui-tsc ok`, `meta ok`, `unit ok`; падают три фазы, не связанные с этой карточкой:

| Фаза | Причина | Отношение к task-295 |
|---|---|---|
| `spike-rust` | `tests/ipc_wiring_contract.rs::every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`: `left: 37, right: 38` — в самом тесте перечислено 37 имён при ожидании 38 | воспроизведено с `git stash` моих двух файлов → тот же fail, то есть baseline |
| `workspace-rust` | ошибки компиляции `crates/discord-core/tests/dispatch_types.rs`: нет метода `to_stream_action` у `DispatchEvent` | Rust-крейты не трогались (write-scope — JS/mjs) |
| `smoke` | live AT-SPI проба `scripts/gui-window-probe.py`: `тумблеров 41, ожидалось 38` | живое окно другого процесса, к voice-панели не относится |

`rust/gui-spike/Cargo.lock` гейт изменил сам (41+/16−); изменение откатано (`git checkout -- rust/gui-spike/Cargo.lock`), `git status` показывает только два файла задачи плюс унаследованные от окружения `D node_modules` и `M package-lock.json` (были до начала работы).

Ни одна из трёх падающих фаз не покрывает изменённые файлы; `ui-tsc` (проверяет `rust/gui-spike/ui/**`) и `unit` зелёные.

## 6. Риски и вопросы

1. **`mountVoicePanel` был выдуман ТЗ, а не найден в коде** (см. §1). Если Team Lead ожидал существующую функцию из другой карточки — нужна сверка, чтобы не появилось две реализации с разной семантикой. Сейчас обёртка одна и она единственная в репозитории.
2. **Две шкалы качества живут рядом:** `pingQuality` (120/250, для `#voice-ping[data-quality]`) и `evaluatePingQuality` (250/500 + потери, из RE-оракула). Это прямое требование §2.1 ТЗ; но при чтении кода легко перепутать, какой порог где. В JSDoc каждой функции указан её источник.
3. **`#voice-status-label` как второй триггер попапа** (§3.1 спеки) не подключён: ТЗ карточки ограничивает триггер `#voice-ping`. `Enter`/`Space` работают сами, потому что элемент — `button`.
4. **Автосозданный контроллер** (`enableDiagnosticsPopout: true`) вызывается через `toggle()`, а не `togglePopout()`: в `voice-diagnostics.js` нет методов `togglePopout`/`openPopout`, только `toggle`. Приоритет вызовов: `togglePopout(anchor)` → `openPopout(anchor?)` → `toggle()`.
5. **Нет DOM-попапа.** Задача — только проводка; рендер `RTCConnectionPopout` (300px, спарклайн 258×80, вкладки) остаётся отдельной карточкой.

## 7. Следующий шаг

Team Lead: приёмка + маршрутизация в PM review и Technical review. Падающие фазы гейта `spike-rust` / `workspace-rust` / `smoke` нужно завести отдельными `bugfix/task-*` от `unstable` (они блокируют зелёный `gate:worker` для всех воркеров ветки).
