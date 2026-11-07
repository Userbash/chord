# REVIEW-201A — Техническое ревью PR-175 (`task-295` / `FE-INT-16`: проводка `voice-diagnostics.js` в `rust/gui-spike/ui/features/voice.js`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-201A
- **Проверяемая ветка:** `feature/task-295-fe-voice-diagnostics-wiring`, HEAD `648ba83` = `unstable` (коммитов в PR нет, ревью по рабочему дереву)
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/feature-task-295-fe-voice-diagnostics-wiring`
- **Дата:** 2026-10-31, Node `v26.13.0`
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, targeted-тесты `114/114` PASS, `unit` `2008/2008` PASS, `meta` `102/102` PASS, оба `tsc` — 0 ошибок.

## 1. Целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до ревью | MD5 после тестов и 5 мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/voice.js` | `ce826be256b0b759ae265503a9d0fd3a` | совпал ✅ | `ce826be256b0b759ae265503a9d0fd3a` ✅ |
| `tests/unit/voice-ui.test.mjs` | `557db0d6401fa917eb3bcf284de2aa0a` | совпал ✅ | `557db0d6401fa917eb3bcf284de2aa0a` ✅ |

`git diff --numstat` → `101 0` (`voice.js`) и `96 0` (`tests/unit/voice-ui.test.mjs`): чистые вставки, ни одна существующая строка не удалена и не переписана. `git diff --check` — чисто.

`git status --porcelain`:

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/features/voice.js
 M tests/unit/voice-ui.test.mjs
```

`D node_modules` и `M package-lock.json` — шум окружения, к PR не относятся: `package-lock.json` (`12:18:13`) изменён до появления кода (`12:24:42`), симлинк `node_modules` (`120000 4b95952c…` в `HEAD`) в рабочей копии заменён каталогом, а дифф лока — только блоки `bin`/`engines` пакета `discord-lab`, которые уже есть в `HEAD:package.json` (`{"bin":{"discord-lab":"./bin/discord-lab.mjs"},"engines":{"node":">=24"}}`), то есть лок отстал от `package.json` и был перегенерирован `npm install`. См. `P2-1`.

## 2. Что проверено в диффе

| Проверка | Результат |
| --- | --- |
| Импорт и реэкспорт ровно двух имён из `./voice-diagnostics.js` | ✅ `import { createVoiceDiagnosticsController, evaluatePingQuality }` + `export { … }`, лишних имён нет |
| `pingQuality(ping)` не тронута, пороги `120/250` прежние | ✅ в диффе строки нет; тест «pingQuality: порог … сломан» зелёный |
| Обратной связи с `voice-diagnostics.js` нет (цикл импортов) | ✅ модуль-источник ничего не импортирует |
| Клик по `#voice-ping` открывает попап только в `connected` | ✅ guard `host.getAttribute("data-state") !== "connected"`; `data-state` панели — единственный источник состояния (`voice.js:612`) |
| Якорь попапа — сама кнопка `#voice-ping` | ✅ `togglePopout(ping)`; соответствует `VOICE-DIAGNOSTICS-SPEC.md` §3.1 («над триггером, выравнивание по левому краю кнопки») |
| Приоритет вызовов: `togglePopout(anchor)` → `openPopout(anchor?)` → `toggle()` | ✅ порядок правильный; `toggle()` — реальный API `createVoiceDiagnosticsController`, методов `togglePopout`/`openPopout` у него нет |
| Отсутствие контроллера → поведение прежнее (клик = no-op) | ✅ ранний `return`, панель без диагностики не меняется |
| `mountVoicePanel`: создание `#voice-panel`, когда его нет, проброс опций, `enableDiagnosticsPopout` | ✅ `root.id === "voice-panel"` → сам root; иначе `querySelector`, иначе `createElement("section")` + `append` |
| Нет сети, нет `console.*`, нет новых каналов IPC | ✅ тест «в коде панели нет media/record/screen API и новых каналов» + «панель не логирует» зелёные, строка `voice.js` сканируется тестом целиком |
| Оракул порогов `evaluatePingQuality`, а не реализация | ✅ тест сверяет `250/251/501/10.1%` с `RTCConnection.quality` (RE-22-04), значения не выведены из констант модуля |
| `mountVoicePanel` не дублирует чужую реализацию | ✅ `grep -rn mountVoicePanel` по репозиторию (без `node_modules`) даёт только новый экспорт и три вызова в тестах; в `HEAD:voice.js` функции не было (0 совпадений) — отклонение от буквы ТЗ, уже зафиксировано в `REPORT.md` §1 и принято PM |

