#!/usr/bin/env python3
"""TR-184A: 5 точечных мутаций app-shell-wiring.js с гарантированным восстановлением."""
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path("/home/sanya/orca/workspaces/project/feature-task-271-ui-app-shell-wiring")
TARGET = REPO / "rust/gui-spike/ui/features/app-shell-wiring.js"
TEST = REPO / "tests/unit/app-shell-wiring.test.mjs"
ORIG_MD5 = "928cbb029bee1ae75ce4e80336a7a356"
BACKUP = Path("/tmp/opencode/mut271/app-shell-wiring.orig.js")
OUT = Path("/tmp/opencode/mut271/logs")
OUT.mkdir(parents=True, exist_ok=True)

MUTATIONS = [
    {
        "id": "M1",
        "title": "openModalLayer не закрывает активную админ-панель",
        "old": "    adminRouter.closeActivePanel();\n    return shellBridge.openLayer(layerId, payload);",
        "new": "    return shellBridge.openLayer(layerId, payload);",
    },
    {
        "id": "M2",
        "title": "openAdminPanel не закрывает модальные слои",
        "old": "    shellBridge.closeAllLayers();\n    return adminRouter.openPanel(panelId, payload);",
        "new": "    return adminRouter.openPanel(panelId, payload);",
    },
    {
        "id": "M3",
        "title": "closeTopLayer игнорирует приоритет админ-панели",
        "old": "    if (adminRouter.getActivePanel() !== null) {\n      return adminRouter.closeActivePanel();\n    }\n    return shellBridge.closeTopLayer();",
        "new": "    return shellBridge.closeTopLayer();",
    },
    {
        "id": "M4",
        "title": "IPC-префикс панели искажён (guild_admin: -> guild_admin-)",
        "old": "`guild_admin:${panelId}:${action}`",
        "new": "`guild_admin-${panelId}-${action}`",
    },
    {
        "id": "M5",
        "title": "destroy не закрывает модальные слои",
        "old": "    adminRouter.destroy();\n    shellBridge.closeAllLayers();",
        "new": "    adminRouter.destroy();",
    },
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_tests():
    proc = subprocess.run(
        ["node", "--test", str(TEST)],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main() -> int:
    shutil.copy2(TARGET, BACKUP)
    start_md5 = md5(TARGET)
    if start_md5 != ORIG_MD5:
        print(f"STOP: стартовый MD5 {start_md5} != {ORIG_MD5}")
        return 2
    print(f"baseline md5 ok: {start_md5}")

    results = []
    for mut in MUTATIONS:
        original = BACKUP.read_text()
        if original.count(mut["old"]) != 1:
            print(f"STOP {mut['id']}: шаблон найден {original.count(mut['old'])} раз")
            shutil.copy2(BACKUP, TARGET)
            return 3
        TARGET.write_text(original.replace(mut["old"], mut["new"]))
        mut_md5 = md5(TARGET)
        if mut_md5 == ORIG_MD5:
            print(f"STOP {mut['id']}: файл не изменился")
            shutil.copy2(BACKUP, TARGET)
            return 4
        code, output = run_tests()
        (OUT / f"{mut['id']}.log").write_text(output)
        killed = code != 0
        fail_line = next(
            (ln for ln in output.splitlines() if ln.startswith("ℹ fail ")), "нет строки fail"
        )
        results.append(
            {
                "id": mut["id"],
                "title": mut["title"],
                "mutant_md5": mut_md5,
                "exit": code,
                "killed": killed,
                "fail_line": fail_line.strip(),
            }
        )
        print(f"{mut['id']}: exit={code} killed={killed} {fail_line.strip()} :: {mut['title']}")
        shutil.copy2(BACKUP, TARGET)
        if md5(TARGET) != ORIG_MD5:
            print(f"STOP {mut['id']}: восстановление не удалось")
            return 5

    final_md5 = md5(TARGET)
    payload = {"original_md5": ORIG_MD5, "final_md5": final_md5, "mutations": results}
    Path("/tmp/opencode/mut271/results.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    )
    print(f"final md5: {final_md5} (match={final_md5 == ORIG_MD5})")
    killed = sum(1 for r in results if r["killed"])
    print(f"killed {killed}/{len(results)}")
    return 0 if final_md5 == ORIG_MD5 and killed == len(results) else 6


if __name__ == "__main__":
    sys.exit(main())
