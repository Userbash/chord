# TASK-231 (FE-P1-24) — Контроллер модального окна создания приглашений на сервер, настроек ссылки и прямого приглашения друзей (`invite-modal.js`, G2)

- **Карточка / Ветка**: `task-231` (`feature/task-231-ui-invite-modal`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `waves/RW-40/DESIGN/INVITES-SPEC.md` (§1), `waves/RW-40/RE-DEEP-REA.md` (§4.3 `POST /channels/{id}/invites`).

## Сделано
1. `tests/unit/invite-modal.test.mjs` (Red → Green, 8 тестов в 2 suites, `// @ts-check` + JSDoc, минимальный DOM-шим):
   - проверка замороженных справочников `INVITE_MAX_AGE_OPTIONS_RU` (`1800`, `3600`, `21600`, `43200`, `86400`, `604800`, `0`) и `INVITE_MAX_USES_OPTIONS_RU` (`0`, `1`, `5`, `10`, `25`, `100`) с русскими подписями;
   - проверка `formatInviteUrl(code)`: обрезка крайних пробелов и возврат `"https://discord.gg/" + trimmed` (либо `""` для пустых/невалидных кодов);
   - проверка `createInviteModalController({ container, onCopyLink, onSendFriendInvite, onRegenerateInvite })`:
     - рендер диалога `[data-invite-modal]` с `role="dialog"`, `aria-modal="true"`, заголовком канала `[data-invite-channel-title]`, полем ссылки `[data-invite-url-input]`, кнопкой копирования `[data-invite-copy-btn]` (`"Копировать"` → `"Скопировано!"` + вызов `onCopyLink(url)`), полем поиска друзей `[data-invite-friend-search]`, строками друзей `[data-invite-friend-row]` с кнопками `[data-invite-friend-btn]` (`"Пригласить"` → disabled `"Отправлено"` + вызов `onSendFriendInvite(userId)`), панелью настроек (`[data-invite-max-age]`, `[data-invite-max-uses]`, `[data-invite-temporary]`, `[data-invite-regenerate-btn]` с вызовом `onRegenerateInvite({ maxAge, maxUses, temporary })`);
     - поддержка методов `open({ channelName, inviteCode, friends })`, `setSearchQuery(q)`, `updateSettings({ maxAge, maxUses, temporary })` и `close()`.
2. `rust/gui-spike/ui/features/invite-modal.js` (`// @ts-check` + JSDoc, ESM):
   - добавлены экспорты `INVITE_MAX_AGE_OPTIONS_RU`, `INVITE_MAX_USES_OPTIONS_RU`, `formatInviteUrl` и `createInviteModalController` с полным сохранением существующих экспортов (`VALID_INVITE_MAX_AGES`, `VALID_INVITE_MAX_USES`, `INVITE_FLAG_IS_GUEST`, `createInviteModal`).

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/invite-modal.test.mjs` → `tests 1, pass 0, fail 1` (`SyntaxError: ... does not provide an export named 'INVITE_MAX_AGE_OPTIONS_RU'`), лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-231-ui-invite-modal/red.log`.
- **Green**: `node --test tests/unit/invite-modal.test.mjs` → `tests 8, suites 2, pass 8, fail 0`, лог сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-231-ui-invite-modal/green.log`.
- **Regression**: `node --test tests/unit/join-server.test.mjs` → `tests 35, suites 4, pass 35, fail 0`.
- **Typecheck (targeted)**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` (`0 errors`) и `0 errors` в `invite-modal.js` / `invite-modal.test.mjs` при проверке `npx tsc -p tsconfig.json --noEmit`.
- **Gate**: полный `gate:worker` не запускался (режим ТЗ карточки); чужие файлы не затронуты.

## Файлы и MD5
```
8e76c874751953ebf55f9bd093da8cbb  rust/gui-spike/ui/features/invite-modal.js
c6a126f7c6d47024e4d50240b60eb3a6  tests/unit/invite-modal.test.mjs
7c833796965b617b9fe73c80b73a6ab2  /home/sanya/orca/project/waves/RW-40/G2/task-231-ui-invite-modal/red.log
88bdc3bfa436389a1202ea38a1395ff1  /home/sanya/orca/project/waves/RW-40/G2/task-231-ui-invite-modal/green.log
```

## Риски
- Существующий контракт `createInviteModal` для `join-server.test.mjs` и `app.js` полностью сохранён (`35/35 pass`).
- `git commit` не выполнялся согласно правилам воркера.
