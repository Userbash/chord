# REVIEW-194A — Техническое ревью PR-165 (`task-283` / `QA-FIX-04`: `ui/tsconfig.json` + `scripts/test-impact.mjs`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-194A
- **Проверяемая ветка:** `qa/task-283-tsconfig-and-test-impact` (база `unstable`, HEAD `886ec1e`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `7/7` убиты, исходные файлы восстановлены (MD5 совпадают)
- **Дополнение scope:** координатор согласовал (`ask`, ответ `A:fix-in-scope`) расширение на `tests/unit/test-impact.test.mjs`: в PR не было ни одного теста, который пинует четыре новых правила `UI_MODULE_TESTS` и точные имена Rust-таргетов. Без этого дополнения мутации выживали `5/7` (см. §5.1). Все три файла идут в merge вместе.

## 1. Объект ревью и целостность файлов

| Файл | MD5 в ТЗ | MD5 до проверки | MD5 после мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/tsconfig.json` | `d36fa8dd3043b17cd5b965de19e2ba28` | совпал ✅ | `d36fa8dd3043b17cd5b965de19e2ba28` ✅ |
| `scripts/test-impact.mjs` | `330caeeac8e6f553df0ecfd25eea84cc` | совпал ✅ | `330caeeac8e6f553df0ecfd25eea84cc` ✅ |
| `tests/unit/test-impact.test.mjs` | — (добавлен по TR-194A) | `260d6af418c05d54f4e4b49010f9ec05` | не мутировался |

`scripts/test-impact.mjs` снимался до и после каждого мутанта внутри харнесса (`mutations/mutate-194a.mjs` бросает исключение, если MD5 после восстановления не сошёлся). Ни один прогон не оставил файл изменённым.

## 2. Состав изменений (`git status --porcelain`)

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/tsconfig.json
 M scripts/test-impact.mjs
 M tests/unit/test-impact.test.mjs
```

- Три `M` — предмет PR-165: `include` в UI-tsconfig, 22 новых правила селектора (18 Rust + 4 UI) и пины к ним в контракте селектора.
- `D node_modules`, `M package-lock.json` — **не часть PR** (симлинк `node_modules` развёрнут в рабочем дереве, lock перегенерирован `npm install`). Координатору при merge брать только три пути, не `git add -A` (см. `P2-3`).

## 3. `rust/gui-spike/ui/tsconfig.json`

Было: `["app.js", "features/app-shell-wiring.js", "features/shell-controllers-registry.js", "features/chat-helpers-bridge.js"]`. Стало: `["app.js", "features/**/*.js"]`.

| Проверка | Результат |
| --- | --- |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0`, ошибок нет (`logs/review-194a-tsc-ui.log`) |
| `--listFiles` по конфигу | **97** файлов `ui/features/`; на диске ровно `97` |
| Проба чувствительности: `features/__probe194a.js` с `/** @type {number} */ const x = "not a number"` | `error TS2322: Type 'string' is not assignable to type 'number'` — расширенный `include` реально проверяет произвольный файл `features/`, а не только импортированные из `app.js`; проба удалена |

Это закрывает `P2-1` из `REVIEW-184A` (зелёный `tsc` на трёх файлах не проверял остальные 94). `tsconfig.agent.json` — exit `0`.

## 4. `scripts/test-impact.mjs`: поведение правил

Сверка с независимым оракулом — реальным layout репозитория (файлы `rust/gui-spike/src/<mod>.rs`, `rust/gui-spike/tests/<mod>_contract.rs`, `tests/unit/*.test.mjs`), а не с выводом самого скрипта. Скрипт: `mutations/probe-rules-194a.mjs`, лог `logs/review-194a-rules-probe.log`.

```
18 rust-правил: cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test <mod>_contract --test ipc_contract, tier=T1
  automod, bff_domains_router, dm_channels, expressions, forum, ipc_ext, moderation, onboarding,
  polls, profile, profile_ext, relationships, roles, scheduled_events, slash_commands, soundboard,
  stage_instances, webhooks — у всех 18 файл модуля и файл контракта на диске
4 ui-правила: node --test tests/unit/<mod>.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs,
  tier=T0, без geometry-check.mjs
  shell-controllers-registry, chat-helpers-bridge, composer-popovers-bridge, app-shell-wiring

итого: 18+4 OK
```

`lib.rs` содержит ровно эти `pub mod`, поэтому инвариант «каждый pub mod покрыт правилом» больше не красный: было `52/53` до правки (красный на `forum`), стало `57/57` вместе с новыми пинами.

## 5. Мутации

Методика: исходник копируется в `mutations/test-impact.orig.mjs`, каждая мутация — точная замена одной конструкции, затем `node --test tests/unit/test-impact.test.mjs`, затем восстановление из бэкапа с проверкой MD5. Логи по мутантам: `mutations/M1.log … M7.log`, сводка — `mutations/results.json`, прогон — `mutations/run.log`.

### 5.1 Первый прогон — до пинов (зафиксирован как red-фаза)

| # | Мутация | Результат |
| --- | --- | --- |
| M1 | `automod_contract` → `automod_contracts` (суффикс-опечатка) | **выжил** |
| M2 | удалено правило `bff_domains_router` | убит |
| M3 | `stage_instances_contract` → `soundboard_contract` | убит |
| M4 | удалено правило `app-shell-wiring.js` | **выжил** |
| M5 | у `composer-popovers-bridge.js` отрезан профильный тест | **выжил** |
| M6 | удалено правило `shell-controllers-registry.js` | **выжил** |
| M7 | `chat-helpers-bridge.js` сшит с чужим профилем `app-shell-wiring.test.mjs` | **выжил** |

**2/7.** Причины:

- Четыре новых UI-правила не были закреплены ничем: таблица `UI_MODULE_TESTS` в `tests/unit/test-impact.test.mjs` описывает свои шесть модулей RW-40, новых ключей в ней нет, других тестов на селектор в репозитории нет (`grep -rln "test-impact" tests/` → один файл). Удаление правила молча откатывало модуль на полный `frontend()` + геометрию (M4, M6), а M7 давал **ложно-зелёный** прогон: изменение `chat-helpers-bridge.js` проверялось бы тестами `app-shell-wiring`.
- Инвариант `каждый pub mod из lib.rs покрыт правилом` сверял таргет подстрокой `has(plan, "--test automod_contract")`, поэтому `--test automod_contracts` проходил проверку (M1). На исполнении cargo такой таргет падает («no test target named»), то есть отказ громкий, но контракт селектора его не ловил.

### 5.2 После пинов — итоговый прогон

Добавлено в `tests/unit/test-impact.test.mjs`:

1. `SHELL_BRIDGE_MODULES` + `describe("test-impact v2: правила модулей интеграции оболочки")`: для каждого из четырёх модулей план сверяется с диском (`модуль есть`, профильный тест есть) и **точно** сравнивается список `node --test`-команд — профильный тест первым, затем `gui-spike-dom` и `gui-spike-features`, без геометрии, tier `T0`, без live-команд.
2. В инварианте `каждый pub mod из lib.rs покрыт правилом` подстрочная проверка заменена на `assert.deepEqual(cargo, [cargoTargets([target, "ipc_contract"])])` плюс проверка существования `rust/gui-spike/tests/<target>.rs`.

| # | Мутация | Итог | Убивший тест |
| --- | --- | --- | --- |
| M1 | `automod_contract` → `automod_contracts` | убит | `каждый pub mod из lib.rs покрыт правилом` |
| M2 | удалено правило `bff_domains_router` | убит | `каждый pub mod из lib.rs покрыт правилом` |
| M3 | `stage_instances_contract` → `soundboard_contract` | убит | `каждый pub mod из lib.rs покрыт правилом` |
| M4 | удалено правило `app-shell-wiring.js` | убит | `features/app-shell-wiring.js → app-shell-wiring.test.mjs + общие UI-контракты` |
| M5 | у `composer-popovers-bridge.js` отрезан профильный тест | убит | `features/composer-popovers-bridge.js → …` |
| M6 | удалено правило `shell-controllers-registry.js` | убит | `features/shell-controllers-registry.js → …` |
| M7 | `chat-helpers-bridge.js` → чужой профиль `app-shell-wiring.test.mjs` | убит | `features/chat-helpers-bridge.js → …` |

**Итого `7/7` (100%), обязательный минимум `5/5` перекрыт.** После цикла `scripts/test-impact.mjs` = `330caeeac8e6f553df0ecfd25eea84cc`.

### 5.3 Red-фаза пинов

Новые пины прогнаны против селектора **до** правил PR-165 (`git show HEAD:scripts/test-impact.mjs`, MD5 `17a665bc74da1e751b1ea38bf380e26e`), лог `mutations/red-before-pr-rules.log`:

```
ℹ tests 57  ℹ pass 52  ℹ fail 5
✖ features/shell-controllers-registry.js → …
✖ features/chat-helpers-bridge.js → …
✖ features/composer-popovers-bridge.js → …
✖ features/app-shell-wiring.js → …
✖ каждый pub mod из lib.rs покрыт правилом
```

После возврата файла — 57/57 (см. §6). То есть тесты действительно проверяют правила PR, а не тавтологию.

## 6. Тесты и регрессия

| Проверка | Результат |
| --- | --- |
| `node --test tests/unit/test-impact.test.mjs` | **57/57 pass**, 0 fail (`mutations/green-test-impact-contract.log`); было `53/53`, +4 новых теста |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0` (`logs/review-194a-tsc-ui.log`) |
| `npx tsc -p tsconfig.agent.json --noEmit` | exit `0` (`logs/review-194a-tsc-agent.log`) |
| `npx tsc -p tsconfig.json --noEmit` | exit `1`, **34** ошибки — все предсуществующие в чужих `tests/unit/*`; в `tests/unit/test-impact.test.mjs` **0** (`logs/review-194a-tsc-root.log`) |
| `npm run test:unit` | **1934/1935**, единственное падение `dispatch_ui.test.mjs` → `A2: FORWARDED_KINDS — 21 имя DispatchKind::as_str…` (`logs/review-194a-unit-full.log`) |

Падение `dispatch_ui` предсуществующее: файл этой ветки не касается, тот же тест красный в `REPORT.md` этой карточки (`unit 1930/1931`) и в `REVIEW-184A` §4.3. Мои правки в `test-impact.test.mjs` ошибок типов не добавляют.

`gate:worker` не перезапускался: в `REPORT.md` §«gate:worker» он красный по причинам вне карточки (`spike-rust` не компилируется из-за отсутствующего `voice_stage`, `workspace-rust` — `DispatchEvent::parse_stream`, `smoke` без собранного бинарника, `meta` 96/102), и эти причины проверены автором через `git stash`. Для TR-194A релевантные проверки — `ui-tsc` (ok в том же gate-артефакте) и контракт селектора (57/57 выше).

## 7. Замечания

Блокирующих нет. Ниже — `P2`.

### P2-1. Пины селектора живут отдельно от таблицы
`tests/unit/test-impact.test.mjs` держит собственную таблицу `UI_MODULE_TESTS` (6 модулей) и теперь отдельный список `SHELL_BRIDGE_MODULES` (4 модуля). Две таблицы описывают одно и то же разными способами. **Рекомендация:** в следующей карточке селектора свести их в одну таблицу-оракул и проверять, что каждый ключ `UI_MODULE_TESTS` из `scripts/test-impact.mjs` либо в таблице теста, либо явно помечен.

### P2-2. Корневой `typecheck` красный на базе
`npm run typecheck` падает на 34 предсуществующих ошибках в `tests/unit/{locale-switcher,inbox,embed-formatter,voice-input-mode,poll-progress,markdown-helper,invite-parser,forum-filter,audit-log,attachment-validator}.test.mjs`. К PR не относится, но гейт `gate:worker` из-за него не позеленеет. **Рекомендация:** отдельная `bugfix`-карта на типы в этих тестах.

### P2-3. Шум в рабочем дереве, не входящий в PR
`D node_modules`, `M package-lock.json` (расхождение `package.json` ↔ lock по `bin`/`engines`). **Рекомендация:** merge только по трём путям; lock чинить отдельно, иначе `npm ci` на CI упадёт.

## 8. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| Файлы соответствуют MD5 из ТЗ | ✅ оба |
| Изменения не выходят за scope | ✅ (тест добавлен с санкции координатора) |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` = 0 ошибок | ✅, охват 97 файлов вместо 3 |
| `node --test tests/unit/test-impact.test.mjs` | ✅ 57/57 |
| Новые правила дают верные команды (18 Rust + 4 UI) | ✅ 22/22 по диску |
| Мутации | ✅ `7/7` убиты, `scripts/test-impact.mjs` восстановлен |
| Регрессия unit | ✅ 1934/1935, падение предсуществующее |
| `git commit` не делался | ✅ |
| `P0 = 0`, `P1 = 0` | ✅ |

## 9. Артефакты

```
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/REVIEW-194A.md             — этот отчёт
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/mutate-194a.mjs  — харнесс мутаций с восстановлением
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/run.log          — итоговый прогон 7/7
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/M1.log … M7.log  — логи мутантов
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/results.json     — сводка и MD5
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/baseline.log     — зелёный baseline
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/red-before-pr-rules.log
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/green-test-impact-contract.log
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/mutations/probe-rules-194a.mjs
waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact/logs/review-194a-*.log     — tsc (ui/agent/root), unit-full, rules-probe
```

**Вердикт: APPROVED.** `P2-1…P2-3` — рекомендации на отдельные карточки, слияние в `unstable` ими не блокируется. Следующий статус — `Ready for Merge`; merge трёх файлов и запись в `waves/db.jsonl` выполняет координатор.
