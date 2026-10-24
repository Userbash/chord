#!/usr/bin/env python3
"""C3a — проверка waves/RW-40/theme-spec.json и THEME-SPEC.md.

Проверяет:
  1. JSON валиден и соответствует схеме rw40/theme-spec-v1.
  2. В спеке и JSON нет url( и @font-face (ассеты не копируются).
  3. Каждое light-значение: [T] существует в дампе и совпадает с ref;
     [A] помечено; имя есть в tokens.css или объявлено новым.
  4. [L] density-значения совпадают с waves/RW-37/RE/layout.json.
  5. Контраст: text >= 4.5, nontext >= 3.0, pair >= 4.5 — пересчёт по sRGB.
Выход: exit 0 — всё сошлось, exit 1 — есть расхождения.
"""
import json
import re
import sys
import colorsys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
SPEC = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "waves/RW-40/theme-spec.json"
DOC = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "waves/RW-40/THEME-SPEC.md"
DUMP = Path("/home/sanya/re-deep/ui/tokens.json")
TOKENS_CSS = ROOT / "rust/gui-spike/ui/tokens.css"
LAYOUT = ROOT / "waves/RW-37/RE/layout.json"

errors = []
checks = 0


def ok(label, cond, detail=""):
    global checks
    checks += 1
    if not cond:
        errors.append(f"{label}: {detail}")
        print(f"FAIL {label} {detail}")
    else:
        print(f"ok   {label} {detail}")


# ---------- 1. JSON ----------
raw = SPEC.read_text(encoding="utf-8")
try:
    spec = json.loads(raw)
except json.JSONDecodeError as exc:
    print(f"FAIL json: {exc}")
    sys.exit(1)
ok("json.valid", True, f"{len(raw)} bytes")
ok("json.schema", spec.get("schema") == "rw40/theme-spec-v1", str(spec.get("schema")))

# ---------- 2. ассеты ----------
for path in (SPEC, DOC):
    if not path.exists():
        ok(f"assets.{path.name}.exists", False, "файла нет")
        continue
    text = path.read_text(encoding="utf-8")
    ok(f"assets.{path.name}.no-url", "url(" not in text, "найден url(")
    ok(f"assets.{path.name}.no-font-face", "@font-face" not in text, "найден @font-face")

# ---------- утилиты цвета ----------
def hsl_to_hex(value):
    m = re.match(r"hsl\(([\d.]+)\s+calc\(1\*([\d.]+)%\)\s+([\d.]+)%", value)
    if not m:
        return None
    h, sat, light = (float(x) for x in m.groups())
    r, g, b = colorsys.hls_to_rgb((h % 360) / 360, light / 100, sat / 100)
    return "#%02x%02x%02x" % tuple(round(v * 255) for v in (r, g, b))


def hex_rgb(value):
    v = value.strip().lower()
    if re.fullmatch(r"#[0-9a-f]{6}", v):
        return tuple(int(v[i:i + 2], 16) for i in (1, 3, 5))
    m = re.fullmatch(r"rgb\((\d+) (\d+) (\d+) / ([\d.]+)%\)", v)
    if m:
        return tuple(int(x) for x in m.groups()[:3])
    return None


def rgba_alpha(value):
    m = re.fullmatch(r"rgb\(\d+ \d+ \d+ / ([\d.]+)%\)", value.strip().lower())
    return float(m.group(1)) / 100 if m else None


def luminance(rgb):
    def channel(c):
        c /= 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (channel(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def blend(fg, alpha, bg):
    return tuple(round(f * alpha + b * (1 - alpha)) for f, b in zip(fg, bg))


# ---------- 3. токены ----------
dump = json.loads(DUMP.read_text(encoding="utf-8"))
css_text = TOKENS_CSS.read_text(encoding="utf-8")
declared = set(re.findall(r"^\s*(--[a-z0-9-]+):", css_text, re.MULTILINE))
light = spec["light"]["tokens"]
new_names = {name for name, item in light.items() if item.get("is_new")}

counts = {"T": 0, "A": 0, "other": 0}
for name, item in light.items():
    tag = item.get("tag")
    counts[tag if tag in ("T", "A") else "other"] += 1
    ok(f"name.{name}.declared", name in declared or name in new_names,
       "имени нет в tokens.css" if name not in new_names else "новое имя")
    value = item["value"]
    if tag == "T":
        ref = item["ref"]
        dump_key = "--" + ref if not ref.startswith("--") else ref
        ok(f"ref.{name}.exists", dump_key in dump, dump_key)
        if dump_key in dump:
            expected = hsl_to_hex(dump[dump_key])
            ok(f"ref.{name}.value", expected == value.lower(), f"{value} != {expected}")
    elif tag == "A":
        ok(f"assumption.{name}", True, item.get("ref", "")[:60])
    else:
        ok(f"tag.{name}", False, str(tag))

# color hexes for contrast
colors = {}
for name, item in light.items():
    rgb = hex_rgb(item["value"])
    if rgb is not None:
        colors[name] = (rgb, rgba_alpha(item["value"]))

rendered = []  # (name, against, ratio, kind, min)
for name, item in light.items():
    aa = item.get("aa")
    if not aa:
        continue
    base = colors.get(name)
    if base is None:
        ok(f"aa.{name}.color", False, "не hex/rgb")
        continue
    base_rgb, alpha = base
    for other in aa["against"]:
        target = colors.get(other)
        if target is None:
            ok(f"aa.{name}.against.{other}", False, "цель не найдена/не hex")
            continue
        target_rgb = target[0]
        actual = base_rgb if alpha is None else blend(base_rgb, alpha, target_rgb)
        ratio = contrast(actual, target_rgb)
        passed = ratio + 1e-9 >= aa["min"]
        ok(f"aa.{name}.{other}", passed, f"{ratio:.3f} >= {aa['min']}")
        rendered.append((name, other, ratio, aa["type"], aa["min"]))

# ---------- 4. density ----------
layout = json.loads(LAYOUT.read_text(encoding="utf-8"))


def layout_value(ref):
    node = layout
    for part in ref.split("."):
        if not isinstance(node, dict) or part not in node:
            return None
        node = node[part]
    return node


for mode in ("cozy", "compact"):
    for name, item in spec["density"][mode].items():
        if item["tag"] != "L":
            continue
        expected = layout_value(item["ref"])
        actual = int(re.match(r"(\d+)px", item["value"]).group(1))
        ok(f"layout.{mode}.{name}", expected == actual, f"{actual} vs layout {expected}")

# ---------- 5. итоги ----------
print()
print(f"tokens: [T]={counts['T']} [A]={counts['A']} other={counts['other']} total={len(light)}")
print(f"contrast checks: {len(rendered)}")
print(f"total checks: {checks}")
if errors:
    print(f"\nFAILED: {len(errors)}")
    sys.exit(1)
print("\nOK: все проверки прошли")
