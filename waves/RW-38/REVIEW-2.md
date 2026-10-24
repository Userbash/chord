# RW-38 — второе независимое ревью (REV-2, после W38f)

Дата: 2026-10-23 (начало) — 2026-10-24 (завершение). Ревьюер: **REV-2**, не автор кода (автор —
W38f). Продуктовый код и тесты не менялись: только чтение, прогоны, временные мутации с
откатом (md5 до/после) и этот файл. Без commit/push, без сети.

Предмет ревью — байты W38f, зафиксированные в `waves/RW-38/workers/W38f/md5-final.txt`:

```
ef633a2e43a213927e7cd3a5f9db54f4  scripts/gui-window-probe.py
7321007b3b414648997a646409167fd4  tests/smoke/gui-spike.smoke.test.mjs
3c41c820626e2260ef7ea69df3fb9018  tests/smoke/gui-window.smoke.test.mjs
9cad6d87bfcfa4936367d24b91c36c94  rust/gui-spike/tests/launch.rs
295b63b2fa5add22d4c6b787bfad1f28  rust/gui-spike/ui/index.html
c056c93af87d65b7534ad373f51e5b63  rust/gui-spike/ui/app.js
9a12bb77880f7f70f0781c9f31874e5f  tests/unit/gui-spike-dom.test.mjs
```

Мой `md5sum` этих 7 файлов 2026-10-23 21:14 совпал с `md5-final.txt` — **7/7**. Проверял
содержимое (файлы untracked, `git diff` недоступен), а не отчёт автора.

Контекст к моменту сдачи: RW-38 уже записан как accepted (`waves/db.jsonl`, событие
`accepted`, evidence `waves/RW-38/REVIEW-pass2.md`). Это ревью — независимая вторая проверка;
вердикт с записанным совпадает, расхождение только в объёме живых прогонов (см. §5).
Имя артефакта, запрошенное у REV-2 (`REVIEW-2.md`), не совпадает с именем в записи
(`REVIEW-pass2.md`) — PM стоит зафиксировать канон.

## 0. Что реально прогнано (не только прочитано)

