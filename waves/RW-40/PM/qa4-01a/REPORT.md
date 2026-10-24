# REPORT — QA4-01a: `cdp eval|clicktext|list|shot`

- Волна: `RW-40`; карточка: **QA4-01a** (P1, G4 QA/Tooling); PM — DeepSeek.
- Вход: `waves/RW-40/QUALITY/TEST-AUDIT.md` (QA4-01), `tests/e2e/cdp-browser.e2e.test.mjs`.
- Ограничения соблюдены: без live Discord, без окна, без сети/установок, `tests/e2e/**`,
  `rust/**`, `logs/`, `functions/`, `evidence/` не менялись, коммитов нет.
- `walk` (QA4-01b) не трогался.

## Сделано

1. `src/core/cdp/client.mjs` (+118 строк, аддитивно):
   - `openPage({endpoint,hint,fetchImpl,WebSocket,timeoutMs})` — `listTargets` → `pickTarget`
     (hint из `CDP_TARGET`) → `createCdpClient`; нет страницы → `Error("нет подходящей страницы …")`;
   - `listElementsExpression` / `listElements` — CSS-селектор → JSON-массив
     `{tag, role, aria, text, visible, rect}` (≤ 200, как `scripts/cdp.mjs list`);
   - `clickTextExpression` / `clickText` — видимые `button`, `[role=button|tab|menuitem|menuitemradio]`,
     `a`, `div[tabindex]`; совпадение по `aria-label`/`title`/тексту (contains, без регистра);
     клик + `{clicked:true, tag, role, aria, text}`; не найдено — `clicktext: элемент не найден: «…»`;
   - `captureScreenshot` — `Page.captureScreenshot {format:"png"}` → `Buffer`; пустой ответ — ошибка.
2. `src/cli/run.mjs`: подкоманды `cdp eval|list|clicktext|shot` (targets не изменён):
   - `eval` → JSON значения в stdout (pretty, 2); JS-исключение/таймаут → код **2**;
   - `list` → JSON-массив в stdout; `clicktext` → `{clicked:true,…}`;
   - `shot` → PNG по пути (каталог создаётся), `{saved,bytes}`; ошибка записи → код **2**;
   - аргументы склеиваются (`cdp clicktext Мой аккаунт`), пустой аргумент → код **1**
     (`cdp eval: укажите …`, `cdp list: укажите CSS-селектор`, и т.п.);
   - неизвестная подкоманда → код **1**; ошибки CDP → код **2** с endpoint в сообщении;
   - `finally { page.cdp.close(); }` — сокет закрывается на всех путях.
3. `HELP` + `README.md` синхронизированы (4 строки справки, 4 строки таблицы команд,
   4 быстрых примера, уточнение кодов выхода: JS-исключение/таймаут/элемент не найден → 2).
4. `tests/unit/cdp.test.mjs` (+191 строка, только добавления): 15 новых тестов —
   `openPage` (выбор по hint / нет страницы), экранирование текста и селектора,
   `clickText` (успех / не найдено / null), `listElements` (+ не массив), `captureScreenshot`
   (байты PNG / пустой ответ), CLI: подкоманды в `--help`, обязательные аргументы → 1,
   неизвестная подкоманда → 1.

## Red (strict TDD)

`node --test tests/unit/cdp.test.mjs` → **exit 1** (`red.log`):

```
SyntaxError: The requested module '../../src/core/cdp/client.mjs'
does not provide an export named 'captureScreenshot'
```

## Green

| Проверка | Команда | Результат | Лог |
| --- | --- | --- | --- |
| unit (файл) | `node --test tests/unit/cdp.test.mjs` | 36/36 pass, exit 0 | `green-unit.log` |
| typecheck | `npm run typecheck` | exit 0 (оба tsconfig) | `typecheck.log` |
| targeted | `node --test tests/unit/cdp.test.mjs tests/meta/docs-sync.test.mjs tests/smoke/cli.smoke.test.mjs` | 44/44 pass, 0 fail, 0 skip, exit 0 | `targeted.log` |
| e2e | `node --test tests/e2e/cdp-browser.e2e.test.mjs` | **skip 1**: «нет браузера: задайте CHROME_BIN» (chromium/chrome/brave нет в PATH и в /usr/bin, /opt, snap), exit 0 | `e2e.log` |
| stub-CDP smoke (ручной) | `node waves/RW-40/PM/qa4-01a/EVIDENCE-stub-cdp-smoke.mjs` | все 9 сценариев pass, exit 0 | `stub-smoke.log` |

Полный `gate:worker` не запускался: работа идёт в batch RW-40, полный прогон —
один раз в integration-лейне (AGENTS §3). Targeted-наборы задания зелёные.

### Stub-CDP smoke (чем закрыт skip e2e)

Браузера в среде нет, поэтому e2e физически не исполняется. Чтобы проверить, что код
**исполняем** и связка CLI↔CDP работает, прогнан одноразовый stub-CDP (loopback
`127.0.0.1:0`, без live Discord и без интернета):
handshake WebSocket + `/json/list` + ответы на `Runtime.evaluate`/`Page.captureScreenshot`.
Проверено: `targets → 0`; `eval document.title → 0` (JSON-строка); `eval 1+1 → 0`;
`eval boom() → 2` с текстом исключения; `clicktext «Мой аккаунт» → 0 {clicked:true}`;
`clicktext «Нет такой» → 2` («элемент не найден»); `list button → 0`, 6 × `BUTTON`;
`shot` → 0, PNG-сигнатура на месте, каталог создан; `shot /dev/full → 2`
(«Не удалось записать скриншот»). Скрипт — evidence, не часть тест-набора
(unit не открывают сокетов, `tests/e2e/**` read-only).

## Файлы и md5

| Файл | md5 |
| --- | --- |
| `src/cli/run.mjs` | `3b3b5554fb2690486a0bac1358881e85` |
| `src/core/cdp/client.mjs` | `c8173e05512a7672d6fcb5b2ea399520` |
| `tests/unit/cdp.test.mjs` | `376c2bff2400c88a49f9e3601c26c8f2` |
| `README.md` | `b2bb127ba86e3f5744483425f3d5ba5b` |

`git status` по этим путям — только эти 4 файла; чужих правок нет.

## Риски / остаток

- **e2e не исполнялся** (нет браузера): контракт `tests/e2e/cdp-browser.e2e.test.mjs`
  покрыт кодом и stub-smoke, но «зелёный» e2e нужно получить в среде с `CHROME_BIN`
  (integration/CI). Ожидаемо 6 `BUTTON` на фикстуре — соответствует.
- **QA4-01b (`walk`)** не реализован: `walk` по-прежнему код 2 (M3), отдельная карточка.
- `cdp clicktext` при ненайденном элементе отдаёт код **2** (как ошибка выполнения в странице,
  в одной конвенции с JS-исключением `eval`); если Team Lead предпочтёт код 1 — правка
  одной ветки `run.mjs` + тест.
- Выражения страницы (`clickTextExpression`/`listElementsExpression`) покрыты unit-тестами
  на экранирование и валидность синтаксиса, но не на DOM-семантику — это зона e2e.
- Тесты не ослаблялись; skip только в e2e по отсутствию браузера (skip-aware, с причиной).
