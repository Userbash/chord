# Независимое ревью Pass 2 — DeepSeek Review 41B: N-107-DM-FRIENDS-UI (task-113)

Дата: 2026-10-30. Ветка: `feature/task-113-dm-friends-ui`, базовый коммит `4e24048`.
Предмет: исправления прохода 2 (`rust/gui-spike/ui/features/dm-friends.js`,
`tests/unit/dm-friends-ui.test.mjs`, `tests/support/gui-dom-harness.mjs`,
`waves/RW-40/PM/task113-dm-friends-ui/REPORT.md`) против `REVIEW.md` (41A).
Режим: read-only по репозиторию. Мутации и эксперименты — на копии в `/tmp/opencode/rw40-review113-pass2`
и `/tmp/opencode/t113-tsc-audit`; в репозиторий внесены только этот файл и три лога рядом.

## Вердикт

**PASS. P0 = 0, P1 = 0, P2 = 7.**

Блокирующий дефект P1.1 закрыт по существу: монтирование `#home-rail-btn` идёт через
`rail.insertBefore(...)` + `existingBrand.remove()`, чтение/запись `parentNode` и обход
`children` как массива из кода убраны (`dm-friends.js:27-34`, `:1534` только чтение).
Регрессионный тест `T-N107-DM-MOUNT-01` подаёт рейл с `Proxy`-коллекцией без `indexOf`/`splice`
и getter-only `parentNode` (`dm-friends-ui.test.mjs:1148-1215`), то есть проверяет именно то,
на чём падал браузер. P2.2–P2.4, P2.6 (кроме `TOKENS-01`) и P2.8 закрыты кодом и тестами,
P2.5 и P2.9 письменно вынесены с владельцем. Отчёт перегенерирован по факту: имена экспортов,
DOM-id и счётчики совпадают с кодом и фикстурой.

## Обязательные прогоны (независимо, на текущих файлах)

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/dm-friends-ui.test.mjs tests/meta/test-policy.test.mjs` | `tests 36 \| pass 36 \| fail 0`, exit 0 (19 модульных + 17 meta) | `pass2-verify.log` (`63d3e2e74f25140ae4b6ffcd73ffeac3`) |
| `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) | 0 ошибок, exit 0 | `pass2-typecheck.log` (`9638c5dccefbd8652188ceed072c13d1`) |
| `node --test 'tests/unit/**/*.test.mjs'` (проверка, что правка шима ничего не сломала) | `tests 898 \| pass 898 \| fail 0`, exit 0 | — |

Отдельно проверил утверждение отчёта §2.2 про транзитную проверку типов: в копии добавил в
`dm-friends.js` намеренную ошибку (`const __probe: number = "not a number"`) — `tsc -p tsconfig.json`
упал с `TS8010` в этом файле. Корневой `tsconfig.json` действительно проверяет модуль через
импорт из `tests/**/*.mjs`, так что P2.7 из `REVIEW.md` снят по факту, а не по обещанию.

## P1.1: что именно проверено

1. Код (`dm-friends.js:27-34`):
   ```js
   export function insertHomeRailButton(rail, button, existingBrand = null) {
     if (existingBrand) {
       rail.insertBefore(button, existingBrand);
       existingBrand.remove();
       return;
     }
     rail.append(button);
   }
   ```
2. `grep -n "children\.indexOf|children\.splice|\.parentNode ="` по модулю даёт ноль
   вхождений (совпадения только в комментарии-объяснении `:20-21` и в тесте, который
   намеренно доказывает отсутствие этих API). `...children` встречается дважды (`:1557`,
   `:1559`) — это spread по `.tree-header`/`.tree-scroll`, `HTMLCollection` в браузере итерируем.
3. `parentNode` в модуле только читается (`:1472` в `isInside`, `:1534`, `:1576`, `:1846`,
   `:1978`, `:2030`).
