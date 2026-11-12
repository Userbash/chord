#!/usr/bin/env python3
"""5 мутационных проверок кода task-137 (ZstdStream на libzstd)."""
import hashlib
import pathlib
import subprocess
import sys

ROOT = pathlib.Path("/home/user/orca/workspaces/project/feature-task-137-zstd-stream-flush")
SRC = ROOT / "rust/crates/discord-codec/src/zstd.rs"
BASE_MD5 = "6563379e4a71ec6875ad1b678d31975b"
LOG_DIR = pathlib.Path(__file__).resolve().parent

MUTATIONS = [
    (
        "M1-window-hold",
        "Вернуть удержание (ruzstd-поведение): выход только после завершения кадра",
        """            if produced > 0 {
                out.extend_from_slice(&output.as_slice()[..produced]);
            }""",
        """            if produced > 0 && self.finished {
                out.extend_from_slice(&output.as_slice()[..produced]);
            }""",
    ),
    (
        "M2-decoder-reset",
        "Терять историю окна: пересоздавать декодер на каждой итерации",
        """        while !self.finished {
            let mut input = InBuffer::around(&self.pending[offset..]);""",
        """        while !self.finished {
            self.decoder = Decoder::new().expect("reset");
            let mut input = InBuffer::around(&self.pending[offset..]);""",
    ),
    (
        "M3-no-frame-end",
        "Не фиксировать завершение кадра (убрать ветку hint == 0)",
        """            if hint == 0 {
                // Кадр дочитан до `last_block`: весь выход уже в `out`.
                self.finished = true;
            } else if taken == 0 && produced == 0 {""",
        """            if taken == 0 && produced == 0 {""",
    ),
    (
        "M4-no-trailing-check",
        "Молча терять байты после завершённого кадра (убрать TrailingData)",
        """        if self.finished && !self.pending.is_empty() {
            self.dead = true;
            return Err(ZstdError::TrailingData {
                bytes: self.pending.len(),
            });
        }
""",
        "",
    ),
    (
        "M5-clear-instead-of-drain",
        "Сбрасывать накопитель целиком вместо съёма принятых байт",
        """        self.pending.drain(..offset);""",
        """        self.pending.clear();""",
    ),
]

def md5(path):
    return hashlib.md5(path.read_bytes()).hexdigest()

def main():
    assert md5(SRC) == BASE_MD5, f"исходник не совпадает с базой: {md5(SRC)}"
    base = SRC.read_text(encoding="utf-8")
    results = []
    for name, desc, old, new in MUTATIONS:
        assert base.count(old) == 1, f"{name}: шаблон найден {base.count(old)} раз"
        SRC.write_text(base.replace(old, new, 1), encoding="utf-8")
        log = LOG_DIR / f"{name}.log"
        with log.open("w", encoding="utf-8") as handle:
            proc = subprocess.run(
                ["cargo", "test", "--offline", "--manifest-path", "rust/Cargo.toml",
                 "-p", "discord-codec", "-p", "discord-core"],
                cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT,
                env={"CARGO_TARGET_DIR": "/home/user/orca/project/rust/target",
                     "PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": "/home/user"},
                timeout=900,
            )
        text = log.read_text(encoding="utf-8")
        failed = sorted({line.split()[1] for line in text.splitlines()
                         if line.startswith("test ") and line.rstrip().endswith("FAILED")})
        results.append((name, desc, proc.returncode, failed, log))
        SRC.write_text(base, encoding="utf-8")
        assert md5(SRC) == BASE_MD5, f"{name}: восстановление не удалось"
        print(f"{name}: exit={proc.returncode}, FAILED={len(failed)}")
        for test in failed:
            print(f"    killed by: {test}")
    print("\n=== ИТОГ ===")
    killed = sum(1 for _, _, code, failed, _ in results if code != 0 and failed)
    for name, desc, code, failed, log in results:
        status = "KILLED" if code != 0 and failed else "SURVIVED"
        print(f"{status:8} {name}: {desc} (exit={code}, log={log})")
    print(f"killed {killed}/{len(results)}")
    return 0 if killed == len(results) else 1

if __name__ == "__main__":
    sys.exit(main())
