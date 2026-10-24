# G6-SYNC-01 — синхронизация матрицы и долгов после batch

Дата: 2026-10-24. Автор: G6 Lead (RE Parity), subagent PM DeepSeek.
Режим: **только документы**. Продуктовый код (`rust/**`, `ui/**`), тесты, гейт, Discord,
live, сеть, `cargo`/`npm`/`node --test`, commit — не запускались и не менялись.

Изменены 3 файла: `PARITY-MATRIX.md`, `GAP-BACKLOG.md` (G6-доки) и аддитивно
`QUALITY/BUGS.md` (реестр G6-дефектов). `TASKS.md`, `QUEUE.md`, `waves/db.jsonl`,
`waves/STATE.md`, §8 `G6-REPORT-RW40-02` — **не** менялись (зона PM/Team Lead).

Источники: `G6-BATCH-MERGE-01.md` (deferred, §2/§4/§5), `G6-REPORT-RW40-02.md` §7.1/§8,
`PARITY-MATRIX.md`, `GAP-BACKLOG.md`, `QUALITY/BUGS.md`, `TASKS.md`, `DECISIONS.md`
RW-39 D-112…D-131, `MERGE-POLICY.md`, `MERGE-MANIFEST.md` (§5), текущий код (read-only сверка).

---

## 0. Резюме

1. Матрица/бэклог синхронизированы: **7 P1-строк** (`G-01`, `C-02`, `G-03`, `G-03a`, `G-04`,
   `M-07`, `L-01`) → `implemented (live unverified)`; `M-06` подтверждён (D-131);
   `V-08` → `partial` (UI на fake); `P-03` остался `partial` (backend-only);
   `ST-01`/`G-01`/`C-02` Gap/Ref/DoD/WSJF поправлены по §8 отчёта.
2. Счётчики: **91 строка**, `implemented 21 / partial 36 / absent 34 / unknown 0`,
   приоритеты `P0 5 / P1 32 / P2 40 / P3 14`; backlog сходится по ID и по классам.
3. `G6-D-01…D-05` заведены в `QUALITY/BUGS.md` с evidence: закрыты `D-01`, `D-02`, `D-04`, `D-05`;
   **открыт только `D-03`** (`ready`/`members_page`/`search_page`/`settings`/`enter_guild`
   не вызываются из UI) — P2, закрывается вместе с `L-02`.
4. Parity-вердикт после sync: **`deferred`** (не `parity-ok`, не `block`).
   До `parity-ok` остаётся: P-03 UI, E3 (P0-01/02/03), e2e не как skip, L-02 dispatch,
   G-05 2/10, M-07 unread, C-02/M-01 paging, G-01b; процессно — журнал RW-40 и stale
   `TASKS.md`/`QUEUE.md`.
5. `implemented (live unverified)` **не** выдаётся за паритет: это offline-путь + тесты,
   без E3. У каждой из 7 строк есть остаток (бэклог §2.4); у `M-06` — live/`MESSAGE_REACTION_*`.
   Строк, закрытых «начисто» (функция + E3), после sync **по-прежнему нет ни одной**.

---

## 1. Применено в `PARITY-MATRIX.md`

