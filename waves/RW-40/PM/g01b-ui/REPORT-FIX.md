# REPORT-FIX — G-01b-UI-SETTINGS-FIX (отдел G2 Frontend Lane A)

- **Дата:** 2026-10-30.
- **Роль:** Воркер G2 Frontend Lane A (RW-40, карточка G-01b-UI-SETTINGS-FIX).
- **Основание:** `waves/RW-40/PM/g01b-ui/REVIEW.md` (дефект P2).
- **Статус:** red → green, targeted-прогон и `tsc` зелёные. Коммитов нет.

---

## 1. Проблема (P2 из REVIEW.md)

В `waves/RW-40/PM/g01b-ui/REVIEW.md` зафиксирован P2:
при выборе гильдии в сервером рейле `app.js` корректно сохранял `lastGuildId` в `UiSettings`. Однако последующее переключение любого тумблера/настройки в `settings.js` вызывало `collectSettings()` → `normalizeSettings()`, которые отбрасывали `lastGuildId` как неизвестный ключ и отправляли `settings_save` без него. В результате при следующем перезапуске приложения (`boot`) сохранённый сервер сбрасывался на первый сервер из списка `READY`.

---

## 2. Strict TDD (Red-first)

Добавлен целевой тест в `tests/unit/guild-rail.test.mjs`:
- **Тест:** `выбор гильдии -> переключение настройки -> settings_save сохраняет lastGuildId -> повторный boot выбирает её`.
- **Сценарий:**
  1. Boot приложения с исходными настройками.
  2. Выбор гильдии в рейле (`tile.click()`) → сохранение `lastGuildId`.
  3. Переключение тумблера настройки (`voice.qos_priority`) → срабатывание debounce и отправка `settings_save`.
  4. Проверка, что `settings_save` сохранил `lastGuildId` целевой гильдии и не затёр его.
  5. Повторный запуск (`bootInstance()`) с сохранённым состоянием настроек → проверка, что восстановилась выбранная пользователем гильдия (`selectedId === target`, `aria-current="true"`).

### Red-прогон
```bash
node --test tests/unit/guild-rail.test.mjs > waves/RW-40/PM/g01b-ui/red-fix.log 2>&1
```
- **Exit code:** 1.
- **Ошибка:**
  ```text
  AssertionError [ERR_ASSERTION]: переключение тумблера стёрло lastGuildId
  + actual - expected
  + undefined
  - '338001249918255106'
  ```
- **Артефакт:** `waves/RW-40/PM/g01b-ui/red-fix.log`.

---

## 3. Реализация (Green)

1. **`rust/gui-spike/ui/features/settings.js`:**
   - `normalizeSettings(raw)`: теперь валидирует и сохраняет `lastGuildId`, если это строка-snowflake (`/^\d{1,20}$/`). При отсутствии или невалидном значении ключ не создаётся (сохраняется контракт чистых настроек без мусора и посторонних ключей, `Object.keys()` для `DEFAULTS` остаётся `["density", "locale", "theme", "toggles", "version"]`).
   - `createSettingsConsole(options)`:
     - Хранит `currentLastGuildId`.
     - Предоставляет методы `setLastGuildId(id)` и `getLastGuildId()`.
     - Поддерживает опциональный геттер `options.getLastGuildId`.
     - При вызове `loadSettings()` обновляет `currentLastGuildId` из загруженного файла.
     - `collectSettings()` формирует объект настроек, включая текущий валидный `lastGuildId` (из геттера или внутреннего состояния), предотвращая его потерю при сохранении тумблеров.
   - `initSettingsConsole(options)`: пробрасывает `options.getLastGuildId` в `createSettingsConsole`.

2. **`rust/gui-spike/ui/app.js`:**
   - При инициализации `initSettingsConsole` передан геттер `getLastGuildId: () => preferredGuildId`.
   - `rememberGuild(id)` и `onChoose(id)` синхронно обновляют `preferredGuildId = id` и вызывают `settingsRef?.setLastGuildId(id)`.
   - В точке IPC (`invoke`) для `settings_load` предотвращено затирание уже выбранного пользователем `preferredGuildId` промежуточным чтением старого файла.
   - Сохранён строгий контракт: ровно один вызов `settings_load` при старте (`boot`).

---

## 4. Green-прогон и верификация

### Targeted unit-тесты
```bash
node --test tests/unit/guild-rail.test.mjs tests/unit/gui-spike-features.test.mjs > waves/RW-40/PM/g01b-ui/green-fix.log 2>&1
```
- **Результат:** 112 тестов, pass: 112, fail: 0, exit code: 0.
- **Артефакт:** `waves/RW-40/PM/g01b-ui/green-fix.log`.

### Проверка типов TypeScript
```bash
npx tsc -p tsconfig.json
```
- **Результат:** exit code: 0, ошибок нет.

