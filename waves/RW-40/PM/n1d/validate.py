#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""N1-D validator (post PM review): JSON shape, evidence marks, endpoints, no confirmed-as-argument."""
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path("/home/sanya/orca/project")
DOC = ROOT / "waves/RW-40/DESIGN/SERVER-MENU.md"
DATA = ROOT / "waves/RW-40/DESIGN/server-menu.json"
TOKENS = ROOT / "rust/gui-spike/ui/tokens.css"
CONTRACTS = ROOT / "waves/RW-40/NETWORK-CONTRACTS.md"
D02 = ROOT / "docs/re/rest-d02.md"

fails = []
checks = 0


def check(ok, msg):
    global checks
    checks += 1
    if not ok:
        fails.append(msg)
    print(("PASS " if ok else "FAIL ") + msg)


def md5(path):
    return hashlib.md5(Path(path).read_bytes()).hexdigest()


data = json.loads(DATA.read_text(encoding="utf-8"))
doc = DOC.read_text(encoding="utf-8")
json_text = DATA.read_text(encoding="utf-8")

check(data.get("schema") == "rw40/server-menu-v1", "schema == rw40/server-menu-v1")
check(data.get("card") == "N1-D", "card == N1-D")
check(len(data["menu_items"]) == 10, "menu_items == 10 (dump D1)")
active = [i for i in data["menu_items"] if not i["aria_disabled"]]
check(len(active) == 3, "active menu items == 3")
check([i["action"] for i in active] == ["guild-invite", "guild-leave", "guild-copy-id"],
      "active action order: invite, leave, copy-id")

# 1. no external refs / fonts
FORBIDDEN = ["url" + "(", "@font" + "-face", "ht" + "tp://", "ht" + "tps://", "@" + "import"]
for path in (DOC, DATA):
    text = path.read_text(encoding="utf-8")
    hits = [p for p in FORBIDDEN if p in text]
    check(not hits, f"no external css/font refs in {path.name}: {hits}")

# 2. confirmed is never an argument / success condition
for text, name in ((doc, "doc"), (json_text, "json")):
    check("confirmed:true" not in text and "confirmed:false" not in text,
          f"{name}: no confirmed:true / confirmed:false literals")
success = json.loads(json_text)["success_rule"]
check("confirmed" not in json.dumps(success).lower(), "success_rule does not mention confirmed")
check("2xx" in json.dumps(success), "success_rule requires 2xx")
for key, ep in data["endpoints"].items():
    check("confirmed" not in json.dumps(ep).lower(), f"endpoint {key} has no confirmed field")

# 3. endpoint evidence
check(data["endpoints"]["invite_accept"]["method"] == "POST", "accept: POST")
check(data["endpoints"]["invite_accept"]["path"] == "/invites/{code}", "accept: path")
check(data["endpoints"]["invite_accept"]["evidence"] == "S", "accept: evidence S")
check(data["endpoints"]["invite_accept"]["live_observed"] is False, "accept: live not observed")
check(data["endpoints"]["enter_guild_lurker"]["method"] == "PUT", "enter: PUT")
check(data["endpoints"]["enter_guild_lurker"]["path"] == "/guilds/{id}/members/@me?lurker=true",
      "enter: lurker path")
check(data["endpoints"]["enter_guild_lurker"]["response_status"] == 204, "enter: 204")
check(data["endpoints"]["enter_guild_lurker"]["evidence"] == "L", "enter: evidence L")
check(data["endpoints"]["leave_guild"]["path"] == "/users/@me/guilds/{id}", "leave: path")
check(data["endpoints"]["leave_guild"]["body"] == "{lurking: boolean}", "leave: body known [S]")
check(data["endpoints"]["leave_guild"]["evidence"] == "S", "leave: evidence S")
check("not read" in data["endpoints"]["leave_guild"]["response"], "leave: response not read")
check(data["endpoints"]["guild_list"]["rest_endpoint"] is False, "guild list has no REST endpoint")

# 4. unmeasured modal geometry
check(data["geometry"]["dialog"]["measurement"] == "unmeasured", "dialog geometry unmeasured")
check(data["geometry"]["join_preview"]["measurement"] == "unmeasured", "preview unmeasured")
check(data["geometry"]["server_menu"]["measurement"] == "measured", "menu measured")

# 5. tokens exist
css = TOKENS.read_text(encoding="utf-8")
defined = set(re.findall(r"(--[a-z0-9-]+)\s*:", css))
refs = sorted(set(re.findall(r"--[a-z0-9-]+", json_text)))
missing = [n for n in refs if n not in defined]
check(not missing, f"tokens exist in tokens.css (refs={len(refs)}, missing={missing})")

# 6. marks
marks = {"T", "L", "S", "M", "A", "D"}


