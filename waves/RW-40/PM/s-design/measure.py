#!/usr/bin/env python3
"""S-D / STREAM: воспроизводимый замер ряда действий голосовой панели.

Источник: /home/user/clean-room-oracle/voice/ui/01-joined.png (2560x1383, dpr 1.25).
Метод: ищем точный цвет заливки кнопок #2e2e33 = rgb(46,46,51) в области панели,
группируем по столбцам/строкам, делим на dpr. Никаких «на глаз».

Запуск: python3 measure.py [--png PATH] [--log PATH]
Код выхода: 0 — ок, 1 — расхождение с контрактом, 2 — нет входа (skip).
"""
import argparse
import sys
from collections import Counter
from pathlib import Path

PNG = Path("/home/user/clean-room-oracle/voice/ui/01-joined.png")
LOG = Path(__file__).with_name("measure.log")
DPR = 1.25
FILL = (46, 46, 51)  # #2e2e33

# область панели в px изображения (CSS 6.4..368.8 x 950.4..1100.8)
REGION = (5, 470, 1180, 1380)


def runs(values, max_gap=2):
    out, start, prev = [], None, None
    for v in sorted(values):
        if start is None:
            start = prev = v
            continue
        if v - prev <= max_gap:
            prev = v
        else:
            out.append((start, prev))
            start = prev = v
    if start is not None:
        out.append((start, prev))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--png", default=str(PNG))
    ap.add_argument("--log", default=str(LOG))
    args = ap.parse_args()

    lines = []
    try:
        from PIL import Image  # noqa: WPS433
    except ImportError:
        lines.append("SKIP pillow: not installed")
        Path(args.log).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print("\n".join(lines))
        return 2

    png = Path(args.png)
    if not png.exists():
        lines.append(f"SKIP png: {png} missing")
        Path(args.log).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print("\n".join(lines))
        return 2

    im = Image.open(png).convert("RGB")
    lines.append(f"input: {png} size={im.size[0]}x{im.size[1]} dpr={DPR}")
    x0, x1, y0, y1 = REGION
    px = im.load()

    xs, ys = [], []
    for y in range(y0, min(y1, im.size[1])):
        for x in range(x0, min(x1, im.size[0])):
            if px[x, y] == FILL:
                xs.append(x)
                ys.append(y)

    if not xs:
        lines.append("FAIL fill: цвет #2e2e33 не найден в области панели")
        Path(args.log).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print("\n".join(lines))
        return 1

    col_runs = [r for r in runs(xs) if r[1] - r[0] > 5]
    row_runs = [r for r in runs(ys, max_gap=1) if r[1] - r[0] > 5]
    widths = [round((b - a + 1) / DPR, 1) for a, b in col_runs]
    gaps = [round((col_runs[i + 1][0] - col_runs[i][1] - 1) / DPR, 1) for i in range(len(col_runs) - 1)]
    steps = [round((col_runs[i + 1][0] - col_runs[i][0]) / DPR, 1) for i in range(len(col_runs) - 1)]
    height = round((row_runs[0][1] - row_runs[0][0] + 1) / DPR, 1) if row_runs else None

    lines.append(f"buttons_with_fill={len(col_runs)} widths_css={widths}")
    lines.append(f"gap_css={gaps} step_css={steps}")
    lines.append(f"fill_height_css={height} (кнопка 32 = --btn-height, радиус 8 съедает заливку)")
    lines.append(f"first_fill_x_css={round(col_runs[0][0] / DPR, 1)} last_fill_x_css={round(col_runs[-1][1] / DPR, 1)}")

    failures = []
    if len(col_runs) != 3:
        failures.append(f"ожидалось 3 залитые кнопки (камера без заливки), получено {len(col_runs)}")
    if any(not (78.0 <= w <= 82.0) for w in widths):
        failures.append(f"ширина вне 78..82: {widths}")
    if any(not (7.0 <= g <= 9.0) for g in gaps):
        failures.append(f"зазор вне 7..9: {gaps}")
    if height is None or not (25.0 <= height <= 33.0):
        failures.append(f"высота заливки вне 25..33: {height}")

    for f in failures:
        lines.append(f"FAIL {f}")
    lines.append(f"checks={4 + len(gaps)} failures={len(failures)}")
    lines.append("RESULT: " + ("OK" if not failures else "FAIL"))

    Path(args.log).write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
