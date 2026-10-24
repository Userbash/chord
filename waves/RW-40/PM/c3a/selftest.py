#!/usr/bin/env python3
"""C3a — негативный self-test валидатора: проверки не должны быть пустыми.

Каждый кейс портит копию theme-spec.json (или THEME-SPEC.md) и требует exit 1
от validate.py. Если валидатор пропускает порчу — это провал.
"""
import copy
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
VALIDATOR = ROOT / "waves/RW-40/PM/c3a/validate.py"
SPEC = ROOT / "waves/RW-40/theme-spec.json"
DOC = ROOT / "waves/RW-40/THEME-SPEC.md"
TMP = Path("/tmp/opencode/c3a-selftest")
TMP.mkdir(parents=True, exist_ok=True)
BASE = json.loads(SPEC.read_text(encoding="utf-8"))
DOC_TEXT = DOC.read_text(encoding="utf-8")

failures = []


def run(spec_path, doc_path):
    proc = subprocess.run(
        [sys.executable, str(VALIDATOR), str(spec_path), str(doc_path)],
        capture_output=True, text=True, cwd=ROOT,
    )
    return proc.returncode, proc.stdout + proc.stderr


def case(label, spec=None, doc_text=None, expect=1, needle="FAIL"):
    spec_path = TMP / f"{label}.json"
    doc_path = TMP / f"{label}.md"
    spec_path.write_text(json.dumps(spec if spec is not None else BASE, ensure_ascii=False),
                         encoding="utf-8")
    doc_path.write_text(doc_text if doc_text is not None else DOC_TEXT, encoding="utf-8")
    code, out = run(spec_path, doc_path)
    hit = needle in out
    ok = (code == expect) and (hit if expect == 1 else not hit)
    print(f"{'ok  ' if ok else 'FAIL'} {label}: exit={code} needle={hit}")
    if not ok:
        failures.append(label)
        print(out[-800:])


# 0. базовая копия проходит
case("baseline", expect=0, needle="FAILED")

# 1. расхождение [T]-значения с дампом
bad = copy.deepcopy(BASE)
bad["light"]["tokens"]["--bg-app"]["value"] = "#ffffff"
case("wrong-token-value", spec=bad)

# 2. неизвестное имя переменной
bad = copy.deepcopy(BASE)
bad["light"]["tokens"]["--not-a-token"] = {"value": "#ffffff", "tag": "A", "ref": "test"}
case("unknown-name", spec=bad)

# 3. провал AA: светлый текст на светлом фоне
bad = copy.deepcopy(BASE)
bad["light"]["tokens"]["--text"]["value"] = "#dddddd"
case("aa-failure", spec=bad)

# 4. плотность расходится с layout.json
bad = copy.deepcopy(BASE)
bad["density"]["cozy"]["--message-gutter"]["value"] = "72px"
case("layout-mismatch", spec=bad)

# 5. внешняя ссылка в документе
case("doc-external-ref", doc_text=DOC_TEXT + "\n<!-- url(x) -->\n")

# 6. шрифтовое подключение в JSON
bad = copy.deepcopy(BASE)
bad["light"]["tokens"]["--bg-app"]["note"] = "@font-face x"
case("json-font-face", spec=bad)

print()
if failures:
    print(f"FAILED self-test: {failures}")
    sys.exit(1)
print("OK: валидатор ловит все 6 мутаций")
