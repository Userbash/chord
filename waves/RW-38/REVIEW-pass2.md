# RW-38 — второе независимое ревью (REV-2, после W38f)

Дата: 2026-10-23. Ревьюер: REV-2 (не автор кода). Продуктовый код и чужие тесты не менялись:
только чтение, прогоны и этот файл. Живое окно **не запускалось** (0 из 1 разрешённого):
P1-1/P1-2 подтверждены кодом + red/green-логами W38f на финальных байтах (md5 совпал),
P2-1/P2-3 — независимыми headless-прогонами. Один процесс `cargo` (fake-бинарь, окна нет),
других `cargo` в системе не было (`pgrep -af "cargo|rustc"` пуст).

## Вердикт

**PASS** — P0 = 0, **P1 = 0** (P1-1, P1-2 закрыты), P2 = 3 (все — вне scope W38f, REV-1 их и
не требовал), P3 = 4. P2-1 и P2-3 закрыты. Рекомендация PM: **accept** RW-38; закрыть
LOOP-01/L-01a…d; P2-2 из pass1 и новые ниты — отдельными карточками (см. §5).

## 1. Что прогнано независимо

| Команда | Результат |
|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` | exit 0, **21/21** pass, 0 skip |
| `PATH=<stub python3 exit 2> GUI_SPIKE_E2E=1 node --test tests/smoke/gui-window.smoke.test.mjs` | **exit 1**, `проба не нашла среду ... (exit 2) — это fail, не skip` (`notStrictEqual`) |
| `GUI_SPIKE_BIN=/tmp/opencode/no-such-spike node --test tests/smoke/gui-spike.smoke.test.mjs` | **exit 1**, `нет бинарника ... на дисплее smoke обязателен` |
| `env -u DISPLAY -u WAYLAND_DISPLAY node --test tests/smoke/gui-window.smoke.test.mjs` | exit 0, `skipped 1` (только без дисплея) |
| `GUI_SPIKE_E2E=1 SPIKE_BIN_OVERRIDE=…/fake-early-spike.sh cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test launch -- --nocapture` | **exit 101**, `окно прожило 1.051386138s < 8s ... преждевременный exit 0` (`launch.rs:138`) |
| `importlib` + fake AT-SPI desktop → `atspi_app(Atspi, 42)` | 0 совпадений → `None`; 1 → app(pid=42); 2 → `Fail: AT-SPI: 2 приложений с PID 42 — неоднозначная привязка` |
| `md5sum` 7 файлов W38f vs `workers/W38f/md5-final.txt` | **7/7 совпали** |
| `find . -newermt 20:55 ! -newermt 21:13` | изменены ровно 7 разрешённых файлов + `waves/RW-38/workers/W38f/**` |

## 2. Находка → проверка → результат

| Находка pass1 | Проверка (чем) | Результат |
|---|---|---|
| **P1-1** окно/приложение не привязаны к своему PID | код `gui-window-probe.py:113-134, 155-163, 244-288, 310, 458, 482-490`; fake-desktop прогон; red/green-логи | **ЗАКРЫТО**: AT-SPI только `get_process_id()==pid`, 0 → `EnvFail`, ≥2 → `Fail`; X11 `_NET_WM_PID==pid`, 0 → `EnvFail`, ≥2 → `Fail`; `pid` в `--json` (успех/Fail/EnvFail) и в `window.pid`; child гасится в `finally`; `SPIKE_WINDOW_TITLE` реально применяется бинарём (`main.rs:51`) |
| **P1-2** `grid` без `row` → пустая таблица в AT-SPI | `index.html:39`, `app.js:495-511`, `gui-window-probe.py:394-405`, `gui-spike-dom.test.mjs:363-386`; red-лог | **ЗАКРЫТО**: `role=listbox` + `aria-label="Эмодзи"`, единственный путь рендера ячеек даёт `role=option` + непустой `aria-label`; проба требует ≥10 именованных ячеек **внутри** своего listbox; DOM-тест фиксирует и статическую разметку, и каждую ячейку |
| **P2-1** тихий skip на дисплее | 3 прогона из §1 + code | **ЗАКРЫТО**: нет бинаря/`status===2` на дисплее → fail; skip только без дисплея или явный `GUI_SPIKE_E2E=0`/`GUI_WINDOW_PROBE_OPTIONAL=1` (последний разрешён BRIEF W38f) |
| **P2-3** живость окна не проверялась | `launch.rs:137-142`, `gui-spike.smoke.test.mjs:40-72`, `gui-window-probe.py:137-152`; fake-прогон | **ЗАКРЫТО**: `lived ≥ SPIKE_EXIT_AFTER_MS−1s` в launch и smoke (fake с корректными маркерами и exit 0 — красный); `proc.poll()` перед шагами и в `wait_for_marker`, смерть → `EnvFail (exit 2)` |
| **Ослабление тестов** | `git status` + инвентарь red/green-логов + ручное чтение `launch.rs` | **Нет ослаблений.** Все 20 прежних DOM-кейсов из `red-p1-2-dom.log` присутствуют в green (21-й добавлен); smoke/launch только ужесточены. Оговорка: `git diff tests/` пуст, т.к. все GUI-тесты **untracked** — аргумент LOOP-LEDGER «в tracked-диффе нет удалённых assert» вакуумный (проверял по содержимому) |
| **Границы файлов** | `find -newermt`, md5 | **Соблюдены**: 7 файлов из BRIEF W38f, чужие не тронуты (`tests/support/`, `bench/model`, `src/`, `gate.mjs` — mtime до 20:55) |

### Детали по P1-1 (главный вопрос — ловится ли старый дефект)

```python
# 113-134
if app_pid == pid: matches.append(app)
if len(matches) > 1: raise Fail(f"AT-SPI: {len(matches)} приложений с PID {pid} …")
return matches[0] if matches else None          # None → own_app() → EnvFail
# 283-288
owned = [(xid, title) for xid, title in found if window_pid(xid) == pid]
if not owned: raise EnvFail(f"своё окно PID {pid} … не найдено")
if len(owned) > 1: raise Fail(f"X11: {len(owned)} окон с PID {pid} …")
```

Чужое окно/приложение не может быть принято дважды: (а) `atspi_app` фильтрует по PID, имена
убраны из критерия; (б) X11 требует одновременно уникальный заголовок пробы (pid+ms) **и**
`_NET_WM_PID`. RED-лог `red-p1-1-foreign-atspi.log` (`AT-SPI PID 210208 != запущенного 210437`)
фиксирует, что старый код брал первое «discord»-приложение; GREEN
`green-p1-1-foreign.log` — при живом чужом экземпляре (pid 219607) проба сняла своё окно
(pid 219815), `window.pid == report.pid`.

Ограничение: ветки 0/≥2 в шипнутом коде не покрыты сохранённым логом (red-лог снят на
промежуточной сборке, его сообщения в финальном коде нет) — проверены мной fake-desktop
прогоном (см. §1), но не автоматическим тестом (P2-1 ниже).

## 3. Что осталось (P2 — не блокирует приёмку, вне scope W38f)

1. **P2-A. Нет автотеста на PID-ветки пробы.** `atspi_app(Atspi, pid)` уже принимает
   «модуль» аргументом → тестируется fake-desktop'ом (мой прогон это доказал), но в `tests/`
   такой кейс отсутствует: `grep -rln "atspi_app" tests/` → пусто. Регресс «снова первое
   приложение» гейт не поймает. Рекомендация: `tests/unit/gui-window-probe.test.mjs`
   (0→EnvFail, 1→ok, 2→Fail) — дешёво и закрывает риск навсегда.
2. **P2-B. «Активный region» в пробе проверяется косвенно** (остаток pass1 P2-2).
   `gui-window-probe.py:396-405` ищет listbox по всему приложению; `entry` — тоже глобально
   (`:392`); `chat_items`/`member_items` (`:433, :441`) по-прежнему глобальные, роли/имена
   строк и `.screen-title` не сверяются (`grep "screen-title\|aria-hidden\|heading"` → пусто).
   Следствие: `switchScreen`-no-op пройдёт, если a11y-мост отдаёт скрытые поддеревья.
3. **P2-C. `gui-window.smoke.test.mjs:29-32` — `spawnSync(timeout: 80s)`** убивает только
   python; при таймауте дочерний спайк живёт до своего `SPIKE_EXIT_AFTER_MS` (40 с).
   Остаток pass1 P2-10; теперь безвреднее (PID-привязка не даст ложного pass), но окно-сирота
   на 40 с остаётся.

## 4. Ниты (P3)

1. **Отчёт W38f неточен по scope.** `REPORT.md:140-142` утверждает, что P2-5 (Xlib-утечки) и
   P2-6 (timeout/лог) «не менялись», но финальная проба их содержит: `_X_ERROR_HANDLER`
   (195-197, 232-235), `XDestroyImage` (315), `XFree` (264), `XCloseDisplay` (318), уникальный
   `mkstemp`-лог (74-77), lifetime `(timeout+10)s` (80). Правки в разрешённом файле и полезные,
   но шапку отчёта надо синхронизировать. Там же: «глобальный счётчик list item заменён» верно
   только для эмодзи-ячеек.
2. **Тест-хуки `GUI_SPIKE_BIN` / `SPIKE_BIN_OVERRIDE`** подменяют бинарь в smoke/launch. Гейт
   их не выставляет (`grep` по `gate.mjs` пуст), но и не чистит: забытая переменная в shell
   подменит проверяемый артефакт. Рекомендация: `delete env` в фазе smoke либо assert «не задан».
3. **Узкая гонка в пробе:** смерть процесса между `require_alive` и `find_all`/`XGetImage`
   даст `Fail` (exit 1), а не `EnvFail` (exit 2): `find_all` глотает исключения (166-177), а
   `x11_capture` на пропавшем окне бросает `Fail` (316). Не воспроизводится — нит.
4. **`GUI_WINDOW_PROBE_OPTIONAL=1`** отключает и `gui-spike.smoke` (не только пробу) — имя
   шире действия; факт skip виден только в TAP-выводе, в JSON гейта следов нет.

## 5. Ограничения этого ревью

- Живое окно не поднималось: GREEN-логи W38f (21:06–21:09) сняты на **финальных** байтах
  (`gui-window-probe.py` mtime 21:02 = md5 `ef633a2e…`; `index.html` 21:04, `app.js` 21:04),
  мои md5 совпали, поэтому перепрогон не добавлял информации; плюс сейчас в `rust/gui-spike/ui/`
  параллельно работает W39c (`tokens.css`, `style.css`, `components/` с 21:13–21:16), и live-
  прогон рисковал снять промежуточную вёрстку.
- `rust/gui-spike/**` целиком untracked → диффа `app.js`/`index.html` против «до» нет; правки
  ARIA проверены по содержимому (единственный путь рендера ячеек) и поведением (21 DOM-тест +
  live smoke/snapshot), но «ничего кроме ARIA не менялось» историей не доказуемо.
- Авторские логи не перепроверялись на подделку; RED-GREEN-пары согласованы по строкам
  (`launch.rs:138`, `gui-spike.smoke:68`, `gui-window.smoke:33`) с финальными файлами.

## 6. Рекомендация PM

**Accept RW-38** (P0=0, P1=0). Возвращать W38f нечего по блокерам; при желании закрыть ниты
до приёмки — вернуть W38f **только P2-A** (юнит-тест `atspi_app`: 0→EnvFail, 1→ok, 2→Fail) и
**P3-1** (правка шапки отчёта), это ~15 минут и тот же файл/каталог.

Follow-up (новые карточки, не в этот batch):

| Кому | Что |
|---|---|
| PM/W38g (проба) | P2-B: роли/имена/`.screen-title`/активный region в AT-SPI, scoped `chat_items`/`member_items` (pass1 P2-2) + P2-A-тест |
| W39d (`ui/app.js`) | OBS-01 из LOOP-LEDGER: `listbox/option` без клавиатуры (`tabindex`/`keydown`/`aria-activedescendant`) |
| PM | P2-C (сирота при таймауте smoke), P3-2 (хуки в env гейта) |
| Team Lead | запись `waves/db.jsonl` + `waves/STATE.md`; LOOP-01 → `closed`, L-01a…d → `done` |
