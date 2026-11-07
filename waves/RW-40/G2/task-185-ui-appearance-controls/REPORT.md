# T-185 — контроллер настроек внешнего вида (G2)

Статус: green, 15/15 тестов. Ветка: feature/task-185-ui-appearance-controls, коммитов нет.

## Файлы и md5
- rust/gui-spike/ui/features/appearance-settings.js — `da96a0ee035e62d771dc741c02f2ce9c`
- tests/unit/appearance-settings.test.mjs — `57976b628ea0709f306af05b94e89f88`

## Red
`red.log`: падение на импорте отсутствующего модуля (ERR_MODULE_NOT_FOUND), до реализации.

## Green
`green.log`: tests 15, pass 15, fail 0, skipped 0.

## Контракт
- `FONT_SCALE_STEPS` [87.5, 93.75, 100, 106.25, 112.5, 125, 150], `GROUP_SPACING_STEPS` [0, 4, 8, 16, 24], `THEMES` [dark, light, ash, onyx], `APPEARANCE_STORAGE_KEY` = "discord.appearance" — все `Object.freeze`.
- `createAppearanceController(doc, { storage })`: гидратация JSON по полям, применение к `documentElement` (`data-theme`, `data-reduced-motion`, `--chat-font-scale` %, `--chat-group-spacing` px).
- Сеттеры возвращают boolean; невалидное значение не меняет state, DOM и storage.
- `getState()` отдаёт копию; `resetDefaults()` возвращает дефолты и перезаписывает storage.
- Сбои storage (getItem/setItem) не ломают старт и не отменяют применение к DOM.

## Риски
- Полный `gate:worker`, `tsc` и чужие тесты не запускались — по token-lean ТЗ.
- Привязка контроллера к реальным контролам UI (слайдер/селект) вне scope карточки.