| Строка | Было | Стало | Основание |
|---|---|---|---|
| G-01 | `partial: 3 статичных кнопки`, WSJF 5.5 | `implemented (live unverified)`, Ref `[L] READY.guilds[]` + `[M]` визуал, WSJF `—`, остаток G-01b | D-112, G6-BATCH §1 |
| C-02 | `partial: фиксированный news`, WSJF 5.0 | `implemented (live unverified)`, gap = paging, WSJF `—`, G6-D-01/D-02 закрыты | D-117/D-120 |
| G-03 | `absent`, `guilds_contract.rs (fake)`, WSJF 4.0 | `implemented (live unverified)`, DoD = 3 nullable-поля `[S]` + `{code}`, WSJF `—` | D-116/D-122 |
| G-03a | `partial: backend accepted, UI absent`, WSJF 4.2 | `implemented (live unverified)`, DoD = 204/1 IPC/плитка не добавляется, WSJF `—` | D-76/D-128 |
| G-04 | `absent`, WSJF 4.5 | `implemented (live unverified)`, `{guildId, lurking}` + confirm, WSJF `—` | D-89/D-92 |
| G-05 | `partial: только aria-expanded` | `partial`: поповер/ARIA/клавиатура есть, **активно 2 из 10**, WSJF 5.0 сохранён | D-92, G6-BATCH §2.2 |
| C-01 | `partial: 3 канала, 2 категории` | `partial`: дерево из READY, типы 0/2/4/5/15; нет open voice/forum и collapse | D-120 (следствие, статус не менялся) |
| M-06 | `implemented`, тест `reactions_contract.rs` | `implemented (live unverified)`: pills/quick на `channel:<channelId>:<item.id>`, остаток live/`MESSAGE_REACTION_*` | D-131 |
| M-07 | `partial: ядро, UI нет`, WSJF 4.5 | `implemented (live unverified)`, DoD `last_viewed`/`token` nullable, WSJF `—` | D-124/D-126 |
| P-03 | `partial: core TTL, нет POST/UI`, тест `rest_typing.rs` | `partial`: **backend implemented (D-108/D-114), UI отсутствует**, тест `typing_contract.rs` | D-114, G6-BATCH §1 |
| ST-01 | Gap «нет 45 подпунктов», WSJF 4.0 | Gap = 63 DOM-строки (2+57+4), подпункты 17 разделов `[A]`, WSJF 6.0, фикстура-оракул | G6-REPORT §8, D-118 |
| V-08 | `absent`, тест `voice_ui.test.mjs` | `partial`: панель на fake store (D-115), реального voice нет | D-115 |
| L-01 | `partial: GatewayStatus, UI нет`, WSJF 6.0 | `implemented (live unverified)`, `#net-status` + retry, WSJF `—` | D-106 |

Шапка матрицы и §14 обновлены (дата sync, правило «`implemented (live unverified)` ≠ паритет»,
команда-оракул пересчёта). Обозначения уровней, легенда и §15 не менялись — `[M]`/`[A]`
по-прежнему не доказательство функции.

---

## 2. Счётчики: до/после и сходимость

| App-статус | До | После | Δ |
|---|---:|---:|---:|
| implemented / verified | 14 | **21** | +7 |
| partial | 40 | **36** | −4 |
| absent | 37 | **34** | −3 |
| unknown | 0 | 0 | 0 |
| **Итого** | **91** | **91** | 0 |

Разбивка после sync: `implemented` P0 3 / P1 16 / P2 2; `partial` P0 2 / P1 14 / P2 18 / P3 2;
`absent` P1 2 / P2 20 / P3 12.

| Приоритет | Кол-во | Backlog § |
|---|---:|---|
| P0 | 5 | §1 (5) |
| P1 | 32 | §2.1 (10) + §2.2 (14) + §2.4 (8) |
| P2 | 40 | §3 (40) |
| P3 | 14 | §4 (14) |

Проверка сходимости (независимый оракул — markdown-файлы, не реализация):

```bash
cd waves/RW-40/PARITY
# статусы/приоритеты матрицы
sed 's/\\|/~/g' PARITY-MATRIX.md | awk -F'|' '
/^\| [A-Z][A-Z0-9]*-[0-9]/ { n=NF; app=$6; prio=$(n-3);
  gsub(/^ +| +$/,"",app); gsub(/^ +| +$/,"",prio); sub(/[ (:].*/,"",app);
  a[app]++; p[prio]++; t++ }
END { for (k in a) print "app",k,a[k]; for (k in p) print "prio",k,p[k]; print "total",t }'
# 91 = 21+36+34; P0 5 / P1 32 / P2 40 / P3 14
```

Результат прогона (2026-10-24, документы): `app implemented 21`, `app partial 36`,
`app absent 34`, `prio P0 5 / P1 32 / P2 40 / P3 14`, `total 91`; сверка ID
матрица↔бэклог — 91/91, пропусков и дублей нет, все строки backlog имеют ровно одно
вхождение (P0 5 + P1 32 + P2 40 + P3 14).

---

## 3. `QUALITY/BUGS.md` — реестр G6

