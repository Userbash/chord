# REPORT — TASK-985: Пользовательские темы оформления и динамическая CSS палитра (`custom_themes.rs` + `custom-themes.js`)

- **Волна / Карточка:** Wave RW-45 / `TASK-985` (`task_64c8e7f99889`, dispatch `ctx_fc69d8f4fb69`)
- **Отдел:** `G2 Frontend`
- **Приоритет:** `P2`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 985.1 — Нативный модуль пользовательских тем и палитры [`rust/discord-gpui/src/custom_themes.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/custom_themes.rs) + WebView/DOM контроллер [`rust/gui-spike/ui/features/custom-themes.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/custom-themes.js):**
   - Реализована поддержка 4 встроенных тем Discord (`dark`, `light`, `midnight`, `amoled` с чистым чёрным `#000000` для OLED-экранов) и реестра пользовательских тем (до `MAX_INSTALLED_CUSTOM_THEMES = 8`, включая импорт пакетов `.serein-extension` v1).
   - Реализовано горячее переключение тем (`applyTheme` / `apply_theme`) без перезагрузки интерфейса (`reloaded: false`, `domReloadCount === 0`, задержка `< 8.33 мс` в бюджете 120 FPS) через динамическую инъекцию CSS custom properties в `:root` (`document.documentElement.style` и `<style id="discord-custom-theme-palette">`), переходы интерактивных состояний `theme-preview` (`dark -> highlight theme -> applied`) и аварийный сброс по хоткею `Ctrl+Shift+F12`.
   - Реализован строгий валидатор контрастности WCAG 2.2 AA/AAA (`validateThemeWcagContrast` / `audit_palette_wcag_aa`: `>= 4.5:1` для обычного текста, `>= 7.0:1` AAA для основного текста `--text-strong`/`--text`, `>= 3.0:1` для нетекстовых индикаторов и `--focus-ring`) и санитайзер CSS-токенов (`sanitizeCustomThemeTokens` / `sanitize_css_token_entry`), блокирующий `url()`, `@import`, `@font-face`, `expression()`, `javascript:`, `!important`.
   - Выполнена сверка с независимыми оракулами (`verifyCustomThemesOracleParity`): [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json) (`TASK-985`), [`rust/gui-spike/ui/tokens.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/tokens.css) (204 базовых токена `:root`), [`waves/RW-40/theme-spec.json`](file:///home/sanya/orca/project/waves/RW-40/theme-spec.json) (57 переопределений `light`), [`rust/gui-spike/ui/features/token-map.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/token-map.js) (25 канонических токенов и 7 разрешённых коллизий), [`docs/re/frontend-d03.json`](file:///home/sanya/orca/project/docs/re/frontend-d03.json) (`theme-dark images-dark`, `customTheme`) и [`midnight.serein-extension`](file:///home/sanya/orca/project/docs/reference-projects/sources/Serein/extensions/themes/midnight.serein-extension).

2. **Подзадача 985.2 — Контрактный и модульный тест [`tests/unit/custom-themes-985.test.mjs`](file:///home/sanya/orca/project/tests/unit/custom-themes-985.test.mjs) (Strict TDD):**
   - Покрыты все 6 тестовых секций (`SUB-985-0`..`SUB-985-5`), все 9 измерений `TEST-POLICY.md` (`evaluateCustomThemesEdgeDimensions`), маскирование секретов (`redactThemeTelemetryLogs`), лимиты размера кода (`< 800` строк на модуль, `< 50` строк на функцию) и мутационный гейт `5/5` (`100%` kill rate) как для Rust, так и для JS.

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-985-custom-themes/red.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-985-custom-themes/red.log):

```text
▶ TASK-985: Пользовательские темы оформления и динамическая CSS палитра (custom_themes.rs + custom-themes.js)
  ✖ SUB-985-0: модули custom_themes.rs и custom-themes.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), rustfmt 2024 и компилируются без предупреждений (1.141196ms)
  ✖ SUB-985-1: сверяет дизайн-токены, макет Theme Picker, палитры (Midnight, Dark, Light, AMOLED) и интерактивные состояния с независимыми оракулами (0.371079ms)
  ✖ SUB-985-2: переключает темы (Dark, Light, Midnight, AMOLED и пользовательские) на лету без перезагрузки интерфейса и управляет состояниями theme-preview (0.278439ms)
  ✖ SUB-985-3: валидирует CSS-токены на WCAG 2.2 AA контрастность (>= 4.5:1 текст, >= 3.0:1 UI/focus) и блокирует CSS-инъекции (0.268989ms)
  ✖ SUB-985-4: покрывает все 9 измерений TEST-POLICY.md (граничные 4.49 vs 4.50, лимит 8 тем, Snowflake, 2000 символов, ошибки 401..5xx, skeleton/202, права, fallback и маскирование) (0.271509ms)
  ✖ SUB-985-5: убивает 5/5 контрольных мутаций в custom_themes.rs и custom-themes.js (100% mutation kill rate) (0.275569ms)
ℹ tests 6 | suites 1 | pass 0 | fail 6
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/FRONTEND/task-985-custom-themes/green.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-985-custom-themes/green.log):

```text
▶ TASK-985: Пользовательские темы оформления и динамическая CSS палитра (custom_themes.rs + custom-themes.js)
  ✔ SUB-985-0: модули custom_themes.rs и custom-themes.js существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), rustfmt 2024 и компилируются без предупреждений (122.576998ms)
  ✔ SUB-985-1: сверяет дизайн-токены, макет Theme Picker, палитры (Midnight, Dark, Light, AMOLED) и интерактивные состояния с независимыми оракулами (6.299747ms)
  ✔ SUB-985-2: переключает темы (Dark, Light, Midnight, AMOLED и пользовательские) на лету без перезагрузки интерфейса и управляет состояниями theme-preview (3.11623ms)
  ✔ SUB-985-3: валидирует CSS-токены на WCAG 2.2 AA контрастность (>= 4.5:1 текст, >= 3.0:1 UI/focus) и блокирует CSS-инъекции (1.514563ms)
  ✔ SUB-985-4: покрывает все 9 измерений TEST-POLICY.md (граничные 4.49 vs 4.50, лимит 8 тем, Snowflake, 2000 символов, ошибки 401..5xx, skeleton/202, права, fallback и маскирование) (2.384457ms)
  ✔ SUB-985-5: убивает 5/5 контрольных мутаций в custom_themes.rs и custom-themes.js (100% mutation kill rate) (544.169463ms)
