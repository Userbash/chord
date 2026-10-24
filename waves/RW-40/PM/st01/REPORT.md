# ST-01 — навигация настроек (RW-40, воркер G2 Frontend)

Дата: 2026-10-24. PM: DeepSeek. Режим: офлайн (`node:test` + DOM-шим), без live/окна/сети/
`cargo`/`gate`/commit. Источник истины по структуре — `G6-REPORT-RW40-02.md` §3.11/§4 и
оракул `/home/sanya/re-deep/ui8/23-settings-categories.json` (63 записи) + HTML-дамп
`/home/sanya/re-deep/ui8/23-settings-voice.html` (для профиля/класса `destructive`).

## Сделано

1. **Профиль в навигации** (`#settings-nav .settings-profile`): аватар-монограмма `SB`
   (переиспользован примитив `.avatar.avatar--lg`), имя `ShadowByte`, действие
   `Редактировать профили` (точный текст дампа; в G6 §3.11 записано «Редактировать
   профиль» — расхождение в пользу дампа). Действие `disabled` с причиной
   (`title` + `aria-describedby`), т.к. панели редактирования в RE нет.
2. **4 подпункта «Учётной записи»** строго из развёрнутого дампа: «Информация об учётной
   записи», «Пароль и безопасность», «Статус учётной записи», «Семейный центр» —
   `#settings-subnav-account` сразу под родителем, `aria-expanded="true"` +
   `aria-controls` у родителя. Панелей подпунктов в дампе нет → `disabled` с причиной.
   17 остальных разделов не развёрнуты — их подпункты **не** выдумывались (ровно 4).
3. **«Выйти»** (`.settings-nav-logout`, destructive): при наличии IPC — активна,
   `invoke("auth_logout")` **без аргументов/токена**; состояние в `data-logout-state`
   (`idle`→`pending`→`done`/`error`), результат и ошибка видны в существующем
   `role=status` (`.settings-save-status`), без `console.error`. Без IPC — `disabled`
   с причиной.
4. **Навигация**: `aria-current="page"` ровно у одного раздела; сравнение по
   `data-section`, а не по тексту; стрелки ↑/↓, Home/End с зацикливанием только по
   доступным пунктам (3 раздела + «Выйти»), disabled-разделы и подпункты фокус не
   получают; причины disabled — `title` + скрытый текст по `aria-describedby`.
5. **Не изменено**: 18 разделов/`.settings-nav-item`, 38 тумблеров, 3 панели,
   `SETTINGS_ENABLED`, контракт тумблера, `tokens.css` (геометрия).
6. `ui/index.html`: шапка настроек больше не дублирует профиль (аватар+имя+подсказка
   убраны, осталась кнопка закрытия; `.settings-head` — `justify-content: flex-end`).
   Профиль теперь один и в навигации, как в оракуле. DOM-шим тестов (`tests/support/**`,
   read-only) по-прежнему строит старую шапку — на поведение/проверки не влияет.

## Red-прогон (до кода)

`node --test tests/unit/gui-spike-features.test.mjs` → 100 тестов, 96 pass, **4 fail**
(новые ST-01), лог: `red.log`.

```
✖ настройки ST-01: профиль и 4 развёрнутых подпункта «Учётной записи» из оракула
✖ настройки ST-01: «Выйти» → auth_logout без токена, результат видим
✖ настройки ST-01: ошибка logout видима, кнопка снова доступна
✖ настройки ST-01: клавиатура, aria-current и причина disabled-раздела
```

Причины падений: нет `.settings-profile`; `.settings-nav-logout` disabled; `auth_logout`
не вызывается; у disabled-разделов нет `aria-describedby`.

## Green-прогон

`green.log`: 8 файлов проверки → **309 тестов, 309 pass, 0 fail** (было 305, +4 новых
ST-01; существующие 305 не менялись и не ослаблялись).

```
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs \
  tests/unit/gui-spike-auth.test.mjs tests/unit/gui-spike-regressions.test.mjs \
  tests/unit/guild-rail.test.mjs tests/unit/voice-ui.test.mjs
→ tests 309 / pass 309 / fail 0
```

- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0, без вывода.
- `node waves/RW-39/PM/w39c/geometry-check.mjs` → «все 31 геометрических значений в допуске».

## Новые тесты (аддитивно, `tests/unit/gui-spike-features.test.mjs`)

| Тест | Что фиксирует |
|---|---|
| профиль и 4 подпункта | имя/монограмма/подпись действия, `aria-controls`/`aria-expanded`, ровно 4 подпункта в порядке дампа, disabled+причина у подпунктов |
| «Выйти» → `auth_logout` | кнопка активна при IPC, ровно один вызов, `args === null` (без токена), `data-logout-state="done"`, видимый статус |
| ошибка logout | `data-logout-state="error"`, видимый текст ошибки IPC, кнопка снова доступна |
| клавиатура/aria-current | один `aria-current="page"`, причина в `title`+`aria-describedby`, ↑/↓/Home/End и зацикливание, фокус не попадает на disabled, активация переключает панель |

Оракул в тестах задан константами `ORACLE_PROFILE`/`ORACLE_ACCOUNT_ITEMS` (значения
извлечены из `23-settings-categories.json`/HTML-дампа, не из `settings.js`); отдельная
TSV-фикстура не создавалась, чтобы не расширять файловую матрицу воркера.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/settings.js` | `5b4e3b926549a49fb33eded9d4e40ba5` |
| `rust/gui-spike/ui/components/settings.css` | `65afbe7089ef268f14c05127d547f679` |
| `rust/gui-spike/ui/index.html` | `cb06c0e21ab5a64396a135f1260542f4` |
| `tests/unit/gui-spike-features.test.mjs` | `b311d133cb0babafdf5825860c68e53b` |
| `waves/RW-40/PM/st01/red.log` | см. файл |
| `waves/RW-40/PM/st01/green.log` | см. файл |

`src/**` и остальной UI — read-only, не менялись.

## Остаток / риски

1. **Переход в `no-session` после «Выйти» не подключён к auth-гейту**: `app.js`/`auth.js`
   вне файловой матрицы ST-01. Сейчас настройки вызывают `auth_logout` и показывают
   результат, но экран входа отрисует только гейт. Нужна отдельная карточка (1–2 строки):
   `initSettingsConsole({ invoke, onLogout })` → `auth.logout()`; порядок создания в
   `app.js` (settings раньше auth) требует колбэка/события — решение за PM.
2. **Оракул — только renderer 1.0.162**, renderer 1.0.166 не снимался: профиль без
   handle, подпункты остальных 17 разделов неизвестны (`[A]`), панели 4 подпунктов
   отсутствуют — поэтому они `disabled`, а не «пустые экраны».
3. `aria-expanded="true"`/`aria-controls` стоят на `disabled`-родителе (раздел без
   панели, но с раскрытым списком из дампа) — осознанное отступление от «disabled не
   раскрывается»; при появлении панели «Учётной записи» родитель станет активным.
4. **Дубль профиля в шапке**: `ui/index.html` почищен, но read-only DOM-шим
   (`tests/support/gui-dom-harness.mjs`) всё ещё создаёт старую шапку — при следующей
   правке фикстуры шапку там стоит синхронизировать.
5. Текст действия профиля: дамп — «Редактировать профили», G6 §3.11 — «Редактировать
   профиль». Взято из дампа; при необходимости PM правит формулировку в матрице.
