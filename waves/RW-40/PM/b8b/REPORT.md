# B8b — гидратация/сохранение настроек в UI: отчёт G2

Дата: 2026-10-24. Воркер: G2 Frontend. PM: DeepSeek.
Карточка: B8b (`waves/RW-40/TASKS.md`). Оракулы: `src/settings_store.rs`
(`UiSettings`: ровно 5 полей, `version=1`, белые списки cozy/compact ·
dark/light · ru/en), `ui/index.html` (`lang="ru" data-density="cozy"`),
`ui/features/settings.js` (38 тумблеров SET-06/07/08).

## 1. Статус

**done (frontend, offline).** Гидратация `settings_load`, debounce-сохранение
`settings_save` (200 мс), видимая ошибка записи, стабильные ASCII-id тумблеров и
миграция старого формата по индексу. Окно/сеть/cargo/gate не запускались;
`src/**`, `fixtures/**` и остальной `ui/**` не менялись.

## 2. Сделано

| Пункт ТЗ | Где | Проверка (тест) |
|---|---|---|
| 1. На старте `settings_load`: 38 тумблеров по id, `data-density`, `data-theme`, `data-locale` (+`html[lang]`); cozy/dark/ru при ошибке/пусто | `settings.js`: `loadSettings`, `applySettings`, `normalizeSettings`; вызов в `initSettingsConsole` | `settings_load на старте…`, `битые/чужие значения → дефолты…`, `settings_load упал → дефолты…` |
| 2. `settings_save` на изменение тумблера/density/theme, debounce 200 мс, один вызов; ошибка видима, состояние не откатывается | `settings.js`: `scheduleSave`, `persist`, `showStatus/.settings-save-status[role=status]`; `app.js` передаёт `invoke` | `change: один settings_save…`, `debounce: несколько изменений…`, `ошибка settings_save видна…` |
| 3. Стабильные ASCII-id (`data-setting-id`), миграция старых без id по индексу, без падения | `TOGGLE_SPECS[].id`, `SETTINGS_TOGGLE_IDS`, `resolveToggleId`, `toggleKey` | `id тумблеров стабильные ASCII…` |
| 4. Только разрешённые значения, без секретов | `normalizeSettings` (boolean-тумблеры, белые списки), `collectSettings` (ровно 38 известных id + version/density/theme/locale) | `change: … только белый список и без секретов`, `id тумблеров…` |

Контракт id: `voice.*`, `input.*`, `noise.*`, `video.*`, `stream.*`,
`sounds.*`, `advanced.*` — ASCII `[a-z0-9_.]`, уникальны, порядок = порядок DOM.
Старые ключи `"5"`, `"toggle_5"` и прежние русские подписи мапятся на id по
индексу/подписи; неизвестные ключи и не-boolean значения отбрасываются.

Ошибка чтения файла не глотается: показывается строка «Настройки не прочитаны,
используются значения по умолчанию: …». Ошибка записи — «Не удалось сохранить
настройки: … Изменения применены, но не сохранены.», UI не откатывается.

## 3. Red → green

RED (`waves/RW-40/PM/b8b/red.log`, exit 1): **59 passed / 7 failed** —
падают ровно новые тесты B8b. Лог снят на состоянии без B8b-веток после
написания тестов (временный откат `loadSettings`/`scheduleSave`/
`data-setting-id`; файлы восстановлены, md5 сверены).

```
✖ settings_load на старте: тумблеры по id, density/theme/locale и lang
✖ change: один settings_save после debounce, только белый список и без секретов
✖ битые/чужие значения → дефолты cozy/dark/ru, без падения и без save
✖ settings_load упал → дефолты и отсутствие молчаливого падения
✖ debounce: несколько изменений (тумблер+density+theme) → один save
✖ ошибка settings_save видна, состояние не откатывается
✖ id тумблеров стабильные ASCII; старый формат по индексу мигрируется
```

GREEN (`waves/RW-40/PM/b8b/green.log`, exit 0): **212 passed / 0 failed**
(база 205 + 7 новых, ничего не ослаблено).

## 4. Проверки

| Проверка | Результат | Лог |
|---|---|---|
| `node --test gui-spike-{dom,model,bench,features,auth}.test.mjs` | exit 0 · **212 passed / 0 failed** | `green.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0 (0 ошибок) | `tsc.log` |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | exit 0 · **31/31 в допуске** | `geometry.log` |

Cargo/gate/window не запускались по запрету ТЗ; backend-набор B8a (149) не
затрагивался — `src/**` только читался.

## 5. Файлы и md5

```
add59870a122918cc55435781b7e283f  rust/gui-spike/ui/app.js
bcf21377457da20abf55cf9f8a17f6d4  rust/gui-spike/ui/features/settings.js
e3972343fbe01138130f600c9930a9cd  rust/gui-spike/ui/components/settings.css
2fa3da372bd9397e2729fc05abe33cf1  rust/gui-spike/ui/index.html   (не изменён)
12c6a1993fd5fe3c7ed1d4a36faaa854  tests/unit/gui-spike-features.test.mjs
12ea7059e2c4de7c64a0daa40711094a  tests/support/gui-dom-harness.mjs
```

`ui/index.html` — только чтение (значения по умолчанию уже отгружены: `lang=ru`,
`data-density=cozy`); `data-theme`/`data-locale` ставятся из `settings_load`.
Harness пополнен аддитивно: дефолтные ответы `settings_load`/`settings_save`.
CSS: стили темы и строки статуса, геометрические токены не менялись.

## 6. Остаток / открытые вопросы

1. **Гонка старта**: изменение настроек до ответа `settings_load` будет
   перезаписано ответом загрузки. На практике диалог настроек открывается
   позже; если PM захочет строгости — нужен флаг «пользователь уже менял» или
   блокировка контролов до загрузки.
2. **Тема — только атрибут**: `data-theme`/`data-locale` применяются, но
   светлая тема и английские строки в CSS/i18n ещё не реализованы (C3/C4).
   UI честно не обещает большего; сохранение уже готово.
3. **Выбор языка в UI отсутствует** (C4): locale читается/сохраняется, но
   изменить его в спайке нечем.
4. **`.local/` в `.gitignore`** — по-прежнему владелец Team Lead (из B8a);
   первый реальный save создаст файл только при запуске окна.
5. Секреты не сохраняются по конструкции (`collectSettings` собирает только 38
   известных boolean-ключей); токены keyring в UI-настройки не попадают.