4. Мутация «вернуть старый код» (`rail.children.indexOf` + `button.parentNode = rail` +
   `rail.children.splice`) убивает `T-N107-DM-MOUNT-01` с тем же `TypeError:
   rail.children.indexOf is not a function`, что был в `REVIEW.md`. Это воспроизводит и
   `mutations-fix.log` автора, и мой прогон.

## Независимый мутационный аудит прохода 2 (7/7 KILLED)

Авторский `fix-red.log` фиксирует только модульный `SyntaxError` (см. P2.1), поэтому
различающую силу новых тестов я проверил сам: семь семантических мутаций на копии, каждая —
отдельный прогон `node --test tests/unit/dm-friends-ui.test.mjs`. Артефакт:
`pass2-mutations-audit.json` (`d380450b192be23583e972474d422834`).

| Мутация | Baseline MD5 | Мутант MD5 | Вердикт | Убил тест |
|---|---|---|---|---|
| `insertHomeRailButton` → старый код на `children`/`parentNode` | `be2e2854…` | `8549bc4d…` | KILLED | `T-N107-DM-MOUNT-01` |
| `retry_after` (секунды) отдаётся как мс (`seconds` вместо `seconds*1000`) | `be2e2854…` | `bcfca187…` | KILLED | `T-N107-RATE-01`, `T-N107-RATE-02` |
| убран `trapTabKey` из `#group-dm-dialog` | `be2e2854…` | `5b7d6174…` | KILLED | `T-N107-A11Y-02` |
| `removeRelationship` без `try/catch` (rejected promise, нет сообщения) | `be2e2854…` | `ca893564…` | KILLED | `T-N107-FRIEND-ERR-01` |
| отключена детерминированная сортировка получателей | `be2e2854…` | `5e58c957…` | KILLED | `T-N107-GDM-SORT-01` |
| `READY` мержит `relationships` вместо замены | `be2e2854…` | `1f5a8793…` | KILLED | `T-N107-READY-02` |
| `clearPendingRequests` без тела `{filters:[1,2]}` | `be2e2854…` | `543016be…` | KILLED | `T-N107-FRIEND-DEL-01` |

Baseline восстановлен: MD5 до и после совпадает (`be2e285411581e26c85b668d98de198d`), post-restore exit 0.
Каждая новая проверка ловит именно своё регрессионное поведение — то, чего не доказывал red-лог.

## Что сходится по факту

1. **MD5-таблица `REPORT.md` §3 совпадает с диском** (проверено `md5sum`):

   | Файл | REPORT | Факт |
   |---|---|---|
   | `rust/gui-spike/ui/features/dm-friends.js` | `be2e285411581e26c85b668d98de198d` | совпадает |
   | `tests/unit/dm-friends-ui.test.mjs` | `ff9cfb71e85eea7dbbb7274914da85d7` | совпадает |
   | `tests/support/gui-dom-harness.mjs` | `67f639a794be8a5145bac99ce42bd775` | совпадает |
   | `fix-red.log` | `cbe0b7fb83667888b78154a52a4fc727` | совпадает |
   | `fix-green.log` | `1c0654746aedfc4c427fc197fdd0b52e` | совпадает |
   | `mutations-fix.log` | `48da9ae98f9635111d804ef4d976f4b1` | совпадает |

2. **Отчёт перегенерирован по коду.** Все 38 имён из `REPORT.md:83-85` реально экспортируются
   (проверено импортом модуля: `missing []`), все 24 метода контроллера на месте.
   `data-action="nav-home"` (`:1519`, `:1541`), `#group-dm-submit` (`:1950`), `#dm-nav-friends`
   (`:2181`), бейдж — `span.guild-mention-badge` без id (`:1525-1528`). Старые выдуманные имена
   (`hydratePrivateChannels`, `RelationshipTypes`, `#home-rail-badge`, `select-home`,
   `#group-dm-submit-btn`) из отчёта убраны.
3. **Счётчики фикстуры** (независимый расчёт функциями модуля на `002-READY.json`):
   `private_channels = 17` (все `type 1`, у всех ровно один получатель, все резолвятся),
   `relationships = 60` (`type 1` × 18, `type 2` × 42), `users = 146`,
   `friendCount 18 / blockedCount 42 / incomingCount 0 / pendingCount 0`. Совпадает с `REPORT.md:89`
   и с `DM-FRIENDS-SPEC.md:65`.
