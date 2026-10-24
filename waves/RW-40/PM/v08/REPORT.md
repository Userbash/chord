# V-08 — голосовой UI на fake store (RW-40, G2 Frontend)

Дата: 2026-10-24 · Воркер: G2 Frontend · PM: DeepSeek
Карточка: **V-08** (`PARITY/G6-REPORT-RW40-02.md` §3.7), дизайн `DESIGN/VOICE.md`
+ `DESIGN/voice-spec.json` (schema `rw40/voice-spec-v1`).

## Что сделано

1. `#voice-panel` — host в колонке дерева (перед `#user-bar`); содержимое строит
   `ui/features/voice.js` одним код-путём для браузера и DOM-шима (как `#auth-gate`).
2. Все **12 состояний** `data-state` из спеки; в текущей сборке реально только
   `disabled` (`service: null`), остальные 11 — через fake store в тестах.
   `idle` достижим на fake store и показывает строку пользователя без блока связи.
3. Панель: join/leave/reconnect, участники (строка **32** высотой, лимит 6 + «Ещё n»),
   speaking-кольцо **2px `--positive`** по op5-флагу (`bool`/число > 0), ping
   `93 ms` в `visually-hidden` + `aria-describedby`, `data-quality` (120/250),
   mute/deafen `role=switch`, PTT-индикация `data-ptt` (idle/held/blocked) с
   текстом в `#voice-a11y` (не только цветом).
4. Без voice-сервиса вход заблокирован: `disabled` + `aria-disabled`, клик join
   не вызывает IPC и не имитирует «подключено», причина уходит в live-регион.
5. Mute/deafen панели — только из store (без оптимистичного переключения);
   `server-mute` блокирует микрофон; user-bar `aria-pressed` (actions.js) не тронут.
6. A11y: русские `aria-label` у всех иконочных кнопок, inline-SVG `aria-hidden`,
   `aria-live="polite"` (статус + действия), ошибка `role="alert"`, фокус на
   «Переподключиться» после ошибки входа, Tab-порядок ping → leave → mute → deafen.
7. Приватность: в `voice.js` нет `navigator.mediaDevices`, `getUserMedia`,
   записи/AEC/loopback, превью экрана, `fetch`/`WebSocket`/`__TAURI__`/`invoke(`.
   Новых IPC-команд и endpoint'ов нет.

## Red-first

Тест `tests/unit/voice-ui.test.mjs` написан до кода; первый прогон — падение
(`ERR_MODULE_NOT_FOUND: ui/features/voice.js`), лог: `waves/RW-40/PM/v08/red.log`.
Тесты сверяют контракт с независимыми оракулами: `DESIGN/voice-spec.json`
(12 состояний и тексты, `copy`-ключи, `dom.ids`/`data_attrs`, row_h=32,
speaking 2px/`--positive`, authored_copy), `index.html`, `tokens.css`.

## Green

| Проверка | Команда | Результат |
|-|-|-|
| voice UI | `node --test tests/unit/voice-ui.test.mjs` | **44/44 pass**, exit 0 (`green-voice.log`) |
| 261 frontend | `node --test tests/unit/gui-spike-dom.test.mjs … gui-spike-model … bench … features … auth … regressions … guild-rail` | **261/261 pass**, exit 0, без ослаблений (`green-frontend.log`) |
| tsc (UI) | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **exit 0** (`green-typecheck-geometry.log`) |
| geometry | `node waves/RW-39/PM/w39c/geometry-check.mjs` | **31/31**, exit 0 |

`gate:worker`/cargo/сеть/окно/commit не запускались — запрещены ТЗ.

## Файлы (md5)

| Файл | md5 |
|-|-|
| `rust/gui-spike/ui/index.html` | `940df0c1fc4dd6e772a05aa2a74c9bc5` |
| `rust/gui-spike/ui/app.js` | `d226da2b382e98c6a4b45b60468cd34d` |
| `rust/gui-spike/ui/features/voice.js` (new) | `29923333ddb67263278972611fad7e23` |
| `rust/gui-spike/ui/components/voice.css` (new) | `5160166f9b62d89d4f591a21dc11138f` |
| `tests/unit/voice-ui.test.mjs` (new) | `1ab81fe664930a75173ed26cfb5715fd` |
| `tests/support/gui-dom-harness.mjs` (аддитивно: host панели) | `8e26df0623a00504fc92e3ed7b01ead8` |
| `waves/RW-40/PM/v08/REPORT.md` | (этот файл) |

## Решения и отклонения (все `[A]`, не паритет)

- **D-V08-1.** В `disabled` блок связи **виден** с текстом «Голосовой канал
  недоступен в этой сборке» (из `authored_copy` спеки), а не скрыт как в
  §2.7: иначе клик по недоступному входу не даёт видимого объяснения (ТЗ п.3).
- **D-V08-2.** `data-action` не используется: `features/actions.js` — READ-ONLY
  по ТЗ, а его глобальный слушатель регистрируется до сборки панели. Кнопки
  имеют стабильные `id` и `data-voice-action`; слушатели — прямые.
- **D-V08-3.** Ширина строки участника 100% колонки (240, D-01), высота 32 `[D]`;
  268 — ширина внутри панели 363 в дампе, к нашему shell не применима.
- **D-V08-4.** Попап устройств, hot-plug, PTT-keybind и чувствительность не
  реализованы — это V-07/ST-02 (`DESIGN/VOICE.md` §3.1–3.4), в ТЗ V-08 их нет.
- **D-V08-5.** Плитка-скелетон в `loading` и пороги 120/250 — наши `[A]`,
  источник ping не установлен (VG-05/06).
- Строки `[D]` (93 ms, `daXg45`, `S69lJR`, `wM2WTM`, `+JQCa/`, `tjtv3P`,
  `NYklhr`, `bgn75v`, `MA+OZh`, `7/2/3M`, `vD60Pv`, `PygWIS`, `1zFMqU`) взяты
  из `copy`-таблицы спеки; паритет анимации/устройств/PTT не заявляется.

## Остаток

1. **Live-gate** (окно + сеть): реальный join/leave через gateway op4,
   `VOICE_SERVER_UPDATE`, RTC-сессия; до этого все состояния — fake.
2. **op5 speaking** — нужен второй участник с согласия (VG-01); сейчас только
   фейковый флаг, анимация не подтверждена.
3. **Источник ping** и пороги fine/average/bad (VG-05/06) — `[A]`.
4. **Reconnect** (3 попытки 0/2/5 с, VG-07) — не реализован: только `error` +
   «Переподключиться» как fallback.
5. **Устройства/PTT-keybind** — V-07/ST-02; в панели только индикация PTT.
6. Root `npx tsc -p tsconfig.json` имеет 5 **предсуществующих** ошибок вне моих
   файлов (`ui/app.js:227`, `ui/features/auth.js:123/182` — Timeout;
   `tests/unit/guild-rail.test.mjs:87` ×2). Требуемая проверка UI-tsconfig чистая.
