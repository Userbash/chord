#!/usr/bin/env python3
"""RW-28 / P7-01c — targeted-мутации relative.rs с восстановлением по md5."""
import hashlib
import os
import pathlib
import re
import subprocess

ROOT = pathlib.Path("/home/sanya/orca/project")
SRC = ROOT / "rust/crates/discord-i18n/src/relative.rs"
RUST = ROOT / "rust"
TARGET = "/tmp/opencode/target-w28"

NUMBER_BLOCK = "    let number = format_integer_unsigned(magnitude, row.locale);"

MUTATIONS = [
    (
        "M1",
        "past/future перепутаны",
        "if value < 0 { row.past } else { row.future }",
        "if value < 0 { row.future } else { row.past }",
    ),
    (
        "M2",
        "категория всегда other",
        ".find(|(candidate, _)| *candidate == category)",
        '.find(|(candidate, _)| *candidate == "other")',
    ),
    ("M3", "Auto игнорируется", "numeric == RelativeNumeric::Auto", "numeric == RelativeNumeric::Always"),
    ("M4", "Short → long", 'Self::Short => "short",', 'Self::Short => "long",'),
    ("M5", "плейсхолдер не замещается", 'template.replace("{0}", &number)', 'template.replace("{0}", "")'),
    ("M6", "без группировки", NUMBER_BLOCK, "    let number = magnitude.to_string();"),
    ("M7", "ноль считается прошлым", "if value < 0 { row.past }", "if value <= 0 { row.past }"),
    ("M8", "Minute → second", 'Self::Minute => "minute",', 'Self::Minute => "second",'),
]


def md5(path: pathlib.Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run() -> tuple[int, str]:
    proc = subprocess.run(
        ["cargo", "test", "--offline", "-p", "discord-i18n", "--test", "relative_contract"],
        cwd=RUST,
        env={**os.environ, "CARGO_TARGET_DIR": TARGET},
        capture_output=True,
        text=True,
        timeout=300,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main() -> int:
    original = SRC.read_text(encoding="utf-8")
    base = md5(SRC)
    rows = []
    caught = 0
    for ident, desc, old, new in MUTATIONS:
        count = original.count(old)
        if count != 1:
            print(f"{ident}: anchor count {count} != 1 -> {desc}")
            return 2
        SRC.write_text(original.replace(old, new), encoding="utf-8")
        try:
            code, output = run()
        finally:
            SRC.write_text(original, encoding="utf-8")
        restored = md5(SRC) == base
        failed = re.findall(r"^test (\S+) \.\.\. FAILED$", output, re.MULTILINE)
        compile_error = "error[E" in output or "could not compile" in output
        verdict = "MISSED" if code == 0 else ("INCONCLUSIVE" if compile_error and not failed else "caught")
        caught += verdict == "caught"
        rows.append((ident, desc, code, verdict, failed[:2], restored))
    for row in rows:
        print(row)
    print(f"caught={caught}/{len(MUTATIONS)} restored_all={all(r[5] for r in rows)} base_md5={base}")
    return 0 if caught == len(MUTATIONS) else 1


if __name__ == "__main__":
    raise SystemExit(main())