## 3. Мои прогоны

| Проверка | Команда | Результат |
| --- | --- | --- |
| Targeted UI + diagnostics | `node --test tests/unit/voice-ui.test.mjs tests/unit/voice-diagnostics.test.mjs` | `114/114 pass`, `0 fail`, exit `0` |
| Типы корня и агента | `npm run typecheck` | 0 ошибок, exit `0` |
| Типы UI-слоя | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | 0 ошибок, exit `0` |
| Весь unit-набор | `node --test tests/unit/` | `2008/2008 pass`, `0 fail` |
| Мета-тесты (в т.ч. `waves-sync`) | `node --test tests/meta/` | `102/102 pass`, `0 fail` |
| Покрытие изменённого файла | `node --test --experimental-test-coverage --test-coverage-include="rust/gui-spike/ui/features/voice.js"` | line `62.75%`, branch `83.25%`, funcs `61.54%`; из **новых** строк не покрыты только `872–873` (бросок `UIError` без root) — см. `P3-2` |

Red-фаза перепроверена по `red.log`: тест падает на `SyntaxError: … does not provide an export named 'createVoiceDiagnosticsController'`, то есть тесты писались до реализации. Слабость: падение модульное, поэтому два кликовых теста в red-фазе вообще не исполнились (см. `P3-5`); их «зубастость» подтверждают мутации M3–M5.

## 4. Мутации — `5/5` убиты

Методика: бэкап `voice.js` в `/tmp/opencode/voice.js.orig-201a`, замена через `node` с контролем «шаблон встречается ровно один раз», прогон `node --test tests/unit/voice-ui.test.mjs`, откат `cp` из бэкапа, сверка MD5. Скрипт — `/tmp/opencode/mutate-201a-detail.mjs`, логи — `logs/review-201a-mutation-M1.log … M5.log` рядом с этим файлом.

| # | Что сломано | Мутация | Итог | Убивший тест |
| --- | --- | --- | --- | --- |
| M1 | публичный контракт модуля | удалён `export { evaluatePingQuality, createVoiceDiagnosticsController };` | убит | весь файл: `SyntaxError … does not provide an export named 'createVoiceDiagnosticsController'` (`pass 0 / fail 1`) |
| M2 | якорь попапа | `togglePopout(ping)` → `togglePopout(host)` | убит | «клик по #voice-ping в connected отдаёт попап diagnosticsController.togglePopout» — падает на `assert.equal(anchors[0], ping)` (`pass 34 / fail 1`; сообщение теряется, см. `P3-1`) |
| M3 | fallback на `openPopout()` | `if (typeof diagnosticsController.openPopout === "function")` → `if (false)` | убит | «openPopout() и enableDiagnosticsPopout покрывают контроллер без DOM-методов» — `AssertionError: openPopout не вызван` (`pass 46 / fail 1`) |
| M4 | guard состояния | `!== "connected"` → `=== "__mutant__"` | убит | «клик по #voice-ping в connected…» — `AssertionError: клик вне connected открыл попап` (`pass 46 / fail 1`) |
| M5 | автосоздание контроллера | `if (!diagnosticsController && options.enableDiagnosticsPopout === true)` → `if (false)` | убит | «openPopout() и enableDiagnosticsPopout…» — `AssertionError: enableDiagnosticsPopout не создал контроллер` (`pass 46 / fail 1`) |

**Итого `5/5` (100%).** M2–M5 ломают ровно один-два целевых assert, остальные 45–46 тестов остаются зелёными: тесты точечные; M1 — модульный отказ импорта, это ожидаемо для удаления реэкспорта. В M2 прогон дополнительно падает на 34-м тесте (репортёр node не смог отформатировать дифф двух `ShimElement`, `P3-1`), но exit code ненулевой и мутация убита. После каждого отката и в конце цикла MD5 `voice.js` = `ce826be256b0b759ae265503a9d0fd3a`, байты идентичны исходным; повторный green после цикла — `114/114`, `typecheck` и `ui-tsc` exit `0`.

## 5. Замечания без блокировки

