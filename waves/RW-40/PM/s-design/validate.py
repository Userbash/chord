#!/usr/bin/env python3
"""S-D / STREAM: валидатор дизайн-контракта (MD + JSON) без сети и без кода продукта.

Проверки:
  1  json валиден, schema/card
  2  каждое число с px в MD помечено [T]/[M]/[D]/[A]/[F]
  3  все токены (--name) из MD и *_token в JSON существуют в tokens.css
  4  каждая строка copy совпадает с ru-catalog.json (независимый оракул)
  5  все id/состояния/ошибки/тесты/пробелы/отклонения из JSON упомянуты в MD
  6  «win32-only» про screen_previews/window_previews сказано явно
  7  нет внешних ассетов (http(s)://, ![ , <img)
  8  слово «паритет» не стоит без оговорки
  9  parity_claim=false и presence unmeasured

Запуск: python3 validate.py [--log PATH]
Код выхода: 0 — все проверки прошли, 1 — есть FAIL, 2 — нет входа.
"""
import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]  # waves/RW-40/PM/s-design -> project root
DOC = ROOT / "waves/RW-40/DESIGN/STREAM.md"
SPEC = ROOT / "waves/RW-40/DESIGN/stream-spec.json"
TOKENS = ROOT / "waves/RW-39/DESIGN/tokens.css"
CATALOG = Path("/home/user/clean-room-oracle/ru-catalog.json")
LOG = HERE / "validate.log"

LABELS = ("[T]", "[M]", "[D]", "[A]", "[F]", "[L]", "[S]")
PARITY_OK = ("не заявлен", "не заявляется", "не паритет", "не выдавать", "гипотеза по коду")
PX_EXEMPT = re.compile(r"^\|\s*Каждое число|^2\. \*\*Не выдумывать px")


def flat(v):
    if isinstance(v, str):
        return v
    if isinstance(v, (int, float)):
        return str(v)
    if isinstance(v, list):
        return "".join(flat(x) for x in v)
    if isinstance(v, dict):
        return "".join(flat(x) for x in v.values())
    return ""


