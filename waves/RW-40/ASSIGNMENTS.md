# RW-40 — ASSIGNMENTS: дизайн / ядро / сеть / frontend / backend

Дата: 2026-10-24. PM: DeepSeek. Каждая функция разложена по командам; один owner на слой.
Порядок — очередь исполнения; зависимости указаны.

## 1. Матрица по функциям

### Реакции (B3)

| Слой | Задача | Owner | Выход | Зависит |
|---|---|---|---|---|
| Design | интеракции: hover, picker, счётчик, свои/чужие, ошибки | **G3 Design** | `DESIGN-MSG.md` §Reactions | — |
| Core | `ApiClient::put/delete` | **G1 Core** | `rest/client.rs` + тесты | — |
| Backend | `toggle_reaction`, валидация emoji/id, 401/429 | **G1 Backend** | `reactions.rs`, `main.rs` | Core |
| Frontend | клик ставит/снимает, optimistic-счётчик, retry | **G2 Frontend** | `reactions.js`, `chat.js`, tests | Backend, Design |
| QA | PUT/DELETE, 401/429, двойной клик, счётчик | **G4 QA** | `QA/REACTIONS.md` | Frontend |

### Вложения (B2)

| Слой | Задача | Owner | Выход |
|---|---|---|---|
| Design | attach/dropzone/progress/preview/ошибки | G3 | `DESIGN-MSG.md` §Attachments |
| Network | multipart upload, лимиты, mime | G1 | `messages.rs` upload, core client |
| Backend | `upload_attachment`, валидация размера/типа | G1 | `messages.rs`, `main.rs`, tests |
| Frontend | drag&drop, прогресс, превью, отмена | G2 | `composer.js`, `chat.js`, CSS |
| QA | размер/тип/сеть/отмена | G4 | `QA/ATTACHMENTS.md` |

### Меню сообщения (B4)

| Слой | Задача | Owner | Выход |
|---|---|---|---|
| Design | 15 пунктов R-16, disabled-правила, confirm | G3 | `DESIGN-MSG.md` §Menu |
| Network | edit/delete/reply API | G1 | routes, `messages.rs` |
| Backend | `edit_message`, `delete_message`, `reply_message` | G1 | `messages.rs`, tests |
| Frontend | меню, inline-edit, confirm, optimistic | G2 | `message-toolbar.js`, `chat.js` |
| QA | права/ошибки/отмена | G4 | `QA/MESSAGE-ACTIONS.md` |

### Треды/пины (C1)

| Слой | Задача | Owner |
|---|---|---|
| Design | панель треда, список пинов | G3 |
| Network | thread/pins endpoints | G1 |
| Backend | `thread_page`, `pins_page` | G1 |
| Frontend | панель, переходы | G2 |
| QA | навигация/ошибки | G4 |

### Участники (B5)

| Слой | Задача | Owner |
|---|---|---|
| Design | группы по ролям/статусу, header 42 | G3 |
| Backend | roles/groups/search API | G1 |
| Frontend | группы, полный поиск, счётчики | G2 |
| QA | группы/поиск/пустые | G4 |

### QuickSwitcher / поиск (B6/B7)

| Слой | Задача | Owner |
|---|---|---|
| Design | оверлей, 8 результатов, клавиши | G3 |
| Network | серверный поиск 7 фильтров | G1 |
| Frontend | QuickSwitcher, overlay, jump | G2 |
| QA | фильтры/сортировка/переход | G4 |

### Настройки/темы/i18n (B8/C3/C4)

| Слой | Задача | Owner |
|---|---|---|
| Design | light/cozy токены, разделы | G3 |
| Core | store/kv, i18n runtime | G1/Core |
| Backend | сохранение настроек | G1 |
| Frontend | сохранение/темы/локаль | G2 |
| QA | persist/перезапуск | G4 |

### Голос/ОС (C5/C6)

| Слой | Задача | Owner |
|---|---|---|
| Design | устройства/PTT/статистика | G3 |
| Core/Platform | voice engine, tray, notifications, updater | G5 |
| Frontend | voice UI, tray/badges | G2 |
| QA | устройства/уведомления | G4 |

### Тесты/релиз (D1–D5)

| Слой | Задача | Owner |
|---|---|---|
| QA | e2e fixture, QA-матрица, coverage | G4 |
| Core | COV-01…05 | G1 |
| PM | live READY/send, merge gate | PM |

## 2. Ответственные (RACI-сводка)

| Эпик | Design | Core | Network/Backend | Frontend | QA |
|---|---|---|---|---|---|
| Реакции | G3 R | G1 R | G1 R | G2 R | G4 A |
| Вложения | G3 R | G1 C | G1 R | G2 R | G4 A |
| Меню сообщения | G3 R | G1 C | G1 R | G2 R | G4 A |
| Треды/пины | G3 R | G1 C | G1 R | G2 R | G4 A |
| Участники | G3 C | G1 C | G1 R | G2 R | G4 A |
| QuickSwitcher/поиск | G3 R | G1 C | G1 R | G2 R | G4 A |
| Настройки/темы/i18n | G3 R | G1 R | G1 R | G2 R | G4 A |
| Голос/ОС | G3 R | G5 R | G5 R | G2 R | G4 A |
| Тесты/релиз | I | C | C | C | G4 R (PM A) |

## 3. Очередь старта

1. **B3a** — backend реакций (in work).
2. **G3 Design** — `DESIGN-MSG.md` (reactions/attachments/menu/threads) — **start now**.
3. **B3b** — UI реакций (после B3a+design).
4. **B4a** — edit/delete/reply backend.
5. **B2a** — upload backend.
6. **B5a** — member groups backend.
7. **B6** — QuickSwitcher (frontend, параллельно при свободном G2).
8. **B7** — server search.
9. **B8/B9/C3/C4** — settings/system/themes/i18n.
10. **C5/C6** — voice/OS.
11. **D1–D5** — QA/live/merge.
