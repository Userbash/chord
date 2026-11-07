# RW-40 / STUB-05-INVITES-MODAL-UI / G-06 — модалка создания приглашения (#invite-create-modal)

- **batch/карточка:** `RW-40 / STUB-05-INVITES-MODAL-UI / G-06` (`waves/RW-40/DESIGN/INVITES-SPEC.md` §1, `waves/RW-40/RE-DEEP-REA.md` §3/§4.3);
- **роль:** Воркер G2 Frontend Lane B;
- **сделано:**
  1. Реализован модуль `rust/gui-spike/ui/features/invite-modal.js` с контроллером `createInviteModal(options)`.
  2. Экспортированы константы оракула:
     - `VALID_INVITE_MAX_AGES = Object.freeze([0, 1800, 3600, 21600, 43200, 86400, 604800])`;
     - `VALID_INVITE_MAX_USES = Object.freeze([0, 1, 5, 10, 25, 50, 100])`;
     - `INVITE_FLAG_IS_GUEST = 1` (`IS_GUEST_INVITE = 1 << 0`).
  3. Прогрессивное создание DOM-структуры без необходимости статической разметки в `index.html`:
     - Backdrop `#create-invite-dialog` (`class="dialog-backdrop"`, `role="presentation"`, `hidden`);
     - Карточка `#invite-create-modal` (`class="dialog invite-modal"`, `role="dialog"`, `aria-modal="true"`, `aria-label="Пригласить людей"`, `data-state="idle|loading|populated|error-429|error-403|error"`).
  4. Форма параметров приглашения:
     - Селект `#invite-max-age` со всеми 7 опциями (`1800`, `3600`, `21600`, `43200`, `86400`, `604800`, `0`), дефолт `86400` («1 день»);
     - Селект `#invite-max-uses` со всеми 7 опциями (`0`, `1`, `5`, `10`, `25`, `50`, `100`), дефолт `0` («Без ограничений»);
     - Тумблер временного членства `#invite-temporary-toggle` (`temporary`, boolean, дефолт `false`);
     - Тумблер гостевого доступа `#invite-guest-toggle` (`guest`, boolean, дефолт `false`; при включении передаёт `flags: 1`, при выключении — `flags: null`);
     - Аккордеон настроек `#invite-settings-toggle` (`aria-expanded="false"`, `aria-controls="invite-settings-panel"`) и форма `#invite-settings-panel`.
  5. Генерация ссылки через IPC `invoke('create_invite', { channelId, maxAge, maxUses, temporary, flags })`:
     - Плейсхолдер `Генерация ссылки...` с `aria-busy="true"` и блокировкой кнопок во время выполнения запроса;
     - Отображение канонической ссылки `https://discord.gg/{code}` в поле `#invite-link-input` (`readonly`).
  6. Копирование ссылки через `navigator.clipboard.writeText`:
     - Текст 'Ссылка скопирована' в `#invite-copy-feedback` и 'Скопировано!' с классом `.btn-positive` на кнопке выставляются **только** после успешного выполнения промиса `writeText`;
     - При reject промиса или отсутствии API — честный статус 'Не удалось скопировать' с резервным `input.select()`, без ложного успеха (дефект D4).
  7. Обработка ошибок:
     - `429` (Rate limited): извлечение `retry_after_ms`, баннер `#create-invite-error`, блокировка кнопки генерации, обратный отсчёт `Слишком много запросов. Повторите через {n} с.` каждую секунду до 0 с автоматическим скрытием баннера и разблокировкой кнопки;
     - `403` (Forbidden): сообщение `У вас нет прав на создание приглашений в этот канал.`, блокировка формы настроек и кнопки генерации;
     - Общие ошибки: переход в `data-state="error"` с выводом ошибки.
  8. a11y, клавиатура и управление фокусом:
     - Закрытие по кнопке `[data-action="create-invite-close"]`, клику по backdrop и клавише `Escape` (`preventDefault` + `stopPropagation`);
     - Возврат фокуса на элемент-триггер (opener).
  9. Тесты: в `tests/unit/join-server.test.mjs` добавлен новый блок `STUB-05-UI / G-06: модалка создания приглашения (#invite-create-modal)` из 11 детальных тестов; исходные 24 теста сохранены без изменений (всего 35 тестов).