| # | Команда | Результат |
|---|---|---|
| 1 | `node --test tests/unit/gui-spike-dom.test.mjs` на байтах W38f (2026-10-23) | exit 0, **21/21** pass, 0 skip (20 прежних + новый P1-2-кейс) |
| 2 | `node --test tests/unit/gui-spike-dom.test.mjs` на текущем `ui/` (2026-10-24) | exit 0, **29/29** pass (W39d добавил кейсы) |
| 3 | `node --test --test-concurrency=1 tests/smoke/{gui-spike,gui-window}.smoke.test.mjs` (2026-10-23 21:14) | **0/2**, оба красные: `бинарь старше ui/*` — W39c в этот момент писал `ui/tokens.css`, `ui/style.css`, `ui/components/*` (mtime 21:13–21:16). Это inter-wave interference, не дефект W38f |
| 4 | `GUI_SPIKE_E2E=1 cargo test ... --test launch -- --nocapture` (2026-10-23, 2 попытки) | **0/1 ×2**: живой процесс печатал `window-ready`, но не `UI_SNAPSHOT`, в stderr `WebProcess didn't exit as expected`; тест падал по `UI_SNAPSHOT отсутствует`. Контроль: тот же хост, другой бинарь (`/tmp/opencode/spike-bin-1857`, сборка до W39c) печатает `UI_SNAPSHOT` — WebKit на хосте жив, красный даёт промежуточное состояние `ui/` W39c (в W38f-байтах `app.js`/`index.html` не менялись, delta — только CSS W39c) |
| 5 | `python3 scripts/gui-window-probe.py --json ... --timeout 30` (2026-10-23) | exit 1, `FAIL: бинарь старше ui/*` — stale-guard сработал correctly (отказ, не ложный pass) |
| 6 | то же на текущем `ui/` (2026-10-24 15:05) | **exit 0**: `ok:true`, `pid == window.pid`, `emoji_cells: 20` (≥10), `settings_toggles: 38`, `chat_items/member_items: 25`, PNG 1280×820, `unique_colors: 5037` |
| 7 | `node --test --test-concurrency=1 tests/smoke/...` (2026-10-24) | **2/2** pass, 0 skip, 14.4 s (ретрай после п.3 — оба результата здесь) |
| 8 | `GUI_SPIKE_E2E=1 cargo test ... --test launch` (2026-10-24) | **1/1** pass, 9.11 s (`lived ≥ 8 s`) |
| 9 | P2-1: `PATH=<stub python3: exit 2> GUI_SPIKE_E2E=1 node --test tests/smoke/gui-window.smoke.test.mjs` | **exit 1**, `проба не нашла среду на дисплее (exit 2) — это fail, не skip`; `skipped 0` |
| 10 | P2-1: `GUI_SPIKE_BIN=/tmp/opencode/no-such-spike node --test tests/smoke/gui-spike.smoke.test.mjs` | **exit 1**, `нет бинарника ... на дисплее smoke обязателен`; `skipped 0` |
| 11 | P2-1: `env -u DISPLAY -u WAYLAND_DISPLAY node --test tests/smoke/gui-window.smoke.test.mjs` | exit 0, `skipped 1` — skip только без дисплея |
| 12 | P2-3: `SPIKE_BIN_OVERRIDE=<fake: window-ready + exit 0> GUI_SPIKE_E2E=1 cargo test ... --test launch` | **exit 101**, `окно прожило 50.19ms < 8s ... преждевременный exit 0` (launch.rs:138) |
| 13 | P1-1 (AT-SPI): ветки `atspi_app()` на фейковом desktop (мой оракул, `/tmp/opencode/rev2-atspi-branches.py`) | 4/4: 1 app pid=42 → app; 0 → `None`; 2 → `Fail`; без `get_process_id` → `EnvFail` |
| 14 | P1-1 (X11): два живых окна с **одинаковым** заголовком, разные PID → `x11_capture(Image, title, pid)` | `pid A → xid A`, `pid B → xid B`, `pid 999999 → EnvFail «своё окно … не найдено»` — фильтр `_NET_WM_PID` работает |
| 15 | P2-3 (проба): fake-бинарь «window-ready + UI_SNAPSHOT + exit 0» через копию пробы | **exit 2**, `SKIP: спайк завершился на шаге «AT-SPI» (exit=0) — окно умерло`, pid/log в JSON |

## 1. Находка → как проверял → результат