4. **P2.2, 429.** `extractRateLimitRetryAfterMs` читает `retry_after_ms` (мс) и `retry_after`
   (секунды) (`:1236-1249`), `isRateLimitError` учитывает `status === 429` (`:1252-1255`),
   `startRateLimitCountdown` ведёт посекундный отсчёт и вызывает `onDone` (`:1390-1413`),
   Add Friend и Group DM возвращают `{ ok:false, rateLimited:true, retryAfterMs }`.
5. **P2.3, ошибки действий.** `reportActionError` (`:1432-1446`) маппит `body.code` в русский
   текст (`30059`, `50003`, `50015`, `80001`, `80013` есть в `ERROR_MESSAGES_BY_CODE`),
   `announceStatus` пишет в `#server-status`; `removeRelationship`, `blockUser`, `ignoreUser`,
   `clearPendingRequests`, `fetchUserNote`, `closePrivateChannel` обёрнуты в `try/catch`
   (`:3030`, `:3053`, `:3083`, `:3111`, `:3139`, `:3328`), состояние меняется после успешного REST,
   при отказе `DELETE` канал возвращается в список и пересортировывается.
6. **P2.4, focus trap.** `trapTabKey` (`:1483-1506`) подключён к `#group-dm-dialog` (`:1962`)
   и `#stranger-confirm-dialog` (`:2013`); `closePrivateChannel` возвращает фокус на соседний
   канал или `#dm-nav-friends`. Тесты `T-N107-A11Y-02/03` проверяют Tab/Shift+Tab, Escape,
   возврат фокуса на `opener` и неизменность `body[data-active-screen]`.
7. **P2.8, детерминированная сортировка.** `recipient_ids`/`rawRecipients` сортируются перед
   вычислением имени (`:585-605`); `type` не подменяется, контроллер фильтрует вход по `{1,3}`.
   `T-N107-GDM-SORT-01` подаёт два порядка `recipients` и получает одинаковые
   `recipient_ids` и имя.
8. **P2.6, матрица AC.** Есть `T-N107-READY-02` и `T-N107-FRIEND-DEL-01`; последний проверяет
   и одиночный `DELETE /users/@me/relationships/{id}`, и массовый
   `DELETE /users/@me/relationships?relationship_type=3` с телом `{"filters":[1,2]}` — это
   маршруты 21/22 спеки (`DM-FRIENDS-SPEC.md:94-95`).
9. **Шим не ослаблен.** `git diff tests/support/gui-dom-harness.mjs` — единственное изменение
   аддитивный `ShimElement.insertBefore` (`:314-327`); `children` остался массивом, `parentNode`
   writable (ограничение шима известно и записано в его шапке). Полный unit-набор 898/898
   подтверждает, что аддитивная правка ничего не сломала. `skip`/`only`/`todo`/`assert.ok(true)`
   в тесте модуля — ноль вхождений, meta-тест `test-policy` 17/17.

## P2 (не блокируют приёмку, закрыть до релиза или вынести карточкой)

1. **`fix-red.log` не доказывает red-first по каждому новому тесту.** Лог содержит один
   `SyntaxError: does not provide an export named 'insertHomeRailButton'`: ни один из девяти
   новых тестов не выполнился в RED, поэтому лог не показывает, что каждый тест падал по своей
   причине. Формально RED есть (`EXIT=1`), но доказательная сила низкая. Я компенсировал это
   независимым мутационным аудитом (7/7 KILLED выше), так что возврат не нужен. На будущее:
   писать стабы/экспорты так, чтобы падали сами тесты, и фиксировать список упавших имён.
2. **`T-N107-TOKENS-01` без письменного waiver.** Отчёт честно пишет, что нужен waiver
   (`REPORT.md:146`), но самого решения в `waves/RW-40/PM/DECISIONS.md` нет. Нужна запись
   Team Lead: CSS-переменные вне этой карточки.