def walk(node, path=""):
    if isinstance(node, dict):
        if "value" in node and isinstance(node.get("value"), (int, float, str)):
            check(node.get("mark") in marks, f"mark present: {path or 'root'} -> {node.get('mark')}")
        for key, val in node.items():
            walk(val, f"{path}.{key}" if path else key)
    elif isinstance(node, list):
        for idx, val in enumerate(node):
            walk(val, f"{path}[{idx}]")


walk(data.get("geometry", {}))

# 7. doc coverage and counts
counts = {m: len(re.findall(rf"\[{m}\]", doc)) for m in ("T", "L", "S", "M", "D", "A")}
print("INFO mark counts:", ", ".join(f"{k}={v}" for k, v in counts.items()))
check(counts["T"] > 0 and counts["L"] > 0 and counts["S"] > 0 and counts["M"] > 0 and counts["A"] > 0,
      "doc uses T/L/S/M/A marks")
for needle in ("2xx", "204", "lurker", "GET /invites/{code}", "POST /invites/{code}",
               "DELETE /users/@me/guilds/{id}", "unmeasured", "timeout"):
    check(needle.lower() in doc.lower(), f"doc covers '{needle}'")
check("unconfirmed" not in doc.lower(), "doc has no unconfirmed feature flag")
check("blocked" not in doc.lower(), "doc has no blocked join state")

# 8. dump facts
dump = Path("/home/sanya/re-deep")
cat = json.loads((dump / "ru-catalog.json").read_text(encoding="utf-8"))
check(len(cat) == 23836, f"ru-catalog keys == 23836 (actual {len(cat)})")
for key in ("cs48bs", "Ts/9Ac", "AVazqJ", "5AkWAd", "FDsl+J"):
    check(key in cat, f"string key present in dump: {key}")
items = json.loads((dump / "ui2/D1-guild-menu-items.json").read_text(encoding="utf-8"))
check(len(items) == 10, "D1 guild menu items == 10")


def joined(item):
    return item["text"] + (f" {item['subtitle']}" if item.get("subtitle") else "")


check([joined(i) for i in data["menu_items"]] == [i["text"] for i in items],
      "JSON menu texts match D1 dump (two-line item 4 joined)")

captures = list(dump.glob("capture-*.jsonl"))
check(len(captures) == 4, f"captures checked == 4 (actual {len(captures)})")
net = {}
for cap in captures:
    text = cap.read_text(encoding="utf-8", errors="replace")
    net[cap.name] = {"invites": text.count("invites/"), "delete": text.count('"DELETE"')}
check(all(v == 0 for c in net.values() for v in c.values()),
      f"no invite/DELETE calls in captures: {net}")

contracts = CONTRACTS.read_text(encoding="utf-8")
d02 = D02.read_text(encoding="utf-8")
check("904 записей" in contracts and "confirmed" in contracts,
      "contracts document the confirmed artifact")
check("PUT /guilds/{id}/members/@me?lurker=true" in d02, "D02 records lurker PUT")
check("204" in d02, "D02 records 204")

# 9. contrast
def lum(hex_color):
    h = hex_color.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))

    def f(c):
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4

    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def ratio(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


pairs = [
    ("#ff9a9e", "#28282d", "danger-text on surface-3", 4.5),
    ("#fbfbfb", "#28282d", "text-strong on surface-3", 4.5),
    ("#96979e", "#242429", "text-muted on surface-2", 4.5),
    ("#abacb2", "#242429", "text-subtle on surface-2", 4.5),
    ("#ffffff", "#cf3a3f", "white on danger-fill", 4.5),
    ("#ffffff", "#5865f2", "white on accent", 4.5),
    ("#a5b0ff", "#242429", "text-accent on surface-2", 4.5),
]
for fg, bg, name, minimum in pairs:
    value = ratio(fg, bg)
    check(value >= minimum, f"contrast {name}: {value:.3f} >= {minimum}")

# 10. md5
print("INFO md5:")
for path in [
    DOC, DATA, TOKENS, CONTRACTS, D02,
    ROOT / "waves/RW-39/DESIGN/SPEC.md",
    ROOT / "waves/RW-40/DESIGN/DESIGN-MSG.md",
    ROOT / "waves/RW-40/THEME-SPEC.md",
    dump / "ui2/D1-guild-menu.png",
    dump / "ui2/D1-guild-menu-items.json",
    dump / "ui2/00-start.fingerprint.json",
    dump / "ru-catalog.json",
    dump / "ui/tokens.json",
]:
    print(f"  {md5(path)}  {path}")

print(f"\nchecks={checks} failures={len(fails)}")
if fails:
    print("FAILED:")
    for f in fails:
        print(" - " + f)
    sys.exit(1)
print("RESULT: OK")
