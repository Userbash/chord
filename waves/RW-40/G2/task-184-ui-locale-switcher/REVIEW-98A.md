# Ревью task-184: UI locale switcher (G2)

Вердикт: **APPROVED** — P0=0, P1=0, мутации 5/5 убиты.

## Проверено
- `rust/gui-spike/ui/features/locale-switcher.js` (135 строк): реестр локалей frozen, ключ `discord.locale`,
  чтение из storage только после `isSupported`, `lang`/`data-locale` на `documentElement`,
  `setLocale` с `RangeError` до записи, отписка через `splice`, `renderSelect` с `data-role`/`aria-label` и стабильным `selectEl`.
- Код совпадает с описанием в `REPORT.md`; чужих правок в модуле нет.

## Прогоны
- `node --test tests/unit/locale-switcher.test.mjs`: tests 14, pass 14, fail 0, skip 0.
- Мутации (каждая ломает тест, файл восстановлен):
  1. `option.selected = false` — fail 1.
  2. удалена валидация `localeByCode(code)` — fail 1.
  3. удалён `data-locale` — fail 3.
  4. отписка не удаляет слушателя — fail 1.
  5. отключён persist в storage — fail 2.
- После восстановления md5 = `bf5bb0850eaf834e938ae071d1f360a1` (совпадает с REPORT).

## Ограничения
Полный `gate:worker`/`tsc` не запускались (TOKEN-LEAN). Коммит не делался.
