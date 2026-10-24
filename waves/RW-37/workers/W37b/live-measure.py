#!/usr/bin/env python3
"""RW-37 / W37b: предварительный перезамер GUI-спайка с оконной моделью.

Адаптация `waves/RW-36/remeasure.py` (тот файл не изменяется): запускает спайк с
autobench, ждёт `BENCH`, затем снимает RSS/PSS дерева процессов и кладёт артефакты
в каталог воркера. Финальный замер на приёмке делает PM своим скриптом.

    python3 waves/RW-37/workers/W37b/live-measure.py [--wait-s 40]

Коды выхода: 0 — артефакты получены, 2 — среда/бинарник/лог недоступны.
"""
import argparse
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
SPIKE = ROOT / "rust" / "gui-spike"
BINARY = SPIKE / "target" / "debug" / "discord-gui-spike"
WAVE = ROOT / "waves" / "RW-37" / "workers" / "W37b"
LOG = WAVE / "app.log"


def run(argv: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(argv, cwd=str(ROOT), text=True, capture_output=True, **kwargs)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wait-s", type=int, default=40)
    args = parser.parse_args(argv)

    if not BINARY.is_file():
        print(f"live-measure: нет бинарника {BINARY}", file=sys.stderr)
        return 2

    env = os.environ.copy()
    env.update({
        "SPIKE_EXIT_AFTER_MS": str(args.wait_s * 1000 + 10_000),
        "SPIKE_AUTOBENCH": "1",
        "GDK_BACKEND": "x11",
        "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
        "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
        "LIBGL_ALWAYS_SOFTWARE": "1",
    })
    LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG, "w", encoding="utf-8") as log:
        proc = subprocess.Popen([str(BINARY)], cwd=str(SPIKE), env=env,
                                stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + args.wait_s
            bench = False
            while time.monotonic() < deadline:
                text = LOG.read_text(encoding="utf-8")
                if "BENCH " in text:
                    bench = True
                    break
                if proc.poll() is not None:
                    break
                time.sleep(0.05)
            if not bench:
                print("live-measure: BENCH не появился", file=sys.stderr)
                return 2

            pid = str(proc.pid)
            checks = [
                (["node", "scripts/gui-spike-bench.mjs", "--pid", pid,
                  "--duration-ms", "3000", "--interval-ms", "200",
                  "--json", str(WAVE / "metrics.json")], "metrics.json"),
                (["node", "scripts/gui-spike-bench.mjs", "--pid", pid, "--tree",
                  "--duration-ms", "3000", "--interval-ms", "200",
                  "--json", str(WAVE / "metrics-tree.json")], "metrics-tree.json"),
                (["python3", "waves/RW-36/measure-pss.py", "--pid", pid,
                  "--json", str(WAVE / "pss.json")], "pss.json"),
            ]
            for cmd, label in checks:
                result = run(cmd, timeout=60)
                if result.returncode != 0:
                    print(f"live-measure: {label} FAILED: {result.stderr.strip()}", file=sys.stderr)
                    return 2
                print(f"{label}: {result.stdout.strip()}")
        finally:
            if proc.poll() is None:
                proc.send_signal(signal.SIGTERM)
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()

    print("app.log:")
    print(LOG.read_text(encoding="utf-8").strip())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