| ID | P | Статус | Evidence | Остаток |
|---|---|---|---|---|
| G6-D-01 | P1 | **closed (D-117 backend / D-120 UI)** | реальный `channelId`/`item.id`; `BENCH_CHANNEL_ID` только в bench-ветке `chat.js:786,1282,1321`; `channel-switch.test.mjs` (10) | live send — P0-02 |
| G6-D-02 | P1 | **closed (D-120)** | `channelItemAt(index).id`, `data-message-id=item.id`; bench-ветка осознанно | live edit/delete — M-03/M-04 |
| G6-D-03 | P2 | **open** | grep `ui/**`: `invoke("ready"/"members_page"/"search_page"/"settings"/"enter_guild")` = 0; используются `channels`, `messages_page`, `read_state`, `ack_message`, `join_guild`, `leave_guild` | закрывается с L-02 (`dispatch_ui.test.mjs`) |
| G6-D-04 | P2 | **closed (G6-SYNC-01 §2)** | ST-01 Gap переписан: 63 = профиль 2 + 18 разделов/«Выйти»×3 + 4 подпункта; WSJF 4.0 → 6.0 | — (нужен оракул `settings-nav-63.tsv`) |
| G6-D-05 | P3 | **closed (G6-SYNC-01 §2)** | G-01 Ref: `[L] READY.guilds[]` (7 гильдий, `002-READY.json`) + `[M]` за визуал | — |

Severity сохранена как в источнике (`G6-REPORT-RW40-02` §7.1); `D-05` — P3 (дефект
документации), не повышалась до P2. Ранее в `BUGS.md` `G6-D-01` стоял как P0 — приведён
к источнику (P1) уже после закрытия инцидента; отдельного понижения severity «по факту»
нет, это унификация с §7.1. Заодно синхронизированы ранее заведённые строки:
`G6-M06` → **closed (D-131)** с остатком live/`MESSAGE_REACTION_*`, `G6-P03UI` → **in work**.
Строк с чужими ID и чужих разделов не менял.

---

## 4. Parity verdict после sync

### Вердикт: **`deferred`** (не `block`, не `parity-ok`)

- `block` нет: P0-дефектов в коде не найдено (§3 `G6-BATCH-MERGE-01` PASS), вымысла нет,
  гейт `passed_with_skips`, секретов не добавлено.
- `parity-ok` нельзя: см. блокеры ниже. Статус `implemented` у 7 строк — offline;
  E3 нет ни у одной live-строки.

### Что осталось до `parity-ok` (в порядке снятия)

| # | Блокер | Тип | Критерий снятия | Кто |
|---:|---|---|---|---|
| 1 | **P-03 UI отсутствует** (`send_typing` = 0, индикатора нет) | P1 код | `POST /typing` из UI + «Печатает…» `aria-live="polite"`, троттлинг `TYPING_RESEND_MS`, guard ≤5, slowmode; UI-тест | G2 (backend D-114 готов) |
| 2 | **L-02 dispatch → UI** | P1 код | READY/CHUNK/PRESENCE/`MESSAGE_REACTION_*` → экраны без reload; закрывает G6-D-03 | G1+G2 |
| 3 | **E3 нет**: P0-01 READY, P0-02 send, P0-03 no-secrets-live | P0 live | окно N0 (1) на тестовом аккаунте; 7 строк `live unverified` → `[L]`/`verified` | PM+G4+G1 |
| 4 | **e2e = 4 skip ≠ pass** | гейт | прогон с `CHROME_BIN` (2 fixture) и `CDP_ENDPOINT` (2 live) до `gate:qualify`; `passed_with_skips` не выдавать за `passed` | G4 |
| 5 | **G-05: 2 из 10 пунктов** | P1 код | остальные 8 активны по правам, invite — после G-06/capability | G2+G3 |
| 6 | **M-07 unread без mentions / badge** | P1 остаток | проекция `badgeCount`/`flags`, сброс по ack (G-08/C-03) | G1+G2 |
| 7 | **C-02/M-01 paging** | P1 остаток | `hasMore/nextCursor`, окно ≤500, открытие на последнем сообщении | G1+G2 |
| 8 | **G-01b restore `lastGuildId`** | P1 остаток | поле в `UiSettings` (контракт настроек) + restore рейла | G1+G2 |
| 9 | M-06 live PUT/DELETE/429 | live | окно N0 + dispatch из #2 | G2+G1 |

