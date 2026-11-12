#!/usr/bin/env python3
"""Independent mutation testing for RW-05/RW-06 (reviewer).

For every mutation: backup -> exact replacement -> targeted cargo test -> restore ->
verify md5. Never touches test files. Results in mutations.txt.
"""
import hashlib
import os
import shutil
import subprocess
import sys

ROOT = "/home/user/orca/project/rust"
SRC = os.path.join(ROOT, "crates/discord-core/src")
OUT = "/tmp/opencode/review-rw-0506"
BK = os.path.join(OUT, "mut-backup")
os.makedirs(BK, exist_ok=True)


def md5(path):
    return hashlib.md5(open(path, "rb").read()).hexdigest()


FILES = {
    "resume": os.path.join(SRC, "resume.rs"),
    "ws": os.path.join(SRC, "ws.rs"),
    "identify": os.path.join(SRC, "identify.rs"),
    "ready": os.path.join(SRC, "ready.rs"),
}
for key, path in FILES.items():
    shutil.copy2(path, os.path.join(BK, key + ".rs"))
BEFORE = {k: md5(p) for k, p in FILES.items()}

# (id, wave, description, file key, old, new, test target)
MUTATIONS = [
    # ---------------- RW-06 ----------------
    (
        "R1", "RW-06", "op6 без конверта {op,d} (payload под d отсутствует)",
        "resume",
        """    let frame = EtfValue::Map(vec![
        (key("op"), EtfValue::Int(6)),
        (
            key("d"),
            EtfValue::Map(vec![
                (key("token"), EtfValue::Str(params.token.clone())),
                (key("session_id"), EtfValue::Str(params.session_id.clone())),
                (key("seq"), int(params.seq)),
            ]),
        ),
    ]);""",
        """    let frame = EtfValue::Map(vec![
        (key("token"), EtfValue::Str(params.token.clone())),
        (key("session_id"), EtfValue::Str(params.session_id.clone())),
        (key("seq"), int(params.seq)),
    ]);""",
        "resume_reconnect",
    ),
    (
        "R2", "RW-06", "seq-1 в RESUME",
        "resume",
        '(key("seq"), int(params.seq)),',
        '(key("seq"), int(params.seq.saturating_sub(1))),',
        "resume_reconnect",
    ),
    (
        "R3", "RW-06", "session_id подменён токеном",
        "resume",
        '(key("session_id"), EtfValue::Str(params.session_id.clone())),',
        '(key("session_id"), EtfValue::Str(params.token.clone())),',
        "resume_reconnect",
    ),
    (
        "R4", "RW-06", "порог 30 000 → 300 000 мс",
        "resume",
        "pub const RESUME_MAX_DOWNTIME_MS: u64 = 30_000;",
        "pub const RESUME_MAX_DOWNTIME_MS: u64 = 300_000;",
        "resume_reconnect",
    ),
    (
        "R5", "RW-06", "игнорируется invalid_session=false",
        "resume",
        """    if invalid_session == Some(false) {
        return ReconnectAction::Identify;
    }
""",
        "",
        "resume_reconnect",
    ),
    (
        "R6", "RW-06", "на resume-соединении приоритет у IDENTIFY (op2 вместо op6)",
        "ws",
        """    let frame = if let Some(params) = resume {
        build_resume_frame(params).map_err(TransportError::Resume)?
    } else if let Some(params) = opts.identify.as_ref() {
        build_identify_frame(params).map_err(TransportError::Identify)?
    } else {
        return Ok(());
    };""",
        """    let frame = if let Some(params) = opts.identify.as_ref() {
        build_identify_frame(params).map_err(TransportError::Identify)?
    } else if let Some(params) = resume {
        build_resume_frame(params).map_err(TransportError::Resume)?
    } else {
        return Ok(());
    };""",
        "resume_reconnect",
    ),
    (
        "R7", "RW-06", "resume_state() всегда None",
        "ws",
        """    pub fn resume_state(&self) -> Option<ResumeState> {
        let session_id = self.last_ready.as_ref()?.session_id.as_ref()?;
        let last_seq = self.session.last_seq()?;
        Some(ResumeState {
            session_id: session_id.clone(),
            last_seq,
        })
    }""",
        """    pub fn resume_state(&self) -> Option<ResumeState> {
        None
    }""",
        "resume_reconnect",
    ),
    (
        "R8", "RW-06", "last_invalid_session_resumable() всегда None",
        "ws",
        """    pub fn last_invalid_session_resumable(&self) -> Option<bool> {
        self.last_invalid_session_resumable
    }""",
        """    pub fn last_invalid_session_resumable(&self) -> Option<bool> {
        None
    }""",
        "resume_reconnect",
    ),
    (
        "R9", "RW-06", "op9 d=0 (int) не распознаётся",
        "ws",
        "        EtfValue::Int(0) => Some(false),\n",
        "",
        "resume_reconnect",
    ),
    (
        "R10", "RW-06", "RESUME до HELLO (снят guard HELLO)",
        "ws",
        "if *sent || pipe.session().hello().is_none() {",
        "if *sent {",
        "resume_reconnect",
    ),
    # ---------------- RW-05 ----------------
    (
        "I1", "RW-05", "capabilities − 1",
        "identify",
        "pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;",
        "pub const CAPABILITIES_ACTIVE: u64 = 1_734_652;",
        "identify_ready",
    ),
    (
        "I2", "RW-05", "compress всегда true",
        "identify",
        '(key("compress"), bool_atom(p.compress)),',
        '(key("compress"), bool_atom(true)),',
        "identify_ready",
    ),
    (
        "I3", "RW-05", "installation_id не попадает в properties",
        "identify",
        """    if let Some(installation_id) = &p.installation_id {
        pairs.push((
            key("installation_id"),
            EtfValue::Str(installation_id.clone()),
        ));
    }
""",
        "",
        "identify_ready",
    ),
    (
        "I4", "RW-05", "READY user.id только Int (строка теряется)",
        "ready",
        """                    EtfValue::Int(number) => Some(number.to_string()),
                    EtfValue::Str(text) => Some(text.clone()),
""",
        """                    EtfValue::Int(number) => Some(number.to_string()),
""",
        "identify_ready",
    ),
    (
        "I5", "RW-05", "READY partial int-1 не читается",
        "ready",
        "        EtfValue::Int(1) => Some(true),\n",
        "",
        "identify_ready",
    ),
    (
        "I6", "RW-05", "IDENTIFY до HELLO (снят guard HELLO)",
        "ws",
        "if *sent || pipe.session().hello().is_none() {",
        "if *sent {",
        "identify_ready",
    ),
    (
        "I7", "RW-05", "IDENTIFY повторяется (снят sent-флаг)",
        "ws",
        "if *sent || pipe.session().hello().is_none() {",
        "if pipe.session().hello().is_none() {",
        "identify_ready",
    ),
    (
        "I8", "RW-05", "ws шлёт payload без конверта {op,d}",
        "ws",
        "build_identify_frame(params).map_err(TransportError::Identify)?",
        "build_identify(params).map_err(TransportError::Identify)?",
        "identify_ready",
    ),
    (
        "I9", "RW-05", "READY top_keys + 1",
        "ready",
        "top_keys: d.as_map().map_or(0, <[_]>::len),",
        "top_keys: d.as_map().map_or(0, <[_]>::len) + 1,",
        "identify_ready",
    ),
    (
        "I10", "RW-05", "READY last_ready не запоминается",
        "ws",
        "self.last_ready = Some(ready);",
        "let _ = ready;",
        "identify_ready",
    ),
]


def restore():
    for key, path in FILES.items():
        shutil.copy2(os.path.join(BK, key + ".rs"), path)


def run_one(mid, wave, desc, fkey, old, new, target):
    path = FILES[fkey]
    text = open(path, encoding="utf-8").read()
    count = text.count(old)
    if count != 1:
        return f"{mid} | {wave} | INVALID | old occurs {count}x | {desc}"
    open(path, "w", encoding="utf-8").write(text.replace(old, new, 1))
    try:
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", "discord-core", "--test", target],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        out = proc.stdout
    finally:
        restore()
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
    print(line, flush=True)
    # md5 must be back to baseline before the next mutation
    now = {k: md5(p) for k, p in FILES.items()}
    if now != BEFORE:
        print("RESTORE MISMATCH", {k: (BEFORE[k], now[k]) for k in BEFORE if BEFORE[k] != now[k]}, file=sys.stderr)
        sys.exit(2)

open(os.path.join(OUT, "mutations.txt"), "w", encoding="utf-8").write("\n".join(lines) + "\n")
print("--- md5 after all mutations ---")
for key, path in FILES.items():
    print(key, md5(path), "==", BEFORE[key], md5(path) == BEFORE[key])
