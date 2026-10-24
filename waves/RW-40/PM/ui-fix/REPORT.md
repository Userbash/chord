# G2 Frontend — исправление P1/P2 из Codex-ревью (`/tmp/opencode/review-ui1.md`)

Ветка/HEAD на момент работы: `0239543` (+ незакоммиченное дерево RW-39/RW-40).
Red-first: тесты добавлены и прогнаны до фикса, затем минимальный продуктовый код.
Файлы: `ui/features/{message-toolbar,chat,auth}.js`,
`tests/unit/gui-spike-{features,auth}.test.mjs`. `src/**`, `ui/app.js`, `ui/model.js`,
`ui/features/virtual-list.js`, harness — не менялись.

## Пункт → тест → результат

| # | Пункт ревью | Тест | Red (до) | Green (после) |
|---|-------------|------|----------|----------------|
| 1 | P1 toolbar: быстрые реакции не отправляли `toggle_reaction` | `gui-spike-features`: «hover-панель: быстрая реакция уходит в IPC один раз и откатывается при 401» | `панель не отправила toggle_reaction: 0 !== 1` | 1 IPC `{channelId:"news",messageId:"26",emoji,active:true}`, optimistic `aria-pressed=true` + пилюля; двойной клик — 1 запрос; 401 → откат `aria-pressed=false` + `.error-state` |
| 2 | P1 delete: строка возвращалась из PageStore, оставалось пустое место | `gui-spike-features`: «B4b: удалённая строка не возвращается из PageStore после выгрузки и не оставляет пустого места» | `удалённая строка осталась в DOM` | tombstone по индексу + индекс высот с дырой: строки 11/13 сомкнулись (`top13 == bottom11`), после LRU-выгрузки страницы 0 (`pages.has(0) === false`) и возврата строки 12 нет, зазора нет |
| 3 | P1 auth: token-login/restore не блокировались, повторный клик слал команды | `gui-spike-auth`: «токен: повторный клик до ответа не шлёт второй IPC, кнопка disabled до ответа», «restore: повторный клик до ответа не шлёт второй auth_restore» | 2 fail: нет `disabled`/`aria-disabled` на время IPC | `tokenBusy`/`restoreBusy`: `disabled`+`aria-disabled`+`title`, ровно 1 `auth_login_token`/`auth_restore`, атрибуты сняты после ответа, ложной ошибки «Введите токен» нет |
| 4 | P1 MFA: после успеха кнопка активна, повторный клик слал пустой код | `gui-spike-auth`: «после submit код очищен, кнопка disabled, второй клик не шлёт пустой auth_mfa» | `updateMfa не вернул disabled после очистки кода` / лишний вызов | после очистки кода вызван `updateMfa` → `aria-disabled=true`; `mfaBusy` блокирует второй submit; `auth_mfa` ровно 1, `code === "123456"`; после `ready` — login |
| 5 | P2 draft: ошибка отправки затирала новый черновик | `gui-spike-features`: «ошибка отправки не затирает набранный поверх черновик, пустое поле — возвращает текст» | `ошибка отправки затёрла новый черновик: "первое"` | непустой черновик не трогается; пустой композер по-прежнему получает неотправленный текст |
| 6 | P2 edit: нет блокировки повторного submit | `gui-spike-features`: «B4b: повторный submit edit не отправляет второй edit_message до ответа» | `повторный submit отправил второй edit_message: 2 !== 1` | `data-pending` на «Сохранить» до ответа, второй Enter/клик игнорируется — 1 `edit_message` |

Реализация:
- `message-toolbar.js`: реакция идёт через `requestReaction` (pending по ключу строка/emoji,
  optimistic `aria-pressed`/пилюля, откат 401/429 транспортом `reactions.js`); edit-save
  блокируется `data-pending` до завершения `onEdit`.
- `chat.js`: `createDeletionHeights` — надстройка над `HeightIndex` (высота удалённой 0,
  строки ниже сдвигаются, `indexAt/top/totalHeight` согласованы); tombstone-источник для
  `VirtualList` (`getRow → null`), поэтому LRU-перезагрузка PageStore не воскрешает строку;
  `relayout()` после delete убирает старые узлы с устаревшими `top`; возврат текста в
  композер только при пустом поле.
- `auth.js`: `tokenBusy/restoreBusy/mfaBusy` + `setBusy()` (disabled/aria-disabled/title),
  `updateMfa()` после очистки MFA-кода, снятие блокировок в `finally`.

Тесты не ослаблялись: существующие `it` не менялись, добавлены 7 новых (4 features + 3 auth);
assert'ы сравнивают IPC-аргументы, состояние DOM и геометрию строк.

## Прогоны

```
node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs \
  tests/unit/gui-spike-auth.test.mjs
→ tests 205, pass 205, fail 0
npx tsc -p rust/gui-spike/ui/tsconfig.json → exit 0
node waves/RW-39/PM/w39c/geometry-check.mjs → все 31 геометрических значений в допуске
```

Полный лог: `waves/RW-40/PM/ui-fix/green.log`.

## md5

```
f8c76cd55dfaf57bc90bcccf863fbb5b  rust/gui-spike/ui/features/message-toolbar.js
48caa73065fd33837af8d8200205fc31  rust/gui-spike/ui/features/chat.js
3f496d0d9f7bd06b94cffc8de2db8d2c  rust/gui-spike/ui/features/auth.js
a44bdf0ee865d7206d193e9c28cf0eac  tests/unit/gui-spike-features.test.mjs
8f45e984b44f14478f461bd826699076  tests/unit/gui-spike-auth.test.mjs
```

## Остаток / риски

- Удаление живёт в памяти экрана (tombstone-набор); после перезапуска приложения сообщение
  снова придёт из источника. Для спайка без БД это ожидаемо — серверный `delete_message`
  вызывается, но локального каталога сообщений нет.
- `data-pending` edit/реакции — атрибуты диагностики; отдельных spinner'ов в спайке нет.
- Конкурентная работа: `tests/unit/gui-spike-features.test.mjs` по ТЗ правит и worker B7b
  (сетевой поиск); в финальном прогоне обе правки сосуществуют (57 `it`, 59 тестов).
  `search-overlay.js`/`app.js` не трогал.
- Окно/cargo/gate/сеть/commit не запускались.
