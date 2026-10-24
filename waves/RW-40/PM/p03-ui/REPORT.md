# P-03-UI — «Печатает…» + исходящий `send_typing`: отчёт G2

Дата: 2026-10-24. Воркер: G2 Frontend. PM: DeepSeek.
Карточка: P-03-UI (deferred P-03; `waves/RW-40/TASKS.md` N1-04/STUB-21/22).
Контекст: backend P-03 D-114 (`waves/RW-40/PM/p03/REPORT.md`), G6-REPORT §3.6,
core `TYPING_TTL_MS = 10_000` (`state/typing/types.rs`), C-02/M-07 реальные channelId.

## 1. Статус

**done (UI, offline/fake).** Red-first, без live/сети/окна/cargo/commit. Индикатор и
исходящий typing работают на fake-событиях/`invoke`; доставка живых `TYPING_START` —
L-02 (вне scope), интерфейс `typing.start(event)` готов для адаптера.

## 2. Требование → реализация → тест

| # | Требование | Реализация | Тест | Результат |
|---|---|---|---|---|
| 1 | Ввод в реальном канале → `send_typing({channelId})` | `composer.onType` (input/вставка эмодзи) → `app.js` проверяет `chat.mode==="channel"` → `typingSender.note(chat.channelId)` | `реальный ввод в канале — один send_typing…` | pass |
| 1 | Один на интервал `TYPING_RESEND_MS` (8–10 с, конфиг), правило `now > prev + interval` | `createTypingSender` с инъекцией часов; `TYPING_RESEND_MS = 8_000` `[A]`, `resendMs` — опция | `первый ввод… повтор — только после интервала` (граница `= interval` — стоп, `+1` — отправка) | pass |
| 1 | Guard ≤5 каналов | `TYPING_MAX_CHANNELS = 5`, отказ `reason:"guard"` при 6-м канале | `guard: не более 5 каналов…` | pass |
| 2 | Строка над композером, `aria-live=polite` | `#typing-indicator` вставлен в `#composer` (absolute над ним, раскладка 56px не меняется), `role=status`, `aria-live=polite`, `aria-atomic=true` | `один пользователь — «Печатает…»…`, `индикатор один, aria-live polite…` | pass |
| 2 | «Печатает…» / «Несколько человек печатают…» | тексты `TYPING_TEXTS`, `data-count` | то же + `двое дают множественную форму` | pass |
| 2 | TTL 10 с из core | `TYPING_TTL_MS = 10_000`, активность `at < receivedAt + TTL` (граница как в core), таймер `TYPING_SWEEP_MS` живёт только при записях | `TTL 10 с: 9 999 мс видим, 10 000 — нет` | pass |
| 2 | Не показывать себя | `self:true`, `selfUserId` (из `ready().user.id`, запоминается в `invoke`) | `себя не показывает…`, app-кейс с `ready` self id | pass |
| 2 | Не дублировать индикаторы | одна запись на `(channelId, userId)`; один узел `#typing-indicator` на приложение | `повтор пользователя не дублирует…`, app-кейс (ровно 1 узел) | pass |
| 3 | Bench не отправляет typing; `CHAT_ROWS=100_000` не ослаблен | guard по снежинке + `chat.mode`; бенч-режим не вызывает IPC | `синтетический bench typing не отправляет (CHAT_ROWS не ослаблен)` | pass |
| 4 | 401/429/slowmode — видимое сообщение, без цикла | `typingFailure()` (фиксированные тексты, 401/403 — постоянная блокировка до `reset()`, 429 — пауза `retry_after`, иначе интервал), slowmode из `messageSendCooldownMs` → «Медленный режим… N с» + пауза | `401… никакого цикла`, `429… пауза без цикла`, `slowmode-кулдаун из 200…`, app-кейсы 401/slowmode | pass |
| 4 | Не логировать токен | в args только `channelId`; тексты ошибок не переиспользуют `message`/тело; модуль не пишет в console | `токен/тело ответа не утекают…` (`Bearer super-secret…`), `consoleErrors/Warns` пусты | pass |
| 5 | 351 frontend не ослаблять; tsc/geometry | существующие тесты не менялись | 351 pass (см. §3) | pass |

## 3. Прогоны

| Проверка | Результат | Лог |
|---|---|---|
| red: `node --test tests/unit/typing-ui.test.mjs` до кода | `ERR_MODULE_NOT_FOUND` `typing-indicator.js` (0 pass / 1 fail, exit 1) | `red.log` |
| red-мутация (копия `ui` в `/tmp/opencode/p03-red`, `onType` отключён, `GUI_DOM_APP`) | **3 fail**: интеграция «ввод→send_typing», slowmode, 401; 17 pass (модульные — сдают настоящий файл) | `red-mutation.log` |
| green: `node --test tests/unit/typing-ui.test.mjs` | **20 pass**, 0 fail (exit 0) | `green.log` |
| green: 11 файлов (dom/model/bench/features/auth/regressions/guild-rail/voice/channel-switch/read-state/join-server) | **351 pass**, 0 fail, 0 skip (exit 0) — уровень до карточки сохранён | `regression.log` |
| смоук: `node --test tests/unit/*.test.mjs` | **643 pass**, 0 fail | разовый прогон, не сохранён |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0, вывода нет (`tsc.log` пуст) | `tsc.log` |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | 31/31 в допуске (exit 0) | `geometry.log` |

