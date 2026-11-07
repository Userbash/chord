# task-214 — UI channel gate (G2)

## Сделано
- `rust/gui-spike/ui/features/channel-gate.js` — чистые предикаты
  `evaluateChannelAgeGate(channel, acknowledgedChannelIds = [])` и
  `evaluateComposerLockState(channel)`, `// @ts-check` + JSDoc-типы.
- `tests/unit/channel-gate.test.mjs` — 10 тестов по контракту из ТЗ:
  NSFW-гейт и подтверждение по id, открытые случаи (`nsfw:false`, нет канала),
  блокировка ветки, обход для `canManageThreads`, отсутствие `canSendMessages`,
  подстановка имени, заглушка `#канал`, приоритет блокировки ветки.

## Red (до реализации)
`node --test tests/unit/channel-gate.test.mjs` — падение: модуль
`channel-gate.js` отсутствует (`ERR_MODULE_NOT_FOUND`), лог `red.log`.

## Green (после реализации)
`node --test tests/unit/channel-gate.test.mjs` — `tests 10 / pass 10 / fail 0`,
exit 0, лог `green.log`.

## Файлы и md5
```
e523199dc9a77608e58b65fa8538e723  rust/gui-spike/ui/features/channel-gate.js
7b34389929079db6074eb34c52521744  tests/unit/channel-gate.test.mjs
```

## Риски и ограничения
- Контракт взят дословно из ТЗ; сверка с RE-дампом и i18n-ключами не выполнялась
  (token-lean режим), строки пока захардкожены по-русски.
- Полный `gate:worker` не запускался; проверен только целевой тест-файл.
- `git commit` не делался.
