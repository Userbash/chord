#!/usr/bin/env python3
"""RW-36 / B07: PSS-замер дерева процессов GUI-спайка.

RSS-сумма завышает память (shared-библиотеки считаются в каждом процессе).
PSS (proportional set size) — честный footprint стека; по нему сверяется
бюджет ARCHITECTURE §9 (≤300 МБ).

    python3 waves/RW-36/measure-pss.py --pid <PID> [--json out.json]

Коды выхода: 0 — успех, 1 — аргументы, 2 — среда/процесс недоступны.
"""
import argparse
import json
import os
import sys


def children_of(pid: int) -> list[int]:
    """Дочерние PID по всем потокам (потомка мог породить не главный поток)."""
    result: list[int] = []
    try:
        for task in os.listdir(f"/proc/{pid}/task"):
            try:
                with open(f"/proc/{pid}/task/{task}/children", encoding="utf-8") as fh:
                    result.extend(int(x) for x in fh.read().split())
            except OSError:
                continue
    except OSError:
        pass
    return result


def collect_tree(root: int) -> list[int]:
    seen: set[int] = set()
    order: list[int] = []
    stack = [root]
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        order.append(pid)
        stack.extend(children_of(pid))
    return order


def smaps(pid: int) -> tuple[int, int]:
    """Возвращает (Rss, Pss) в КиБ; недоступный /proc → (0, 0) с пропуском."""
    rss = pss = 0
    try:
        with open(f"/proc/{pid}/smaps_rollup", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("Rss:"):
                    rss = int(line.split()[1])
                elif line.startswith("Pss:"):
                    pss = int(line.split()[1])
    except OSError:
        return 0, 0
    return rss, pss


def comm(pid: int) -> str:
    try:
        with open(f"/proc/{pid}/comm", encoding="utf-8") as fh:
            return fh.read().strip()
    except OSError:
        return "?"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--json", dest="json_path")
    try:
        args = parser.parse_args(argv)
    except SystemExit as err:
        return 0 if err.code == 0 else 1

    if args.pid <= 0 or not os.path.isdir(f"/proc/{args.pid}"):
        print(f"measure-pss: /proc/{args.pid} недоступен (skip-aware)", file=sys.stderr)
        return 2

    processes = []
    total_rss = total_pss = 0
    for pid in collect_tree(args.pid):
        rss, pss = smaps(pid)
        processes.append({"pid": pid, "comm": comm(pid), "rss_kb": rss, "pss_kb": pss})
        total_rss += rss
        total_pss += pss

    result = {
        "pid": args.pid,
        "processes": processes,
        "total_rss_kb": total_rss,
        "total_pss_kb": total_pss,
        "total_pss_mb": round(total_pss / 1024, 1),
    }
    payload = json.dumps(result, ensure_ascii=False)
    if args.json_path:
        try:
            with open(args.json_path, "w", encoding="utf-8") as fh:
                fh.write(payload + "\n")
        except OSError as err:
            print(f"measure-pss: не удалось записать {args.json_path}: {err}", file=sys.stderr)
            return 2
    print(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
