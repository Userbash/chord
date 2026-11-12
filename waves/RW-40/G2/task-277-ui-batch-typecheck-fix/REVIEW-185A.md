# REVIEW — PR-156 / task-277 (`FE-FIX-02`, `TR-185A`)

- **Вердикт:** `APPROVED`
- **Блокирующие замечания:** `P0 = 0`, `P1 = 0`
- **Мутации:** `5/5` убиты на `rust/gui-spike/ui/features/thread-badge.js`, файл восстановлен
- **Ветка:** `bugfix/task-277-ui-batch-typecheck-fix` (HEAD `355be21` = `unstable`; изменения PR — рабочее дерево)
- **Ревьюер:** Технический ревьюер (DeepSeek) — `TR-185A`

## 1. Состав PR и целостность файлов

MD5 семи файлов совпадают с `ACCEPT.md`:

| Файл | MD5 | Совпадает |
| --- | --- | --- |
| `rust/gui-spike/ui/features/thread-badge.js` | `8a8dbe830a80e378e3956ad614d29fc6` | да |
| `tests/unit/thread-badge.test.mjs` | `6d3d5ff74edd2275036d992c8e40c966` | да |
| `tests/unit/chat-helpers-bridge.test.mjs` | `5cd90592ac62d354d0c64c1d236dccfd` | да |
| `tests/unit/slowmode.test.mjs` | `47f98b38846d38d7bdd898873c1090cf` | да |
| `tests/unit/event-banner.test.mjs` | `df7199b63cd808f01081639e74063cf1` | да |
| `tests/unit/guild-boost.test.mjs` | `2b715cebeef80042911e35721c73e483` | да |
| `tests/unit/poll-card.test.mjs` | `ff5b0b0f4fd7f8395b28667c3e9d19ba` | да |

Все изменения укладываются в Write-Scope `TASK.md` (7 файлов). Продуктовый диф минимальный:
один файл, `+4/-1`; шесть тестовых — только JSDoc-аннотации и приведение типов.

## 2. Typecheck (заявленный блокер §5 `QA-BUILD-003`)

```
npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "thread-badge|chat-helpers-bridge|slowmode\.test|event-banner\.test|guild-boost\.test|poll-card\.test"
```

Результат — **0 строк** (grep exit `1`). Все 10 ошибок из `red.log` устранены, ни одна не «спрятана».

Общий вывод `tsc` по проекту — 170 строк ошибок в файлах вне scope (`profile-modal` 29,
`svg-icons` 27, `appearance-settings` 22, `channel-tree` 17, `locale-switcher` 9, `inbox`,
`embed-formatter`, `voice-input-mode`, `poll-progress`, `markdown-helper`, `svg-icons.js`,
`invite-parser.js`). Это остаток `FE-FIX-02` следующего прохода, а не регрессия PR-156:
карточка ограничивала scope семью файлами батча. **Риск:** `npm run typecheck` целиком
по-прежнему красный, закрывать пункт §5 целиком по этому PR нельзя. `P2`, вне вердикта.

## 3. Unit-тесты

```
node --test tests/unit/thread-badge.test.mjs tests/unit/chat-helpers-bridge.test.mjs \
  tests/unit/slowmode.test.mjs tests/unit/event-banner.test.mjs \
  tests/unit/guild-boost.test.mjs tests/unit/poll-card.test.mjs
```

`tests 68 / pass 68 / fail 0 / skipped 0` — совпадает с заявленным `68/68`. Прогон повторён
после восстановления мутированного файла: снова `68/68`, поведение не поехало.

## 4. Мутационное тестирование (`thread-badge.js`)

База: MD5 `8a8dbe830a80e378e3956ad614d29fc6`, прогон после каждой мутации —
`node --test tests/unit/thread-badge.test.mjs`.