- **red:** `node --test tests/unit/join-server.test.mjs > waves/RW-40/PM/stub05-invites-ui/red.log 2>&1` → `exit=1`, 24 passed, 11 failed; лог: `waves/RW-40/PM/stub05-invites-ui/red.log`.
- **green:** `node --test tests/unit/join-server.test.mjs > waves/RW-40/PM/stub05-invites-ui/green.log 2>&1` → `exit=0`, 35 passed, 0 failed; лог: `waves/RW-40/PM/stub05-invites-ui/green.log`.
- **targeted:**
  - `node --test tests/unit/join-server.test.mjs` → `35 passed; 0 failed` (`exit=0`);
  - `npx tsc -p tsconfig.json` → 0 ошибок (`exit=0`);
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json` → 0 ошибок (`exit=0`);
  - `git grep "@ts-ignore" rust/gui-spike/ui/features/invite-modal.js tests/unit/join-server.test.mjs` → 0 совпадений.
- **oracle:** `waves/RW-40/DESIGN/INVITES-SPEC.md` §1.1–§1.5, `waves/RW-40/RE-DEEP-REA.md` §3, §4.3 (`4d2a64cb5df5fc4e_0@632264`, `@270160`, `@1933819`).
- **мутации:** 5/5 `KILLED`, эквивалентных — 0, survivors — 0 (`waves/RW-40/PM/stub05-invites-ui/mutations.log`):
  - `MUT-1-default-max-age` (md5 `8238236fa9830752b129e5ad1261f781`) — замена дефолтного `max_age` с `86400` на `3600` → `KILLED` тестами `#invite-create-modal и элементы формы` и `open() запускает генерацию с дефолтными параметрами` (`exit=1`);
  - `MUT-2-guest-flag-null` (md5 `2c24d99044bd9cf8ab74c94839dbccec`) — потеря передачи флага `INVITE_FLAG_IS_GUEST` при включённом guest-тумблере → `KILLED` тестом `генерация новой ссылки с изменёнными max_age, max_uses, temporary и guest флагом` (`exit=1`);
  - `MUT-3-false-success-on-clipboard-error` (md5 `8a80937106c6644da4cf115b90e961b0`) — ложное сообщение об успехе при reject промиса clipboard → `KILLED` тестом `копирование ссылки: честная ошибка при reject промиса clipboard` (`exit=1`);
  - `MUT-4-drop-429-cooling-disable` (md5 `9f3e758f5933b4177e7376f5dcf76307`) — отсутствие блокировки кнопки генерации при ошибке 429 → `KILLED` тестом `обработка ошибки 429: retry_after_ms, обратный отсчёт и блокировка кнопки генерации` (`exit=1`);
  - `MUT-5-drop-escape-focus-restore` (md5 `9b898899ab2b38aee018e7f614c73eed`) — отсутствие возврата фокуса на триггер при закрытии модалки → `KILLED` тестом `закрытие по Escape, клику на backdrop и кнопке закрытия с возвратом фокуса на триггер` (`exit=1`).
- **baseline/restore:** baseline `rust/gui-spike/ui/features/invite-modal.js` = `d3d96c622325715a437334af431f91f7`, `tests/unit/join-server.test.mjs` = `8b790b9178f40aeed91346e5984713f9`; после каждого мутанта выполнен restore и проверено совпадение md5 (`match=true`).

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/invite-modal.js` | `d3d96c622325715a437334af431f91f7` |
| `tests/unit/join-server.test.mjs` | `8b790b9178f40aeed91346e5984713f9` |
| `waves/RW-40/PM/stub05-invites-ui/red.log` | `c7d905110ed73f07fe89e613bbab0092` |
| `waves/RW-40/PM/stub05-invites-ui/green.log` | `d6223f19ab1d04e00c557c3ca5c4693b` |
| `waves/RW-40/PM/stub05-invites-ui/mutations.log` | `996fe994b872302f4a12cfe04e33d280` |

## Риски и примечания

1. **Файловые границы:** из продуктового кода создан исключительно `rust/gui-spike/ui/features/invite-modal.js`. Файлы `app.js`, `chat.js`, `actions.js`, `composer.js`, `members.js`, `search.js`, `index.html` не модифицировались.
2. **Интеграция вызова:** интеграция открытия модалки из пункта меню сервера (`#server-menu [data-action="guild-invite"]`) и из кнопки канала в дереве каналов будет производиться на этапе склейки Lane Team Lead. Контроллер `createInviteModal` готов к подключению через интерфейс `open({ channelId, channelName, triggerElement })`.
