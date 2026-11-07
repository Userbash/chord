# REPORT-PASS4 — INT-03-SAVE-QUEUE-FIX (Воркер G2 Frontend Lane B)

- **Дата:** 2026-10-30.
- **Роль:** Воркер G2 Frontend Integration Lane B (RW-40, карточка INT-03-SAVE-QUEUE-FIX / G-01b-UI Pass 4).
- **Основание:**
  1. `waves/RW-40/PM/g01b-ui/REVIEW-PASS3.md` (P1 — устаревший `settings_save` для первого выбора A мог завершиться позже нового сохранения для B при одновременных/отложенных IPC-запросах; требование отдельного артефакта `targeted-pass4.log` с явными кодами выхода `exit_code=0` для 98 targeted тестов и обоих запусков `tsc`).
- **Назначенные файлы (строго только они):**
  - `rust/gui-spike/ui/app.js`
  - `tests/unit/guild-rail.test.mjs`
- **Статус:** Strict TDD (red → green), targeted-прогоны (98 тестов, exit 0), оба `tsc` (exit 0) и 5 мутаций (5/5 KILLED) с верификацией восстановления baseline MD5 завершены. Коммитов нет.

---

## 1. Проблема и контекст (Review Pass 3 P1)

В Pass 3 гонка выбора гильдий при быстрых кликах A -> B была закрыта проверкой поколения `latestGuildId` после асинхронного `settings_load`. Однако оставался сценарий гонки при задержке самого IPC `settings_save`:
- Если запрос A уже прошёл проверку `settings_load` и начал `invoke("settings_save")`, а пользователь в этот момент выбрал B, сохранение B без сериализации стартовало параллельно.
- Если сохранение B завершалось быстрее, чем задержанный ответ `settings_save` от первого клика A, запоздалый ответ A мог зафиксировать устаревшее состояние на диске/в хранилище.
- В тестах Pass 3 моделировалась только задержка `settings_load`, но не задержанное завершение уже начатого `settings_save`.

---

## 2. Strict TDD (Red-first)

В `tests/unit/guild-rail.test.mjs` добавлен целевой red-тест с моделированием задержки завершения промиса `settings_save` для гильдии A:
- **Тест:** `быстрые клики серверов A -> B с задержкой завершения settings_save от A сохраняют итоговый выбор B`.
- **Сценарий:**
  1. Boot приложения с перехватом `settings_load` и `settings_save`.
  2. Клик по плитке сервера A (`tileA.click()`): `settings_load` отдаёт данные сразу, первый вызов `settings_save` задерживается в промисе `firstSaveDeferred` (фиксируется событие `start:A`).
  3. Клик по плитке сервера B (`tileB.click()`) до разрешения промиса сохранения A.
  4. Разрешение задержанного промиса сохранения A (`resolveFirstSave()`) и стабилизация microtasks (`await app.settle(6)`).
  5. Проверка инвариантов:
     - Итоговый `currentStore.lastGuildId` равен `targetB`.
     - События сохранения выполняются в строгом порядке очереди: `['start:A', 'finish:A', 'start:B', 'finish:B']` (устаревший `settings_save` от A никогда не завершается после B).

### Red-прогон
```bash
node --test tests/unit/guild-rail.test.mjs > waves/RW-40/PM/g01b-ui/red-pass4.log 2>&1
```
- **Exit code:** 1.
- **Ошибка:**
  ```text
  ✖ быстрые клики серверов A -> B с задержкой завершения settings_save от A сохраняют итоговый выбор B
    AssertionError [ERR_ASSERTION]: гонка при отложенном settings_save от A: сохранён 294064825540280335, ожидался последний выбор 338001249918255106
    + actual - expected
    + '294064825540280335'
    - '338001249918255106'
  ```
- **Артефакт:** `waves/RW-40/PM/g01b-ui/red-pass4.log`.

---

## 3. Реализация (Green)

В `rust/gui-spike/ui/app.js`:
1. Введена сериализующая очередь сохранения настроек:
   ```javascript
   let latestGuildId = "";
   /** Очередь сохранения настроек: сериализует чтение и запись UiSettings. */
   let saveQueue = Promise.resolve();
   ```
2. Функция `rememberGuild(id)` переведена на строго последовательную обработку:
   ```javascript
   async function rememberGuild(id) {
     latestGuildId = id;
     preferredGuildId = id;
     settingsRef?.setLastGuildId(id);

     saveQueue = saveQueue
       .catch(() => {})
       .then(async () => {
         if (latestGuildId !== id) return;
         try {
           const raw = await invoke("settings_load");
           if (latestGuildId !== id) return;
           preferredGuildId = id;
           settingsRef?.setLastGuildId(id);
           await invoke("settings_save", { settings: { ...normalizeSettings(raw), lastGuildId: id } });
         } catch (error) {
           if (latestGuildId === id) {
             // Последствие видимое: выбор уже применён в UI, но в файл не попал.
             surface(error);
           }
         }
       });
     return saveQueue;
   }
   ```
3. Гарантии архитектурного решения:
   - **Строгая сериализация:** каждый новый вызов `rememberGuild` через цепочку `.then()` дожидается завершения предыдущего in-flight `settings_save` перед тем, как читать (`settings_load`) и писать (`settings_save`) файл настроек.
   - **Исключение гонки завершений:** устаревший `settings_save` физически не может завершиться позже нового, так как новый запрос даже не начинает сохранение до завершения предыдущего.
   - **Оптимизация промежуточных кликов:** проверка `if (latestGuildId !== id) return;` на входе в задачу очереди отсекает промежуточные клики (например, A -> B -> C), не выполняя лишних дисковых операций для устаревшего выбора B.
   - **Устойчивость очереди:** префикс `.catch(() => {})` в цепочке гарантирует, что падение предыдущего IPC-запроса не сломает выполнение последующих сохранений.

