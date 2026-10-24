# G-03-UI — модалка join by invite (frontend, red-first)

Дата: 2026-10-24. Воркер: G2 Frontend (subagent PM DeepSeek), batch RW-40.
Статус: **готово к приёмке**. Окно/live/cargo/gate/сеть/commit не использовались.

## 1. Контракт (источник)

- `waves/RW-40/DESIGN/SERVER-MENU.md` §2.2 (вход «+»), §2.3 (два шага
  resolve → accept), §3.1 (ошибки 401/403/404/429/сеть), §4.3/§6 (DOM/токены),
  §5.1 (копирайт), §7.3 (SM-20…SM-39).
- `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.3: `resolve_invite(code)` →
  `{code, guildId, guildName, icon?, memberCount?, presenceCount?, expiresAt?, channelId?}`;
  `accept_invite(code)` → `{guildId, targetChannelId?, targetMessageId?}`; accept —
  только 2xx.
- Принятый backend D-116 (`waves/RW-40/PM/g03/REPORT.md`,
  `rust/gui-spike/src/invites.rs`): тело accept — три nullable поля, UI их не
  подставляет; bearer — только в backend.

## 2. Сделано

| Файл | Что |
|---|---|
| `ui/features/join-server.js` (new) | `normalizeInvite` (зеркало backend `normalize_code`), `inviteFailure` (kind → текст §3.1), `createJoinServer`: DOM §6, состояния `idle/resolving/preview_ready/accepting/accepted/error`, preview только из 2xx, accept `{code}` (nullable-поля не подставляются), 429-cooldown, блокировка pending, Esc/«Отмена»/клик вне с возвратом фокуса, `#server-status` успех |
| `ui/app.js` (аддитивно) | импорт join; `createGuildRail`: «+» `.guild-add[data-action="add-server"]` последней в группе серверов, `addGuild(guild)` (после accept: плитка + выбор → `channels(guildId)`); создание `join` до `initActions`; `App.join` |
| `ui/features/actions.js` | `join` в контексте; обработчики `add-server`/`join-submit`/`join-cancel`/`dialog-close`; Esc-слой join (pending не закрывается); клик вне карточки; `topLayer()` — join первым (Tab-trap) |
| `ui/index.html` | host `#join-server-dialog` (пустой, содержимое собирает JS — как `#auth-gate`) |
| `ui/components/server-menu.css` | join-форма/input/preview/ошибка/«×», primary-кнопка, «+» рейла; только токены (одно исключение — 16px имени preview, токена нет) |
| `tests/unit/join-server.test.mjs` (new) | 16 тестов, оракул — документы выше, среда — DOM-шим + IPC-заглушка |

`src/**`, `search/chat/voice`, `rail.css`, `tokens.css`, `fixtures/**`, `logs/`,
`functions/`, `evidence/` не изменялись; `tokens.css` не тронут (geometry 31/31).

## 3. RED (до кода)

`waves/RW-40/PM/g03-ui/red.log` — тесты написаны до `join-server.js`:

```
Error: ENOENT: no such file or directory, open '…/ui/features/join-server.js'
✖ tests/unit/join-server.test.mjs (module missing)
ℹ tests 1  pass 0  fail 1
```

Продуктового кода на момент red не было. Промежуточный прогон после первой
версии модуля был красным по 5 проверкам (нормализация `…/invite/`, guard
`index.html`, повтор accept после 403, атрибут `disabled` в 429, regex токена);
все пять закрыты правкой кода, а не ослаблением тестов (две формулировки в
тестах были исправлены как ошибки оракула — 429 проверяет наличие `disabled`,
а не его отсутствие).

## 4. GREEN

Targeted (`join-server.test.mjs`) — **16 passed, 0 failed** (`targeted.log`).
Frontend-набор RW-38/39/40 — **319 passed, 0 failed** (`frontend-319.log`), без
изменения тестов.

| Команда | Итог |
|---|---|
| `node --test tests/unit/join-server.test.mjs` | 16 pass / 0 fail |
| `node --test …(9 файлов: dom/model/bench/features/auth/regressions/guild-rail/voice-ui/channel-switch)` | **319 pass / 0 fail** |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0, 0 ошибок (`tsc.log`) |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | **31/31** (`geometry.log`) |

Что проверяют тесты:

1. `normalizeInvite`: ссылки/коды/query/хвостовой `/`, мусор → `""` (без IPC).
2. «+»: `aria-label="Добавить сервер"`, `aria-haspopup="dialog"`,
   `aria-expanded`; открытие → `#join-server-dialog`, `role=dialog`,
   `aria-modal`, `data-state=idle`, фокус в `#join-invite-input`.
3. Пустое поле → `#join-submit disabled`; ввод включает; Enter = ровно 1
   `resolve_invite`.
4. Неверный формат → `aria-invalid`, `resolve_invite` не вызван.
5. resolve 2xx → `preview_ready`, preview с name/`memberCount`/`presenceCount`
   из ответа, `accept_invite` ещё не вызван, `<img>/src` нет (icon — хеш, не URL).
6. Двойной/тройной клик = 1 IPC; `aria-busy`, «Приглашение обрабатывается»;
   Esc в pending не закрывает.
7. accept 2xx → закрытие, `#server-status` «Вы присоединились к серверу «…»!»,
   плитка `data-guild-id=999` выбрана, `channels({guildId:"999"})` вызван;
   аргументы accept — **ровно** `{code}`, в IPC нет токена/`session_id`.
8. resolve 404 → «Приглашение недействительно…», preview скрыт, код сохранён,
   accept не вызван, submit доступен для повтора.
9. accept 403 → «Недостаточно прав…», без success-текста/плитки; повтор шлёт
   только accept (resolve не повторяется).
10. 401 → «Сессия истекла. Войдите снова.», без успеха и плитки.
11. 429 → текст с секундами, `data-retry-after="5"`, submit disabled; после
    таймера снова enabled.
12. accept 2xx без `guildId` → видимая ошибка, рейл не мутируется.
13. «Отмена»/Esc → закрытие, `aria-expanded=false`, фокус на «+», поле сброшено.
14. В модуле нет `fetch`/storage и подстановки nullable-полей accept.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/index.html` | `417bd6687f54a2063232390e579ffe39` |
| `rust/gui-spike/ui/app.js` | `4591bdf4c2ea793ecbb35221ddcaf2ef` |
| `rust/gui-spike/ui/features/join-server.js` (new) | `2efcea7d3245e1370cffc0d78617d994` |
| `rust/gui-spike/ui/features/actions.js` | `90f4f4a0449de59046002a91cede488e` |
| `rust/gui-spike/ui/components/server-menu.css` | `1c18124f0958f8d9c43bf54ff9543640` |
| `tests/unit/join-server.test.mjs` (new) | `2cd44b2da1debec0c0091cc9c8dad2e5` |

Логи: `red.log`, `targeted.log`, `frontend-319.log`, `tsc.log`, `geometry.log`.

## 6. Остаток / риски

1. **401 не открывает `#auth-gate`**: §3.1 просит auth-gate, но `features/auth.js`
   вне файловой матрицы задачи; сейчас 401 виден в `#join-error`, успеха нет.
   Нужна отдельная карточка на связку join ↔ auth-gate.
2. **Отдельной кнопки «Повторить» нет**: в DOM-контракте §6 её нет; повтор делает
   та же `#join-submit` (retry только упавшего шага). Если PM хочет отдельный
   контрол — аддитивно, но это меняет §6.
3. **403 `USER_BANNED`/screening/age-gate отдельными текстами не различаются** —
   backend (D-116) отдаёт только `kind: forbidden` без машинного `code`; тексты
   из §5.1 не выдумывались. Нужно тело ошибки из live-окна.
4. **Lurker-вход (`enter-guild`, SM-68/69) не делался** — вне ТЗ G-03-UI.
5. **Live-коды/тело 429 не наблюдались** (`[A]`): `retry_after_ms` берётся из
   backend, `0` → «Повторите позже» без отсчёта.
6. **Нормализация `abc/def`**: UI зеркалит принятый backend `normalize_code`
   (последний сегмент), поэтому такой ввод станет кодом `def`, а не ошибкой
   SM-24. Расхождение осознанное: один источник истины — backend D-116; при
   смене правила правится одна функция в обоих местах.
7. **Иконка сервера**: backend отдаёт `icon` (хеш CDN), не URL; рисуется
   монограмма, CDN-адрес не склеивается (§2.1).
8. **Окно не запускалось** (запрет): визуальная сверка геометрии join-модалки
   (`unmeasured`, §4.3) — после первого живого окна.
9. Коммит не делался (уровень L1).
