#!/usr/bin/env python3
"""RW-38: live-мутации GUI — доказывают, что AT-SPI-проба ловит регрессии."""
import hashlib, json, pathlib, subprocess, sys
ROOT = pathlib.Path(__file__).resolve().parents[2]
APP = ROOT / "rust" / "gui-spike" / "ui" / "app.js"
CARGO = ["cargo", "build", "--offline", "--manifest-path", "rust/gui-spike/Cargo.toml"]
PROBE = ["python3", "scripts/gui-window-probe.py", "--json", "/tmp/opencode/gui-mut.json"]
MUTS = [
    ("G1", "MEMBER_ROWS 24229 -> 100",
     "const MEMBER_ROWS = 24_229;", "const MEMBER_ROWS = 100;"),
    ("G2", "убрана настройка «Превью» (38 -> 37)",
     '"Разрешение", "FPS", "Зеркало", "Превью",',
     '"Разрешение", "FPS", "Зеркало",'),
]
def md5(b): return hashlib.md5(b).hexdigest()
orig = APP.read_bytes(); base = md5(orig)
log = [f"# GUI mutations, app.js md5={base}"]
try:
    for mid, desc, old, new in MUTS:
        if old not in orig.decode():
            log.append(f"{mid} INVALID: anchor not found"); continue
        APP.write_text(orig.decode().replace(old, new, 1))
        b = subprocess.run(CARGO, cwd=ROOT, capture_output=True, text=True)
        p = subprocess.run(PROBE, cwd=ROOT, capture_output=True, text=True, timeout=90)
        status = "caught" if p.returncode == 1 else ("survived" if p.returncode == 0 else f"env/{p.returncode}")
        err = ""
        try: err = json.loads((ROOT/".."/".."/"tmp"/"opencode"/"gui-mut.json").resolve().read_text()).get("error","")
        except Exception: err = (p.stderr or "").strip().splitlines()[-1] if p.stderr else ""
        log.append(f"{mid} {status}: {desc} | build_rc={b.returncode} | {err[:160]}")
        APP.write_bytes(orig)
        subprocess.run(CARGO, cwd=ROOT, capture_output=True, text=True)
finally:
    APP.write_bytes(orig)
    subprocess.run(CARGO, cwd=ROOT, capture_output=True, text=True)
after = md5(APP.read_bytes())
log.append(f"# restore: {after} {'OK' if after == base else 'MISMATCH'}")
(ROOT / "waves" / "RW-38" / "mutations-gui.log").write_text("\n".join(log) + "\n")
print("\n".join(log))
sys.exit(0 if after == base else 1)