| Находка pass1 | Как проверял (команда/код) | Результат |
|---|---|---|
| **P1-1** окно/приложение не привязаны к своему PID | чтение `gui-window-probe.py` (W38f: 113-134, 155-163, 200-318, 482-490; текущая версия: 128-151, 172-180, 219-335, 545); п.13, п.14; мутации M-1/M-2 | **ЗАКРЫТО.** AT-SPI: `get_process_id()==pid` — единственный критерий выбора, имени в фильтре нет; 0 → `None` → `own_app()` → `EnvFail`; ≥2 → `Fail`. X11: одновременно уникальный `SPIKE_WINDOW_TITLE` (pid+ms) **и** `_NET_WM_PID==pid`; 0 → `EnvFail`; ≥2 → `Fail`; `pid` пишется в `--json` на успехе/`Fail`/`EnvFail` и в `window.pid`; `require_alive()` (`proc.poll()`) перед каждым шагом и внутри `wait_for_marker`; child гасится в `finally` (`terminate → wait(5) → kill`). Обходов «первое окно»/«по имени»/тихого fallback в X11/AT-SPI нет. |
| **P1-2** `grid` без row → пустая таблица в AT-SPI | `ui/index.html:39` (W38f) / `:150` (текущий): `role="listbox"` + `aria-label="Эмодзи"`; `app.js:503-508` (W38f) / `features/emoji.js:400-401`: `role="option"` + непустой `aria-label`; проба 420-429: `find_all(grids[0], role in CELL_ROLES and name≠"")`, `<10 → Fail`; DOM-тест 498-512: `cells.length>=10` + для **каждой** ячейки роль и имя; мутации M-3/M-4 | **ЗАКРЫТО.** Ячейки ищутся **внутри своего** listbox (не глобально), порог 10 именованных; DOM-тест не пустой цикл (guard ≥10 и assert в теле). `emoji_cells: 20` на живой пробе сегодня. |
| **P2-1** тихий skip на дисплее | п.9–11 + код обоих smoke: `forced==="0"`/`optional`/`!hasDisplay` → skip; иначе нет бинаря → `assert.fail`, `status===2` → `assert.notEqual(...,2)`; skip-ветки стоят **до** fail-проверок и не перекрывают их | **ЗАКРЫТО.** На дисплее отсутствие бинаря/среды = fail (exit 1, skip 0); skip только без дисплея или явный opt-out. |
| **P2-3** живость окна не проверялась | `launch.rs:137-142` (`lived >= 9000ms-1s`), `gui-spike.smoke:41-72` (`minAlive=exitAfterMs-1000`), `gui-window-probe:153-169,407,432,477,491,499,509`; п.12, п.15 | **ЗАКРЫТО.** Ранний exit 0 → красный в launch (exit 101) и в smoke; смерть в пробе → `EnvFail`/exit 2 с причиной. Тайминги не флейкуют: таймер приложения стартует на `window-ready`, а `lived` меряется от `spawn`, т.е. запас только растёт (реально 9.1–10.1 s при порогах 8/9 s). |
| **Ослабление/регресс** | ручное чтение всех 4 тестовых файлов + сравнение с red-логами W38f; `rg "assert.ok(true)|t.skip|.only|todo"`; DOM-прогон показывает все 20 прежних кейсов + 1 новый; smoke/launch только ужесточены | **Ослаблений нет.** Удалённых assert не найдено (файлы untracked — проверял по содержимому и по списку кейсов). `t.skip` только с причиной; `assert.ok(true)`/`.only`/`todo` отсутствуют. |
| **Границы файлов** | `md5sum` 7 файлов vs `md5-final.txt`; `git status` / mtime | **Соблюдены** на момент сдачи: 7/7 md5. Позже (W39c/W39d/W39h4/W39g) файлы `ui/*`, пробу и smoke меняли другие волны — это уже не W38f (см. §5). |

## 2. P0 / P1 / P2 / P3

- **P0 = 0.**
- **P1 = 0.** P1-1 и P1-2 закрыты; P2-1 и P2-3 тоже закрыты.
- **P2 = 3** (не блокируют приёмку, P2-2/P2-4…P2-10 — вне scope W38f):
  1. **P2-A. Нет автотеста на ветки PID пробы.** `atspi_app()` и X11-фильтр покрыты только
     моим ad-hoc оракулом (§0 п.13-14); в `tests/` вызова `atspi_app` нет — регресс «снова
     первое приложение» гейт не поймает. Рекомендация: `tests/unit/gui-window-probe.test.mjs`
     (0→EnvFail, 1→ok, 2→Fail) — дешёво, закрывает риск навсегда.
  2. **P2-B. AT-SPI-проверки остаются глобальными/по имени** (рейл, `entry`, chat/member
     `list item`), роли/имена/`.screen-title`/активный region не сверяются — остаток pass1
     P2-2, вынесен из scope W38f.
  3. **P2-C. Гейт не считает skip live-smoke провалом.** Фаза `smoke` в `scripts/gate.mjs`
     без `failOnSkips`; при `GUI_SPIKE_E2E=0`/`GUI_WINDOW_PROBE_OPTIONAL=1` на машине с
     дисплеем гейт даёт `passed_with_skips` (warning), т.е. live-слой может снова исчезнуть
     без красного вердикта. Это уровень гейта (вне файлов W38f), но риск тот же, что в P2-1.