3. **P2.5 (ник и заметка) вынесен без номера карточки.** Владелец и срок в отчёте указаны
   («интеграционный/фронтовый лейн, до релиза RW-40», `REPORT.md:144`), но карточки в `TASKS.md`
   нет — без номера пункт потеряется при планировании.
4. **Модуль по-прежнему не подключён к `app.js`** (`grep dm-friends` по `app.js` и
   `features/*.js` пуст; `data-action="nav-home"` в `app.js` не обрабатывается). Это P2.9 из
   `REVIEW.md`, зафиксировано в `REPORT.md:143`; интеграционный лейн должен начинаться с этого,
   иначе фича не появится в приложении.
5. **Ветка 429 не учитывает `errorUxConfig`.** Спека (`DM-FRIENDS-SPEC.md:164`) привязывает
   показ текста и таймера к `errorUxConfig === 0`; текущая реализация показывает отсчёт всегда.
   Для дефолтной конфигурации поведение верное, расхождение проявится только при смене режима
   ошибок — нужно либо прочитать флаг, либо записать «всегда» как решение.
6. **Гидратор сохраняет `recipient_ids`, хотя спека требует их удалять.** §1.1 п.3
   (`DM-FRIENDS-SPEC.md:61-62`) говорит, что READY-гидратор разворачивает `recipient_ids` по
   `users[]`, **удаляет** `recipient_ids` и хранит `recipients: string[]` + `rawRecipients`.
   `hydratePrivateChannel` возвращает `recipient_ids` и не отдаёт `recipients`
   (`dm-friends.js:674-678`). Для UI это удобно и тесты на этом построены, но при передаче
   состояния в Rust-стор или при сверке с бандлом форма не совпадёт — надо согласовать на
   интеграции.
7. **`T-N107-DM-MOUNT-01` проверяет helper, а не полный монтаж контроллера.** Тест вызывает
   `insertHomeRailButton` на строгом рейле, но `createDmFriendsController` в браузерно-точном
   DOM (строгая коллекция + getter-only `parentNode` во всём дереве) не прогоняется. Сейчас это
   безопасно — других обращений к `children`/`parentNode` в модуле нет (проверено grep), — но
   следующий подобный дефект снова пройдёт мимо. Рекомендую расширить тест до монтажа
   контроллера на строгом DOM.

## Замечание по артефактам (не про код)

Копия `REPORT.md` в `/home/sanya/orca/project/waves/RW-40/PM/task113-dm-friends-ui/`
устарела: `e398fa9fdb302212842d4e61989b8f32`, 14499 байт — это отчёт прохода 1.
Актуальный отчёт (проход 2) лежит в рабочей копии ветки:
`e2504323282fbfbbd9a0ede31b8dfcf8`, 16181 байт. Ревью проведено по актуальной копии;
координатору стоит синхронизировать каталог артефактов, иначе следующий ревьюер снова
прочитает старый текст.

## Что нужно для финальной приёмки (не блокирует PASS этого прохода)

1. Waiver по `TOKENS-01` от Team Lead (P2.2 выше).
2. Номер карточки для ника/заметки и для подключения к `app.js`.
3. На интеграции: согласовать форму гидратированного канала (`recipients` vs `recipient_ids`)
   и учесть `errorUxConfig` в ветке 429.

Источники вердикта: `REVIEW.md` (41A), `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` §0–§6,
`waves/RW-40/DM-FRIENDS-RE.md` §0–§12, `fixtures/gateway/by-type/002-READY.json`,
`rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js`, `tsconfig.json`,
`rust/gui-spike/ui/features/dm-friends.js`, `tests/unit/dm-friends-ui.test.mjs`,
`tests/support/gui-dom-harness.mjs`, `waves/RW-40/PM/task113-dm-friends-ui/REPORT.md`,
`fix-red.log`, `fix-green.log`, `mutations-fix.log`; собственные прогоны
`pass2-verify.log`, `pass2-typecheck.log`, `pass2-mutations-audit.json`.