def collect(obj, key):
    out = []
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k == key and isinstance(v, str):
                out.append(v)
            out.extend(collect(v, key))
    elif isinstance(obj, list):
        for v in obj:
            out.extend(collect(v, key))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", default=str(LOG))
    args = ap.parse_args()

    log, fails, skips, checks = [], 0, 0, 0

    def check(ok, name, detail=""):
        nonlocal fails, checks
        checks += 1
        if ok:
            log.append(f"PASS {name}" + (f": {detail}" if detail else ""))
        else:
            fails += 1
            log.append(f"FAIL {name}" + (f": {detail}" if detail else ""))

    def skip(name, reason):
        nonlocal skips
        skips += 1
        log.append(f"SKIP {name}: {reason}")

    for p in (DOC, SPEC, TOKENS):
        if not p.exists():
            log.append(f"FATAL missing input: {p}")
            Path(args.log).write_text("\n".join(log) + "\n", encoding="utf-8")
            print("\n".join(log))
            return 2

    md = DOC.read_text(encoding="utf-8")
    spec = json.loads(SPEC.read_text(encoding="utf-8"))
    tokens_css = TOKENS.read_text(encoding="utf-8")

    check(True, "json: valid")
    check(spec.get("schema") == "rw40/stream-spec-v1", "schema == rw40/stream-spec-v1")
    check(spec.get("card") == "S-D", "card == S-D")
    check(spec.get("doc", "").endswith("STREAM.md"), "doc указывает на STREAM.md")

    # 2. px-метки
    bad = [
        f"{i}: {line.strip()[:110]}"
        for i, line in enumerate(md.splitlines(), 1)
        if "px" in line
        and not any(lbl in line for lbl in LABELS)
        and not PX_EXEMPT.search(line)
    ]
    check(not bad, "md: каждое число с px помечено", f"{len(bad)} строк без метки" if bad else "0")
    for b in bad:
        log.append(f"  px-bad {b}")

    # 3. токены
    used = set(re.findall(r"--([a-z0-9-]+)", md))
    used |= set(re.findall(r'"_token":\s*"([a-z0-9-]+)"', SPEC.read_text(encoding="utf-8")))
    used |= {t for t in collect(spec, "height_token") + collect(spec, "radius_token")}
    used |= set(collect(spec, "width_token")) | set(collect(spec, "bg_token")) | set(collect(spec, "z_token"))
    used |= set(collect(spec, "shadow_token")) | set(collect(spec, "bar_token")) | set(collect(spec, "timer_token"))
    used |= set(collect(spec, "gap_token")) | set(collect(spec, "avatar_token")) | set(collect(spec, "padding_token"))
    used |= set(collect(spec, "opacity_token")) | set(collect(spec, "tile_radius_token")) | set(collect(spec, "tile_bg_token"))
    used |= set(collect(spec, "stage_bg_token")) | set(collect(spec, "placeholder_bg_token")) | set(collect(spec, "tile_min_width_basis_token"))
    used |= set(collect(spec, "nameplate_height_token"))
    for lst in collect(spec, "tokens") + collect(spec, "padding_tokens") + collect(spec, "title_tokens") + collect(spec, "badge_tokens"):
        pass
    for key in ("tokens", "padding_tokens", "title_tokens", "badge_tokens"):
        for item in collect(spec, key):
            if isinstance(item, str) and item.startswith(("modal", "font", "line", "badge", "space")):
                used.add(item)
    missing = sorted(t for t in used if f"--{t}:" not in tokens_css)
    check(not missing, "tokens: все имена существуют в tokens.css", f"нет: {missing}" if missing else f"{len(used)} имён")

    # 4. копирайт
    if not CATALOG.exists():
        skip("copy: сверка с ru-catalog.json", "каталог недоступен")
    else:
        catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
        bad_copy, n = [], 0
        for row in spec.get("copy", []):
            n += 1
            # compare with NBSP normalized: каталог использует U+00A0 в «… 60 FPS»
            want = flat(catalog.get(row["key"])).replace("\u00a0", " ")
            got = row["text"].replace("\u00a0", " ")
            if want != got:
                bad_copy.append(f"{row['key']}: md={row['text']!r} catalog={flat(catalog.get(row['key']))!r}")
        check(not bad_copy, "copy: строки совпали с ru-catalog.json", f"{n} строк" if not bad_copy else f"{len(bad_copy)} расхождений")
        for b in bad_copy[:10]:
            log.append(f"  copy-bad {b}")

    # 5. согласованность MD <-> JSON
    dom_ids = list(spec["dom"]["ids"]) + list(spec["dom"]["voice_ids"])
    not_in_md = [i for i in dom_ids if f"#{i}" not in md and f"`{i}`" not in md and i not in md]
    check(not not_in_md, "md: все id из DOM-контракта упомянуты", f"нет: {not_in_md}" if not_in_md else f"{len(dom_ids)} id")

    tests = spec["tests"]["g2"] + spec["tests"]["g4"] + spec["tests"]["g5"]
    miss_tests = [t["id"] for t in tests if t["id"] not in md]
    check(not miss_tests, "md: все тест-кейсы описаны", f"нет: {miss_tests}" if miss_tests else f"{len(tests)} кейсов")

    gaps = [g["id"] for g in spec["gaps"]]
    miss_gaps = [g for g in gaps if g not in md]
    check(not miss_gaps, "md: все пробелы описаны", f"нет: {miss_gaps}" if miss_gaps else f"{len(gaps)} пробелов")

    devs = [d["id"] for d in spec["deviations"]]
    miss_devs = [d for d in devs if d not in md]
    check(not miss_devs, "md: все отклонения описаны", f"нет: {miss_devs}" if miss_devs else f"{len(devs)} отклонений")

    states = spec["states"]["stream"] + spec["states"]["camera"] + spec["states"]["view"]
    miss_states = [s for s in states if s not in md]
    check(not miss_states, "md: все состояния описаны", f"нет: {miss_states}" if miss_states else f"{len(states)} состояний")

    reasons = [e["reason"] for e in spec["errors"]]
    miss_reasons = [r for r in reasons if r not in md]
    check(not miss_reasons, "md: все коды ошибок описаны", f"нет: {miss_reasons}" if miss_reasons else f"{len(reasons)} кодов")

    risks = [r["id"] for r in spec["risks"]]
    miss_risks = [r for r in risks if r not in md]
    check(not miss_risks, "md: все риски описаны", f"нет: {miss_risks}" if miss_risks else f"{len(risks)} рисков")

    # 6. платформа
    check("screen_previews" in md and "win32" in md, "linux: screen_previews помечен win32-only")
    check("window_previews" in md and "win32" in md, "linux: window_previews помечен win32-only")
    check("v4l2" in md and "PipeWire" in md and "X11" in md, "linux: PipeWire/X11/v4l2 описаны")

    # 7. ассеты
    ext = [p for p in ("http://", "https://", "![", "<img") if p in md]
    check(not ext, "assets: внешних ссылок/вставок нет", f"найдено: {ext}" if ext else "0")

    # 8. паритет
    bad_parity = [
        f"{i}: {line.strip()[:100]}"
        for i, line in enumerate(md.splitlines(), 1)
        if "паритет" in line.lower() and not any(q in line.lower() for q in PARITY_OK)
    ]
    check(not bad_parity, "md: «паритет» только с оговоркой", f"{len(bad_parity)} строк" if bad_parity else "0")
    for b in bad_parity:
        log.append(f"  parity-bad {b}")

    check(spec.get("checks", {}).get("parity_claim") is False, "json: parity_claim == false")
    check(md.count("unmeasured") >= 3, "md: unmeasured использован", f"{md.count('unmeasured')} раз")

    # 9. consent
    consent = spec.get("consent", {}).get("invariants", [])
    check(len(consent) == 8, "consent: 8 инвариантов", str(len(consent)))
    forbidden = ["saveClipEx", "setClipsRecordingEnabled", "startLocalAudioRecording", "saveScreenshot", "getScreenPreviews"]
    check(all(f in md for f in forbidden), "consent: запрещённые API перечислены")

    log.append(f"checks={checks} failures={fails} skips={skips}")
    log.append("RESULT: " + ("OK" if fails == 0 else "FAIL"))
    Path(args.log).write_text("\n".join(log) + "\n", encoding="utf-8")
    print("\n".join(log))
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
