# Отчёт: Воркер G2 Frontend Integration Lane A (RW-40, карточка INT-02-UI-CHAT-MARKDOWN-TYPING)

Дата: 2026-10-30.  
Роль: Воркер G2 Frontend Integration Lane A (RW-40).  
Карточка: `INT-02-UI-CHAT-MARKDOWN-TYPING`.  
Назначенные файлы:
- `rust/gui-spike/ui/features/chat.js`
- `rust/gui-spike/ui/components/composer.css`
- `tests/unit/gui-spike-model.test.mjs`

## 1. Статус

**done (frontend integration, offline).** Подключён безопасный рендер `renderMessageMarkdown` из `./markdown.js` для текста сообщений в `rust/gui-spike/ui/features/chat.js` (`renderChatRow`, `renderChannelMessage`, `appendLocalMessage`) с сохранением всех существующих классов и атрибутов строк чата (`DOM-CONTRACT §7.1`), а также добавлены токенизированные стили `.md-code`, `.md-code-block`, `.md-quote`, `.md-link`, `.md-spoiler`, `.mention` в `rust/gui-spike/ui/components/composer.css`.

## 2. Что сделано

1. **`rust/gui-spike/ui/features/chat.js`**:
   - Импортирован `renderMessageMarkdown` из `./markdown.js`.
   - В `renderChatRow(row, item, index, context)` заполнение `.text > .text-body` переведено на `renderMessageMarkdown(body, item.text, context ?? item.context)` с сохранением структуры строки (`.author` + `.text`, `.text-body` первым ребёнком, `data-group`, `data-index`, `data-message-id`, `aria-setsize`, `aria-posinset`, `aria-label` и все extras).
   - В `renderChannelMessage(row, item, index, total, options)` заполнение `.text > .text-body` переведено на `renderMessageMarkdown(body, item.text, options.context ?? item.context)` с сохранением `white-space: pre-wrap`, `data-channel-message="true"`, `data-message-id`, разделителей дня, отметки `(изменено)` и пилюль реакций.
   - В `appendLocalMessage(message)` отправка локальной строки также рендерит текст через `renderMessageMarkdown`.
2. **`rust/gui-spike/ui/components/composer.css`**:
   - Добавлены токенизированные правила `.md-code`, `.md-code-block`, `.md-code-block .md-code`, `.md-quote`, `.md-link`, `.md-spoiler` (включая `[data-spoiler="hidden"]` и `[data-spoiler="revealed"]`), `.mention` и отключение переходов при `prefers-reduced-motion: reduce`.
   - Все цвета, отступы, радиусы и типографика привязаны к `var(--token)` из `rust/gui-spike/ui/tokens.css` (без raw hex/rgb/hsl и без `!important`).
3. **`tests/unit/gui-spike-model.test.mjs`**:
   - Существующие 48 тестов (строки 1–860) не изменялись.
   - В конец файла добавлен describe-блок `"INT-02: интеграция renderMessageMarkdown в строки чата и стили composer.css"` (5 интеграционных тестов).

## 3. Red → Green → Targeted (TDD Evidence)

- **RED**: `node --test tests/unit/gui-spike-model.test.mjs` → **exit 1**, `tests 53 / pass 48 / fail 5` (`waves/RW-40/PM/int02-chat-md/red.log`). Все 5 новых интеграционных тестов упали до внесения продуктового кода.
- **GREEN**: `node --test tests/unit/gui-spike-model.test.mjs` → **exit 0**, `tests 53 / pass 53 / fail 0` (`waves/RW-40/PM/int02-chat-md/green.log`).
- **TARGETED + Регрессии чата + Typecheck**:
  - `node --test tests/unit/gui-spike-model.test.mjs tests/unit/read-state-ui.test.mjs tests/unit/channel-paging.test.mjs tests/unit/channel-switch.test.mjs tests/unit/chat-reactions.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-regressions.test.mjs tests/meta/test-policy.test.mjs` → **exit 0**, `tests 272 / pass 272 / fail 0`.
  - `npx tsc -p tsconfig.json` → **exit 0** (0 ошибок, 0 `@ts-ignore`).
  - Лог: `waves/RW-40/PM/int02-chat-md/targeted.log`.

## 4. Мутационное тестирование (5 мутаций)

Лог: `waves/RW-40/PM/int02-chat-md/mutations.log` — **5/5 CAUGHT**, MD5 восстановлен после каждой мутации:

| Мутация | Суть изменения | Статус | Убивший тест |
|---|---|---|---|
| `M1` | В `renderChatRow` замена `renderMessageMarkdown(...)` на `body.textContent = item.text` | **CAUGHT** | `renderChatRow: рендерит markdown внутри .text-body…`, `renderChatRow: блоки кода…`, `renderChatRow: блокирует XSS…` |
| `M2` | В `renderChatRow` сброс передачи `context` в `renderMessageMarkdown(body, item.text)` | **CAUGHT** | `renderChatRow: блоки кода, цитаты, спойлеры и упоминания в строке чата` |
| `M3` | В `renderChannelMessage` замена `renderMessageMarkdown(...)` на `body.textContent = item.text` | **CAUGHT** | `renderChannelMessage: рендерит markdown, спойлеры и упоминания в реальном канале…` |
| `M4` | В `composer.css` замена селектора `.md-spoiler[data-spoiler="hidden"]` | **CAUGHT** | `composer.css: токенизированные стили .md-code, .md-code-block, .md-spoiler, .mention без hex/rgb` |
| `M5` | В `composer.css` подстановка raw hex `#5865f2` вместо `var(--accent-soft)` в `.mention` | **CAUGHT** | `composer.css: токенизированные стили .md-code, .md-code-block, .md-spoiler, .mention без hex/rgb` |

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/chat.js` | `d2e3bb0958fbafb44dc14fe72cee0426` |
| `rust/gui-spike/ui/components/composer.css` | `d62aa5c55b033f3fe7da1708e151d717` |
| `tests/unit/gui-spike-model.test.mjs` | `0eee4284af263c3d412b0afe4910bd58` |
| `waves/RW-40/PM/int02-chat-md/red.log` | `2a8356b8b44033d314f139a8c45b88d1` |
| `waves/RW-40/PM/int02-chat-md/green.log` | `d2860b21c89730c19293b9ea7613d458` |
| `waves/RW-40/PM/int02-chat-md/targeted.log` | `71d49c1de8176862dd9c51632ed2c6f7` |
| `waves/RW-40/PM/int02-chat-md/mutations.log` | `9c40fc8a350769faa5b5803895470fbe` |

## 6. Риски и наблюдения

- Чужие файлы вне файловой матрицы не затрагивались.
- Все существующие тесты чата, пагинации (`channel-paging`, `channel-switch`), реакций (`chat-reactions`), состояния прочтения (`read-state-ui`), DOM и модели проходят без изменений.
