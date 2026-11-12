# STUB-TRACKER — ход замены заглушек

Дата: 2026-10-24. Обновлено: **2026-10-30** (RW-40, batch 06, G6 Parity, read-only).
Статусы: `open` · `in work` · `ready` · `blocked` · `accepted` · `deferred`.

- `ready` — контракт подтверждён независимым оракулом (дамп/live/схема), блокеров нет,
  реализации ещё нет; карточка может идти в работу.
- Ссылки: `STUB-MAP.md`, `STUB-IMPLEMENTATION-PLAN.md`, `PARITY-MATRIX.md`,
  оракул волны — `reference-oracle-REA.md` (+ `ATTACH-RE.md`), срез — `PARITY/MERGE-READINESS-06.md`.

## Сводка

| Показатель | Значение |
|---|---|
| Всего карточек | 35 |
| accepted | 10 (STUB-01, 02, 04, 05, 06, 09, 21, 24, 31, 32) |
| partial | 9 (STUB-15, 18, 19, 22, 23, 26, 28, 29, 30) |
| ready | 4 (STUB-12, 13, 14, 27) |
| open / queued | 12 (STUB-03, 07, 08, 10, 11, 16, 17, 20, 25, 33, 34, 35) |
| in work | 0 |
| blocked | 0 |
| P0-строки | P0-01, P0-02 — blocked (нет окна/токена) |
| P1 | 22 |
| P2 | 11 |
| P3 | 2 |

Изменения против 2026-10-24: `blocked` → `0` (STUB-13 снят), `ready` = 4 (STUB-12, 13,
14, 27), `partial` +2 (STUB-15, 22 — принят dispatch-канал L-02), `accepted` пересчитан
по фактическим строкам таблицы (в прежней сводке стояло 4 при 10 строках с D-*).

## Принятые срезы RW-40 (2026-10-30)

Проверено `md5sum` (`PARITY/MERGE-READINESS-06.md §2`): на 16:23 совпадали **8/8**
файлов. К 16:27 `ui/app.js` ушёл (`1ac5c369…` → `5f081fdd…`, правка G-01b UI), поэтому
у L-02-UI сейчас **2/3** — перевыпуск пина входит в шаги до merge, а не в эту таблицу.

| Срез | ACCEPT | md5 = дерево (16:23 → 16:27) | Что даёт заглушкам |
|---|---|---|---|
| **G-01b-CONTRACT** (`lastGuildId` в `UiSettings`) | `PM/g01b-contract/ACCEPT.md` | **2/2** | STUB-01: restore выбранного сервера |
| **L-02-CORE** (flush dispatches до `?` + redaction) | `waves/RW-40/PM/l02-core/ACCEPT.md` | **3/3** | STUB-01/15/22: READY, `GUILD_MEMBERS_CHUNK`, `GUILD_MEMBER_LIST_UPDATE`, `PRESENCE_UPDATE` доходят из ядра |
| **L-02-UI** (`gateway://dispatch` → store/rail) | `waves/RW-40/PM/l02-ui/ACCEPT.md` | **3/3 → 2/3** (`app.js`) | STUB-01/15/22: те же события в UI (43 unit, 5/5 мутантов) |

Ограничение: срез принят **offline**; live-доставка (E3) не проверялась, `live`-строки
STUB-01/15/22 остаются открытыми.

## Phase 0 — trust/live

| ID | Задача | Owner | Статус | Next / evidence |
|---|---|---|---|---|
| STUB-31 | `walk` CLI | G4 | **accepted (D-123)** | e2e CHROME_BIN |
| STUB-32 | `cdp eval/clicktext/list/shot` | G4 | **accepted (D-90)** | e2e нужен CHROME_BIN |
| STUB-33 | e2e fixture 4 skip | G4 | open | после 31/32; skip≠pass |
| STUB-34 | gate/workspace | G4 | open | `cargo test --workspace`, CLI coverage |
| STUB-35 | token behavior test | G1+G4 | open | keyring→Network behavior |
| P0-01 | live READY | PM+G1+G4 | blocked | токен+окно |
| P0-02 | live send | PM+G1+G4 | blocked | после P0-01 |

## Phase 1 — servers/channels/text

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-01 | rail из READY | G2+G1 | **accepted (D-112) + G-01b/L-02** | G-01b restore принят; live READY — P0-01 |
| STUB-02 | guild menu | G2+G3 | **accepted (D-92)** | invite/owner C2 |
| STUB-04 | enter guild UI | G2+G1 | **accepted (D-128)** | live 204 |
| STUB-05 | invite join | G1+G2 | **accepted (D-116/D-122)** | тела/коды подтверждены `reference-oracle-REA §4`; G-03a UI |
| STUB-06 | leave server | G1+G2 | **accepted (D-89/D-92)** | invite/owner C2 |
| STUB-08 | channel tree | G2+G1 | open | C-01 |
| STUB-09 | channel switch | G2+G1 | **accepted (D-120)** | history paging |
| STUB-14 | search filters | G2+G1 | **ready** | оракул `reference-oracle-REA §2`: GET guild/channel search, все 7 фильтров (`author_id`, `mentions`, `has`, `channel_id`, `pinned`, `author_type`, `min_id`/`max_id`) + `link_hostname`/`attachment_extension`/`attachment_filename`/`include_nsfw`; страница 25, вкладки = пресеты, `202`+`retry-after` (≤5), `total_results`. Gap: максимальный `offset` `[A]` |
| STUB-21 | read state/unread | G1+G2 | **accepted (D-126)** | badge/live (read_state не входит в 6 форвардящихся типов) |
| STUB-22 | typing/presence | G1+G2 | **partial** | `PRESENCE_UPDATE` идёт в UI (L-02 принят); `TYPING_START` не форвардится (`FORWARDED_DISPATCHES` = 6 без TYPING) → P-01…03 |
| DP-02 | 26/48/+22, member 42 | G2+G4 | **accepted (D-102)** | legacy tokens P2 |