Процессные блокеры закрытия batch (не G6): `waves/db.jsonl` — 0 событий RW-40;
`waves/STATE.md` от 2026-10-23; stale-строки `TASKS.md`/`QUEUE.md` (§6).
`MERGE-POLICY §4`: перенос P1 в deferred — только письменно; текущий вердикт `deferred`
и есть эта фиксация, waiver на «backend-only P-03» не выдавался.

---

## 5. Что НЕ считается паритетом

1. `implemented (live unverified)` — offline-путь + тесты, без E3: не приёмка PM.
2. `[M]`-строки: V-08 speaking-анимация, G-05-заглушки, ST-01 подпункты, N-03, DP-02-геометрия.
3. `[A]`: TTL typing 10 с, live-коды invite/leave, `USER_BANNED`-текст, порог idle, consent.
4. `partial` строки (`P-03`, `G-05`, `C-01`, `M-01`, `G-08`, `C-03`, `ST-01`, `V-08`,
   presence/voice/members) — функции нет end-to-end.
5. `V-08` UI на fake store — не голосовой паритет (V-01…V-05 не реализованы live).

---

## 6. Handoff (PM / Team Lead, не G6)

1. **`TASKS.md`**: устарели `G-04` — «in work» (принят D-89/D-92), `G-05/G-04 UI` — «in work»
   (D-92, остаток 2/10), `STUB-01/24` — «queued» (D-112), `STUB-04` — «backend accepted, UI»
   (D-128), `STUB-05` — «queued» (D-116/D-122), `STUB-08/09` — «queued» (D-120),
   `STUB-21/22` — «queued» (M-07 D-124/D-126), `DP-02` — «queued» (D-102), `ST-01` (D-118),
   `P-03` (backend D-114, UI in work).
2. **`QUEUE.md`**: `G6-D-01` числится «P0 в очереди» — закрыт; `G6-D-02` закрыт; в очередь
   встают `P-03 UI` и `L-02`.
3. **Журнал**: `waves/db.jsonl` (0 событий RW-40) + `waves/STATE.md` — только Team Lead.
4. **Split N-01** (§8 отчёта: N-01a policy / N-01b wiring) — **не применён**: он меняет
   число строк 91 → 92. Решение PM; при применении пересчитать обе таблицы и §14.
5. **P2 md5 drift** тестовых файлов (`G6-BATCH §4`, п.1) — нужен отдельный red/green-лог
   или подтверждение, что правки аддитивные; границы baseline `rust/gui-spike/**` вне git.
6. **§8 отчёта** применён полностью, кроме split N-01 (см. п.4) — включая ST-01 Gap/WSJF/Test,
   G-01 Ref/DoD, G-03 DoD, G-03a Ready, C-02 Gap, M-07 DoD.

---

## 7. Файлы и md5

| Файл | md5 | Δ |
|---|---|---|
| `PARITY/PARITY-MATRIX.md` | `c0789f87d4a9fd5f488e56cf1bc8027e` | 13 строк + шапка + §14 |
| `PARITY/GAP-BACKLOG.md` | `30be4326ab42deefa249309dc5435102` | §2.1/§2.2/§2.4, ST-01, §6/§7 |
| `QUALITY/BUGS.md` | `056ae00dd09ea0000707f6a75d691075` | G6-блок (аддитивно) + 2 статуса |
| `PARITY/G6-BATCH-MERGE-01.md` | `b609e73f93f43f7c3bbc29e29f4bfb55` | не менялся (источник) |

Сам отчёт в таблицу не входит (self-hash невозможен) — его md5 считает координатор
после записи. Продуктовые файлы не менялись: `rust/**`, `ui/**`, `tests/**`, `scripts/**`,
`CONTRACTS/**` — без правок.

## 8. Проверки, которые НЕ выполнялись

`cargo`/`npm`/`node --test`/`gate:worker` (документы не влияют на код, но и не подтверждают
его), Discord/live, сеть, CDP/AT-SPI, мутации, commit. Все числа тестов — из
`G6-BATCH-MERGE-01` §4 и `.gate/release-2026-10-24T09-19-36-946Z.json`, не перепроверялись.
Сверка кода — read-only grep: `ui/**` invoke-команды, `chat.js` live/bench-ветки,
`renderChannelMessage` (M-06), `send_typing` (P-03).
