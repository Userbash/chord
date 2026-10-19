#!/usr/bin/env python3
"""Дополнительные мутации ревьюера: I8 (корректная версия) и R11.

I8: ws шлёт payload без конверта {op,d} через crate::identify::build_identify
    (в I8 исходного прогона замена на локальный build_identify не собралась — нет импорта).
R11: путаница вариантов EmptyToken/EmptySessionId в resume.rs (тест проверяет только is_err).
Каждая мутация: backup -> замена -> targeted cargo test -> restore -> проверка md5.
"""
import hashlib
import os
import shutil
import subprocess
import sys

ROOT = "/home/sanya/orca/project/rust"
SRC = os.path.join(ROOT, "crates/discord-core/src")
OUT = "/tmp/opencode/review-rw-0506"
BK = os.path.join(OUT, "mut-backup")

def md5(path):
    return hashlib.md5(open(path, "rb").read()).hexdigest()

FILES = {
    "ws": os.path.join(SRC, "ws.rs"),
    "resume": os.path.join(SRC, "resume.rs"),
}
BEFORE = {k: md5(p) for k, p in FILES.items()}

MUTATIONS = [
    (
        "I8-v2", "RW-05", "ws шлёт payload без конверта {op,d} (crate::identify::build_identify)",
        "ws",
        "build_identify_frame(params).map_err(TransportError::Identify)?",
        "crate::identify::build_identify(params).map_err(TransportError::Identify)?",
        "identify_ready",
    ),
    (
        "R11", "RW-06", "перепутаны варианты EmptyToken/EmptySessionId",
        "resume",
        """    if params.token.is_empty() {
        return Err(ResumeError::EmptyToken);
    }
    if params.session_id.is_empty() {
        return Err(ResumeError::EmptySessionId);
    }""",
        """    if params.token.is_empty() {
        return Err(ResumeError::EmptySessionId);
    }
    if params.session_id.is_empty() {
        return Err(ResumeError::EmptyToken);
    }""",
        "resume_reconnect",
    ),
]


def run_one(mid, wave, desc, fkey, old, new, target):
    path = FILES[fkey]
    text = open(path, encoding="utf-8").read()
    count = text.count(old)
    if count != 1:
        return f"{mid} | {wave} | INVALID | old occurs {count}x | {desc}"
    # backup именно этого файла в отдельный .bak
    shutil.copy2(path, os.path.join(OUT, f"{fkey}-{mid.lower()}.bak"))
    open(path, "w", encoding="utf-8").write(text.replace(old, new, 1))
    try:
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", "discord-core", "--test", target],
            cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        out = proc.stdout
    finally:
        shutil.copy2(os.path.join(BK, fkey + ".rs"), path)
    compile_err = "error[E" in out or "\nerror: could not compile" in out
    failed = [line.split()[1] for line in out.splitlines() if line.startswith("test ") and line.endswith("FAILED")]
    summary = next((line for line in out.splitlines() if line.startswith("test result:")), "no test result")
    if compile_err:
        verdict = "COMPILE-FAIL"
    elif failed:
        verdict = "CAUGHT"
    else:
        verdict = "SURVIVED"
    return f"{mid} | {wave} | {verdict} | {summary} | failing: {', '.join(failed) or '-'} | {desc}"


lines = []
for m in MUTATIONS:
    line = run_one(*m)
    lines.append(line)
    now = {k: md5(p) for k, p in FILES.items()}
    status = "OK" if now == BEFORE else "RESTORE MISMATCH"
    print(f"{line}\n    restore: {status}", flush=True)
    if now != BEFORE:
        sys.exit(2)

open(os.path.join(OUT, "mutations-extra.txt"), "w", encoding="utf-8").write("\n".join(lines) + "\n")
print("--- md5 after extra mutations ---")
for key, path in FILES.items():
    print(key, md5(path), "==", BEFORE[key], md5(path) == BEFORE[key])
