#!/usr/bin/env python3
"""RW-29 / W29b — targeted-мутации RTP-медиа с восстановлением по md5.

Мутируется только продукт (`src/media/rtp.rs`, `src/media/jitter.rs`); тесты,
Cargo-файлы и golden-фикстура не меняются. Каждая мутация откатывается из
in-memory копии, после прогона проверяется md5 всех файлов.

Запуск:
    python3 waves/RW-29/run-mutations-w29b.py

Итог печатает строкой вида:
    caught=9/9 restored_all=True base_md5=rtp:<md5>,jitter:<md5>
"""

import hashlib
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path("/home/user/orca/project")
RUST = ROOT / "rust"
SRC = RUST / "crates/discord-voice/src/media"
RTP = SRC / "rtp.rs"
JITTER = SRC / "jitter.rs"
TARGET = "/tmp/opencode/target-w29b"

MUTATIONS = [
    (
        "M1",
        RTP,
        "профиль расширения 0xBEDE подменён",
        "pub const EXTENSION_PROFILE: u16 = 0xBEDE;",
        "pub const EXTENSION_PROFILE: u16 = 0x0BED;",
    ),
    (
        "M2",
        RTP,
        "words = len/4 считается как len/8",
        "u16::try_from(data.len() / EXTENSION_WORD)",
        "u16::try_from(data.len() / (EXTENSION_WORD * 2))",
    ),
    (
        "M3",
        RTP,
        "marker сдвинут в бит 6 вместо 7",
        "bytes.push((u8::from(self.marker) << 7) | (self.payload_type & 0x7f));",
        "bytes.push((u8::from(self.marker) << 6) | (self.payload_type & 0x7f));",
    ),
    (
        "M4",
        RTP,
        "timestamp растёт на 2 кадра вместо 1",
        "self.timestamp = self.timestamp.wrapping_add(FRAME_TICKS);",
        "self.timestamp = self.timestamp.wrapping_add(FRAME_TICKS * 2);",
    ),
    (
        "M5",
        RTP,
        "marker не снимается после первого пакета",
        "self.marker_pending = false;",
        "self.marker_pending = true;",
    ),
    (
        "M7",
        RTP,
        "порог comfort-noise 3 Б → 2 Б",
        "const COMFORT_NOISE_MAX_ENCRYPTED_LEN: usize = 3;",
        "const COMFORT_NOISE_MAX_ENCRYPTED_LEN: usize = 2;",
    ),
    (
        "M8",
        JITTER,
        "уже отданный seq не считается дублем",
        "if self.slots.contains_key(&sequence) || self.delivered.contains(&sequence) {",
        "if self.slots.contains_key(&sequence) {",
    ),
    (
        "M9",
        JITTER,
        "окно не расширяется назад до первой выдачи",
        "        if let Some(start) = self.window_start\n            && self.started\n            && is_behind(sequence, start)",
        "        if let Some(start) = self.window_start\n            && is_behind(sequence, start)",
    ),
    (
        "M10",
        JITTER,
        "выданный seq не сдвигает окно",
        "            self.window_start = Some(start.wrapping_add(1));\n            self.started = true;\n            self.record_delivered(start);",
        "            self.window_start = Some(start);\n            self.started = true;\n            self.record_delivered(start);",
    ),
]


def md5(path: pathlib.Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_tests() -> tuple[int, str]:
    proc = subprocess.run(
        ["cargo", "test", "--offline", "-p", "discord-voice", "--test", "media_contract"],
        cwd=RUST,
        env={**os.environ, "CARGO_TARGET_DIR": TARGET},
        capture_output=True,
        text=True,
        timeout=600,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main() -> int:
    originals = {RTP: RTP.read_text(encoding="utf-8"), JITTER: JITTER.read_text(encoding="utf-8")}
    base = {path: md5(path) for path in originals}
    print(f"base_md5=rtp:{base[RTP]},jitter:{base[JITTER]}")

    caught = 0
    restored_all = True
    for ident, path, description, old, new in MUTATIONS:
        source = originals[path]
        count = source.count(old)
        if count != 1:
            print(f"{ident}: FAIL anchor count {count} != 1 ({description})", flush=True)
            return 2
        path.write_text(source.replace(old, new), encoding="utf-8")
        try:
            code, output = run_tests()
        finally:
            path.write_text(originals[path], encoding="utf-8")
            # mtime должен быть новее артефакта мутанта, иначе cargo переиспользует его
            path.touch()
        restored = md5(path) == base[path]
        restored_all = restored_all and restored

        failed = re.findall(r"^test (\S+) \.\.\. FAILED$", output, re.MULTILINE)
        compile_error = "error[E" in output or "could not compile" in output
        if code == 0:
            verdict = "MISSED"
        elif compile_error and not failed:
            verdict = "INCONCLUSIVE (compile)"
        else:
            verdict = "caught"
            caught += 1
        tests = ",".join(failed) if failed else "—"
        print(
            f"{ident}: exit={code} {verdict} restored={restored} [{tests}] — {description}",
            flush=True,
        )
        if not restored:
            print(f"{ident}: RESTORE FAILED", file=sys.stderr)
            return 3

    total = len(MUTATIONS)
    print(
        f"caught={caught}/{total} restored_all={restored_all} "
        f"base_md5=rtp:{base[RTP]},jitter:{base[JITTER]}"
    )
    return 0 if caught == total and restored_all else 1


if __name__ == "__main__":
    sys.exit(main())
