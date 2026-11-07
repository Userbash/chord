# Независимое ревью Pass 3 — task-125-test-impact-v2 (Review 44C)

Дата: **2026-10-30, 21:38 +05**. Ревьюер: `DeepSeek V4.1 Flash` (`Review 44C`), независимая сессия.
Репозиторий: `/home/sanya/orca/workspaces/project/qa-task-125-test-impact-v2`, HEAD `4e24048`
(`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`), ветка `qa/task-125-test-impact-v2`.
Проверялись исправления прохода 3 против `PM/task125-test-impact-v2/REVIEW-PASS2.md` (Review 44B).

Артефакты (MD5 на момент проверки; любая правка инвалидирует ревью):

| Артефакт | MD5 |
|---|---|
| `scripts/test-impact.mjs` (311 строк) | `bff340a33607ed596569e06ade3fd90a` |
| `tests/unit/test-impact.test.mjs` (596 строк) | `3beaf9f181e17ac2bd2ebb0d6ab9255f` |
| `waves/RW-40/G4/task-125-test-impact-v2/REPORT.md` (проход 3) | совпадает с указанными в нём MD5 |

Оракул: `TEST-STRATEGY-V2.md` §5.2.1–§5.2.3; реальный layout `unstable`/целевого дерева
(`/home/sanya/orca/project`: `rust/gui-spike/ui/features/*.js`, `tests/unit/*.test.mjs`),
`rust/gui-spike/tests/*.rs`, `rust/gui-spike/src/lib.rs`, `package.json`.
Продуктовый код и тесты ревьюер не менял. Созданы этот файл и шесть логов в `evidence/`.

## Вердикт

**PASS.** P0 = 0, P1 = 0, P2 = 0, P3 = 3 (остаточные, не блокируют).

Все четыре P1/P2 замечания 44B закрыты по существу и подтверждены независимыми прогонами,
а не только отчётом. Ключевое: тест больше не держится на снимке ветки — на дереве с шестью
`ui/features/*.js` и четырьмя профильными тестами (файлы взяты из целевого дерева, не
синтетические заглушки) прогон **48/48**. Правки закреплены тестами: откат каждой из них
(mutation-прогоны §3) снова красит набор.

