# Отчёт воркера: TASK-627 (`G2 Frontend` — Interactive Nitro Gift Picker Modal & Customization Flow)

## 1. Карточка и контекст
- **Волна / Отдел:** `RW-40` / `G2 Frontend`
- **Задача:** `TASK-627` (`task_f557913900d9`, dispatch `ctx_bf1d5aac4517`)
- **Ветка:** `feature/task-627-gift-modal-ui`
- **Назначение:** Реализация интерактивного модального окна выбора и кастомизации подарка Discord Nitro (`#gift-modal`) в `rust/gui-spike/ui/features/gift-modal.js`, таблицы стилей `rust/gui-spike/ui/styles/gift-modal.css` и полного набора модульных тестов `tests/unit/gift-modal.test.mjs`.

## 2. Сверка с эталонным RE-дампом (`reference-oracle/`) и спецификациями
Все константы, структуры запросов, коды ошибок, лимиты и состояния UI сверены с эталонным RE-оракулом и контрактом `TASK-626`:
1. **Каталог SKU, планов подписки и стилей открыток (`reference-oracle/decoded/4d2a64cb5df5fc4e_0`, модуль `202541`, `@6789126..@6797000`):**
   - `NITRO_APPLICATION_ID = "521842831262875670"` (экспорт `TV`).
   - 6 канонических `GIFT_SKU_IDS` (`NONE`, `NITRO_BASIC_TIER_0`, `NITRO_CLASSIC_TIER_1`, `NITRO_STANDARD_TIER_2`, `GUILD_BOOST`, `NITRO_LEGACY`, экспорт `pe`).
   - 10 канонических планов `GIFT_SUBSCRIPTION_PLANS` / `DEFAULT_NITRO_GIFT_PLANS` (`NITRO_BASIC_MONTH/YEAR`, `NITRO_CLASSIC_MONTH/YEAR`, `NITRO_MONTH/YEAR/3_MONTH/6_MONTH`, `GUILD_BOOST_MONTH/YEAR`, экспорт `gD`).
   - 12 стилей подарочных открыток `GIFT_CARD_STYLES` (`SNOWGLOBE = 1 .. NITROWEEN_STANDARD = 12`, экспорт `t_`) и источники запуска `GIFTING_ORIGINS` (`DM_CHANNEL = 0 .. BADGE_DIRECTORY = 5`, экспорт `tg`).
2. **Утилиты подарочных кодов и `GiftStyle` (`reference-oracle/decoded/4d2a64cb5df5fc4e_0`, модуль `45938`, `@9711598..@9712600`):**
   - `GIFT_STYLE_DEFAULT = 0`, `GIFT_STYLE_CUSTOM_STYLE = 1`, `GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD = 2`.
   - `resolveGiftStyle(customMessage, options)`: возвращает `2` при наличии непустого сообщения (`<= 200` символов), выбранного эмодзи-конфетти (`emojiConfetti`) или звукового эффекта (`soundEffect`), иначе `1`.
   - `normalizeGiftCode(raw)` и `extractGiftCodesFromText(content)`: удаление дефисов `-`, валидация `^[A-Za-z0-9_]{2,64}$`, извлечение до `MAX_EXTRACTED_GIFT_CODES = 3` уникальных кодов из `discord.gift/<code>`, `discord.com/gifts/<code>`, `discordapp.com/gifts/<code>` и `promos.discord.gg/?code=<code>`.
3. **Поток кастомизации и валидации (`reference-oracle/decoded/3e8824f7eca8914f_0`, модули `968060`, `274076`, `354972`, `650588`, `472468`, `@200300`, `@229000..@233000`, `@372000..@385000`):**
   - Кастомизация стиля открытки (`setCardStyle`), эмодзи-конфетти (`setEmojiConfetti`), звукового эффекта (`setSoundEffect`) и персонального поздравления (`#gift-custom-message` со счётчиком `#gift-custom-count` `0 / 200` и индикацией `data-near-limit="true"` при `>= 180` символах).
   - Запрет отправки подарка самому себе (`validateGiftRecipient`: `recipient.id === currentUserId`) и строгая проверка границ Snowflake (`1..=18446744073709551615`, без ведущего нуля).
