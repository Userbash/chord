#!/usr/bin/env python3
"""RW-39 / W39a — пиксельные замеры референсов (read-only).

Источники: /home/sanya/re-deep/ui/00-app.png (PNG, lossless),
/home/sanya/re-deep/ui8/22-members.jpg (JPEG, панель участников),
/home/sanya/re-deep/ui/log.json (dpr/размер окна), ui8/*.json (DOM rect'ы).

Все числа на выходе — CSS px (пиксели PNG / dpr 1.25). Скрипт ничего не пишет
в /home/sanya/re-deep. Запуск:
    python3 waves/RW-39/PM/w39a/measure.py            # таблица в stdout
    python3 waves/RW-39/PM/w39a/measure.py --json     # + measure.json рядом

Статус чисел: [M] measured (этот скрипт) — см. SPEC.md §1.
"""
from __future__ import annotations

import json
import statistics
import sys
from collections import Counter
from pathlib import Path

from PIL import Image

RE = Path('/home/sanya/re-deep')
OUT = Path(__file__).resolve().parent
APP = RE / 'ui/00-app.png'
MEMBERS = RE / 'ui8/22-members.jpg'
LOG = RE / 'ui/log.json'


def css(px: float, dpr: float) -> float:
    return round(px / dpr, 2)


def runs(values: list[tuple[int, int, int]]) -> list[tuple[int, int, tuple[int, int, int]]]:
    out, prev, start = [], None, 0
    for i, v in enumerate(values):
        if v != prev:
            if prev is not None:
                out.append((start, i - 1, prev))
            prev, start = v, i
    out.append((start, len(values) - 1, prev))
    return out


