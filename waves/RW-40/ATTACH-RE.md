# ATTACH-RE — загрузка вложений и серверный поиск (по дампу)

Дата: 2026-10-24. Источник: read-only RE по `/home/sanya/re-deep` (Codex).
Метки: `[D]` — цитата файла/дампа, `[A]`/`gap` — не подтверждено.

## 1. Серверный поиск — подтверждено частично

| Факт | Метка | Источник |
|---|---|---|
| `GET /api/v9/guilds/{guild_id}/messages/search` | `[D]` | `ui8/20f-search-network.json:4-9` |
| Параметры: `content`, `channel_id`, `sort_by=timestamp`, `sort_order=desc`, `search_session_id`, `search_query_id`, `offset` | `[D]` | `ui8/20i-scope-compare.json:5-16` |
| Ответ: `messages`, `doing_deep_historical_index`, `total_results`; 200 | `[D]` | `ui8/20f-search-network.json:7-9` |
| UI: 7 фильтров (автор, канал, тип, упоминания, дата, тип автора, закрепление) | `[D]` | `ui8/20b-search-filters.json:4` |
| Сортировка: «По новизне», «Самое старое», «Самое актуальное» | `[D]` | `ui8/20c-search-sort.json:2-4` |
| Результаты 3 302 / 49 887 | `[D]` | `ui8/20g-search-probe.json:27-35` |
| Точные REST-параметры остальных 6 фильтров, лимит страницы, max offset, `/channels/.../search` | gap | — |

**Вывод:** можно реализовать guild-search с `content`, `channel_id`, `sort_by`, `sort_order`,
`offset`; остальные фильтры — только UI/локально до отдельного capture.

## 2. Вложения — не подтверждено

| Факт | Метка | Источник |
|---|---|---|
| Upload endpoint / multipart-поля / `upload_url` / PUT | **gap** | в дампе нет |
| Лимиты размера/типа, 413/429, несколько файлов | **gap** | в дампе нет |
| `/channels/{id}/attachments` в REST-снимке 47 endpoint'ов | отсутствует `[D]` | `rest-d02.md:88-118` |
| 429 в снимке — только `GET /users/@me/survey`, не upload | `[D]` | `rest-d02.md:120-143` |

**Вывод:** B2 (вложения) **заблокирован** до отдельного network/runtime evidence
(согласованный live-захват upload). Реализовывать «по догадке» запрещено.

## 3. Следствия

- B7a: backend guild-search по подтверждённым параметрам + fake-тесты.
- B2a/B2b: **blocked** (нужен `ATTACH-RE-live`).
- UI search-overlay: локальный fallback остаётся; сетевой путь — после B7a.