Окно/live/сеть/cargo/`gate:*`/commit не запускались.

## 4. Изменения по файлам

- `rust/gui-spike/ui/features/typing-indicator.js` (**новый**) — `TYPING_TTL_MS`,
  `TYPING_RESEND_MS`, `TYPING_MAX_CHANNELS`, `TYPING_TEXTS`; `createTypingIndicator`
  (live-регион, дедуп по `(channelId,userId)`, TTL/sweep, self-фильтр, notice) и
  `createTypingSender` (троттлинг, guard, 401/403/429/slowmode без цикла) + `typingFailure`.
- `rust/gui-spike/ui/features/composer.js` — опция `onType`, вызовы после непустого
  input и вставки эмодзи; ошибка колбэка → `onError` (ввод не ломается).
- `rust/gui-spike/ui/app.js` — импорт модуля, `typing`/`typingSender` в `App`
  (для диагностики/тестов), self id из `ready().user.id`, `onType` только в
  channel-режиме, refresh индикатора после `openChannel`, `reset()/clear()` на logout.
- `rust/gui-spike/ui/components/composer.css` — `.typing-indicator` (absolute над
  композером, над чипом ответа), `.typing-indicator-notice` (`--danger-text`),
  пустой регион `hidden`; раскладка composer 56px не меняется.
- `tests/unit/typing-ui.test.mjs` (**новый**) — 20 кейсов: константы/политика,
  индикатор (TTL/дедуп/self/канал/aria), sender (интервал/guard/401/429/slowmode/
  токен), интеграция с app (реальный канал из READY, bench, self id, slowmode, 401).
- `rust/gui-spike/ui/components/chat.css`, `ui/index.html`, `ui/features/chat.js` —
  **не изменялись** (индикатор создаётся в `#composer` из JS; `chat.js` уже отдаёт
  реальный `channelId`). `tests/support/gui-dom-harness.mjs` — не изменялся.

## 5. md5

```
09f9f53870b669a382d52ecadcabb799  rust/gui-spike/ui/features/typing-indicator.js
78039af36b09fa94e5d92b207f5ce977  rust/gui-spike/ui/app.js
373bf1056562fa8ca4587c9e3ebdb709  rust/gui-spike/ui/features/composer.js
d562afe38ba96109053821fd48aed3e6  rust/gui-spike/ui/components/composer.css
896ae41cfa2d493498236c9f14ee4433  tests/unit/typing-ui.test.mjs
3e242d8b06d74e29ef6f49f04f891e56  rust/gui-spike/ui/components/chat.css (без изменений)
bc694b178cf4ded2df09c889d730d877  waves/RW-40/PM/p03-ui/red.log
1e37ee1cb257d0a7a578af2542ecbfce  waves/RW-40/PM/p03-ui/red-mutation.log
ae4d45f7e596abd7808ffccdb4082672  waves/RW-40/PM/p03-ui/green.log
b65ac85f2a98fb59391bd70d10746fd1  waves/RW-40/PM/p03-ui/regression.log
d41d8cd98f00b204e9800998ecf8427e  waves/RW-40/PM/p03-ui/tsc.log (пусто = чисто)
37409be26e9b9ee425140b67567aa8c2  waves/RW-40/PM/p03-ui/geometry.log
```

## 6. Остаток / решения на приёмку

1. **`TYPING_RESEND_MS = 8_000`** — нижняя граница диапазона 8–10 с (ТЗ), G6 §3.6
   допускает «дефолт = core TTL». Выбрано 8 с, чтобы повтор приходил до истечения
   TTL 10 с у собеседника; если PM хочет ровно 10 000 — одна константа
   (`typing-indicator.js`), тест проверяет правило, а не число.
2. **Источник `TYPING_START` — L-02.** Gateway→UI адаптера в сборке нет; UI принимает
   события через `typing.start({channelId,userId,name?,self?,at?})`. Это не «мёртвый»
   код: до появления L-02 индикатор проверяется fake-событиями в тестах и доступен
   через `currentApp().typing`.
3. **`selfUserId`** берётся из `ready().user.id` (обёртка `invoke` в `app.js`);
   отдельного поля в `UiSettings` не добавлял.
4. **Guard считает каналы с успешным typing** (не с попыткой): отказ 401/403 не
   занимает слот; 429-пауза ограничена `retry_after`, slowmode — кулдауном из 200.
5. **`index.html` не тронут** — узел индикатора создаётся в `#composer` (как счётчик/
   чип ответа), поэтому контракт шима (`id/class` из html ⊆ шим-DOM) не расширялся.
6. **Поведение при logout:** `typingSender.reset()` (снимает 401/403-блок) и
   `typing.clear()` (снимает записи/notice) — следующий вход не наследует состояние.
