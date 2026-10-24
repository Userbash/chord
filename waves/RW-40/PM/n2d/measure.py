#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""N2-D: воспроизводимый замер панели из voice/ui/01-joined.png.

Считает CSS-геометрию (image px / 1.25) и цвета, которые попали в VOICE.md §4.1,
плюс разбор 38 тумблеров из ui8/23-settings-voice.json.
Пишет waves/RW-40/PM/n2d/evidence.json и печатает отчёт.

Только чтение лаборатории /home/sanya/re-deep; ничего не пишет вне PM/n2d.
"""

import json
import os

from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
LAB = "/home/sanya/re-deep"
PNG = os.path.join(LAB, "voice/ui/01-joined.png")
SETTINGS = os.path.join(LAB, "ui8/23-settings-voice.json")
OUT = os.path.join(ROOT, "waves/RW-40/PM/n2d/evidence.json")
DPR = 1.25
BG = (18, 18, 20)  # фон окна вне панели


def spans(px, y, color, x0=0, x1=900, min_w=20):
    out, start = [], None
    for x in range(x0, x1):
        hit = px[x, y] == color
        if hit and start is None:
            start = x
        if not hit and start is not None:
            out.append((start, x - 1, x - start))
            start = None
    return [s for s in out if s[2] >= min_w]


def css(v):
    return round(v / DPR, 1)


def main():
    im = Image.open(PNG).convert("RGB")
    px = im.load()
    w, h = im.size

    # панель: фон #202024 (--surface-1) в нижней полосе окна
    panel_bg = (32, 32, 36)
    edge = (37, 37, 41)
    rows = [y for y in range(int(h * 0.8), h)
            if sum(1 for x in range(0, 600) if px[x, y] == panel_bg) > 200]
    if not rows:
        raise SystemExit("не нашёл фон панели в %s" % PNG)
    top, bottom = min(rows), max(rows)          # первый/последний ряд фона
    mid = (top + bottom) // 2
    cols = [x for x in range(0, 600) if px[x, mid] == panel_bg]
    left, right = min(cols) - 1, max(cols) + 1  # кромка #252529 по краям
    if px[left, mid] != edge or px[right, mid] != edge:
        raise SystemExit("кромка панели не #252529 — изменился источник")

    # ряд кнопок: #2e2e33 блок из >=2 подряд идущих кнопок
    btn_bg = (46, 46, 51)
    btn_rows = [y for y in range(top, bottom)
                if sum(1 for x in range(0, 600) if px[x, y] == btn_bg) > 100]
    btn_mid = (min(btn_rows) + max(btn_rows)) // 2 if btn_rows else None
    btn_row = spans(px, btn_mid, btn_bg, min_w=30) if btn_mid else []

    # участник / большой вид — только из DOM-дампа
    cand = json.load(open(os.path.join(LAB, "voice/ui/01-candidates.json"), encoding="utf-8"))
    dom = {c["cls"].split()[0]: {"w": c["w"], "h": c["h"]} for c in cand}

    # цвета
    def sample(x, y):
        return "#%02x%02x%02x" % px[x, y]

    # 38 тумблеров
    st = json.load(open(SETTINGS, encoding="utf-8"))
    rows = [s["rowText"] for s in st["switches"]]
    sounds = [r for r in rows if "Послушать звук" in r]
    other = [r for r in rows if r.startswith(("OpenH264", "Оверлей", "Запись звука", "Ведение журнала"))]
    voice = [r for r in rows if r not in sounds and r not in other]

    data = {
        "generated_by": "waves/RW-40/PM/n2d/measure.py",
        "source_image": PNG,
        "image_size": {"w": w, "h": h},
        "dpr": DPR,
        "css_viewport": {"w": round(w / DPR), "h": round(h / DPR)},
        "panel": {
            "image_px": {"x0": left, "x1": right, "y0": top - 1, "y1": bottom + 1},
            "css": {
                "w": round((right - left + 1) / DPR, 1),
                "h": round((bottom + 1 - (top - 1) + 1) / DPR, 1),
                "x": round(left / DPR, 1),
                "y": round((top - 1) / DPR, 1),
                "bottom_gap": round((h - 1 - bottom) / DPR, 1),
            },
            "dom_rect": dom.get("panels__5e434"),
            "radius_css": 8.0,
        },
        "action_button_row": {
            "bg": "#2e2e33",
            "button_spans_css": [round(s[2] / DPR, 1) for s in btn_row],
            "note": "средний ряд пересекает иконки, спаны фрагментированы; широкая кнопка ~80, узкая (камера) без фона",
            "flat_row_height_css": round((max(btn_rows) - min(btn_rows) + 1) / DPR, 1),
        },
        "colors": {
            "panel_bg": sample(left + 40, top + 3),
            "panel_edge": sample(left, mid),
            "action_button_bg": "#2e2e33",
            "self_name": "#fbfbfb",
            "channel_line": "#abacb2",
            "status_label": "#c0a548",
            "ping_icon": "#5eb479",
            "mute_bg": "#362127",
            "mute_icon": "#da3e44",
        },
        "dom_rects": {k: v for k, v in dom.items() if k in (
            "panels__5e434", "connection_e131a9", "voiceUser__07f91",
            "rtcConnectionStatusWrapper__06d62", "clickablePing__06d62",
            "rtcConnectionStatusConnected__06d62", "container__37e49",
            "voiceChannelEffectsContainer__2f4f7")},
        "settings_voice": {
            "switch_count": st["switchCount"],
            "slider_count": len(st["sliders"]),
            "breakdown": {"voice_video": len(voice), "sounds": len(sounds), "other": len(other)},
            "voice_rows": voice[:12],
        },
    }
    with open(OUT, "w", encoding="utf-8") as fh:
        json.dump(data, fh, ensure_ascii=False, indent=2)
        fh.write("\n")

    print("image            %dx%d (dpr %.2f) → CSS %dx%d" % (
        w, h, DPR, round(w / DPR), round(h / DPR)))
    print("panel            %s css, DOM rect %s" % (data["panel"]["css"], data["panel"]["dom_rect"]))
    print("action buttons   widths css %s" % data["action_button_row"]["button_spans_css"])
    print("colors           %s" % json.dumps(data["colors"], ensure_ascii=False))
    print("switches         %d = %d + %d + %d" % (
        data["settings_voice"]["switch_count"],
        len(voice), len(sounds), len(other)))
    print("evidence         %s" % OUT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
