#!/usr/bin/env python3
"""C3a — расчёт контраста light-палитры из theme-spec.json (WCAG 2.2 AA).

Печатает три таблицы: текст, non-text, пары «текст на заливке».
Значения не хранятся в этом файле — единственный источник theme-spec.json.
"""
import json
import re
import colorsys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
spec = json.loads((ROOT / "waves/RW-40/theme-spec.json").read_text(encoding="utf-8"))
light = spec["light"]["tokens"]


def hex_rgb(value):
    v = value.strip().lower()
    if re.fullmatch(r"#[0-9a-f]{6}", v):
        return tuple(int(v[i:i + 2], 16) for i in (1, 3, 5))
    m = re.fullmatch(r"rgb\((\d+) (\d+) (\d+) / [\d.]+%\)", v)
    if m:
        return tuple(int(x) for x in m.groups())
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


def ratio(name, other):
    item = light[name]
    base = hex_rgb(item["value"])
    alpha = rgba_alpha(item["value"])
    target = hex_rgb(light[other]["value"])
    if base is None or target is None:
        return None
    fg = base if alpha is None else blend(base, alpha, target)
    return contrast(fg, target)


SURFACES = ["--bg-app", "--bg-chat", "--surface-1", "--surface-2",
            "--surface-3", "--surface-4", "--surface-sunken", "--surface-input"]
TEXT = ["--text-strong", "--text", "--text-subtle", "--text-muted", "--text-faint",
        "--text-link", "--text-accent", "--danger-text", "--warning-text",
        "--positive-text", "--info"]

print("== Поверхности (light) ==")
for name in SURFACES:
    item = light[name]
    print(f"{name:18} {item['value']:9} [{item['tag']}] {item['ref']:12} {item['use']}")

print("\n== Текст: контраст к поверхностям (мин. 4.5) ==")
header = "| Токен | " + " | ".join(s.replace("--", "") for s in SURFACES) + " | min |"
print(header)
print("|" + "---|" * (len(SURFACES) + 2))
for name in TEXT:
    cells = []
    values = []
    for surface in SURFACES:
        r = ratio(name, surface)
        if r is None:
            cells.append("—")
        else:
            values.append(r)
            cells.append(f"{r:.2f}" + ("" if r >= 4.5 else " ✗"))
    minimum = min(values)
    print(f"| `{name}` `{light[name]['value']}` | " + " | ".join(cells) + f" | {minimum:.2f} |")

print("\n== Non-text: контраст к поверхностям (мин. 3.0) ==")
NONTEXT = ["--accent", "--accent-hover", "--accent-active", "--danger", "--positive",
           "--status-online", "--status-idle", "--status-dnd", "--status-offline",
           "--status-streaming", "--border-strong", "--focus-ring", "--scrollbar-thumb"]
for name in NONTEXT:
    values = [ratio(name, s) for s in SURFACES if ratio(name, s) is not None]
    print(f"{name:22} {light[name]['value']:9} min={min(values):.3f} "
          + " ".join(f"{v:.2f}" for v in values))

print("\n== Пары «текст на заливке» (мин. 4.5) ==")
PAIRS = [("--text-on-accent", "--accent"), ("--text-on-danger", "--danger-fill"),
         ("--text-on-warning", "--warning"), ("--text-inverse", "--accent")]
for fg, bg in PAIRS:
    r = ratio(fg, bg)
    print(f"{fg:20} на {bg:16} {light[fg]['value']} / {light[bg]['value']} = {r:.3f}")
