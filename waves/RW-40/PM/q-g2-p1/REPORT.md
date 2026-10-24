# Q-G2-P1 — отчёт воркера G2 Frontend

Дата: 2026-10-24. PM: DeepSeek. Задача: `Q-G2-P1` — 3 P1 из
`waves/RW-40/QUALITY/G2-AUDIT.md` (G2-01, G2-02, G2-04), red-first.
Режим: без live/сети/окна/cargo/gate/commit. HEAD `0239543` (workspace грязный — чужие
изменения не трогались; `rust/gui-spike/ui/**` и `waves/RW-40/**` в git ещё untracked).

## 1. Итог

| Проверка | Команда | Exit | Результат | Лог |
|---|---|---:|---|---|
| unit | `node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-auth.test.mjs` | **0** | tests **215**, pass **215**, fail **0**, skip **0**, 1353 ms | `green-node-test.log` |
| tsc | `npx tsc -p rust/gui-spike/ui/tsconfig.json` | **0** | вывод пустой | `tsc.log` |
| geometry | `node waves/RW-39/PM/w39c/geometry-check.mjs` | **0** | 31/31 в допуске | `geometry.log` |
| meta (доп.) | `node --test tests/meta/test-policy.test.mjs tests/meta/design-breakpoints.test.mjs` | 0 | 10/10 | — |

Было 212 тестов → стало **215** (+3, ровно новые G2-кейсы). Существующие assert не менялись,
`skip`/`only`/ослаблений нет. DP-02 и `search-overlay.js` не трогались.

## 2. RED (до кода)

`node --test tests/unit/gui-spike-features.test.mjs` → **exit 1**, 3 fail, лог `red-features.log`.

```
✖ G2-01 … AssertionError: keydown x отменил reply (G2-01)
    actual: 'false', expected: 'true'
✖ G2-02 … AssertionError: .content не является containing block для .error-state (G2-02)
    actual: '', expected: 'relative'
✖ G2-04 … AssertionError: Tab #1 не удержан модальным гейтом (G2-04)
    actual: false, expected: true
```

Все три падения — по заявленной причине аудита, а не по опечатке в тесте.
Тесты добавлены отдельным `describe("G2 (RW-40): P1 — reply, error-overlay, auth focus-trap")`
в конец `tests/unit/gui-spike-features.test.mjs`; ни один существующий кейс не переписан.

## 3. Что сделано (green)

### G2-01 — `features/composer.js`
`onKeydown`: вместо `if (reply) clearReply()` (любая клавиша) —
`if (event.key !== "Escape") return;` + ветка Escape (`reply` → `clearReply()`, иначе
`onEscape`). Ввод/`Backspace`/стрелки больше не сбрасывают режим ответа; отмена — только
Escape и крестик чипа. Enter-ветка не менялась.

### G2-02 — `features/errors.js`, `app.js`
- `.error-state` получил кнопку `.error-state-close` (`type=button`,
  `aria-label="Закрыть сообщение об ошибке"`), клик → `clearErrorState(container)`.
  `showErrorState` по-прежнему сначала чистит старый оверлей → повторная ошибка заменяет.
- Containing block: `.content` получает `position: relative`. CSS-файлы
  (`ui/style.css`, `ui/components/primitives.css`) — вне файловой матрицы задачи
  (`остальной ui/**` read-only), поэтому позиционирующий контекст выставлен в `app.js`
  при загрузке (комментарий на месте). При переносе строки в `style.css` inline-значение
  остаётся тем же и ничего не ломает — рекомендация G3/PM.
- Снятие по успеху: единая точка `invoke` в `app.js` после успешного ответа вызывает
  `clearErrorState(content)` для команд-действий (`search_remote`, `send_message`,
  `reply_message`, `edit_message`, `delete_message`, `toggle_reaction`, `settings_save`).
  Служебные вызовы (метрики/снимки/`settings_load`) оверлей не трогают — иначе фоновая
  телеметрия стирала бы чужую ошибку. `configureReactionTransport` переведён на тот же
  `invoke` (минус второй путь `window.__TAURI__`).

### G2-04 — `features/actions.js`
- `authGateLayer()`: видимый `#auth-gate` (`data-state != "ready"`, без `hidden`,
  `display != none`); при открытом поиске/switcher гейт фокус не перехватывает.
