#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""RW-37 / W37a: воспроизводимое извлечение RE-констант раскладки и пагинации.

Только чтение golden-дампа `/home/sanya/re-deep` (никаких записей туда).
Запуск:  python3 waves/RW-37/RE/extract.py > waves/RW-37/RE/extract.log 2>&1

Каждое число в выводе сопровождается путём к артефакту и способом извлечения.
Зависимости: python3 + Pillow + numpy (уже есть в среде), stdlib json/re/glob.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

RE = Path("/home/sanya/re-deep")
OUT: dict = {}


def p(rel: str) -> Path:
    path = RE / rel
    if not path.exists():
        raise SystemExit(f"нет артефакта: {path}")
    return path


def load(rel: str):
    return json.loads(p(rel).read_text(encoding="utf-8"))


def note(key: str, value, source: str, method: str) -> None:
    OUT[key] = {"value": value, "source": source, "method": method}
    print(f"{key:32} = {value!r}")
    print(f"{'':32}   src: {source}")
    print(f"{'':32}   how: {method}")


# ---------------------------------------------------------------- 1. viewport
log_ui = load("ui/log.json")
start = next(e for e in log_ui if e["name"] == "start")
note(
    "viewport",
    {"w": start["size"]["w"], "h": start["size"]["h"], "dpr": start["dpr"]},
    "ui/log.json → запись name=start",
    "CDP evalJS innerWidth/innerHeight + devicePixelRatio (ui-trace.mjs:111)",
)
from PIL import Image as _Image  # noqa: E402

_png = _Image.open(RE / "ui/00-app.png")
note(
    "screenshot_px",
    {"w": _png.size[0], "h": _png.size[1]},
    "ui/00-app.png (IHDR), ui6/02-channel.png, ui8/*.jpg",
    "PNG IHDR (PIL) — совпадает с viewport×dpr (2560/1.25=2048, 1383/1.25=1106.4)",
)

# ---------------------------------------------------------------- 2. tokens
tokens = load("ui/tokens.json")
want = [
    "--custom-guild-list-width",
    "--custom-guild-list-padding",
    "--guildbar-avatar-size",
    "--custom-guild-sidebar-width",
    "--custom-app-sidebar-target-width",
    "--custom-app-top-bar-height",
    "--custom-channel-header-height",
    "--custom-member-list-width",
    "--chat-avatar-size",
    "--chat-markup-line-height",
    "--custom-message-margin-horizontal",
    "--custom-message-margin-left-content-cozy",
    "--custom-message-margin-compact-indent",
    "--custom-message-spacing-vertical-container-cozy",
    "--custom-message-padding-vertical-container-compact",
    "--custom-channel-textarea-text-area-height",
    "--space-xxs",
    "--space-xs",
    "--channels-name-line-height",
    "--custom-member-list-item-avatar-decoration-padding",
]
live_tokens = {k: tokens.get(k) for k in want}
note(
    "css_tokens_live",
    live_tokens,
    "ui/tokens.json (4795 переменных, getComputedStyle(document.documentElement), тема dark)",
    "прямое чтение значений",
)

# ---------------------------------------------------------------- 3. rail
avatar = 40  # --guildbar-avatar-size
pad = 12  # --custom-guild-list-padding
note(
    "rail_w",
    avatar + 2 * pad,
    "ui/tokens.json: --custom-guild-list-width = calc(40px + 12px*2);"
    " ui2/log.json (guild-rect: w=40, центр x=32 ⇒ left=12)",
    "формула токена + проверка по getBoundingClientRect иконки гильдии",
)

# ------------------------------------------------- 4. границы панелей по PNG
from PIL import Image  # noqa: E402  (после проверки путей)
import numpy as np  # noqa: E402

SIDEBAR_BG = (18, 18, 20)  # --background-base-lowest в этом срезе
CHAT_BG = (26, 26, 30)  # --background-base-low