---

## 4. Green-прогон и Targeted-тесты

### Green-прогон целевого набора
```bash
node --test tests/unit/guild-rail.test.mjs > waves/RW-40/PM/g01b-ui/green-pass4.log 2>&1
```
- **Exit code:** 0.
- **Результат:** 16 passed, 0 failed.
- **Артефакт:** `waves/RW-40/PM/g01b-ui/green-pass4.log`.

### Полный targeted-набор и Type Check (отдельный артефакт)
В `waves/RW-40/PM/g01b-ui/targeted-pass4.log` зафиксированы команды, полные логи и явные exit codes:
```bash
# 1. Targeted suite (4 test files):
node --test tests/unit/guild-rail.test.mjs tests/unit/server-menu.test.mjs tests/unit/join-server.test.mjs tests/unit/channel-paging.test.mjs
# 98 passed, 0 failed, EXIT_CODE: 0

# 2. Root TypeScript check:
npx tsc -p tsconfig.json
# EXIT_CODE: 0

# 3. UI TypeScript check:
npx tsc -p rust/gui-spike/ui/tsconfig.json
# EXIT_CODE: 0
```
- **Артефакт:** `waves/RW-40/PM/g01b-ui/targeted-pass4.log`.

---

## 5. Мутационное тестирование (Mutation Proof)

Проведено мутационное тестирование с автоматической проверкой восстановления исходного MD5 после каждого мутанта:
- **Baseline MD5:**
  - `rust/gui-spike/ui/app.js`: `8f5d84c14dc6e9aea749a202ecb3a96b`
  - `tests/unit/guild-rail.test.mjs`: `76a0acca4dcc95ac21ec1d7f90370a32`

| ID | Цель / Описание | Мутант MD5 | Результат | Убивший тест |
|---|---|---|---|---|
| **M1** | `app.js`: Снятие сериализации `saveQueue` (`Promise.resolve().then(...)` без ожидания предыдущего save) | `fa0191badf7fd479cf8e5493d68c264f` | **KILLED** (exit 1) | `быстрые клики серверов A -> B с задержкой завершения settings_save от A сохраняют итоговый выбор B` |
| **M2** | `app.js`: Инвертирование проверки токена поколения `if (latestGuildId === id) return;` на входе в задачу очереди | `c9bd121b8b19d862eba8db357ae0d793` | **KILLED** (exit 1) | `клик по плитке сохраняет lastGuildId, не теряя остальные настройки` |
| **M3** | `app.js`: Потеря существующих настроек при сохранении (пропуск `normalizeSettings(raw)`) | `f3f0b6d55e1814a5ece5d04a7ae1dcd9` | **KILLED** (exit 1) | `клик по плитке сохраняет lastGuildId, не теряя остальные настройки` |
| **M4** | `app.js`: Искажение payload `lastGuildId: ""` вместо `id` | `8b3b877965734025193317f539fb793c` | **KILLED** (exit 1) | `клик по плитке сохраняет lastGuildId, не теряя остальные настройки` |
| **M5** | `app.js`: Потеря экспорта `pinsThreads: null` из `currentApp()` | `1c83ba3cc43da98e0c01867fc5897328` | **KILLED** (exit 1) | `STUB-10 / STUB-05: currentApp() экспортирует рабочие контроллеры pinsThreads и inviteModal` |

Все 5 мутантов уничтожены (**5/5 KILLED**, survivors: 0). После каждого мутанта выполнено восстановление и сверен MD5 (`match: OK`).
Подробный протокол: `waves/RW-40/PM/g01b-ui/mutations-pass4.log`.

---

## 6. Итоговые контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/app.js` | модифицирован | `8f5d84c14dc6e9aea749a202ecb3a96b` |
| `tests/unit/guild-rail.test.mjs` | модифицирован | `76a0acca4dcc95ac21ec1d7f90370a32` |
| `waves/RW-40/PM/g01b-ui/red-pass4.log` | создан | `3ff6b3351ba249dfe1d6057615ee2bf5` |
| `waves/RW-40/PM/g01b-ui/green-pass4.log` | создан | `8cf57dfc620b335495a5b1590a4a78cd` |
| `waves/RW-40/PM/g01b-ui/targeted-pass4.log` | создан | `dc9b2849884853eb4e663cad4564d730` |
| `waves/RW-40/PM/g01b-ui/mutations-pass4.log` | создан | `d0162e673aa757b6a1c318c7a2cc1f23` |
| `waves/RW-40/PM/g01b-ui/REPORT-PASS4.md` | создан | `waves/RW-40/PM/g01b-ui/REPORT-PASS4.md` |

---

## 7. Риски и выводы

- **Архитектурные риски:** отсутствуют. Изменения локализованы строго в `rememberGuild` в `rust/gui-spike/ui/app.js` и тестовом сценарии в `tests/unit/guild-rail.test.mjs`. Чужие модули и тесты не затрагивались.
- **Надёжность очереди:** очередь промисов гарантирует отсутствие параллельных несогласованных записей в файл настроек `settings.json`, предотвращая перезапись более свежего выбора устаревшим задержанным запросом.
- Замечания `REVIEW-PASS3.md` (P1 и требования к артефактам) полностью устранены. Карточка **INT-03-SAVE-QUEUE-FIX / G-01b-UI Pass 4** готова к финальной приёмке.