- `topLayer()`: явно открытые `settings`/`emoji` остаются верхними (их Tab-цикл не изменён);
  в `chat`/`members` верхний слой — auth-гейт. Tab в гейте идёт по `focusables(gate)`
  с `preventDefault`; `focusables` теперь отбрасывает скрытые (`hidden`, inline
  `display:none`) и disabled-узлы — Tab не попадает в `aria-hidden`-shell и в скрытые
  секции MFA/captcha.
- Escape при верхнем auth-гейте: `preventDefault`, экран не переключается, фокус
  остаётся внутри гейта (иначе уходил в `.rail-btn` под `aria-hidden`).
- Восстановление фокуса при закрытии: `app.js onReady` переключает chat и фокусирует
  `#chat-list` — покрыто новым тестом; второй механизм не добавлялся, чтобы не спорить
  за фокус. После `ready` гейт больше не перехватывает Tab (`prevented === false`).

## 4. Тесты (диф — только добавление)

`tests/unit/gui-spike-features.test.mjs` (+184 строки, 3 `it`):
1. G2-01: `data-reply` переживает `keydown x/a/Backspace/ArrowLeft` и ввод текста;
   Escape → `false` и черновик цел; крестик → `false`.
2. G2-02: `.content.style.position === "relative"`; ошибка `search_remote` → ровно 1
   `.error-state` внутри `.content`; кнопка закрытия с `aria-label`; повторная ошибка не
   копит оверлеи; dismiss → 0; успешный `search_remote` после ошибки → 0 и статус `ok`.
3. G2-04: `no-session` при `shell[aria-hidden=true]`; 30×Tab — все `prevented`, фокус
   только внутри `#auth-gate`, посещено ≥3 узла (цикл); Escape не выводит фокус; после
   входа (`auth_login_token` → ready) гейт скрыт, shell снова доступен, фокус —
   `#chat-list`; закрытый гейт Tab не перехватывает.

`tests/support/gui-dom-harness.mjs` — **не менялся** (аддитивные правки не потребовались:
`focus()`/`activeElement`/`hidden` уже есть, `contains` заменён локальным обходом
`parentNode`). md5 ниже совпадает с исходным.

## 5. Файлы + md5

| Файл | md5 | Δ |
|---|---|---|
| `rust/gui-spike/ui/features/composer.js` | `0c833491aaef7b8f6dfdb33b1d0d1164` | onKeydown (G2-01) |
| `rust/gui-spike/ui/features/errors.js` | `50744345fbc74dd0902f05fddcc614ea` | кнопка закрытия (G2-02) |
| `rust/gui-spike/ui/features/actions.js` | `0946777fdcb3cf71c19e6f2c11197b57` | auth topLayer/trap/Escape, focusables (G2-04) |
| `rust/gui-spike/ui/app.js` | `77de6025fb63bbfc0791aef38900f67b` | `.content` containing block, clear-on-success, reactions через `invoke` (G2-02) |
| `tests/unit/gui-spike-features.test.mjs` | `03e95cc03adb87432a38085c8ed42cfe` | +3 теста |
| `tests/support/gui-dom-harness.mjs` | `12ea7059e2c4de7c64a0daa40711094a` | без изменений |

## 6. Риски / остаток

- **CSS-позиционирование** (`.content{position:relative}`) сейчас inline из `app.js`, потому
  что `ui/**` кроме перечисленных файлов read-only. Перенести правило в `style.css` при
  разморозке CSS (G3/W39d) — поведение не изменится.
- `inert` на `.shell` не выставлялся (в шиме нет inert, аудит допускал альтернативу):
  закрыто Tab-trap + `preventDefault` + Escape-guard. AT-дерево shell остаётся в DOM, но
  `aria-hidden=true` уже стоит. Полный `inert` — отдельная карточка доступности.
- Приоритет слоёв: при открытых `settings`/`emoji` Tab остаётся в них даже если auth-гейт
  видим (иначе ломаются существующие кейсы). В реальном сценарии вход блокирует эти экраны
  отдельно — контракт зафиксирован комментарием в `topLayer`.
- Остальные находки аудита (P2/P3: G2-03, G2-05…G2-30, split-план §3) не трогались.
- gate/cargo/live/commit не запускались (запрещено ТЗ); `npm run gate:worker` — за PM/лидом.