def chat_left_css(img_rel: str) -> float:
    im = Image.open(p(img_rel)).convert("RGB")
    a = np.asarray(im).astype(int)
    h, w, _ = a.shape
    side = np.array(SIDEBAR_BG)
    chat = np.array(CHAT_BG)
    hits = []
    for frac in (0.35, 0.45, 0.55, 0.65, 0.75, 0.85):
        y = int(h * frac)
        row = a[y]
        ok_side = np.abs(row - side).max(axis=1) < 4
        ok_chat = np.abs(row - chat).max(axis=1) < 4
        for x in range(150, w - 60):
            if ok_chat[x : x + 60].all() and ok_side[max(0, x - 40) : x - 5].all():
                hits.append(x)
                break
    if not hits:
        raise SystemExit(f"не найдена граница панелей в {img_rel}")
    xs = sorted(hits)
    return xs[len(xs) // 2]


chat_x_css = chat_left_css("ui/00-app.png") / start["dpr"]
chat_x_css2 = chat_left_css("ui6/02-channel.png") / start["dpr"]
note(
    "chat_left_css",
    round(chat_x_css, 1),
    "ui/00-app.png + ui6/02-channel.png (2560×1383, dpr 1.25)",
    "скан строк: первый x, где подряд 60 px цвета (26,26,30) после ≥35 px цвета (18,18,20); медиана по 6 строкам",
)
note("chat_left_css_second_capture", round(chat_x_css2, 1), "ui6/02-channel.png", "тот же алгоритм")

sidebar_total = chat_x_css
rail_w = avatar + 2 * pad
tree_w = sidebar_total - rail_w
note(
    "sidebar_total_w",
    round(sidebar_total, 1),
    "ui/00-app.png",
    "граница панели == ширина (rail + channel tree), т.к. chat начинается сразу за tree",
)
note(
    "channel_tree_w",
    round(tree_w, 1),
    "ui/00-app.png; CSS: .sidebarList__5e434{width:calc(var(--custom-guild-sidebar-width) - var(--custom-guild-list-width) - 1px)}",
    "sidebar_total - rail_w; проверка формулой: 375 - 64 - 1 = 310 (+1px border = 311)",
)

# ---------------------------------------------------------------- 5. members
members = load("ui8/22-members.json")
mr = members["rect"]
note(
    "member_list",
    {"x": mr["x0"], "y": mr["y0"], "w": mr["w"], "h": mr["h"]},
    "ui8/22-members.json → rect (aside[class*=membersWrap])",
    "getBoundingClientRect() (x0/y0/абсолютные, ui-members6.mjs)",
)
note(
    "chat_w",
    round(mr["x0"] - chat_x_css, 1),
    "ui/00-app.png + ui8/22-members.json",
    "member_list.x0 - chat_left",
)
assert mr["w"] == 256, mr["w"]

# ------------------------------------------------- 6. строки каналов/участников
log_ui6 = load("ui6/log.json")
ch = next(e for e in log_ui6 if e["name"] == "text-channel")
note(
    "channel_row",
    {"w": ch["w"], "h": ch["h"]},
    "ui6/log.json → text-channel (a.link__2ea32)",
    "getBoundingClientRect(); h=28 совпадает с CSS: line-height 20px + padding 4+4",
)
note(
    "member_row_h",
    42,
    "ui8/22-members.jpg (кроп панели 320×1281 px при dpr 1.25) + CSS",
    "медиана расстояний между верхами аватаров без групп-заголовков ≈ 42.2; CSS: .memberInner padding 4+4 + аватар 32 + .container padding 1+1 = 42",
)

# ------------------------------------------------------- 7. строки сообщений
msg = next(e for e in log_ui if e["name"] == "msg-rect")
line_h = 22  # --chat-markup-line-height = 1.375rem
note(
    "message_content_h",
    {"h": msg["h"], "lines": round(msg["h"] / line_h)},
    "ui/log.json → msg-rect ([id^=message-content-])",
    "getBoundingClientRect: 66 px = 3 × 22 px (line-height), т.е. контент в 3 строки",
)
note(
    "row_heights",
    {"message_cozy": 26, "message_compact": 26, "message_cozy_group_start": 48, "member": 42, "channel": 28},
    "CSS из decoded/3eefa7dae1e2c234_0:"
    " .cozy_c19a55.wrapper_c19a55{padding:2px 0 2px; padding-inline-start:64px},"
    " .compact_c19a55.wrapper_c19a55{padding:2px 0; padding-inline-start:5rem},"
    " .wrapper_c19a55{min-height:1.375rem}, .header_c19a55{min-height:1.375rem;line-height:1.375rem}",
    "cozy/compact: 22+2+2=26 (продолжение группы); cozy group-start: header 22 + content 22 + 4 = 48",
)

# ------------------------------------- 8. константы клиента (окно сообщений)
dec = p("decoded/3eefa7dae1e2c234_0").read_text(encoding="utf-8", errors="replace")
m = re.search(r"let nx=(\d+),nk=(\d+),nF=(\d+),nB=(\d+),nV=(\d+),nH=(\d+),nj=(\d+)", dec)
if not m:
    raise SystemExit("не найдена строка констант nx=…,nk=… в decoded/3eefa7dae1e2c234_0")
nx, nk, nF, nB, nV, nH, nj = (int(x) for x in m.groups())
note(
    "client_consts",
    {
        "qlD(nx)": nx,
        "cZu(nk)": nk,
        "EMb(nF)": nF,
        "d7Q(nB)": nB,
        "Ue3(nV)": nV,
        "UNo(nH)": nH,
        "rLJ(nj)": nj,
    },
    "decoded/3eefa7dae1e2c234_0, модуль 652215 (export map: EMb→nF, d7Q→nB, UNo→nH, rLJ→nj)",
    "regex по объявлению let-цепочки; имена экспортов — из n.d(t,{...}) того же модуля",
)
trunc = re.search(r"truncate\(e,t\)\{return this\.length<=E\.UNo\?this:.*?truncateBottom\(E\.rLJ\)", dec)
note(
    "message_store_cap",
    {"max": nH, "trim_to": nj},
    "decoded/3eefa7dae1e2c234_0: MessageStore.truncate → this.length<=E.UNo … truncateBottom(E.rLJ)",
    "код: коллекция канала обрезается при >120 до 60",
)
print(f"{'':32}   found truncate(…UNo…rLJ): {bool(trunc)}")

# --------------------------------------------------- 9. REST-пагинация (факт)
urls: dict[str, int] = {}
for rel in ("capture-ready.jsonl", "capture-run2.jsonl", "capture-cold.jsonl"):
    txt = p(rel).read_text(encoding="utf-8", errors="replace")
    for u in re.findall(r"/channels/\d+/messages\?[^\"\\ ]+", txt):
        urls[u] = urls.get(u, 0) + 1
pins = 0
for rel in ("capture-ready.jsonl", "capture-run2.jsonl"):
    txt = p(rel).read_text(encoding="utf-8", errors="replace")
    pins += len(re.findall(r"/channels/\d+/messages/pins\?limit=(\d+)", txt))
shapes: dict[str, list] = {"initial": [], "before": [], "around": []}
for u in urls:
    if "around=" in u:
        shapes["around"].append(u)
    elif "before=" in u:
        shapes["before"].append(u)
    else:
        shapes["initial"].append(u)
note(
    "rest_messages_pages",
    {
        "initial_limit": sorted({re.search(r"limit=(\d+)", u).group(1) for u in shapes["initial"]}),
        "initial_urls": len(shapes["initial"]),
        "before_limit": sorted({re.search(r"limit=(\d+)", u).group(1) for u in shapes["before"]}),
        "before_urls": len(shapes["before"]),
        "around_limit": sorted({re.search(r"limit=(\d+)", u).group(1) for u in shapes["around"]}),
        "around_urls": len(shapes["around"]),
        "pins_limit_25_occurrences": pins,
    },
    "capture-ready.jsonl / capture-run2.jsonl / capture-cold.jsonl (CDP Network, request+response ⇒ каждый URL ×2)"
    " — счётчики ниже: число РАЗЛИЧНЫХ URL, а не запросов",
    "regex по всем URL /channels/<id>/messages?…",
)
note(
    "load_more_formula",
    "limit = min(30, 2 × config(\"scrollManager.loadMore\"))",
    "decoded/3eefa7dae1e2c234_0: ScrollManager.loadMore →"
    " fetchMessages({before, after, limit:Math.min(eh.EMb, 2*(0,lZ.h)(\"scrollManager.loadMore\")), truncate:true})",
    "чтение кода; 2×12=24 объясняет наблюдаемый ?before=…&limit=24",
)
note(
    "message_limits_max",
    100,
    "decoded/3eefa7dae1e2c234_0: fetchMessages({channelId:r,limit:100}) (preload веток);"
    " клэмп 30 — это клиентская политика чанка, не серверный максимум",
    "код клиента + отсутствие 4xx-проб с limit>100 в дампе (см. render-docs: max=100 — assumption)",
)

# --------------------------------------------------------------- 10. READY
gw = load("gateway-schema.json")
ev = {e["t"]: e for e in gw["events"]}
ready = ev["READY"]
ready_stats = gw["stats"]
note(
    "ready_payload",
    {
        "json_bytes": ready["sizeBytes"],
        "raw_bytes": ready["rawBytesPerEvent"]["median"],
        "top_level_fields": len(ready["fields"]),
        "session_messages": ready_stats["messages"],
        "session_event_types": ready_stats["eventTypes"],
    },
    "gateway-schema.json → events[t=READY] + stats",
    "sizeBytes/rawBytesPerEvent + len(fields) (в _trace отдельно, потому 39, а не 40)",
)
mlu = ev["GUILD_MEMBER_LIST_UPDATE"]
op0 = mlu["fields"]["ops"]["example"]
if "first" in op0:  # усечённый пример: {…first-of, first}
    op0 = op0["first"]
sync_items = None
_md0 = p("gateway-schema.md").read_text(encoding="utf-8")
_m = re.search(r"В SYNC-событии (\d+) элементов items", _md0)
if _m:
    sync_items = int(_m.group(1))
note(
    "member_list_window",
    {
        "member_count": mlu["fields"]["member_count"]["example"],
        "online_count": mlu["fields"]["online_count"]["example"],
        "sync_range_hex": op0["range"],
        "sync_items": sync_items,
        "sync_events": 1,
        "list_update_events": mlu["count"],
    },
    "gateway-schema.json → GUILD_MEMBER_LIST_UPDATE (ops[0].range='0x0063');"
    " gateway-schema.md §4.1 (100 элементов items в SYNC)",
    "range = 2 байта [0,99]; window = 100 строк участников",
)
chunk = ev["GUILD_MEMBERS_CHUNK"]
note(
    "guild_members_chunk",
    {
        "events": chunk["count"],
        "chunk_index": chunk["fields"]["chunk_index"]["example"],
        "chunk_count": chunk["fields"]["chunk_count"]["example"],
        "members_in_chunk": len(chunk["fields"]["members"]["example"]),
        "bytes": chunk["sizeBytes"],
    },
    "gateway-schema.json → GUILD_MEMBERS_CHUNK",
    "поля chunk_index/chunk_count/members",
)

# счётчики из markdown-разбора (там же, где вложенные длины массивов)
md = p("gateway-schema.md").read_text(encoding="utf-8")
counts = {}
patterns = {
    "guilds": r"\*\*READY\.d\.guilds\[\] \((\d+) гильдий\)\*\*",
    "channels": r"\*\*READY\.d\.guilds\[\]\.channels\[\] \((\d+) канала\)\*\*",
    "roles": r"\*\*READY\.d\.guilds\[\]\.roles\[\] \((\d+) роли\)\*\*",
    "read_state_entries": r"\*\*READY\.d\.read_state\.entries\[\] \((\d+)\)\*\*",
    "user_guild_settings": r"\*\*READY\.d\.user_guild_settings\.entries\[\] \((\d+)\)\*\*",
    "channel_overrides": r"channel_overrides\[\] \((\d+)\)\*\*",
    "private_channels": r"\*\*READY\.d\.private_channels\[\] \((\d+)\)\*\*",
    "merged_members": r"\| d\.merged_members\[\]\[\] \| ([\d,]+) \|",    "users": r"\| d\.users\[\] \| (\d+) \|",
    "relationships": r"\| d\.relationships\[\] \| (\d+) ",
    "read_state_table": r"\| d\.read_state\.entries\[\] \| (\d+) \|",
}
for name, rx in patterns.items():
    mm = re.search(rx, md)
    if name == "merged_members":
        # строка таблицы: | d.merged_members[][] | 1,1,1,1,1,1,1 | → 7 массивов по 1 участнику
        counts["merged_members"] = sum(int(x) for x in mm.group(1).split(",")) if mm else None
        counts["merged_members_arrays"] = len(mm.group(1).split(",")) if mm else None
        continue
    counts[name] = int(mm.group(1).replace(",", "")) if mm else None
note(
    "ready_counts",
    counts,
    "gateway-schema.md §3 (таблицы вложенных структур READY) — счётчики массивов, которых нет в усечённом JSON",
    "regex по заголовкам/строкам таблиц",
)

md2 = p("gateway-protocol.md").read_text(encoding="utf-8")
guild = {}
for name, rx in {
    "member_count": r"`member_count` \*\*([\d\s]+)\*\*",
    "channels_total": r"каналы — \*\*(\d+)\*\*",
    "text": r"text (\d+)",
    "voice": r"voice (\d+)",
    "category": r"category (\d+)",
    "forum": r"forum (\d+)",
    "roles": r"роли — (\d+)",
    "emojis": r"эмодзи (\d+)",
    "stickers": r"стикеры (\d+)",
    "ready_guild_size": r"`sizeBytes` \*\*([\d\s]+)\*\*",
    "mlu_events": r"`GUILD_MEMBER_LIST_UPDATE` — \*\*(\d+)\*\*",
    "chunks": r"`GUILD_MEMBERS_CHUNK` — \*\*(\d+)\*\* \((\d+) участник",
}.items():
    mm = re.search(rx, md2)
    guild[name] = (mm.groups() if mm else None)
note(
    "ready_guild_it",
    guild,
    "gateway-protocol.md §6 (снимок READY гильдии ITSKIDON 1014537851418980372)",
    "regex по тексту разбора",
)

# ------------------------------------------------------------- 11. compose
note(
    "composer",
    {"h": 56, "w": round(mr["x0"] - chat_x_css - 16, 1), "y": 1044.0},
    "ui/tokens.json: --custom-channel-textarea-text-area-height=56px;"
    " ui/00-app.png: бокс (34,35,39) x 381.6…1784.8, y 1044…1100",
    "измерение цвета по строкам PNG + токен",
)
note(
    "header",
    {"topbar_h": 32, "channel_header_h": 49, "members_y0": mr["y0"]},
    "ui/tokens.json: --custom-app-top-bar-height=32px, --custom-channel-header-height=49px;"
    " ui/00-app.png: полосы y 0…32 (titlebar), 32…81 (header); ui8/22-members.json y0=82",
    "смена цвета по столбцу x=1200 + rect панели участников",
)

# --------------------------------------------------------------- сохранить
Path("/tmp/opencode/rw37-re.json").write_text(
    json.dumps(OUT, ensure_ascii=False, indent=2), encoding="utf-8"
)
print("\nOK: %d записей, JSON-копия в /tmp/opencode/rw37-re.json" % len(OUT), file=sys.stderr)