### Контрольный прогон всех unit-тестов
```bash
node --test 'tests/unit/**/*.test.mjs'
```
- **Результат:** 776 тестов, pass: 776, fail: 0, exit code: 0.

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 | Описание |
|---|---|---|
| `rust/gui-spike/ui/features/settings.js` | `040013d5212a3a0b8e50c045270144ec` | Поддержка и сохранение `lastGuildId` в `normalizeSettings` и `collectSettings` |
| `rust/gui-spike/ui/app.js` | `70bcf5e7edca4b2c788508d226fe25df` | Связка `preferredGuildId` с консолью настроек |
| `tests/unit/guild-rail.test.mjs` | `8e44df12e01f2fd8a831ec615e30a360` | Red/Green тест персистентности «выбор → toggle → boot» |
| `waves/RW-40/PM/g01b-ui/red-fix.log` | `5aef0245d5e06be07006bfc38cb7b32e` | Лог падения до исправления (red-first) |
| `waves/RW-40/PM/g01b-ui/green-fix.log` | `9985f345da2d89f36e1131723c7c3fb7` | Лог успешного targeted-прогона (112 тестов) |
| `waves/RW-40/PM/g01b-ui/REPORT-FIX.md` | — | Данный отчёт об исправлении P2 |

---

## 6. Mutation gate (REVIEW-PASS2, карточка G-01b-UI-SETTINGS-MUTATIONS)

Закрывает процессный P2 из `REVIEW-PASS2.md` — отсутствие mutation evidence для
инварианта сохранения `lastGuildId`. Прогон выполнен воркером G2 Frontend Lane C
по карточке G-01b-UI-SETTINGS-MUTATIONS: пять мутаций только в
`rust/gui-spike/ui/features/settings.js`, целевой тест —
`tests/unit/guild-rail.test.mjs`. Код и тест этим прогоном не менялись, их md5
совпадают с разделом 5.

Каждая мутация — anchor-замена с проверкой «ровно одно вхождение»; после прогона
файл восстанавливался из эталонной копии, md5 сверялся до следующей мутации.

```
node --test tests/unit/guild-rail.test.mjs
```

Baseline: 12 passed / 0 failed, exit 0.

| # | Мутант в `settings.js` | MD5 мутанта | Exit | Killed by | Вердикт |
|---|---|---|---|---|---|
| M1 | `normalizeSettings` не возвращает `lastGuildId` (старое поведение) | `d9710c44216fedf7dc922ee82dbf386d` | 1 | «выбор → toggle → settings_save → boot» | KILLED |
| M2 | `normalizeSettings` теряет точность snowflake: `String(Number(id))` | `930e188ec1764dec6cee9504b1a954e2` | 1 | «выбор → toggle → settings_save → boot» | KILLED |
| M3 | `collectSettings` не добавляет `lastGuildId` при сохранении тумблера | `325766e027a3d568ce8b14d08ea6b8bf` | 1 | «выбор → toggle → settings_save → boot» | KILLED |
| M4 | `collectSettings` пишет `last_guild_id` вместо `lastGuildId` | `e31b9d883450069749df2cec38db0fb3` | 1 | «выбор → toggle → settings_save → boot» | KILLED |
| M5 | `persist()` вырезает `lastGuildId` из payload перед `settings_save` | `7dd78df676e17351dcf067d9265ee3ac` | 1 | «выбор → toggle → settings_save → boot» | KILLED |

Итого: **5/5 KILLED**. MD5 до/после восстановления:

| Файл | MD5 (до = после) |
|---|---|
| `rust/gui-spike/ui/features/settings.js` | `040013d5212a3a0b8e50c045270144ec` |
| `tests/unit/guild-rail.test.mjs` | `8e44df12e01f2fd8a831ec615e30a360` |

Артефакт: `mutations-fix.log` (md5 `e9723bdb7c6b51412d879472eb2f9ea5`).
Контрольный прогон после последнего восстановления: 12 passed / 0 failed, exit 0.

Предыдущий смешанный прогон (M1–M2 по `settings.js`, M3–M5 по `app.js`) сохранён
без изменений как `mutations-fix-prev.log` (md5 `6ba8989e88b48467e432c9fdaa68ac8d`).

**Граница прогона:** закрыт только процессный P2 (mutation proof). P1 из
`REVIEW-PASS2.md` — гонка `A → B` при быстрых кликах (`rememberGuild` без
очереди) — этим прогоном не исправлялся: код `app.js` не менялся, red-тест на
out-of-order `settings_load`/`settings_save` не добавлялся. Для приёмки карточки
P1 нужно закрыть отдельной правкой.

---

## 7. Границы и ограничения

- Изменялись **только 3 назначенных файла**: `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/ui/app.js` и `tests/unit/guild-rail.test.mjs`.
- Чужие модули, тесты и доки не затрагивались.
- Команды `cargo` не запускались (соблюдение правил параллелизма).
- `git commit` не выполнялся (уровень L1 / координатор).
- Ослаблений тестов, `.only`, необоснованных `skip` нет.
