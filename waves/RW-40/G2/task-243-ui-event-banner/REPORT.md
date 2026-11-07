# task-243 — UI event banner (G2)

Карточка: task-243-ui-event-banner (TOKEN-LEAN, 2 файла, чистый JS).

## Сделано
- `tests/unit/event-banner.test.mjs` — red-first, 11 тестов на контракт.
- `rust/gui-spike/ui/features/event-banner.js` — `// @ts-check`, только чистые функции: `EVENT_ENTITY_LABELS_RU`, `EVENT_STATUS_LABELS_RU` (оба `Object.freeze`), `formatEventSummaryRu`, `toggleEventRsvpState`.

## Red -> Green
- Red: `node --test tests/unit/event-banner.test.mjs` -> exit 1, `ERR_MODULE_NOT_FOUND` (модуль ещё не существует). Лог: `red.log`.
- Green: тот же прогон -> exit 0, `tests 11 / pass 11 / fail 0`. Лог: `green.log`.

## Поведение контракта
- `title`: trimmed `name` либо `"Событие без названия"`.
- `entityLabel`: `EVENT_ENTITY_LABELS_RU[entityType] || "Неизвестно"`.
- `statusLabel`: `EVENT_STATUS_LABELS_RU[status] || "Запланировано"`.
- `interestedLabel`: `"Интересуются: N"`, N = `Math.max(0, Math.trunc(Number(userCount) || 0))`.
- `isActive`: строго `status === 2`.
- `toggleEventRsvpState`: инвертирует интерес и сдвигает счётчик на ±1 с зажимом в 0.

## Файлы и md5
```
235ec0e4c33062b2c8928a99fc9defd3  rust/gui-spike/ui/features/event-banner.js
08e92860bfc7b01f99e61115125c3f08  tests/unit/event-banner.test.mjs
```

## Риски / что осталось
- `gate:worker`, `cargo`, `git commit` не запускались по ТЗ (token-lean) — полный гейт за integration-лейном.
- UI-подключение баннера (DOM/WCAG) вне scope этой карточки: здесь только слой представления.
- `location` принимается в typedef, но в выводе пока не используется — по ТЗ отдельного поля нет.