### P2-1. `package-lock.json` и `node_modules` в рабочем дереве
К PR не относятся (§1), но при оформлении нельзя делать `git add -A`: в коммит должны попасть только `rust/gui-spike/ui/features/voice.js` и `tests/unit/voice-ui.test.mjs`. Ветка не содержит коммита (HEAD = `unstable 648ba83`), поэтому дифф принят по рабочему дереву — перед пушем стоит `git show --stat` сверить состав.

### P2-2. Клик открывает попап, но ARIA-состояния кнопки ещё нет
`VOICE-DIAGNOSTICS-SPEC.md` §6 требует на `#voice-ping` `aria-haspopup="dialog"`, `aria-controls="voice-rtc-popout"` и переключаемый `aria-expanded`, а §3.1 — второй триггер `#voice-status-label`. Этой карточкой не реализовано (её write-scope — только проводка клика), попапа в DOM тоже нет, поэтому сейчас скринридер не узнает, что кнопка что-то раскрывает. Это не блокер для `task-295`, но карточка рендера `RTCConnectionPopout` (RE-22-04) без этих трёх атрибутов и второго триггера будет неполной по спеке.

### P3-1. `assert.equal(anchors[0], ping)` роняет диагностику теста
При падении этой проверки `node --test` тратит **~17.8 с CPU** и печатает только `✖ tests/unit/voice-ui.test.mjs … 'test failed'` без сообщения: репортёр форматирует дифф двух `ShimElement`, а каждый тянет за собой `ownerDocument` со всем деревом приложения. Проверено изолированно: `node --test --test-name-pattern="клик по #voice-ping"` с мутацией M2 даёт `pass 0 / fail 1` и те же 17.8 с. Достаточно `assert.ok(anchors[0] === ping, "в togglePopout ушла не кнопка #voice-ping")` — тогда убийство мутации видно сразу, без спина. Правка тестовая, к поведению не относится.

### P3-2. Не покрыты две ветки нового кода
Не покрыты `voice.js:872–873` (бросок `UIError("ELEMENT_MISSING")` при `mountVoicePanel(null, …)`) и ветка `root.id === "voice-panel"` (монтирование в готовый host). Поведение простое, но именно эти ветки обычно ломаются при рефакторинге — два дешёвых assert закрыли бы дыру.

### P3-3. Дубль JSDoc над `diagnosticsController`
В `createVoicePanel` перед объявлением идут два подряд блока: `/** Попап диагностики снаружи… */` и `/** @type {VoiceDiagnosticsController | null} */`. Второй перекрывает первый для типизации; читаемость страдает, поведение — нет. Достаточно объединить в один блок с `@type`.

### P3-4. `mountVoicePanel(root, store, service, null)` даёт `TypeError`
Дефолт `options = {}` срабатывает только для `undefined`; явный `null` падает на `options.diagnosticsController` вместо понятного `UIError`. `createVoicePanel` ведёт себя так же, поэтому это осознанная совместимость со стилем файла, а не регрессия; ловить стоит лишь если `mountVoicePanel` начнут звать из динамического кода.

### P3-5. Red-фаза покрывает только реэкспорт
`red.log` показывает модульную ошибку импорта, поэтому оба кликовых теста в красной фазе не исполнялись. Для strict TDD это приемлемо (тесты добавлены раньше кода), но доказательство «тесты ловят поведение» дают не red/green, а мутации M3–M5 — их стоит упоминать при приёмке следующих карточек как основной аргумент.

## 6. Соответствие DoD (`TASK-TR-201A.md`)

| Критерий | Статус |
| --- | --- |
| Изменены только `voice.js` и `tests/unit/voice-ui.test.mjs` | ✅; шум `package-lock.json`/`node_modules` — до начала работы (`P2-1`) |
| `node --test tests/unit/voice-ui.test.mjs tests/unit/voice-diagnostics.test.mjs` | ✅ `114/114`, exit `0` |
| `npm run typecheck` | ✅ 0 ошибок |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | ✅ 0 ошибок |
| 5 точечных мутаций с побайтным откатом | ✅ `5/5` убиты, MD5 до/после совпал |
| `P0 = 0`, `P1 = 0` | ✅ |

**Вердикт: APPROVED.** Блокирующих замечаний нет. `P2-1` — гигиена коммита для интегратора, `P2-2` — обязательный хвост для карточки рендера попапа; `P3-1` стоит поправить при следующем касании `tests/unit/voice-ui.test.mjs`, чтобы падения мутаций читались без 18-секундного спина.
