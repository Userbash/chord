#!/usr/bin/env python3
"""REVIEW-199A — 5 точечных мутаций `rust/gui-spike/ui/app.js` с обязательным откатом.

Методика: каждая замена проверяется на уникальность (ровно 1 вхождение, иначе abort),
после прогона `node --test tests/unit/app-shell-wiring.test.mjs` файл восстанавливается
из исходного снимка в памяти, MD5 сверяется с базовым после каждой мутации.

Запуск: python3 waves/RW-40/G2/task-292-fe-app-shell-mount/mutations/mutate-199a.py
Логи:   mutations/M1.log … M5.log + mutations/run.log
Выход:  0 — все 5 убиты и файл побайтно восстановлен; 1 — есть выжившая мутация/рассинхрон.
"""

import hashlib
import pathlib
import subprocess
import sys

ROOT = pathlib.Path("/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount")
APP = ROOT / "rust/gui-spike/ui/app.js"
OUT = ROOT / "waves/RW-40/G2/task-292-fe-app-shell-mount/mutations"
APP_CARD = pathlib.Path("/home/user/orca/project/waves/RW-40/G2/task-292-fe-app-shell-mount/mutations")
TEST_FILE = "tests/unit/app-shell-wiring.test.mjs"

MUTATIONS = [
    (
        "M1",
        "import createAppShellWiring: путь ./features/app-shell-wiring.js -> ./features/shell-wiring.js",
        b'import { createAppShellWiring } from "./features/app-shell-wiring.js";',
        b'import { createAppShellWiring } from "./features/shell-wiring.js";',
    ),
    (
        "M2",
        "shellWiring.onDispatchIpc: invoke(command, payload) -> () => undefined",
        b"""  const shellWiring = createAppShellWiring({
    onDispatchIpc: (command, payload) =>
      invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
  });""",
        b"""  const shellWiring = createAppShellWiring({
    onDispatchIpc: () => undefined,
  });""",
    ),
    (
        "M3",
        "boot(): shellWiring больше не возвращается в объекте",
        b"    shellWiring,\n    composerPopoversBridge,\n",
        b"    composerPopoversBridge,\n",
    ),
    (
        "M4",
        "composerPopoversBridge.onDispatchIpc: invoke(command, payload) -> (payload), вызова IPC нет",
        b"""      onDispatchIpc: (command, payload) =>
        invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
    }),""",
        b"""      onDispatchIpc: (command, payload) =>
        (payload),
    }),""",
    ),
    (
        "M5",
        "финальный export: убран createComposerPopoversBridge",
        b"  createAppShellWiring,\n  createComposerPopoversBridge,\n};",
        b"  createAppShellWiring,\n};",
    ),
]


def main() -> int:
    original = APP.read_bytes()
    base_md5 = hashlib.md5(original).hexdigest()
    OUT.mkdir(parents=True, exist_ok=True)
    summary = [f"# REVIEW-199A mutation run", f"# app.js base md5: {base_md5}", ""]
    print(f"base md5: {base_md5}", flush=True)
    survived = 0

    for name, description, old, new in MUTATIONS:
        count = original.count(old)
        if count != 1:
            print(f"ABORT {name}: шаблон встречается {count} раз(а)", flush=True)
            return 2
        APP.write_bytes(original.replace(old, new))
        proc = subprocess.run(
            ["node", "--test", TEST_FILE],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        output = proc.stdout + proc.stderr
        (OUT / f"{name}.log").write_text(
            f"# {name}: {description}\nmutation applied, exit={proc.returncode}\n\n{output}",
            encoding="utf-8",
        )
        killed = proc.returncode != 0
        failing = [line.strip() for line in output.splitlines() if line.strip().startswith("\u2716 app.js")]
        print(f"\n=== {name}: {description} ===", flush=True)
        print(f"exit={proc.returncode} => {'KILLED' if killed else 'SURVIVED'}", flush=True)
        for line in failing[:4]:
            print(f"  {line}", flush=True)
        survived += 0 if killed else 1
        APP.write_bytes(original)
        restored_md5 = hashlib.md5(APP.read_bytes()).hexdigest()
        if restored_md5 != base_md5:
            print(f"RESTORE MISMATCH {restored_md5} != {base_md5}", flush=True)
            return 3
        summary.append(
            f"{name}\t{'KILLED' if killed else 'SURVIVED'}\t{description}\t"
            f"killer={'; '.join(failing[:2]) or '-'}\tmd5_after_restore={restored_md5}"
        )

    summary.append("")
    summary.append(f"RESULT: killed {len(MUTATIONS) - survived}/{len(MUTATIONS)}")
    summary.append("FINAL md5: " + hashlib.md5(APP.read_bytes()).hexdigest() + f" (base {base_md5})")
    (OUT / "run.log").write_text("\n".join(summary) + "\n", encoding="utf-8")
    if APP_CARD.resolve() != OUT.resolve():
        (APP_CARD / "run.log").write_text("\n".join(summary) + "\n", encoding="utf-8")
    print("\n".join(summary[-3:]), flush=True)
    return 0 if survived == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
