# REVIEW-67A: task-153 (FE-P1-01: Live DOM wiring C17..C22 in dispatch-view.js)

- **Карточка:** task-153 (FE-P1-01 / RW-40 / G2 Frontend)
- **Ревьюер:** Технический ревьюер (Review 67A)
- **Worktree:** `/home/user/orca/workspaces/project/feature-task-153-dispatch-view-live-dom`
- **Файлы под ревью:**
  - `rust/gui-spike/ui/features/dispatch-view.js` (md5: `17582e7a211b45d3b35866e64d46e0d0`)
  - `tests/unit/dispatch_ui.test.mjs` (md5: `b799592654fe99ef371d20a548b28681`)

---

## 1. Проверка тестов и типов

- `node --test tests/unit/dispatch_ui.test.mjs`:
  - **57 passed, 0 failed, 0 skipped** (4 suites, длительность ~380ms).
- `npx tsc -p tsconfig.json`:
  - **0 ошибок** (exit 0).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json`:
  - **0 ошибок** (exit 0).

---

## 2. Проверка сценариев C17..C22 и обратной совместимости C1..C16

- **C17 (MESSAGE_CREATE):**
  - При активном канале новое сообщение корректно рендерится в `#message-list` / `#chat-list` через `renderChannelMessage` без перезапроса истории (`messages_page` не вызывается).
  - При неактивном канале выставляются класс `is-unread` и атрибуты `data-unread`, а также инкрементируется `data-mention-count` со значком `.channel-badge`.
  - Входящее сообщение гасит активный статус тайпинга автора.
- **C18 (MESSAGE_UPDATE):**
  - Обновляет текст сообщения (`.text-body`), проставляет отметку `.edited` (`(изменено)`), дополняет `data-extras` и обновляет `aria-label`.
- **C19 (MESSAGE_ACK):**
  - Снимает `is-unread`, очищает `data-unread`, `data-mention-count` и удаляет значок `.channel-badge` на плитке канала. В активном канале скрывает разделитель непрочитанных.
- **C20 (TYPING_START):**
  - Активирует `#typing-indicator` (`data-visible="true"`, счётчик и список юзеров), проставляет `data-typing-users` на композере и чат-контейнере.
  - При приходе `MESSAGE_CREATE` от этого пользователя тайпинг корректно очищается.
- **C21 (VOICE_STATE_UPDATE / VOICE_SERVER_UPDATE):**
  - Подсчитывает участников голоса, обновляет `data-voice-participants`, `data-stream-status`, `data-self-stream` на `#voice-panel` и плитке канала.
  - `VOICE_SERVER_UPDATE` обновляет `data-endpoint` и статус `data-voice-server="connected"`.
- **C22 (THREAD_LIST_SYNC):**
  - Корректно фильтрует активные (не архивированные) ветки, обновляет `[data-action="threads"]` (`data-thread-count`), дерево `#channel-tree` (`data-active-threads`) и атрибуты `data-thread-count` родительских каналов.
- **C1..C16:**
  - Все предыдущие сценарии (READY, LIST_UPDATE, CHUNK, PRESENCE, REACTIONS ADD/REMOVE, снапшоты, безопасность токенов/секретов) полностью сохранены и проходят.

---

## 3. Мутационное тестирование (M1..M5)

В файле `rust/gui-spike/ui/features/dispatch-view.js` проведены 5 независимых мутаций:

1. **M1 (applyMessageCreate / C17):**
   - *Мутация:* удаление `list.append(row);` при создании новой строки сообщения.
   - *Результат:* **УБИТ** тестом C17 (`AssertionError: новая строка сообщения не добавлена в #message-list / #chat-list, 1 !== 2`).
2. **M2 (applyMessageUpdate / C18):**
   - *Мутация:* отключение обновления текста сообщения `if (textBody) textBody.textContent = change.content;`.
   - *Результат:* **УБИТ** тестом C18 (`AssertionError: текст сообщения не обновился, 'строка для live-реакций' !== 'обновлённый текст сообщения'`).
3. **M3 (applyMessageAck / C19):**
   - *Мутация:* удаление `tile.classList.remove("is-unread");`.
   - *Результат:* **УБИТ** тестом C19 (`AssertionError: is-unread остался на плитке канала, true !== false`).
4. **M4 (applyVoiceState / C21):**
   - *Мутация:* искажение подсчёта голосовых участников `const count = guildMap.size + 1;`.
   - *Результат:* **УБИТ** тестом C21 (`AssertionError: счётчик голосовых участников не обновился до 1, '2' !== '1'`).
5. **M5 (applyThreads / C22):**
   - *Мутация:* удаление обновления кнопки тредов `threadsBtn.setAttribute("data-thread-count", String(totalActive));`.
   - *Результат:* **УБИТ** тестом C22 (`AssertionError: счётчик тредов не сбросился до 0, '1' !== '0'`).

**Итог мутаций:** 5/5 мутантов убиты целевыми тестами.
Файл `rust/gui-spike/ui/features/dispatch-view.js` восстановлен в исходное состояние, контрольная сумма md5 подтверждена (`17582e7a211b45d3b35866e64d46e0d0`).

---

## 4. Вердикт

- **Уровень дефектов:** P0=0, P1=0, P2=0, P3=0
- **Статус:** **PASS / Ready for Merge**
