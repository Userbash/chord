# BRIEF W38f — снять P1 RW-38 (разрыв петли LOOP-01)

Владелец: W38f (fix). PM: DeepSeek. Ревьюер: REV-2 (не автор). Итерация: 2.
Причина: ревью `waves/RW-38/REVIEW-codex.md` = **FAIL, P0=0, P1=2**; находки без исполнителя.

## Файлы (только эти)

- `scripts/gui-window-probe.py`
- `tests/smoke/gui-spike.smoke.test.mjs`, `tests/smoke/gui-window.smoke.test.mjs`
- `rust/gui-spike/tests/launch.rs`
- `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js`
- `tests/unit/gui-spike-dom.test.mjs`
- отчёт/логи: `waves/RW-38/workers/W38f/`

Всё остальное — read-only. `waves/RW-36/37`, `src/`, `docs/` не трогать. Без commit/push, сети.

## Задачи (red → green, каждая отдельно)

1. **P1-1 PID-привязка пробы.** AT-SPI: `get_process_id()==pid`; X11: `_NET_WM_PID==pid`,
   ≥2 совпадений → fail, 0 → `EnvFail`; писать `pid` в `--json`; `proc.poll()` перед каждым
   шагом; kill child в `finally`. Red: старый код на подставном «чужом» окне даёт pass.
2. **P1-2 a11y эмодзи-сетки.** `role="listbox"` на `#emoji-grid` + `role="option"` на ячейках
   (визуал не менять). Проба: ≥10 ячеек с непустыми именами **внутри активного region**.
   DOM-тест: у каждой ячейки допустимая роль + непустой `aria-label`.
3. **P2-1** при `DISPLAY`/`WAYLAND` и отсутствии `GUI_SPIKE_E2E=0`/`GUI_WINDOW_PROBE_OPTIONAL=1`:
   нет бинаря или `status===2` → **fail**, не skip. Skip — только без дисплея.
4. **P2-3** launch/smoke: после snapshot процесс должен быть жив до `SPIKE_EXIT_AFTER_MS-1s`;
   преждевременный exit 0 → fail; в пробе смерть процесса → `EnvFail`.

## Команды (в этом порядке, один cargo за раз)

```bash
node --test tests/unit/gui-spike-dom.test.mjs
cargo build --offline --manifest-path rust/gui-spike/Cargo.toml
python3 scripts/gui-window-probe.py --json /tmp/opencode/gui-probe.json --timeout 30
GUI_SPIKE_E2E=1 cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test launch -- --nocapture
node --test --test-concurrency=1 tests/smoke/gui-spike.smoke.test.mjs tests/smoke/gui-window.smoke.test.mjs
npm run gate:worker
```

Хост флейкует (окно умирает 1,6–3 с). Один ретрай допустим — фиксировать оба результата.

## Запрещено

- Ослаблять/удалять ассерты, `t.skip` без причины, менять чужие тесты.
- Чинить продукт под тест; менять `style.css`/layout (это W39c).

## Отчёт

`waves/RW-38/workers/W38f/REPORT.md` по PROCESS §4.2: red-лог (до), green-лог (после),
список файлов + md5, что осталось (P2-2/P2-4…P2-10 — не в scope), риски.
Если 2 попытки не дали green — писать `BLOCKED` с сигнатурой отказа и гипотезой, не долбить.