## Phase 2 — completeness

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-03 | create server | G1+G2+G3 | open | `reference-oracle-REA §3`: маршрут `GUILDS:"/guilds"` есть, вызов в ленивом чанке, тело `[A]`; нужен live-захват создания |
| STUB-07 | invites | G1+G2 | open | тела/коды подтверждены `reference-oracle-REA §4` (create/list/revoke); G-06 |
| STUB-10 | pins/threads | G2+G1 | open | M-10/11 |
| STUB-11 | inbox | G2+G1 | open | N-03 |
| STUB-12 | gift | G6+G2 | **ready** | оракул `reference-oracle-REA §5`: кнопка + `openGiftModal` + `POST /users/@me/entitlements/gift-codes` `{sku_id,subscription_plan_id,gift_style}`, плюс GET/revoke/resolve/redeem. Gap: разметка модалки `806677` и выбор SKU `[A]` |
| STUB-13 | attachments | G1+G2 | **ready** (было `blocked`) | оракул `reference-oracle-REA §1` + `ATTACH-RE §2`: `POST /channels/{id}/attachments` `{files:[…]}` → `upload_url`/`upload_filename` → `PUT` (+`Content-Range`/`308`) → `uploaded_filename` в `POST /channels/{id}/messages`; лимиты 10 файлов, `max(20 МиБ, tier)`, 500 МБ/1 ГиБ, коды `40005/40006/50035`, `429`+`retry-after`. Не входит: сжатие/превью |
| STUB-16 | profile popover | G2 | open | MB-03 |
| STUB-19 | settings sections | G2+G3 | partial | ST-01…08 |
| STUB-28 | notifications | G5+G2 | partial | N-01/02 |
| STUB-29 | settings sync | G1 | partial | ST-08 |
| STUB-30 | i18n | G1+G2 | partial | ST-04 |

## Phase 3+ — voice/video/platform

| ID | Задача | Owner | Статус | Next |
|---|---|---|---|---|
| STUB-26 | voice session | G1+G5 | partial | V-01…06, live voice |
| STUB-17 | mute/deafen/PTT | G5+G2 | open | V-07; VOICE spec D-94 |
| STUB-18 | voice UI | G2+G3 | **partial** (UI accepted D-115) | voice live-gate |
| STUB-27 | video/Go Live | G5+G1 | **ready** (design D-109) | оракул `reference-oracle-REA §6`: opcodes 18 `STREAM_CREATE` / 19 `DELETE` / 20 `WATCH` / 21 `PING` / 22 `SET_PAUSED` с полями; 4 входящих пакета; REST `/streams/{id}/{preview,notify,stream}`. Блокирует не контракт, а media-путь (capture/encode/RTP) и live-окно |
| STUB-20 | QR/passkey | G1+G5 | open | A-04/05 |
| STUB-15 | live members | G1+G2 | **partial** | `GUILD_MEMBERS_CHUNK` + `GUILD_MEMBER_LIST_UPDATE` идут в UI (L-02 принят); лимиты/догрузка — MB-01 |
| STUB-23 | live history | G1+G2 | partial | M-01 |
| STUB-24 | live guild list | G1+G2 | **accepted (D-112)** | live E3 |
| STUB-25 | member list | G1+G2 | open | MB-01 |

## Оракулы RW-40: что закрыто, что осталось

| Пункт | Метка | Остаток |
|---|---|---|
| STUB-13 вложения | `[D]` полностью | нет |
| STUB-14 поиск | `[D]` все фильтры + `202` | максимальный `offset` `[A]` |
| STUB-12 gift | `[D]` endpoint + обработчик | модалка `806677`, SKU `[A]` |
| STUB-27 stream | `[D]` opcodes + поля + REST | media-путь, live-окно |
| STUB-05/07 инвайты | `[D]` тела/коды | live-коды (окно RE-2) |
| STUB-03 create server | `[D]` только маршрут | тело `POST /guilds` `[A]` |
| STREAM capture/encode/RTP | `[A]` | отдельная задача G5 |

Следствие для `PARITY-MATRIX.md` (правит G6 Lead, не этот файл): строки `AT-01`
(«blocked»), `M-13` («4/7 фильтров»), `S-02` («STREAM [A]») устарели и требуют
синхронизации по `reference-oracle-REA.md`.

## Правило обновления

1. После accepted batch: обновить строку + ссылку на D-* и на ACCEPT-файл.
2. `in work` без evidence 2 batch → вернуть/декомпозировать.
3. `blocked` — указать конкретный контракт/окно, не «нет времени». `ready` — указать
   оракул и раздел; без оракула статус остаётся `open`.
4. Удалять заглушку из карты только после G6 parity + PM accept.