| # | Мутация | Убийца (первый упавший тест) | Итог |
| --- | --- | --- | --- |
| M1 | `n > 99` → `n >= 99` (граница бейджа) | `formatUnreadCountBadge: 1..99 renders the number` | KILLED |
| M2 | `n === 0` → `n < 0` (пустой бейдж) | `formatUnreadCountBadge: zero and empty inputs render empty string` (+`clamps negatives`) | KILLED |
| M3 | убран `Math.max(0, …)` у `messageCount` | `formatThreadStatusSummaryRu: message count label is sanitized` | KILLED |
| M4 | приоритет `locked`/`archived` переставлен | `formatThreadStatusSummaryRu: locked wins over archived…` | KILLED |
| M5 | `DEFAULT_ARCHIVE_DURATION = 1440` → `60` | `normalizeArchiveDurationMinutes: unknown/invalid defaults to 1440` (+2) | KILLED |

Итог: **5/5 убиты**, все exit-коды `1`. Файл восстановлен из копии, а не через
`git checkout`, поэтому сохранилась правка PR. Контроль после восстановления:

- `md5sum` → `8a8dbe830a80e378e3956ad614d29fc6` (совпадает с `ACCEPT.md`);
- `git diff --stat` → `1 file changed, 4 insertions(+), 1 deletion(-)`, ровно хунк PR, без остатков мутаций.

## 5. Ревью кода

**Продуктовый файл.** Правка в `formatThreadStatusSummaryRu`: к индексации справочника добавлен
`?? "24 часа"`, что снимает `noUncheckedIndexedAccess`-ошибку без `as` и без `@ts-ignore`.
`normalizeArchiveDurationMinutes` возвращает только ключи `60/1440/4320/10080`, а набор
`ALLOWED_ARCHIVE_DURATIONS` совпадает с ключами `THREAD_ARCHIVE_DURATIONS_RU`, поэтому ветка
`??` недостижима и наблюдаемое поведение не меняется — тесты `archive label follows normalized duration`
и `empty thread falls back to defaults` это подтверждают. Комментарий в коде объясняет, зачем литерал.

**Тесты.** Ни одна проверка не ослаблена:
- `chat-helpers-bridge`, `event-banner`, `slowmode`, `thread-badge` — добавлен `/** @type {any} */`
  только вокруг заведомо невалидных входов негативных тестов (`locked: 1`, `userCount: "abc"`,
  `name: 42`, `messageCount: "abc"`, `undefined`); утверждения прежние.
- `guild-boost` — литерал массива получил тип `Array<0 | 1 | 2 | 3>`, цикл и проверки те же.
- `poll-card` — проверка **усилена**: `submitted[0].length` заменён на деструктуризацию с
  `assert.ok(votePayload, …)` плюс прежнее сравнение длины.

Новых `@ts-ignore` / `@ts-expect-error` / `@ts-nocheck` PR не добавляет (`grep` по семи файлам
даёт только `slowmode.test.mjs:21`, существовавший до PR — вне диффа). Стиль и русские
комментарии соответствуют окружающему коду.

## 6. Замечания `P2` (не блокируют merge, учесть при коммите)

1. **Гигиена коммита.** В рабочем дереве кроме семи файлов лежат `package-lock.json`
   (`bin`/`engines` досинхронизированы из `package.json` — побочный эффект `npm install`) и
   удалённый симлинк `node_modules` → `/home/user/orca/project/node_modules`. В Write-Scope
   `TASK.md` их нет. Коммит PR-156 должен содержать **только семь файлов**; `package-lock.json`
   и `node_modules` в коммит не включать — иначе в `unstable` уедет `node_modules` как удалённый.
2. **Недостижимый `??`** (см. §5) не покрывается мутациями по построению. Если в будущем
   справочник и `ALLOWED_ARCHIVE_DURATIONS` разойдутся, литерал молча отдаст «24 часа» —
   при добавлении нового срока архивации нужно править оба места.
3. **`npm run typecheck` целиком** остаётся красным (170 строк вне scope, §2) — нужен следующий
   проход `FE-FIX`.

## 7. Итог

Продуктовый код PR-156 корректен и минимален, заявленный блокер по семи файлам снят,
`68/68` тестов зелёные, `5/5` мутаций убиты, файл восстановлен и побайтно совпадает с `ACCEPT.md`.
`P0 = 0`, `P1 = 0` → **`APPROVED`**, можно переводить в `Ready for Merge` при условии, что коммит
включает только семь файлов из §1.
