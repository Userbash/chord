# TASK-200 — UI voice quality badge (G2)

## Сделано
- `tests/unit/voice-quality.test.mjs` — 10 тестов (red → green), покрывают unknown/числовые края, границы полос 100/250 мс и 2/10 %, клампы битрейта 8..512 кбит/с, бакеты разрешения и FPS, склейку бейджа.
- `rust/gui-spike/ui/features/voice-quality.js` — `classifyRtcConnectionQuality`, `formatAudioBitrateRu`, `formatStreamBadgeRu`, `// @ts-check` + JSDoc.

## Контракт
- Полосы качества проверяются по порядку: `100 && <=2` → good, иначе `250 && <=10` → average, иначе poor. Поэтому 100 мс с 3 % потерь — average, а не poor.
- `formatAudioBitrateRu`: NaN → нижний кламп, `Infinity` → верхний.

## Прогоны
- red: `red.log` — модуль отсутствовал, suite падал (ERR_MODULE_NOT_FOUND).
- green: `green.log` — tests 10, pass 10, fail 0.
- Полный `gate:worker` не запускался (token-lean режим ТЗ).

## Файлы
MD5 будет добавлен ниже.

```
6a5ba080de177bc9c09c6fb9f6670d17  rust/gui-spike/ui/features/voice-quality.js
aab455dfe4b28d97d1eb446714bbcd3f  tests/unit/voice-quality.test.mjs
```

## Риски
- Бейдж и качество — чистые форматтеры без DOM-монтажа; в UI пока не подключены (вне write-scope задачи).