✔ TASK-985: Пользовательские темы оформления и динамическая CSS палитра (custom_themes.rs + custom-themes.js) (682.653271ms)
ℹ tests 6 | suites 1 | pass 6 | fail 0 | skipped 0
```

---

## 4. Targeted & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/custom-themes-985.test.mjs`):** `6/6 pass (0 fail, 0 skip)`.
- **Rust (`rustc --test --edition 2024 -D warnings` + `rustfmt --check --edition 2024`):** `4/4 pass`, `0 warnings`.
- **Мутационный гейт ([`waves/RW-45/FRONTEND/task-985-custom-themes/mutation.log`](file:///home/sanya/orca/project/waves/RW-45/FRONTEND/task-985-custom-themes/mutation.log)):**
  1. `MUT-985-RS-01` / `JS-01` (Ослабление порога WCAG 2.2 AA `4.5 -> 1.5`) — **KILLED**
  2. `MUT-985-RS-02` / `JS-04` (Принудительная перезагрузка `reloaded: false -> true`) — **KILLED**
  3. `MUT-985-RS-03` / `JS-03` (Подмена истинного чёрного `#000000` в теме AMOLED) — **KILLED**
  4. `MUT-985-RS-04` / `JS-02` (Обход санитайзера CSS-инъекций `url()`) — **KILLED**
  5. `MUT-985-RS-05` / `JS-05` (Обход лимита `MAX_INSTALLED_CUSTOM_THEMES = 8`) — **KILLED**
  - Итог: **5/5 мутаций убито (100% kill rate)**.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `rust/discord-gpui/src/custom_themes.rs` | `b2507caa8e889f295a04eb13d670334d` |
| `rust/gui-spike/ui/features/custom-themes.js` | `bd93e2c70b726c557cd89eb6f9aa4a5f` |
| `tests/unit/custom-themes-985.test.mjs` | `75e6e277c074cc9b5aed97bec3dd7169` |
| `waves/RW-45/FRONTEND/task-985-custom-themes/red.log` | `0d20f8bb8016a0916741e8ce9f19c56a` |
| `waves/RW-45/FRONTEND/task-985-custom-themes/green.log` | `e6ba648f74228bcb6a5faeab3f491a7d` |
| `waves/RW-45/FRONTEND/task-985-custom-themes/mutation.log` | `818b5b8894b2ff0d4e095021bfbe745d` |

---

## 6. Риски и замечания

- Все 4 встроенные темы (`Dark`, `Light`, `Midnight`, `AMOLED`) и импортируемые пакеты `.serein-extension` проходят строгий аудит контрастности WCAG 2.2 AA/AAA и применяются на лету без перезагрузки интерфейса.
- Блокеров нет (`P0 = 0`, `P1 = 0`).
