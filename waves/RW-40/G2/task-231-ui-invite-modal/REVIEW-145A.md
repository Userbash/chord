# REVIEW-145A — PR-116 / task-231 (T-231 FE-P1-24, `invite-modal.js`)

**Verdict: APPROVED**

## Проверка

- Targeted: `node --test tests/unit/invite-modal.test.mjs` → pass 8, fail 0, 0 skipped.
- MD5 до мутаций: `8e76c874751953ebf55f9bd093da8cbb`; в REPORT.md: `8e76c874751953ebf55f9bd093da8cbb` — совпадает: **да**.
- MD5 после восстановления: `8e76c874751953ebf55f9bd093da8cbb` — совпадает с REPORT.md: **да**.
- Повторный прогон после восстановления: pass 8, fail 0.
- Мутаций убито: **5/5**.

## Мутации (5/5)

| # | Мутация | pass/fail | Убита | Первый упавший тест |
|---|---------|-----------|-------|---------------------|
| M1 | formatInviteUrl не обрезает пробелы по краям | 6/2 | да | INVITE_MAX_AGE_OPTIONS_RU, INVITE_MAX_USES_OPTIONS_RU и formatInviteUrl |
| M2 | справочник INVITE_MAX_AGE_OPTIONS_RU: 604800 -> 604801 | 7/1 | да | INVITE_MAX_AGE_OPTIONS_RU, INVITE_MAX_USES_OPTIONS_RU и formatInviteUrl |
| M3 | фильтр друзей игнорирует displayName | 7/1 | да | createInviteModalController |
| M4 | кнопка друга остаётся активной после отправки (нет disabled) | 7/1 | да | createInviteModalController |
| M5 | close() не очищает контейнер от модалки | 7/1 | да | createInviteModalController |

## Замечания

- Тесты проверяют наблюдаемое поведение через DOM-шим (атрибуты, текст, колбэки, состояние), а не внутренности; каждая мутация из таблицы падает на конкретном тесте.
- Оракул независим от реализации: ожидаемые значения (`1800…604800`, `https://discord.gg/<code>`, русские подписи, `Отправлено`) заданы в тесте литералами.
- Существующий контракт `createInviteModal` для `join-server`/`app.js` сохранён: новые экспорты добавлены рядом со старыми.
- P3 (не блокирует): `btn.disabled = true` дублирует `setAttribute("disabled", "")`; guard `if (invitedUserIds.has(...) || btn.disabled) return` недостижим в тесте, потому что `FakeElement.click()` на disabled-кнопке не диспатчит событие. Поведение пользователя корректно, но guard остаётся непокрытым — при желании добавить прямой `dispatchEvent("click")` в тест.
- P0/P1 замечаний нет: сетевых вызовов, секретов и записи в golden-пути в диффе нет.
- `git commit` не выполнялся.
