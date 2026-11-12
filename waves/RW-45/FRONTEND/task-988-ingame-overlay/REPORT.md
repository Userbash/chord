# REPORT — TASK-988: Глобальные хоткеи и Wayland In-Game Overlay (`ingame_overlay.rs` + `ingame-overlay.js`)

- **Волна / Карточка:** Wave RW-45 / `TASK-988` (`task_feefed3fe369`, dispatch `ctx_c02827426ede`)
- **Отдел:** `G2 Frontend`
- **Приоритет:** `P2`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 988.1 — Нативный модуль GPUI [`rust/discord-gpui/src/ingame_overlay.rs`](file:///home/user/orca/project/rust/discord-gpui/src/ingame_overlay.rs) и модуль интерфейса [`rust/gui-spike/ui/features/ingame-overlay.js`](file:///home/user/orca/project/rust/gui-spike/ui/features/ingame-overlay.js):**
   - Реализована поддержка оверлея поверх полноэкранных игр под Wayland через протокол `ext-foreign-toplevel-list-v1` (`ext_foreign_toplevel_list_v1` / `ext_foreign_toplevel_handle_v1`, события `toplevel -> app_id -> title -> identifier -> done -> closed/finished`) и fallback на X11 (`OverrideRedirect` + `XShapeCombineRegion` `ShapeInput`).
   - Реализован прозрачный клик-сквозь (`input passthrough`) в режиме HUD (`wl_surface.set_input_region` с пустым `wl_region` `[]` под Wayland, `XShapeCombineRegion` с пустым регионом `ShapeInput` под X11, CSS `pointer-events: none`, `background: transparent`, `dimensions: "transparent 100vw x 100vh"`), при котором все клики мыши пробрасываются напрямую в полноэкранную игру (`forwardedToGame: true`, `interceptedByOverlay: false`).
   - Реализована машина интерактивных состояний `overlay-chat-bubble` (`default: "semi-transparent" -> hover: "opaque" -> active: "typing"`) и глобальные хоткеи (`Shift+Backquote` для разблокировки/блокировки оверлея, `Ctrl+Shift+M`, `Ctrl+Shift+D`, `CapsLock` Push-to-Talk) через портал `org.freedesktop.portal.GlobalShortcuts` и контракт `discord_utils.node` (`inputEventRegister`, `inputEventUnregister`, `inputWatchAll`, запрет одиночных символьных скан-кодов Linux `[24..33]`, `[38..46]`, `[52..58]`, исключение левой кнопки мыши `1`, лимит `4` событий и таймаут `5000` мс).
   - Реализован виджет отображения говорящих участников в верхнем левом углу (`anchor: "top-left"`, `x = 16`, `y = 16`, `padding = 16px`, `spacing = 8px`) с приоритетной сортировкой говорящих (`Voice OP 5 SPEAKING` биты `MICROPHONE = 1` и `SOUNDSHARE = 2`) и зелёным кольцом активности `#23a55a`.
   - Выполнена сверка с независимыми оракулами: [`waves/kanban-board.json`](file:///home/user/orca/project/waves/kanban-board.json) (`TASK-988`), [`docs/re/main-arch.md`](file:///home/user/orca/project/docs/re/main-arch.md) (`--overlay-host`, `discord_overlay2/standalone_host.js`, `outOfProcessOverlay`), [`docs/re/backend-ipc.json`](file:///home/user/orca/project/docs/re/backend-ipc.json) (`DISCORD_WINDOW_IS_ALWAYS_ON_TOP`, `DISCORD_WINDOW_SET_ALWAYS_ON_TOP`, `DISCORD_GET_MOUSE_COORDINATES`) и [`docs/re/native-re.md`](file:///home/user/orca/project/docs/re/native-re.md) (§5 `discord_utils.node`).

2. **Подзадача 988.2 — Контрактный и модульный тест [`tests/unit/ingame-overlay-988.test.mjs`](file:///home/user/orca/project/tests/unit/ingame-overlay-988.test.mjs) (Strict TDD):**
   - Покрыты все 6 подсекций (`SUB-988-0`..`SUB-988-5`), все 9 измерений `TEST-POLICY.md` (`evaluateOverlayPolicyDimensions`), маскирование секретов (`redactOverlayTelemetry`), лимиты размера кода (`< 50` строк на функцию, `< 800` строк на модуль, `#![forbid(unsafe_code)]`, `0` `.unwrap()`/`.expect()` в prod-коде) и мутационный гейт `5/5` (`100%` kill rate) для Rust и JS модулей.

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-988-ingame-overlay/red.log`](file:///home/user/orca/project/waves/RW-45/FRONTEND/task-988-ingame-overlay/red.log) (запуск до создания `ingame_overlay.rs` и `ingame-overlay.js`):

```text
▶ TASK-988: Глобальные хоткеи и Wayland In-Game Overlay (ingame_overlay.rs + ingame-overlay.js)
  ✖ SUB-988-0: модули rust/discord-gpui/src/ingame_overlay.rs и rust/gui-spike/ui/features/ingame-overlay.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений (0.770805ms)
  ✖ SUB-988-1: сверяет дизайн-токены, состояния overlay-chat-bubble, протокол ext-foreign-toplevel-list-v1 и IPC/NAPI контракты с независимыми оракулами репозитория (0.252232ms)
  ✖ SUB-988-2: обеспечивает прозрачный клик-сквозь (input passthrough) в режиме HUD, переходы overlay-chat-bubble и глобальные хоткеи с защитой скан-кодов Linux (0.239473ms)
  ✖ SUB-988-3: отображает говорящих участников в верхнем левом углу (16px padding, 8px spacing) и отслеживает полноэкранные игры через ext-foreign-toplevel-list-v1 (0.215836ms)
  ✖ SUB-988-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake/2000 символов/25 МБ/offset=9975/0..200%/99+, ошибки 401..5xx, права, fallback и маскирование секретов) (0.150763ms)
  ✖ SUB-988-5: проходит мутационный гейт 5/5 (100% kill rate) для Rust и JS реализаций по контракту scripts/quality-gate.mjs (0.132346ms)
ℹ tests 6 | suites 1 | pass 0 | fail 6
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-988-ingame-overlay/green.log`](file:///home/user/orca/project/waves/RW-45/FRONTEND/task-988-ingame-overlay/green.log):

```text
▶ TASK-988: Глобальные хоткеи и Wayland In-Game Overlay (ingame_overlay.rs + ingame-overlay.js)
  ✔ SUB-988-0: модули rust/discord-gpui/src/ingame_overlay.rs и rust/gui-spike/ui/features/ingame-overlay.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений (122.862701ms)
  ✔ SUB-988-1: сверяет дизайн-токены, состояния overlay-chat-bubble, протокол ext-foreign-toplevel-list-v1 и IPC/NAPI контракты с независимыми оракулами репозитория (7.10895ms)
  ✔ SUB-988-2: обеспечивает прозрачный клик-сквозь (input passthrough) в режиме HUD, переходы overlay-chat-bubble и глобальные хоткеи с защитой скан-кодов Linux (1.291047ms)
  ✔ SUB-988-3: отображает говорящих участников в верхнем левом углу (16px padding, 8px spacing) и отслеживает полноэкранные игры через ext-foreign-toplevel-list-v1 (5.811443ms)
  ✔ SUB-988-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake/2000 символов/25 МБ/offset=9975/0..200%/99+, ошибки 401..5xx, права, fallback и маскирование секретов) (1.284988ms)
  ✔ SUB-988-5: проходит мутационный гейт 5/5 (100% kill rate) для Rust и JS реализаций по контракту scripts/quality-gate.mjs (560.021407ms)
✔ TASK-988: Глобальные хоткеи и Wayland In-Game Overlay (ingame_overlay.rs + ingame-overlay.js) (701.048605ms)
ℹ tests 6 | suites 1 | pass 6 | fail 0 | skipped 0
```

---

## 4. Targeted & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/ingame-overlay-988.test.mjs`):** `6/6 pass (0 fail, 0 skip)`.
- **Rust (`rustfmt --check --edition 2024` & `rustc --test --edition 2024 -D warnings`):** `0 errors, 0 warnings`.
- **TypeScript (`tsc`):** `0 errors` в `rust/gui-spike/ui/features/ingame-overlay.js` и `tests/unit/ingame-overlay-988.test.mjs`.
- **Мутационный гейт ([`waves/RW-45/FRONTEND/task-988-ingame-overlay/mutation.log`](file:///home/user/orca/project/waves/RW-45/FRONTEND/task-988-ingame-overlay/mutation.log)):**
  1. `M1` (`Hud inputPassthrough: true -> false`) — **KILLED** (Rust & JS)
  2. `M2` (`OVERLAY_PADDING_PX: 16 -> 64`) — **KILLED** (Rust & JS)
  3. `M3` (`ChatBubble Hover: "opaque" -> "semi-transparent"`) — **KILLED** (Rust & JS)
  4. `M4` (`Linux forbidden single scancode guard [24..33, 38..46, 52..58] -> false`) — **KILLED** (Rust & JS)
  5. `M5` (`ext_foreign_toplevel_handle_v1.done commit skipped`) — **KILLED** (Rust & JS)
  - Итог: **5/5 мутаций убито в Rust и 5/5 в JS (100% kill rate)**, оригиналы файлов побайтно проверены по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `rust/discord-gpui/src/ingame_overlay.rs` | `a0e3e02fdb5abdf96f5035d0fea87d1a` |
| `rust/gui-spike/ui/features/ingame-overlay.js` | `a1bf2c235d05174ee19a7a062721e8e1` |
| `tests/unit/ingame-overlay-988.test.mjs` | `eb8afc16f64d87deb7482b0ca7ba7467` |
| `waves/RW-45/FRONTEND/task-988-ingame-overlay/red.log` | `555204fd7056dd2257dab1bb9a0b4897` |
| `waves/RW-45/FRONTEND/task-988-ingame-overlay/green.log` | `a339018d89f5515c056a2d856e36c8dc` |
| `waves/RW-45/FRONTEND/task-988-ingame-overlay/mutation.log` | `720ab1d7f1ee6534772f9ca2600c9697` |

---

## 6. Риски и замечания

- В сессиях X11 или при перезапуске композитора Wayland контроллер автоматически переключается на X11-оверлей (`OverrideRedirect` + `XShapeCombineRegion` с пустым `ShapeInput`), сохраняя прозрачный клик-сквозь (`input passthrough`) и отображение говорящих в верхнем левом углу.
- Блокеров нет (`P0 = 0`, `P1 = 0`).
