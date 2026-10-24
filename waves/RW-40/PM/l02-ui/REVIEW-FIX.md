# Повторное ревью L-02-UI-FIX

Дата: 2026-10-24. Режим: read-only review целевого кода; записан только этот отчёт.

## Вердикт

**RETURN** — обе P2 исправлены, targeted проверки проходят, но приёмка блокируется отсутствием mutation evidence для новых инвариантов (TEST-POLICY §3.6 и §9).

## Проверка P2

- **P2: пустой READY оставлял плитки рейла. Закрыто.** В [dispatch-model.js](/home/sanya/orca/project/rust/gui-spike/ui/features/dispatch-model.js:493) первый пустой READY не считается duplicate без предыдущего dispatch snapshot; код сохраняет `source="dispatch"` и возвращает `guilds: []` в [строках 505–517](/home/sanya/orca/project/rust/gui-spike/ui/features/dispatch-model.js:505). В [app.js](/home/sanya/orca/project/rust/gui-spike/ui/app.js:626) `applyGuilds([])` вызывает `renderEmpty()`. Тесты B16/B16b и C15/C15b проверяют очистку store, обычный и первый пустой READY, сохранение add-плитки и повторную идемпотентность: [dispatch_ui.test.mjs](/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs:457), [DOM cases](/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs:1259).
- **P2: unsafe numeric self ID терял точность. Закрыто.** [asSnowflake](/home/sanya/orca/project/rust/gui-spike/ui/features/dispatch-model.js:81) разрешает только строки и safe integers. READY fallback использует её в [dispatch-model.js](/home/sanya/orca/project/rust/gui-spike/ui/features/dispatch-model.js:488), а IPC `ready()` применяет только непустой результат в [app.js](/home/sanya/orca/project/rust/gui-spike/ui/app.js:1189). B17/C16 подтверждают отказ unsafe числа, точное сохранение строкового snowflake и отсутствие ложного `me`: [dispatch_ui.test.mjs](/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs:486), [C16](/home/sanya/orca/project/tests/unit/dispatch_ui.test.mjs:1321).

## Evidence и блокер

- `node --test tests/unit/dispatch_ui.test.mjs` → exit 0, 43 passed / 0 failed / 0 skipped (воспроизведено при ревью).
- `fix-red.log` и `fix-red2.log` содержат падения целевых assert; `fix-green.log` содержит 43/43, `fix-targeted-ui.log` — 347/347, `fix-tsc.log` присутствует.
- Текущие md5 совпадают с FIX-REPORT: `dispatch-model.js` `37dacc61d7daf74c144681637dded1f5`; `app.js` `1ac5c36976a064120529c132e61cfb54`; `dispatch_ui.test.mjs` `f2806f3f4088b0527f6a31ff1d7bce2a`.
- Mutation-запуска/лога для новых инвариантов в каталоге карточки нет. Согласно TEST-POLICY §3.6 и §9, без caught-мутантов эти новые проверки не готовы к приёмке. Предъявить mutation evidence и повторить review.
- Полный gate, cargo и live не запускались; для данной UI fix-проверки это отдельно зафиксировано в FIX-REPORT.
