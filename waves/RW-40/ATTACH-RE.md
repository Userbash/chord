# ATTACH-RE — загрузка вложений и серверный поиск (по дампу)

Дата: 2026-10-24. Обновлено: 2026-10-30 (RE-DEEP-REA).
Источник: read-only RE по `/home/sanya/re-deep` + изолированная runtime-сессия.
Метки: `[D]` — цитата файла/дампа, `[S]` — статическая схема или live-скрипт, `[A]`/`gap` — не подтверждено.
Полный разбор с цитатами и смещениями — `RE-DEEP-REA.md`.

## 0. Итог обновления

Вердикт «вложения заблокированы» **снят**. Endpoint, структура запроса, PUT, лимиты
и передача `uploaded_filename` найдены в дампе. Прежний вывод «в дампе нет» был следствием
того, как `rest-static.json` ищет call-site: он видит только прямую подстановку `Rsh.*`
в вызов, а upload идёт через
`MessageAttachmentUploadTarget.getCreateAttachmentURL()` → `Rsh.MESSAGE_CREATE_ATTACHMENT_UPLOAD`.
Отсюда `callSiteCount: 0` и ложный gap.

Поиск: подтверждены все 7 фильтров, а не 5 параметров.

## 1. Серверный поиск — подтверждено

| Факт | Метка | Источник |
|---|---|---|
| `GET /api/v9/guilds/{guild_id}/messages/search` | `[D]` | `ui8/20f-search-network.json:4-9`; `4d2a64cb5df5fc4e_0@279632` |
| `GET /api/v9/channels/{channel_id}/messages/search` | `[D]` | `4d2a64cb5df5fc4e_0@280974` |
| Запрос — GET, параметры в query-string | `[D]` | `82a5af57ee11dcd5_0` (`SearchFetcher.makeRequest`) |
| `content`, `sort_by=timestamp`, `sort_order=desc`, `offset` | `[D]` | `capture-run2.jsonl`; `4d2a64cb5df5fc4e_0@7542421` |
| `search_session_id`, `search_query_id` (UUID) | `[D]` | `capture-run2.jsonl` |
| Автор → `author_id` | `[D]` | `4d2a64cb5df5fc4e_0@7564745` |
| Упоминания → `mentions` | `[D]` | там же |
| Тип вложения → `has` (`link,embed,poll,snapshot,file,video,image,sound,sticker`) | `[D]` | там же; `@7559957` |
| Канал → `channel_id` | `[D]` | там же |
| Закрепление → `pinned` (`true`/`false`) | `[D]` | там же |
| Тип автора → `author_type` (`user`,`bot`,`webhook`) | `[D]` | там же |
| Дата → `min_id`, `max_id` (snowflake из timestamp) | `[D]` | `4d2a64cb5df5fc4e_0@7550420` |
| Дополнительно: `link_hostname`, `attachment_extension`, `attachment_filename`, `include_nsfw` | `[D]` | там же; `@7550145` |
| Размер страницы — 25 (`T_y`), `limit` не передаётся | `[D]` | `4d2a64cb5df5fc4e_0@318812` |
| Ответ: `messages[][]`, `total_results`, `doing_deep_historical_index`, `documents_indexed`, `analytics_id`, `channels`, `threads`, `members` | `[D]` | `b9e153f9dfaafc9f_0@29799` |
| `202` = глубокая индексация: до 5 повторов, пауза `retry-after` (по умолчанию 5000 мс) | `[D]` | `82a5af57ee11dcd5_0` |
| Вкладки поиска = пресеты фильтров (`media`/`files`/`links`/`pins`) | `[D]` | `4d2a64cb5df5fc4e_0@7542421` |
| Максимальный `offset` | gap | константы в клиенте нет, нужен live-запрос |

**Вывод:** можно реализовать guild- и channel-search со всеми 7 фильтрами и пагинацией.
Обязательно обработать `202` + `retry-after` и показывать `total_results`.
Сортировки: «По новизне» = `sort_by=timestamp&sort_order=desc`, «Самое старое» =
`sort_by=timestamp&sort_order=asc`.

## 2. Вложения — подтверждено

| Факт | Метка | Источник |
|---|---|---|
| `POST /channels/{id}/attachments` (ключ `MESSAGE_CREATE_ATTACHMENT_UPLOAD`) | `[D]` | `4d2a64cb5df5fc4e_0@269215` |
| Тело `{"files":[{filename,file_size,id,is_clip,…}]}` | `[D]` | `@2572444` (`getUploadPayload`), `@2573535` |
| Заголовок `X-Discord-Original-MD5` | `[D]` | `@2598776` |
| Ответ `attachments[0].upload_url`, `attachments[0].upload_filename` | `[D]` | `@2573535` |
| `PUT {upload_url}`, `Content-Type` = mime или `application/octet-stream` | `[D]` | `@2567497` |
| Докачка: `GET` с `Content-Range: bytes */*`, `308` → продолжение с `Range` | `[D]` | `@2568630` |
| `uploaded_filename` в `POST /channels/{id}/messages` | `[D]` | `@667084`, `@9867091`, `@2362936` |
| Лимит на файл: `max(20 МиБ, guild tier)` | `[D]` | `@6011428`, `@6804500` |
| Tier NONE/T1 = 10 МиБ, T2 = 50 МиБ, T3 = 100 МиБ | `[D]` | `@314301`, `@6805978` |
| Суммарно: 500 МБ (или 1 ГиБ при эксперименте) | `[D]` | `@9868110` |
| Максимум 10 вложений | `[D]` | `@314301` (`n4=10`), `23e2050b01c07deb_0` |
| `413` сервер отдаёт как `40005 ENTITY_TOO_LARGE`; локальная предпроверка тоже `40005` | `[D]` | `@2572800`, `@322893` |
| `429` → `retry-after` из заголовка | `[D]` | `@2361900` |
| Независимое подтверждение схемы (виджеты профиля): POST → upload_url → PUT | `[D]` | `153cd69d0210c61b_0@823` |

**Вывод:** B2 больше не blocked. Реализуемо полностью: запрос слота → PUT файла →
`uploaded_filename` в теле сообщения. Сжатие и превью — отдельная задача, для контракта не нужны.

## 3. Следствия

- B7a: backend guild/channel-search по всем подтверждённым параметрам + fake-тесты,
  включая ветку `202` и `total_results`.
- B2a/B2b: **разблокированы**. `ATTACH-RE-live` больше не нужен для контракта;
  live-захват остаётся полезен только для проверки серверных кодов на реальном upload.
- UI search-overlay: сетевой путь можно включать сразу, локальный fallback оставить
  на случай `202`.
- Gift и STREAM закрыты отдельно — см. `RE-DEEP-REA.md` §5, §6.