- **P3 = 4:**
  1. `find_all()` глотает исключения: смерть процесса между `require_alive` и `find_all`
     даст `Fail` (exit 1), а не `EnvFail` (exit 2) — нит, не воспроизводится.
  2. `x11_capture()` на «своё окно <200×200 / XGetImage вернул NULL» бросает `Fail` (exit 1),
     хотя причина средовая (окно не смаплено/свернулось) — ложный красный, не ложный pass.
  3. Stale-guard (`gui-spike.smoke:newestUiMtime`, проба:`glob("*")`) учитывает mtime
     **каталогов** `ui/components`: соседний воркер, добавивший файл, делает бинарь «старым» →
     ложный fail live-слоя. Ровно это случилось 2026-10-23 21:13–21:16 и 2026-10-24 15:06.
  4. Отчёт W38f неточен по scope: утверждает, что P2-5 (Xlib-утечки) и P2-6 (timeout/лог)
     «не менялись», но финальная проба их содержит (`_X_ERROR_HANDLER`, `XDestroyImage`,
     `XFree`, `XCloseDisplay`, уникальный `mkstemp`-лог, `SPIKE_EXIT_AFTER_MS=(timeout+10)s`);
     «глобальный счётчик list item заменён» верно только для эмодзи-ячеек.

## 3. Мутации (каждая откатана; md5 до/после)

| # | Инвариант | Мутация | Поймано? |
|---|---|---|---|
| M-1 | P1-1 AT-SPI PID | копия пробы: `if app_pid == pid` → `if "discord" in name`, убран `≥2 → Fail` | **Да**, оракул: `0 apps → None` и `2 apps → Fail` перестали выполняться (`FAILED ['0 apps…','2 apps…']`) |
| M-2 | P1-1 X11 PID | копия пробы: `owned = [… if window_pid(xid) == pid]` → без фильтра; на стенде 2 окна с одинаковым заголовком | **Да**: unmutated — каждый PID получает своё окно, чужой PID → `EnvFail`; mutated → `Fail: 2 окон … неоднозначная привязка` |
| M-3 | P1-2 роль контейнера | `ui/index.html`: `#emoji-grid role="listbox"` → `"grid"` | **Да**, DOM-тест: `#emoji-grid должен иметь role=listbox` (exit 1); restore: `417bd668…` = до мутации |
| M-4 | P1-2 роль ячейки | `ui/features/emoji.js`: `role="option"` → `"gridcell"` | **Да**, DOM-тест: `ячейка «😀» должна иметь role=option` (exit 1); restore: `df0ad211…` = до мутации |
| M-5 | P2-3 ранний exit | `SPIKE_BIN_OVERRIDE` = fake «window-ready + exit 0» | **Да**, launch: exit 101, `прожило 50ms < 8s` |
| M-6 | P2-3 смерть = env | копия пробы: `require_alive` → `raise Fail` вместо `EnvFail`; fake «окно умерло после snapshot» | **Да**: эталон — exit 2 (`SKIP … окно умерло`), мутант — exit 1 (`FAIL`) |

Мутации M-3/M-4 применялись к **текущему** `ui/` (после W39c/W39d), а не к байтам W38f: `ui/`
переписан следующими волнами. Инвариант и тест те же; md5 восстановлены. Мутации M-1/M-2/M-6 —
на копиях пробы в `/tmp/opencode`, продуктовый файл не менялся.

## 4. Вне scope этого ревью

- **P2-2** (роли/имена/заголовок активного экрана, scoped chat/member, роль рейла) — pass1;
  W38f её не брал. Часть позже добавлена W39h4 (проба теперь 566 строк), но приёмка RW-38
  этим не обусловлена.
- **P2-4** Rust-слой в гейте: `spike-rust` в `gate.mjs` появился позже; `launch` всё ещё
  skip без `GUI_SPIKE_E2E=1` — skip выглядит как pass. Вне файлов W38f.