def main() -> int:
    dpr = json.loads(LOG.read_text(encoding='utf-8'))[0]['dpr']
    app = Image.open(APP).convert('RGB')
    px = app.load()
    W, H = app.size
    m = {}  # type: dict[str, object]

    m['viewport'] = {
        'png_px': [W, H],
        'dpr': dpr,
        'css': [css(W, dpr), css(H, dpr)],
        'source': 'ui/log.json[0].{dpr,size} + PNG IHDR',
    }

    # --- shell: верхние полосы (titlebar/header) по столбцу в чате ---
    col = [px[1500, y] for y in range(0, 300)]
    bands = [(a, b, c) for a, b, c in runs(col) if b - a >= 2]
    titlebar_px = bands[0][1] + 1
    header_a, header_b = bands[1][0], bands[1][1]
    header_px = header_b - header_a + 1
    m['titlebar'] = {'px': titlebar_px, 'css': css(titlebar_px, dpr), 'source': 'столбец x=1500: полоса #121214 до первой смены цвета'}
    m['header'] = {
        'band_px': header_px,
        'band_css': css(header_px, dpr),
        'border_px': bands[2][0] - header_b - 1,
        'with_border_css': css(header_px + 1, dpr),
        'source': 'столбец x=1500: полоса (26,26,30) + 1px граница; токен --custom-channel-header-height=49px',
    }

    # --- граница дерева и чата (x) ---
    row = [px[x, 900] for x in range(0, W)]
    chat_left_px = next(i for i, c in enumerate(row) if c == (26, 26, 30))
    m['tree_right_edge_css'] = css(chat_left_px, dpr)
    m['tree_w'] = {'css': css(chat_left_px, dpr) - 64, 'formula': 'граница панелей − rail 64 [L]',
                   'source': 'строка y=900: первый x с цветом чата (26,26,30)'}

    # --- панель участников (DOM-rect из ui8) ---
    mem = json.loads((RE / 'ui8/22-members.json').read_text(encoding='utf-8'))
    m['member_panel'] = {
        'x0': mem['rect']['x0'], 'y0': mem['rect']['y0'],
        'w': mem['rect']['w'], 'h': mem['rect']['h'],
        'source': 'ui8/22-members.json rect (getBoundingClientRect, CSS px)',
    }
    m['chat_w'] = {
        'css': round(mem['rect']['x0'] - css(chat_left_px, dpr), 1),
        'source': 'x0 панели участников − левая граница чата',
    }

    # --- composer и user bar (нижние полосы) ---
    col_bottom = [px[1500, y] for y in range(H - 120, H)]
    bb = [(a + H - 120, b + H - 120, c) for a, b, c in runs(col_bottom) if b - a >= 2]
    comp = [b for b in bb if b[2] == (34, 35, 39)]
    if comp:
        a, b, _ = comp[0]
        m['composer'] = {'y_px': [a, b], 'css': {'y': [css(a, dpr), css(b + 1, dpr)], 'h': css(b - a + 1, dpr)},
                         'source': 'столбец x=1500: бокс #222327 (34,35,39)'}
    # ширина composer: столбцы, закрашенные #222327 на всю высоту бокса
    if comp:
        y0, y1 = comp[0][0], comp[0][1]
        cols = [x for x in range(W)
                if sum(1 for y in range(y0 + 1, y1) if px[x, y] == (34, 35, 39)) > (y1 - y0) * 0.8]
        if cols:
            m['composer_width'] = {
                'css': {'x': [css(cols[0], dpr), css(cols[-1] + 1, dpr)], 'w': css(cols[-1] - cols[0] + 1, dpr)},
                'source': 'столбцы, закрашенные (34,35,39) на ≥80% высоты composer',
            }

    # user bar (в дереве каналов)
    col_tree = [px[400, y] for y in range(H - 120, H)]
    tb = [(a + H - 120, b + H - 120, c) for a, b, c in runs(col_tree) if b - a >= 2]
    ub = [b for b in tb if b[2] == (32, 32, 36)]
    if ub:
        a, b, _ = ub[0]
        m['userbar'] = {'css': {'y': [css(a, dpr), css(b + 1, dpr)], 'h': css(b - a + 1, dpr)},
                        'source': 'столбец x=400 (дерево): полоса #202024 (32,32,36)'}

    # --- строка канала: высота выделенной строки и шаг ---
    sel = [y for y in range(400, 1300) if px[300, y] == (44, 44, 48)]
    if sel:
        m['channel_row_selected'] = {'css': css(sel[-1] - sel[0] + 1, dpr),
                                     'y_px': [sel[0], sel[-1]],
                                     'source': 'x=300: непрерывная полоса выделения #2c2c30'}
    # шаг: центры текстовых полос в дереве
    ys = [y for y in range(560, 1300)
          if any(max(abs(px[x, y][i] - 18) for i in (0, 1, 2)) > 8 for x in range(96, 460))]
    clusters, s, p = [], ys[0], ys[0]
    for y in ys[1:]:
        if y - p > 1:
            clusters.append((s, p))
            s = y
        p = y
    clusters.append((s, p))
    centers = [(a + b) / 2 for a, b in clusters]
    diffs = [centers[i + 1] - centers[i] for i in range(len(centers) - 1)]
    diffs = [d for d in diffs if 25 < d < 45]
    m['channel_row_pitch'] = {'css': css(statistics.median(diffs), dpr), 'n': len(diffs),
                              'source': 'медиана шага текстовых полос дерева (x 96…460, y 560…1300)'}

    # --- шаг строки участника по JPEG-кропу панели ---
    jpg = Image.open(MEMBERS).convert('RGB')
    jp = jpg.load()
    JW, JH = jpg.size
    bg = Counter(jp[x, y] for x in range(0, JW, 3) for y in range(0, JH, 3)).most_common(1)[0][0]
    prof = [max(max(abs(jp[x, y][i] - bg[i]) for i in (0, 1, 2)) for x in range(8, 52)) for y in range(JH)]
    blocks, on, start = [], False, 0
    for y, v in enumerate(prof):
        if v > 30 and not on:
            on, start = True, y
        elif v <= 30 and on:
            on = False
            blocks.append((start, y - 1))
    if on:
        blocks.append((start, JH - 1))
    # строки участников = блоки высотой с аватар (35…50 px); заголовки групп короче
    tops = [a for a, b in blocks if 35 <= b - a + 1 <= 50]
    steps = [tops[i + 1] - tops[i] for i in range(len(tops) - 1)]
    steps = [s for s in steps if 45 <= s <= 60]
    m['member_row_pitch'] = {'css': css(statistics.median(steps), dpr), 'n': len(steps),
                             'source': 'ui8/22-members.jpg: медиана шага верхушек аватаров (x 8…52); JPEG → допуск ±1'}

    # --- цвета: контрольные точки ---
    def hexat(x, y):
        r, g, b = px[x, y]
        return '#%02x%02x%02x' % (r, g, b)

    m['colors'] = {
        'app_bg_rail': hexat(20, 700),
        'app_bg_tree': hexat(460, 700),
        'chat_bg': hexat(1500, 600),
        'header_bg': hexat(1500, 70),
        'selected_channel': hexat(300, 590),
        'userbar': hexat(400, 1340),
        'composer': hexat(1000, 1340),
        'accent_unread_bar': hexat(1200, 120),
        'titlebar_bg': hexat(1500, 10),
        'source': 'точечные пробы PNG (lossless)',
    }
    # статус-точки в панели участников
    dots = Counter()
    for x in range(2230, W):
        for y in range(90, H):
            r, g, b = px[x, y]
            if max(r, g, b) > 90 and max(r, g, b) - min(r, g, b) > 55:
                dots[(r, g, b)] += 1
    m['colors']['member_panel_saturated'] = [
        {'hex': '#%02x%02x%02x' % c, 'px': n} for c, n in dots.most_common(6)
    ]

    # --- DOM-rect'ы из ui8 (настройки/поиск) ---
    sc = json.loads((RE / 'ui8/23-settings-categories.json').read_text(encoding='utf-8'))
    sv = json.loads((RE / 'ui8/23-settings-voice.json').read_text(encoding='utf-8'))
    sb = json.loads((RE / 'ui8/20a-searchbar.json').read_text(encoding='utf-8'))
    m['settings_modal'] = {'rect': sc['rect'], 'rows': sc['count'],
                           'source': 'ui8/23-settings-categories.json rect (DOM, CSS px)'}
    m['settings_voice'] = {'rect': sv['rect'], 'switches': sv['switchCount'],
                           'source': 'ui8/23-settings-voice.json rect + switchCount'}
    m['search_field'] = {'boxes': sb['bars'][:3],
                         'source': 'ui8/20a-searchbar.json bars[] (DOM, CSS px)'}

    text = json.dumps(m, ensure_ascii=False, indent=1)
    print(text)
    if '--json' in sys.argv:
        (OUT / 'measure.json').write_text(text + '\n', encoding='utf-8')
        print(f'\nwritten {OUT / "measure.json"}', file=sys.stderr)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
