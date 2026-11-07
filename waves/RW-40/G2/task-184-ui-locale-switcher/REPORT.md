# Отчёт task-184: UI locale switcher (G2)

Статус: green. Ветка `feature/task-184-ui-locale-switcher`, коммит не делался.

## Файлы
| Файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/features/locale-switcher.js` | `bf5bb0850eaf834e938ae071d1f360a1` |
| `tests/unit/locale-switcher.test.mjs` | `8af66523021b9c4dc51068ac5f469854` |

## Что сделано
- `SUPPORTED_LOCALES` (frozen): `ru`/`ru-RU`/Русский, `en-US`/`en-US`/English, US.
- `LOCALE_STORAGE_KEY = "discord.locale"`.
- `createLocaleSwitcher(doc, { storage, defaultLocale = "ru" })`: чтение из storage с валидацией и откатом на `ru`;
  синхронизация `lang` + `data-locale` на `documentElement`; `getLocale`, `setLocale` (RangeError на неизвестный код,
  персист в storage, уведомление `onChange`), `renderSelect` (`data-role="locale-select"`,
  `aria-label="Язык интерфейса"`, опции из реестра, `change` → `setLocale`).
- Ошибка одного подписчика `onChange` и некорректное значение из DOM не роняют переключатель.

## Прогоны
- red: `red.log` — падение до реализации (модуль отсутствовал).
- green: `green.log` — 14 тестов, pass 14, fail 0, skip 0.

## Ограничения
Полный `gate:worker` не запускался по указанию TOKEN-LEAN; targeted-тест зелёный, чужие файлы не тронуты.