- **P2-5** Xlib-ресурсы (в W38f-пробе фактически уже закрыто: handler удержан,
  `XDestroyImage`/`XFree`/`XCloseDisplay` есть; см. P3-4 про отчёт).
- **P2-6** `--timeout`/лог/child — закрыто в пробе, но в отчёте W38f описано как «не менялось».
- **P2-7** мутации на финальном `app.js` — мои M-3/M-4 прогнаны на текущем `ui/`; на байтах
  W38f мутации не перепрогонялись (файл с тех пор заменён W39d).
- **P2-8** `--bench-log` без `BENCH`; **P2-9** `aria-live`/FPS; **P2-10** сирота при таймауте
  smoke (`spawnSync(timeout:80s)` убивает python, child живёт до `SPIKE_EXIT_AFTER_MS`).

## 5. Ограничения этого ревью

1. **Живые прогоны 2026-10-23 красные не по вине W38f.** В 21:13–21:16 параллельно работал
   W39c и менял `ui/tokens.css`, `ui/style.css`, `ui/components/*`; stale-guard честно
   отказывал, а бинарь, собранный из промежуточного `ui/`, не отдавал `UI_SNAPSHOT`
   (контрольный бинарь до W39c — отдаёт). На байтах W38f живой green подтверждён логами
   W38f (`green-launch.log`, `green-smokes-final.log`, `green-probe.json`) при совпавших md5.
2. **Живые прогоны 2026-10-24 зелёные** (проба exit 0, smoke 2/2, launch 9.11 s), но это уже
   `ui/` после W39c/W39d; прямой перепрогон байтов W38f невозможен (файлы untracked и
   заменены). Поэтому по P1-1/P1-2 опора — чтение кода + мои мутации/оракулы + живые ветки
   на текущей пробе, где PID-логика идентична.
3. **Параллельные сессии.** Во время ревью 2026-10-24 15:06–15:08 другой воркер правил
   `gui-window.smoke.test.mjs`, `scripts/gui-window-probe.py` и `rust/gui-spike/tests/launch.rs`
   (md5 менялись: `3c41c8…`→`8678287…`, `d885e4…`→`f62b8b…`, `9cad6d…`→`0f710c…`) и запускал
   `gate:worker`/`cargo`. Блок P2-3 в `launch.rs` при этом не изменился (константы
   `SPIKE_EXIT_AFTER_MS=9000`/`LIVENESS_MARGIN=1000` и assert `lived >= min_alive` на месте,
   строки сместились на 1). Мои мутации не пересекались с их файлами; восстановление
   `ui/index.html` и `ui/features/emoji.js` проверено md5 до/после.
4. Авторские red/green-логи не проверялись на подделку; они согласованы по строкам с
   финальными файлами, но первичны для меня код и мои прогоны.
5. `rust/gui-spike/**` целиком untracked → «что было до волны» историей не доказуемо; P1-2
   проверен по содержимому (единственный путь рендера ячеек) и поведением.

## 6. Вердикт

**PASS** — **P0 = 0, P1 = 0** (P1-1 и P1-2 закрыты; P2-1 и P2-3 закрыты), P2 = 3 (P2-A…P2-C,
все вне файлов W38f, кроме P2-C — уровень гейта), P3 = 4. Живые прогоны: проба — exit 0,
smoke — 2/2 (0 skip, ретрай после inter-wave stale-конфликта), launch — 1/1 (9.11 s),
DOM — 21/21 на байтах W38f и 29/29 на текущих. Все 6 мутаций пойманы.

Рекомендация PM: приёмку RW-38 оставить в силе (запись `accepted` уже есть), P2-A оформить
отдельной карточкой (юнит-тест веток `atspi_app`), P2-C — на уровне гейта (`failOnSkips` для
live-smoke), P3-3 — исключить каталоги из stale-guard, чтобы параллельные волны не красили
live-слой ложно.
