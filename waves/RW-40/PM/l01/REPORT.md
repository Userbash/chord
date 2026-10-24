# L-01 — видимый reconnect/offline state в UI

- Волна: RW-40, карточка L-01 (`PARITY-MATRIX` P1, WSJF 6.0), воркер G2 Frontend.
- Скоуп: `ui/app.js`, `ui/index.html`, `ui/components/net-status.css` (новый),
  `ui/features/actions.js`, `tests/unit/gui-spike-features.test.mjs`,
  `tests/support/gui-dom-harness.mjs` (аддитивно).
- Live/сеть/окно/cargo/gate/commit не использовались: ответы IPC задаёт fake invoke.

## Что сделано

1. **Баннер `#net-status`** в `.topbar` (в потоке, не `fixed` — чат не перекрывается):
   `role="status"`, `aria-live="polite"`, `aria-atomic="true"`.
   - `ready` → `hidden`;
   - `connecting` / `reconnecting` → «Переподключение…», retry скрыт;
   - `offline` / `error` → «Нет соединения» + «Повторить».
2. **Поллинг `net_status`** каждые 2 с (`NET_POLL_MS`) без аргументов (токен в args не
   попадает). Старт — только в `onReady` гейта входа, стоп — `dispose()` и клик
   `#auth-logout`; при `document.hidden === true` IPC не шлётся. Поздний ответ после
   stop/dispose UI не меняет.
3. **Ручной retry** → `data-action="net-retry"` (общий путь §8.1 в `actions.js`) →
   ровно один `net_connect` на клик (busy-защита). Ошибка 401/unauthorized/токен →
   видимый текст «Сессия истекла. Войдите снова.», прочее — «Не удалось подключиться.
   Проверьте сеть и повторите.»; авто-повторов нет, статус-поллинг продолжается.
4. **Фокус**: появление баннера фокус не крадёт; если баннер скрылся, пока фокус был
   на «Повторить», фокус возвращается в `#chat-list`.
5. **Харнесс**: добавлен дефолтный ответ `net_status → "ready"` (иначе после auth ready
   в существующих тестах команда была бы неизвестной).

## Red (до кода)

`node --test --test-name-pattern "L-01" tests/unit/gui-spike-features.test.mjs`
— 8/8 падений (`нет #net-status`, нет css, таймера/IPC нет). Лог: `red.log`.

## Green

- targeted: 8/8 pass (`targeted.log`);
- обязательные 6 файлов: **253 pass / 0 fail** (было 245 + 8 новых), `tests.log`;
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → 0 (`checks.log`);
- `node waves/RW-39/PM/w39c/geometry-check.mjs` → 31/31 (`checks.log`).

## Файлы и md5

```
9a8d676f106760f71223d6f471f1a693  rust/gui-spike/ui/app.js
a8405e1b4a5bed0d332de26d78bfa0b0  rust/gui-spike/ui/index.html
8476e12a2de921f280e37f92d29343b8  rust/gui-spike/ui/components/net-status.css
ef6d54fdecd67b904b963977c19835d0  rust/gui-spike/ui/features/actions.js
c2babf59d3295e5c839bf9d2785f16bb  tests/unit/gui-spike-features.test.mjs
5ddf181223a2ddb9b5c497a8bffac897  tests/support/gui-dom-harness.mjs
```

## Остаток / риски

- Тексты «Переподключение…»/«Нет соединения» взяты из ТЗ L-01 (в дампе 1.0.166 также
  встречаются «Ожидание сети...»/«Снова в сети» — паритет текстов не подтверждён `[L]`).
- `net_status` при ошибке самого IPC показывает «Нет соединения» (без оверлея и без
  console.error); отдельного текста «ядро недоступно» нет.
- Реальный gateway resume/4013/4014 остаётся на стороне G1 (`net_contract.rs`); UI
  отображает только `GatewayStatus` из `net_status`, семантику close-кодов не дублирует.
- Визуальная проверка в окне не выполнялась (запрещено условиями задачи); геометрия
  подтверждена только geometry-check 31/31 и отсутствием `position: fixed` у баннера.