## 1. Запрошенные прогоны

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/test-impact.test.mjs tests/meta/test-policy.test.mjs` | **exit 0**, tests 65, pass 65, fail 0 (48 + 17) | `evidence/targeted-pass3.log` |
| `node --test tests/unit/test-impact.test.mjs` | **exit 0**, 48/48 | `evidence/unit-only-pass3.log` |
| `node scripts/test-impact.mjs --changed …` (LICENSE, `ui/app.js`, `features/chat.js`, `features/notifications.js`, неизвестный feature) | поведение ниже | `evidence/probes-pass3.log` |
| Симуляция дерева после merge | **exit 0**, 48/48 | `evidence/integration-layout-pass3.log` |
| `npx tsc -p tsconfig.json` | **exit 1**, 16 ошибок, все в `tests/unit/dispatch_ui.test.mjs`; в файлах карточки 0 | `evidence/typecheck-pass3.log` |
| `node --test tests/meta/plan-sync.test.mjs` | **exit 1**, 4/5, кейс «тестовые файлы без строки в §8» — внешний блокер | — |

Проверки CLI:

- `--changed LICENSE` → `T2 (release)`, `typecheck` + `node --test tests/unit/test-impact.test.mjs`
  + `npm test` + `cargo … gui-spike` + `cargo … --workspace`. Это ровно §5.2.3 «неизвестный путь →
  `all: true`».
- `--changed rust/gui-spike/ui/app.js` → `T2 (release)`, тот же полный набор плюс `geometry-check.mjs`
  (обоснованно: `npm test` = `node --test 'tests/**/*.test.mjs'`, геометрию он не запускает).
- `--changed rust/gui-spike/ui/features/chat.js` → `T0`, три существующих теста
  (`channel-switch`, `gui-spike-features`, `gui-spike-model`), `attachments-render` не выдан.
- `--changed rust/gui-spike/ui/features/notifications.js` в этой ветке → `T0`,
  `gui-spike-dom` + `gui-spike-features`; в целевом дереве — плюс `notifications-ui`.
- Неизвестный feature-модуль (`features/unknown-feature.js`) → полный `FRONTEND` + геометрия:
  новый модуль без правила не теряет покрытие.

## 2. Статус замечаний 44B

| Замечание | Статус | Чем подтверждено |
|---|---|---|
| P1-1 снимок списка модулей | **закрыто** | `layoutProblems` вместо `deepEqual`; merge-симуляция 48/48; mutation §3 |
| P2-1 неизвестный путь без полного прогона | **закрыто** | `LICENSE` → `T2` + 4 строки полного прогона; тест падает при откате |
| P2-2 `ui/app.js` вне safety net | **закрыто** | `app.js` → `T2` + полный прогон + геометрия; тест падает при откате |
| P2-3 команда на отсутствующий профильный тест | **закрыто** | `testOnDisk` перед выдачей строки; тесты ловят откат в layout ветки |
| P3-1 whitelist не сверяется с внешним источником | **не закрыто, обоснование принято** | `BOARD.md`/`TEST-STRATEGY-V2.md` лежат вне git-дерева (`waves/` не отслеживается), читать их тестом нельзя; компенсация — инвариант + mutation §3 |
| P3-2 baseline красного прогона | **закрыто** | `REPORT.md` §«Baseline» назвал селектор прохода 1; `red3-*` логи дают явные baseline (41/7, 20/28) |

Замечание 44B про `deepEqual` закрыто именно так, как просило ревью: «на диске ровно N файлов»
из теста убрано, инвариант проверяет
`module ∈ диск ∪ PLANNED_MODULES`, `test ∈ диск ∪ PLANNED_TESTS`, отсутствие пометок вне таблицы
и отсутствие правил вне `ui/features/`. Проверку «диск ⊆ таблица» авторы не переносили: она
воспроизвела бы тот же снимок; потеря покрытия для неучтённого модуля компенсируется полным
`FRONTEND`-прогоном (проверено пробой выше), поэтому это не дефект.

## 3. Независимые проверки (mutation)

Собрана копия дерева карточки; в неё скопированы реальные файлы целевого дерева:
`features/notifications.js`, `features/dm-friends.js`, `features/voice-devices-settings.js` и
`tests/unit/{notifications-ui,voice-devices-settings,dm-friends-ui,attachments-render}.test.mjs`.
Затем в копию селектора по одному вносились откаты исправлений (лог `evidence/mutations-pass3.log`):

| Mutation | Ожидание | Факт |
|---|---|---|
| merged layout, без mutation | 48/48 | **48/48** |
| неизвестный путь → старый `plan-sync` фоллбэк | падение кейса §5.2.3 | **1 fail**, «неизвестный путь переключает полный прогон» |
| `ui/app.js` убран из `isSafetyNet` | падение safety net | **2 fail**, оба кейса `app.js` |
| `addTest` без `existsSync`, layout ветки | падение P2-3 | **4 fail**: `notifications`, `dm-friends`, `voice-devices-settings`, `chat.js` |
| опечатка ключа таблицы скрипта (`notificationz.js`), layout ветки | падение инварианта | **2 fail**, включая кейс таблицы |

Последняя mutation важна для P3-1: опечатка в ключе **скрипта** ловится не только helper-тестом,
но и на ветке — план для `notifications.js` уходит в `FRONTEND` с геометрией, и кейс модуля падает.
То есть риск «фантомный ключ в whitelist проходит зелёным» уже, чем записано в отчёте.

MD5 обоих файлов после всех прогонов совпали с `REPORT.md`; ревьюер правил только файлы в `/tmp`.

## 4. Остаточные P3 (не блокируют приёмку)

- **P3-1 (перенос из 44B).** Пометки `PLANNED_MODULES`/`PLANNED_TESTS` не сверяются с внешним
  источником: `BOARD.md` и `TEST-STRATEGY-V2.md` вне git-дерева. Причина уважительная, но риск
  остаётся: если смежная карточка переименует профильный тест, `testOnDisk` молча выкинет строку
  из плана, а тест останется зелёным (старое имя помечено запланированным). Рекомендация на
  будущее: при появлении файла со смежным именем или при переносе таблиц в общий модуль добавить
  сверку, либо печатать предупреждение вместо тихого пропуска.
- **P3-2 (новая, косметика).** `scripts/test-impact.mjs:214` — ветка
  `raw.startsWith("rust/gui-spike/ui/app.js")` для точного `ui/app.js` недостижима: файл
  перехватывает `isSafetyNet`. Ветка срабатывает только на экзотических именах (`app.js.map`).
  Либо сузить её до `index.html`, либо оставить с комментарием.
- **P3-3 (housekeeping).** В рабочем дереве `M package-lock.json` (6 строк: `bin`, `engines`),
  расхождение с `package.json` из HEAD. Отчёт говорит, что файл был изменён до начала работы;
  к карточке он не относится. Перед коммитом решить судьбу отдельно, чтобы не тащить в дифф.

## 5. Внешние блокеры (не дефекты карточки)

| Блокер | Факт | Статус |
|---|---|---|
| `typecheck` | 16 ошибок `TS7006/TS2339/TS2345`, все в `tests/unit/dispatch_ui.test.mjs` из `HEAD` | чинится вне карточки |
| `plan-sync` | 1 из 5 кейсов: тестовые файлы без строки в `TDD-PLAN.md §8` | чинится вне карточки |
| `npm test` | из-за `plan-sync` safety net на этой ветке будет красным независимо от карточки | ограничение ветки |

## 6. Что ревью не проверяло

- `npm run gate:worker`, `gate:release`, `gate:qualify`, `cargo` не запускались (cargo блокирует
  target-dir и не нужен для проверки селектора); проверены только имена таргетов и строки плана.
- Живой Discord, окно и сеть не использовались.
- Модули и тесты смежных карточек (notifications, dm-friends, voice-devices-settings,
  attachments-render) брались как файлы целевого дерева, их содержимое не ревьюировалось.
- Поведение после реального merge-коммита не проверялось — только симуляция копией дерева.

## 7. Рекомендация

Принять проход 3. Остаточные P3 внести в бэклог карточки или следующей волны; повторный проход
ревью для них не нужен. При коммите не смешивать `package-lock.json` с файлами карточки.
