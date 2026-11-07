# REVIEW-151A — TASK-236 (FE-P0-09) `shell-bridge.js`

- **Карточка**: `task-236`, отдел G2 Frontend
- **Ветка**: `feature/task-236-ui-shell-bridge`
- **Режим**: token-lean, targeted-проверка + mutation testing (без `gate:worker`, без `git commit`)
- **Ревьюер**: worker `term_3888caf0-d501-42bd-a3c2-3ec1020ddf1a`

## Verdict: APPROVED

## 1. Область проверки

| Файл | MD5 (факт) | MD5 в `REPORT.md` | Совпадение |
|---|---|---|---|
| `rust/gui-spike/ui/features/shell-bridge.js` | `3389f3aad9c5265135ba5876daea149d` | `3389f3aad9c5265135ba5876daea149d` | да |
| `tests/unit/shell-bridge.test.mjs` | `70f337e6960ec99be7d168d0029ecd82` | `70f337e6960ec99be7d168d0029ecd82` | да |

## 2. Targeted-прогон

`node --test tests/unit/shell-bridge.test.mjs` → exit `0`

```
tests 7 | suites 2 | pass 7 | fail 0 | skipped 0 | todo 0 | duration_ms 33.79
```

Покрыты: заморозка и порядок `SHELL_MODAL_LAYERS` (10 слоёв), `registerController`/`getController`
с валидацией `layerId`, `openLayer` с payload и взаимным исключением 5 модалок, LIFO
`closeTopLayer`/`closeAllLayers`, `handleGlobalKeyDown` (`Ctrl+K`/`Ctrl+F`/`Escape` через
`normalizeKeyCombo`), делегирование `onDispatchIpc`/`onNavigate`.

## 3. Mutation testing — 5/5 KILLED

Каждая мутация вносилась в `rust/gui-spike/ui/features/shell-bridge.js`, прогонялся targeted-тест,
затем файл восстанавливался из побайтовой копии с проверкой md5.

| # | Мутация | Exit | `ℹ fail` | MD5 мутанта | Итог |
|---|---|---|---|---|---|
| M1 | Перестановка `"context-menu"` ↔ `"threads-drawer"` в `SHELL_MODAL_LAYERS` | 1 | 1 | `b40221b848f4260b23b38489c60eb258` | KILLED |
| M2 | Удаление `"forward-modal"` из `EXCLUSIVE_MODAL_LAYERS` | 1 | 1 | `fc697db4c5e913a1f904cd92b2945d21` | KILLED |
| M3 | `closeTopLayer`: `pop()` → `shift()` (LIFO → FIFO) | 1 | 2 | `99c9c11c2c919ee828a204aa604ed30a` | KILLED |
| M4 | `Ctrl+F` открывает `"quick-switcher"` вместо `"search-panel"` | 1 | 1 | `9574a8ac074e1adc8cc84ef0e4e4164e` | KILLED |
| M5 | Отключение whitelist `layerId` в `assertValidLayerId` (`includes(...)` → `false`) | 1 | 1 | `deefbe6351d974866fa832100fadaa1e` | KILLED |

Итог: **mutations 5/5 (100%)**, порог `TEST-POLICY.md` выполнен. Ни одна мутация не выжила,
следовательно тесты проверяют контракт, а не текущую реализацию.

## 4. Восстановление исходника

```
restored md5 3389f3aad9c5265135ba5876daea149d (match=True)
mutations killed 5/5
```

Файл восстановлен побайтово, md5 совпадает с `REPORT.md` и с prerequisитом к мутациям.
Резервная копия: `/tmp/opencode/shell-bridge.orig.js`.

## 5. Замечания

Блокирующих (`P0`/`P1`) — **0**. Неблокирующих (`P2`) — 0.

Наблюдения (не требуют правок в этой карточке):
- `openLayer` для неисключающих слоёв удаляет существующую запись и пушит заново, но не вызывает
  `controller.close?.()` для промежуточного состояния — поведение соответствует контракту теста,
  однако при подключении к DOM стоит подтвердить, что повторное открытие панели не дублирует
  подписки (вынесено в интеграционную карточку, п. 4 `REPORT.md`).
- Подключение `createShellFeatureBridge` к корневому `app.js` вне scope T-236.

## 6. Границы

`gate:worker`, `cargo`, полный `node --test`, `git commit`/`push` не запускались — режим
token-lean и правила воркера соблюдены. Чужие файлы не изменялись.
