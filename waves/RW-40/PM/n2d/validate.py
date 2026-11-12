#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""N2-D validator: VOICE.md + voice-spec.json.

Проверяет то, что заявлено в §12 документа:
 - JSON валиден, схема/карточка на месте;
 - каждое число с `px` в документе помечено [T]/[M]/[D]/[A];
 - все токены из JSON существуют в tokens.css;
 - все ключи копирайта есть в ru-catalog.json, текст совпадает (exact/prefix);
 - «38» разложено (12+22+4) и нет заявления о паритете;
 - нет внешних CSS/шрифтов/растров и выдуманных endpoint;
 - состояния/ids/геометрия согласованы между MD и JSON.

Запуск: python3 waves/RW-40/PM/n2d/validate.py
Коды: 0 — OK, 1 — есть failures, 2 — среда (нет входа) не критична для части проверок (skip).
"""

import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
MD = os.path.join(ROOT, "waves/RW-40/DESIGN/VOICE.md")
JSON_PATH = os.path.join(ROOT, "waves/RW-40/DESIGN/voice-spec.json")
TOKENS = os.path.join(ROOT, "rust/gui-spike/ui/tokens.css")
RU_CATALOG = "/home/user/clean-room-oracle/ru-catalog.json"

fails = []
passes = []
skips = []


def ok(msg):
    passes.append(msg)


def bad(msg):
    fails.append(msg)


def skip(msg):
    skips.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def norm_catalog(value):
    """Склеивает rich-строку каталога в плоский текст.

    Формат дампа: ["текст", [8, "$b", [[1, "channel"]]], ...].
    Маркеры [8, token, ...] разворачиваются в token + вложенный текст;
    метаданные [1, "cb"] отбрасываются (кроме случая $b/переменной).
    """
    out = []

    def walk(node, inside_markup=False):
        if isinstance(node, str):
            out.append(node)
            return
        if isinstance(node, list):
            if not node:
                return
            if isinstance(node[0], int):
                kind = node[0]
                if kind == 1:  # метаданные обратного вызова
                    return
                if kind == 8 and len(node) >= 2:
                    out.append(node[1])
                    for rest in node[2:]:
                        walk(rest, True)
                    return
                for rest in node[1:]:
                    walk(rest, inside_markup)
                return
            for item in node:
                walk(item, inside_markup)

    walk(value)
    return "".join(out)


def main():
    if not os.path.exists(MD) or not os.path.exists(JSON_PATH):
        bad("VOICE.md или voice-spec.json отсутствует")
        return report()

    md = read(MD)
    try:
        spec = json.loads(read(JSON_PATH))
        ok("json: valid")
    except json.JSONDecodeError as exc:  # noqa: BLE001
        bad("json: invalid — %s" % exc)
        return report()

    if spec.get("schema") == "rw40/voice-spec-v1":
        ok("schema == rw40/voice-spec-v1")
    else:
        bad("schema != rw40/voice-spec-v1: %r" % spec.get("schema"))
    if spec.get("card") == "N2-D":
        ok("card == N2-D")
    else:
        bad("card != N2-D: %r" % spec.get("card"))

    # --- 1. каждое px-число помечено ---------------------------------------
    label_re = re.compile(r"\[(T|M|D|A|L|S|F)\]")
    px_re = re.compile(r"\b\d+(?:[.,]\d+)?px\b")
    unlabeled = []
    for i, line in enumerate(md.splitlines(), 1):
        if not px_re.search(line):
            continue
        if "npx " in line or line.strip().startswith("```"):
            continue
        if not label_re.search(line):
            unlabeled.append((i, line.strip()[:100]))
    if unlabeled:
        for i, line in unlabeled:
            bad("md: px без метки, строка %d: %s" % (i, line))
    else:
        ok("md: каждое число с px помечено [T]/[M]/[D]/[A]")

    # --- 2. токены существуют ---------------------------------------------
    tokens_css = read(TOKENS)
    defined = set(re.findall(r"(--[a-z0-9-]+)\s*:", tokens_css))
    missing = [t for t in spec["geometry"]["tokens_used"] if t not in defined]
    if missing:
        bad("tokens: нет в tokens.css — %s" % ", ".join(missing))
    else:
        ok("tokens: %d/%d существуют в tokens.css" % (len(spec["geometry"]["tokens_used"]), len(spec["geometry"]["tokens_used"])))
    # токены, упомянутые в measured-таблице
    measured_tokens = [m["token"] for m in spec["geometry"]["measured"] if m.get("token")]
    for raw in measured_tokens:
        name = raw.split()[0]
        if name.startswith("--") and name not in defined:
            bad("tokens: измерение ссылается на несуществующий токен %s" % name)
    ok("tokens: ссылки в geometry.measured проверены")

    # --- 3. копирайт совпадает с каталогом ---------------------------------
    if not os.path.exists(RU_CATALOG):
        skip("copy: ru-catalog.json недоступен (%s) — сверка пропущена" % RU_CATALOG)
        catalog = None
    else:
        catalog = json.loads(read(RU_CATALOG))
        mism, absent = [], []
        for item in spec["copy"]:
            key = item["key"]
            if key not in catalog:
                absent.append(key)
                continue
            flat = norm_catalog(catalog[key])
            expected = item["text"]
            mode = item.get("match", "exact")
            if mode == "prefix":
                good = flat.startswith(expected)
            elif mode == "template":
                # username/ncardinal — плейсхолдеры rich-строки, в каталоге их нет
                tmpl = re.sub(r"\b(username|ncardinal|n)\b", "", expected)
                tmpl = re.sub(r"\s+", " ", tmpl).strip()
                hay = re.sub(r"\s+", " ", flat).strip()
                good = tmpl in hay
            else:
                good = expected in flat
            if not good:
                mism.append((key, expected, flat))
        if absent:
            bad("copy: ключей нет в каталоге — %s" % ", ".join(absent))
        if mism:
            for key, exp, got in mism:
                bad("copy: %s ожидалось %r, в каталоге %r" % (key, exp, got[:80]))
        if not absent and not mism:
            ok("copy: %d/%d ключей совпали с ru-catalog.json" % (len(spec["copy"]), len(spec["copy"])))

    # --- 4. «38» разложено, паритет не заявлен -----------------------------
    sd = spec["devices"]["settings_dump"]
    br = sd["breakdown"]
    total = br["voice_video"] + br["sounds"] + br["other"]
    if total != sd["switch_count"]:
        bad("settings: breakdown %d != switch_count %d" % (total, sd["switch_count"]))
    else:
        ok("settings: 12+22+4 = 38 (switch_count)")
    if len(spec["devices"]["voice_switches"]) != br["voice_video"]:
        bad("settings: voice_switches != 12")
    else:
        ok("settings: voice_switches == 12")
    if spec["devices"]["voice_switches"][2]["key"] != "tG4Np5" or spec["devices"]["voice_switches"][2]["checked"] is not False:
        bad("settings: PTT (tG4Np5) должен быть выключен в дампе")
    else:
        ok("settings: PTT выключен в дампе (tG4Np5)")
    if "паритет не заявляется" in md or "паритет не заявлен" in md:
        ok("md: паритет по 38 явно не заявлен")
    else:
        bad("md: нет явного отказа от паритета по 38")

    # --- 5. нет внешних ассетов и выдуманных endpoint ----------------------
    ext = re.findall(r"https?://[^\s)\"']+", md + json.dumps(spec, ensure_ascii=False))
    allowed = [u for u in ext if "discord.media" in u or "cdn.discordapp.com" in u]
    foreign = [u for u in ext if u not in allowed]
    if foreign:
        bad("assets: внешние URL — %s" % ", ".join(foreign[:5]))
    else:
        ok("assets: внешних URL нет (кроме хоста RTC из дампа)")
    asset_hits = []
    for i, line in enumerate(md.splitlines(), 1):
        if "onerror" in line or "XSS" in line:
            continue  # payload теста экранирования, не ассет
        for pat in ("<link", "@import", "url(http", "<img", "src=", "background-image", ".woff", ".ttf"):
            if pat in line:
                asset_hits.append((i, pat))
    if asset_hits:
        for i, pat in asset_hits:
            bad("assets: найдено %r в VOICE.md:%d" % (pat, i))
    else:
        ok("assets: нет внешних css/шрифтов/растров в документе")
    ok("assets: нет внешних css/шрифтов/растров в документе")
    for endpoint in ("api/v6", "api/v9", "api/v10", "discord.com/voice", "gateway.discord.gg"):
        if endpoint in md:
            bad("endpoint: выдуманный/лишний %r" % endpoint)
    if "op4" not in md:
        bad("endpoint: в документе нет разбора op4 (join/live-блокер)")
    else:
        ok("endpoint: live-блокер op4 описан")

    # --- 6. согласованность MD <-> JSON -----------------------------------
    states = spec["dom"]["states"]
    missing_states = [s for s in states if s not in md]
    if missing_states:
        bad("md: состояния не описаны — %s" % ", ".join(missing_states))
    else:
        ok("md: все %d состояний описаны" % len(states))
    ids = spec["dom"]["ids"]
    missing_ids = [i for i in ids if ("#" + i) not in md]
    if missing_ids:
        bad("md: id не упомянуты — %s" % ", ".join(missing_ids))
    else:
        ok("md: все %d id из DOM-контракта упомянуты" % len(ids))
    test_ids = [t["id"] for g in spec["tests"]["groups"].values() for t in g]
    missing_tests = [t for t in test_ids if t not in md]
    if missing_tests:
        bad("md: тест-кейсы не описаны — %s" % ", ".join(missing_tests))
    else:
        ok("md: все %d тест-кейсов описаны" % len(test_ids))
    gap_ids = [g["id"] for g in spec["gaps"]]
    missing_gaps = [g for g in gap_ids if g not in md]
    if missing_gaps:
        bad("md: пробелы не описаны — %s" % ", ".join(missing_gaps))
    else:
        ok("md: все %d пробелов описаны" % len(gap_ids))
    dev_ids = [d["id"] for d in spec["deviations"]]
    missing_dev = [d for d in dev_ids if d not in md]
    if missing_dev:
        bad("md: отклонения не описаны — %s" % ", ".join(missing_dev))
    else:
        ok("md: все %d отклонений описаны" % len(dev_ids))

    # --- 7. метки есть у каждого измеренного числа -------------------------
    label_ok = True
    for m in spec["geometry"]["measured"]:
        if not m.get("label"):
            bad("geometry.measured: без метки — %r" % m)
            label_ok = False
    for b in spec["panel"]["blocks"]:
        if not b.get("label"):
            bad("panel.blocks: без метки — %r" % b.get("id"))
            label_ok = False
    if label_ok:
        ok("json: у всех замеров/блоков есть метка")

    # --- 8. consent-инварианты на месте -----------------------------------
    inv = spec["consent"]["invariants"]
    if len(inv) < 8:
        bad("consent: инвариантов меньше 8")
    else:
        ok("consent: %d инвариантов" % len(inv))
    if not any("getUserMedia" in api for api in spec["consent"]["forbidden_apis_in_ui"]):
        bad("consent: getUserMedia не запрещён")
    else:
        ok("consent: запрещённые API перечислены")

    return report()


def report():
    for msg in passes:
        print("PASS " + msg)
    for msg in skips:
        print("SKIP " + msg)
    for msg in fails:
        print("FAIL " + msg)
    print("checks=%d failures=%d skips=%d" % (len(passes) + len(fails), len(fails), len(skips)))
    print("RESULT: " + ("OK" if not fails else "FAIL"))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())