4. **Коды ошибок Discord и машина состояний (`4d2a64cb5df5fc4e_0` модуль `652215` `@320207`, `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §4.1–§4.6):**
   - Классификация ошибок `classifyGiftError`: `429` (`error-429` с обратным отсчётом `tickRateLimitSecond`), `401/403` (`error-403`), `400/404` и коды Discord `10038`, `50035`, `50050`, `50051`, `50054`, `50092`, `50095`, `50097` (`error-400`), сетевые сбои (`error-network`).
   - Защита от двойного клика в состоянии `submitting`, блокировка закрытия модалки (`Escape`, клик по фону, крестик) во время активного запроса `create_gift_code`, клавиатурная навигация по радиокнопкам планов (`ArrowDown`/`ArrowRight`/`ArrowUp`/`ArrowLeft`/`Home`/`End`), фокус-ловушка `Tab`/`Shift+Tab` и возврат фокуса на триггер при закрытии.

## 3. Red-прогон (Strict TDD)
- До создания `rust/gui-spike/ui/features/gift-modal.js` выполнен запуск `node --test tests/unit/gift-modal.test.mjs`, зафиксировано падение (`ERR_MODULE_NOT_FOUND`, `EXIT=1`).
- Полный лог сохранён в `waves/RW-40/G2/task-627-gift-modal-ui/red.log`.

## 4. Green-прогон и Targeted-проверки
- `node --test tests/unit/gift-modal.test.mjs` — **12/12 pass (`0 fail`)**, лог сохранён в `waves/RW-40/G2/task-627-gift-modal-ui/green.log`.
- `npm run typecheck` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — **0 ошибок TypeScript**.
- `node --test tests/meta/*.test.mjs` — **все мета-тесты зелёные (`0 fail`)**, включая `plan-sync.test.mjs` и `test-policy.test.mjs`.
- `node waves/RW-39/PM/w39c/geometry-check.mjs` — **31/31 геометрических значений в допуске**.
- Затронутые unit-тесты (`composer.test.mjs`, `gui-spike-dom.test.mjs`, `gui-spike-features.test.mjs`, `gift-modal.test.mjs`) — **333/333 pass (`0 fail`)**.

## 5. Мутационное тестирование (`5/5 KILLED = 100%`)
Проверены 5 независимых мутаций в `rust/gui-spike/ui/features/gift-modal.js` с последующим побайтовым восстановлением файла (лог в `waves/RW-40/G2/task-627-gift-modal-ui/mutations.log`):
- `M1 KILLED`: `MAX_GIFT_CUSTOM_MESSAGE_CHARS` изменён с `200` на `250`.
- `M2 KILLED`: `resolveGiftStyle` игнорирует эмодзи-конфетти и звук (`hasExtras = false`).
- `M3 KILLED`: лимит `extractGiftCodesFromText` изменён с `MAX_EXTRACTED_GIFT_CODES (3)` на `5`.
- `M4 KILLED`: отключена защита от отправки подарка самому себе в `validateGiftRecipient`.
- `M5 KILLED`: отключена блокировка закрытия модалки в состоянии `submitting` внутри `close()`.

## 6. Затронутые файлы и контрольные суммы (`md5sum`)
| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/features/gift-modal.js` | `095bb5b314bf57fc15d5862eb0556aee` | Создан |
| `rust/gui-spike/ui/styles/gift-modal.css` | `a1257098e0dbccd640326b8436a54ef2` | Создан |
| `tests/unit/gift-modal.test.mjs` | `8cead562478c1b9a383fb6df26e2046b` | Создан |
| `TDD-PLAN.md` | `c6db7d24605192066b8d4673fd544494` | Обновлён (§8 таблица синхронизации тестов) |

## 7. Риски и замечания
- Блокирующих рисков нет (`P0 = 0`, `P1 = 0`).
- Стили `gift-modal.css` используют исключительно CSS-токены из `rust/gui-spike/ui/tokens.css` (0 сырых hex/rgb/hsl цветов, 0 `!important`, 0 `:has()`, 0 внешних URL).
- Код `gift-modal.js` не вызывает запрещённых нативных/медиа API и не пишет в `console`.
