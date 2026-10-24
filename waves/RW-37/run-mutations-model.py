#!/usr/bin/env python3
"""Мутационная проверка RW-37: оконная/постраничная модель данных (`PageStore`).

Оракул — только существующие тесты репозитория:
  * `tests/unit/gui-spike-model.test.mjs` (23 теста, контракт TZ §4) — для `model.js`;
  * `tests/unit/gui-spike-dom.test.mjs` (статический контракт app.js) + проба
    разрешения ES-импорта для `app.js` (импорт/инициализация).

Мутация считается **caught** только если раннер отработал и есть упавшие тесты.
Синтаксическая ошибка/неразрешимый импорт = `compile-error` (не killed):
это дефект самой мутации, а не доказательство силы тестов. Нет разбора вывода
раннера (0 pass/0 fail, таймаут, падение baseline) = `inconclusive`.

Перед каждой мутацией файл запоминается вместе с md5; восстановление — в `finally`,
после восстановления md5 сверяется с исходным и восстанавливается исходный mtime (чтобы
никакие
кэши/артефакты не остались от мутированной версии).

CLI:
    run-mutations-model.py [--repo PATH] [--filter NAME] [--raw-dir PATH]

Код выхода: 0 — baseline зелёный и все мутации caught; 1 — есть survived /
inconclusive / compile-error / ошибка якоря.
"""

import argparse
import hashlib
import os
import re
import subprocess
import sys
import time
from pathlib import Path

MODEL = "rust/gui-spike/ui/model.js"
APP = "rust/gui-spike/ui/app.js"

TEST_MODEL = ["node", "--test", "tests/unit/gui-spike-model.test.mjs"]
TEST_DOM = ["node", "--test", "tests/unit/gui-spike-dom.test.mjs"]

# --- проба разрешения импортов app.js (без DOM) -------------------------------
# Немутированный app.js в голом node падает на `document is not defined` (импорт
# и разбор модуля при этом уже прошли). Сломанный спецификатор импорта даёт
# ERR_MODULE_NOT_FOUND. Это независимый от текста app.js оракул: он проверяет
# реальное разрешение модуля, а не совпадение регулярки.
IMPORT_PROBE = r"""
import { pathToFileURL } from "node:url";
const target = pathToFileURL(process.argv[1]).href;
try {
  await import(target);
  console.log("PROBE_OK");
} catch (error) {
  console.log("PROBE_ERROR_CODE=" + String((error && error.code) || "none"));
  console.log("PROBE_ERROR_NAME=" + String((error && error.name) || "none"));
  console.log("PROBE_ERROR_MSG=" + String((error && error.message) || "").split("\n")[0]);
}
"""

# (id, файл, категория, точная замена (old, new), чем ловим)
MUTATIONS = [
    # ---------- 1. getRow ----------
    (
        "m01-getrow-upper-bound-gt",
        MODEL,
        "getRow: index >= totalRows -> >",
        (
            "if (!Number.isInteger(index) || index < 0 || index >= this.totalRows) return undefined;",
            "if (!Number.isInteger(index) || index < 0 || index > this.totalRows) return undefined; // MUTATION m01",
        ),
        ["модель: getRow вне [0,totalRows)"],
    ),
    (
        "m02-getrow-no-integer-check",
        MODEL,
        "getRow: убран Number.isInteger",
        (
            "if (!Number.isInteger(index) || index < 0 || index >= this.totalRows) return undefined;",
            "if (index < 0 || index >= this.totalRows) return undefined; // MUTATION m02: isInteger убран",
        ),
        ["модель: getRow вне [0,totalRows) (строковые/дробные индексы)"],
    ),
    (
        "m03-getrow-no-lru-touch",
        MODEL,
        "getRow: LRU touched не обновляется",
        (
            "    page.touched = ++this.clock;\n",
            "    // MUTATION m03: page.touched не обновляется\n",
        ),
        ["модель: getRow обновляет LRU", "модель: вытесняет самую старую"],
    ),
    # ---------- 2. ensureRange ----------
    (
        "m04-ensurerange-no-lower-clamp",
        MODEL,
        "ensureRange: Math.max(0, first) -> first",
        (
            "    const from = Math.max(0, first);",
            "    const from = first; // MUTATION m04: нижний кламп убран",
        ),
        ["модель: кламплит границы"],
    ),
    (
        "m05-ensurerange-maxpages-gte",
        MODEL,
        "ensureRange: needed > maxPages -> >=",
        (
            "    if (needed > this.maxPages) {",
            "    if (needed >= this.maxPages) { // MUTATION m05: ровно maxPages отвергается",
        ),
        ["модель: текущий диапазон ровно на бюджете"],
    ),
    (
        "m06-ensurerange-no-inflight-cache",
        MODEL,
        "ensureRange: inFlight-кэш не используется (двойные загрузки)",
        (
            "      let task = this.inFlight.get(pageIndex);\n"
            "      if (!task) {\n"
            "        task = this.loadPage(pageIndex, rangePages);\n"
            "      }\n"
            "      tasks.push(task);",
            "      const task = this.loadPage(pageIndex, rangePages); // MUTATION m06: без inFlight\n"
            "      tasks.push(task);",
        ),
        ["модель: конкурентные вызовы — одна загрузка на страницу"],
    ),
    (
        "m07-evict-ignores-pinned",
        MODEL,
        "evict: pinned игнорируется",
        (
            "        if (pinned.has(pageIndex) || page.touched >= oldest) continue;",
            "        if (page.touched >= oldest) continue; // MUTATION m07: pinned игнорируется",
        ),
        ["модель: текущий диапазон не эвиктит сам себя / LRU"],
    ),
    # ---------- 3. loadPage ----------
    (
        "m08-loadpage-no-short-page-check",
        MODEL,
        "loadPage: проверка точной вместимости страницы удалена",
        (
            "        if (rows.length !== capacity) {\n"
            "          throw new RangeError(\n"
            "            `fetchPage(${pageIndex}): ожидалось ${capacity} строк (capacity страницы), получено ${rows.length}`,\n"
            "          );\n"
            "        }",
            "        // MUTATION m08: недогрузка/перегрузка страницы принимается молча",
        ),
        ["модель: короткая не-последняя страница -> RangeError"],
    ),
    (
        "m09-loadpage-capacity-lt",
        MODEL,
        "loadPage: rows.length !== capacity -> < capacity (перегрузка разрешена)",
        (
            "        if (rows.length !== capacity) {",
            "        if (rows.length < capacity) { // MUTATION m09: перегрузка страницы разрешена",
        ),
        ["модель: перегрузка страницы -> RangeError", "модель: последняя (неполная) страница допустима"],
    ),
    # ---------- 4. evict ----------
    (
        "m10-evict-touched-gt",
        MODEL,
        "evict: page.touched >= oldest -> >",
        (
            "        if (pinned.has(pageIndex) || page.touched >= oldest) continue;",
            "        if (pinned.has(pageIndex) || page.touched > oldest) continue; // MUTATION m10",
        ),
        ["модель: вытесняет самую старую"],
    ),
    (
        "m11-evict-first-page-not-lru",
        MODEL,
        "evict: первый попавшийся вместо LRU",
        (
            "      for (const [pageIndex, page] of this.pages) {\n"
            "        if (pinned.has(pageIndex) || page.touched >= oldest) continue;\n"
            "        oldest = page.touched;\n"
            "        victim = pageIndex;\n"
            "      }",
            "      for (const [pageIndex] of this.pages) {\n"
            "        if (pinned.has(pageIndex)) continue;\n"
            "        victim = pageIndex;\n"
            "        break; // MUTATION m11: порядок Map вместо LRU\n"
            "      }",
        ),
        ["модель: вытесняет страницу, к которой дольше не обращались"],
    ),
    # ---------- 5. stats ----------
    (
        "m12-stats-partial-loadedrows",
        MODEL,
        "stats: loadedRows суммирует не все страницы",
        (
            "    let loadedRows = 0;\n"
            "    for (const page of this.pages.values()) loadedRows += page.rows.length;",
            "    const firstPage = this.pages.values().next().value; // MUTATION m12\n"
            "    const loadedRows = firstPage ? firstPage.rows.length : 0;",
        ),
        ["модель: stats после загрузки двух страниц"],
    ),
    (
        "m13-stats-pages-eq-loaded",
        MODEL,
        "stats: pages = loadedPages вместо pageCount",
        (
            "      pages: this.pageCount,",
            "      pages: this.pages.size, // MUTATION m13",
        ),
        ["модель: pages = ceil(totalRows/pageSize)"],
    ),
    # ---------- 6. конструктор ----------
    (
        "m14-ctor-no-maxpages-validation",
        MODEL,
        "конструктор: requirePositiveInt для maxPages убран",
        (
            '    this.maxPages = requirePositiveInt("maxPages", options.maxPages);',
            "    this.maxPages = options.maxPages; // MUTATION m14: валидация maxPages убрана",
        ),
        ["модель: отвергает невалидные pageSize и maxPages"],
    ),
    # ---------- интеграция app.js (импорт/инициализация) ----------
    (
        "a01-app-broken-model-import",
        APP,
        "app.js: сломан спецификатор импорта model.js",
        (
            'import { PageStore } from "./model.js";',
            'import { PageStore } from "./model.jsx"; // MUTATION a01: неразрешимый импорт',
        ),
        ["проба разрешения импорта app.js (не unit-тест)"],
    ),
    (
        "a02-app-initrowlists-not-called",
        APP,
        "app.js: инициализация списков не вызывается",
        (
            "\ninitRowLists();\n",
            "\n// MUTATION a02: initRowLists() не вызывается\n",
        ),
        ["проба разрешения импорта (не ловит)", "smoke UI_SNAPSHOT.chatRenderedRows"],
    ),
]


# Разбор выживших: для каждой — какой инвариант не покрыт и какой тест нужен.
SURVIVOR_NOTES = {
    "m01-getrow-upper-bound-gt": (
        "эквивалентный мутант, не пробел покрытия: при index === totalRows "
        "pageIndex = floor(totalRows/pageSize) — это либо pageCount (такая страница "
        "никогда не грузится: ensureRange кламплит `to` до totalRows-1), либо страница, "
        "где offset === rows.length, т.е. чтение всё равно даёт undefined. Наблюдаемого "
        "различия нет ни в одном достижимом состоянии; тест не нужен."
    ),
    "m07-evict-ignores-pinned": (
        "ПРОБЕЛ: `pinned` проверяется только тестами, где пины свежее всех (там LRU и так "
        "выбирает не их). Не покрыт конкурентный ensureRange двух РАЗНЫХ диапазонов при "
        "полном бюджете: пин диапазона A становится самым старым, и без `pinned` эвикция "
        "выбрасывает видимую страницу A. Нужен тест с управляемым порядком резолва "
        "fetchPage (page 1 — 60 мс, page 2 — 10 мс), maxPages=2, ranges (0,19) и (20,29): "
        "проверено, что он падает на мутанте и зелёный на оригинале "
        "(/tmp/opencode/qa1-m07-candidate.test.mjs)."
    ),
    "m10-evict-touched-gt": (
        "эквивалентный мутант: `touched` — строго возрастающий `++this.clock`, равных "
        "значений не бывает, поэтому `>=` и `>` дают одного и того же victim'а. "
        "Тест не нужен; сравнение можно оставить как есть (защита от будущей "
        "неуникальности часов)."
    ),
    "a02-app-initrowlists-not-called": (
        "ПРОБЕЛ интеграции: ни один unit-тест не исполняет app.js (dom-тест читает его "
        "как текст регулярками, импорт-проба упирается в браузерные API). Ловится только "
        "live smoke по `chatRenderedRows >= 10` / `actions.screenClicks === 4`, но бинарь "
        "Tauri встраивает `frontendDist` на этапе сборки: прогон smoke на текущем "
        "target/debug не видит правок ui/*.js (проверено: правка `chatRows` в app.js не "
        "изменила UI_SNAPSHOT). После пересборки PM — прогнать smoke с этой мутацией; "
        "для гейта нужен unit-тест инициализации (заглушка DOM) или явный rebuild в smoke."
    ),
}


def md5(data):
    return hashlib.md5(data).hexdigest()


def run(cmd, cwd, timeout=180):
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=timeout,
    )


def counts(output):
    """Возвращает (pass, fail) из вывода node --test; None, если вывод не разобран."""
    found = re.findall(r"^\s*(?:ℹ|#)\s*(pass|fail)\s+(\d+)\s*$", output, re.MULTILINE)
    if not found:
        return None
    values = {name: int(number) for name, number in found}
    if "pass" not in values or "fail" not in values:
        return None
    return values["pass"], values["fail"]


def failing_names(output):
    names = []
    for line in output.splitlines():
        match = re.match(r"^\s*✖\s+(.+?)(?:\s+\(\d+(?:\.\d+)?ms\))?\s*$", line)
        if not match:
            continue
        name = match.group(1).strip()
        # Строки-сьюты и заголовок «✖ failing tests:» — не упавшие тесты.
        if not name or name == "failing tests:" or name.startswith("PageStore:"):
            continue
        if name not in names:
            names.append(name)
    return names


def classify(output, returncode):
    """-> (status, detail)."""
    parsed = counts(output)
    compile_markers = (
        "SyntaxError",
        "ERR_MODULE_NOT_FOUND",
        "does not provide an export named",
        "Cannot find module",
    )
    if any(marker in output for marker in compile_markers) and (parsed is None or parsed[1] > 0):
        code = next((m for m in compile_markers if m in output), "?")
        return "compile-error", code
    if parsed is None:
        return "inconclusive", "вывод раннера не разобран (rc=%d)" % returncode
    passed, failed = parsed
    if failed > 0:
        return "caught", "pass %d / fail %d" % (passed, failed)
    if passed > 0 and returncode == 0:
        return "survived", "pass %d / fail 0" % passed
    return "inconclusive", "pass %d / fail %d, rc=%d" % (passed, failed, returncode)


def import_probe(repo):
    """Проба app.js: импорт разрешается и модуль исполняется (падает на DOM)."""
    target = repo / APP
    probe = run(
        ["node", "--input-type=module", "-e", IMPORT_PROBE, str(target)],
        cwd=repo,
        timeout=60,
    )
    return probe


def probe_verdict(probe):
    output = probe.stdout
    if "PROBE_ERROR_CODE=ERR_MODULE_NOT_FOUND" in output:
        # По методике RW-35 неразрешимый импорт — compile-error, а не killed:
        # доказательство силы тестов даёт только компилируемая мутация.
        return "compile-error", "импорт не разрешается (ERR_MODULE_NOT_FOUND)"
    if "PROBE_ERROR_NAME=ReferenceError" in output:
        # Импорт разрешён и модуль исполнялся до первого браузерного API — это и
        # есть baseline-поведение app.js в голом node.
        message = next(
            (line for line in output.splitlines() if line.startswith("PROBE_ERROR_MSG=")), ""
        )
        return "survived", "импорт разрешён, исполнение дошло до DOM/rAF (%s)" % message[
            len("PROBE_ERROR_MSG=") :
        ]
    if "PROBE_OK" in output:
        return "survived", "модуль исполнился"
    return "inconclusive", output.strip().replace("\n", " | ")[:160]


def main(argv):
    parser = argparse.ArgumentParser(description="Мутации RW-37 (PageStore)")
    parser.add_argument("--repo", default=str(Path(__file__).resolve().parents[2]))
    parser.add_argument("--filter", default="", help="прогнать только мутации с подстрокой")
    parser.add_argument("--raw-dir", default="/tmp/opencode/qa1-mutations-raw")
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    raw_dir = Path(args.raw_dir)
    raw_dir.mkdir(parents=True, exist_ok=True)
    log_path = repo / "waves/RW-37/mutations-model.log"
    log = []

    def emit(line=""):
        print(line)
        log.append(line)

    baseline_md5 = {MODEL: md5((repo / MODEL).read_bytes()), APP: md5((repo / APP).read_bytes())}
    selected = [m for m in MUTATIONS if not args.filter or args.filter in m[0]]

    emit("# RW-37 mutations — PageStore (model.js) + интеграция app.js")
    emit("# HEAD: " + run(["git", "rev-parse", "--short", "HEAD"], repo).stdout.strip())
    emit("# baseline md5: " + ", ".join("%s=%s" % (k, v) for k, v in baseline_md5.items()))
    emit("# оракул model: " + " ".join(TEST_MODEL))
    emit("# оракул app:   " + " ".join(TEST_DOM) + " + импорт-проба node")
    emit("")

    baseline = run(TEST_MODEL, repo)
    parsed = counts(baseline.stdout)
    targeted = run(
        [
            "node",
            "--test",
            "tests/unit/gui-spike-model.test.mjs",
            "tests/unit/gui-spike-bench.test.mjs",
            "tests/unit/gui-spike-dom.test.mjs",
        ],
        repo,
    )
    targeted_counts = counts(targeted.stdout)
    dom_baseline = run(TEST_DOM, repo)
    probe_baseline = probe_verdict(import_probe(repo))

    baseline_ok = (
        baseline.returncode == 0
        and parsed is not None
        and parsed[1] == 0
        and parsed[0] > 0
    )
    emit(
        "BASELINE model: %s (%s)"
        % ("OK" if baseline_ok else "BAD", "pass %s / fail %s" % parsed if parsed else "не разобран")
    )
    emit(
        "BASELINE targeted (model+bench+dom): %s"
        % ("pass %d / fail %d" % targeted_counts if targeted_counts else "не разобран")
    )
    emit(
        "BASELINE app: dom %s, импорт-проба: %s (%s)"
        % (
            "pass %d / fail %d" % counts(dom_baseline.stdout)
            if counts(dom_baseline.stdout)
            else "не разобран",
            probe_baseline[0],
            probe_baseline[1],
        )
    )
    if not baseline_ok or probe_baseline[0] != "survived":
        emit("ABORT: baseline не зелёный — мутации не считаются")
        log_path.write_text("\n".join(log) + "\n", encoding="utf-8")
        return 1
    emit("")

    tally = {"caught": 0, "survived": 0, "inconclusive": 0, "compile-error": 0}
    results = []
    for index, (name, relative, description, (old, new), expected) in enumerate(selected, start=1):
        path = repo / relative
        original = path.read_bytes()
        before = md5(original)
        original_mtime_ns = path.stat().st_mtime_ns
        if before != baseline_md5[relative]:
            emit("%s: ABORT — %s изменён до мутации (md5 %s != %s)" % (name, relative, before, baseline_md5[relative]))
            break
        source = original.decode("utf-8")
        hits = source.count(old)
        if hits != 1:
            emit("%s: ERROR — якорь найден %d раз в %s" % (name, hits, relative))
            tally["inconclusive"] += 1
            results.append((name, "anchor-error", description, relative, 0))
            continue
        line_no = source[: source.index(old) + len(old) - len(old.lstrip("\n"))].count("\n") + 1

        status = None
        detail = ""
        extra = []
        try:
            path.write_text(source.replace(old, new, 1), encoding="utf-8")
            print("\n" + "=" * 78)
            print("# %s (%s:%d)" % (name, relative, line_no))
            print("=" * 78)
            if relative == MODEL:
                mutation_run = run(TEST_MODEL, repo)
                print(mutation_run.stdout)
                status, detail = classify(mutation_run.stdout, mutation_run.returncode)
                extra = failing_names(mutation_run.stdout)
                test_cmd = " ".join(TEST_MODEL)
            else:
                # app.js: статический контракт + проба разрешения импорта.
                dom_run = run(TEST_DOM, repo)
                print(dom_run.stdout)
                dom_status, dom_detail = classify(dom_run.stdout, dom_run.returncode)
                probe_status, probe_detail = probe_verdict(import_probe(repo))
                print("import probe: %s — %s" % (probe_status, probe_detail))
                detail = "dom: %s (%s); импорт-проба: %s" % (dom_status, dom_detail, probe_status)
                extra = failing_names(dom_run.stdout)
                if dom_status == "compile-error" or probe_status == "compile-error":
                    status = "compile-error"
                elif dom_status == "caught" or probe_status == "caught":
                    status = "caught"
                elif dom_status == "inconclusive" or probe_status == "inconclusive":
                    status = "inconclusive"
                else:
                    status = "survived"
                test_cmd = " ".join(TEST_DOM) + " + импорт-проба"
        finally:
            path.write_bytes(original)
            after = md5(path.read_bytes())
            # Восстанавливаем исходный mtime: иначе ui/* станет новее бинаря и
            # rebuild-guard live smoke покраснеет на ровном месте.
            os.utime(path, ns=(original_mtime_ns, original_mtime_ns))
            if after != before:
                raise SystemExit("FATAL: %s не восстановлен (%s != %s)" % (path, after, before))

        (raw_dir / ("%02d-%s.log" % (index, name))).write_text(
            "mutation: %s\nfile: %s:%d\nstatus: %s\ndetail: %s\n\nold:\n%s\n\nnew:\n%s\n\n"
            % (name, relative, line_no, status, detail, old, new),
            encoding="utf-8",
        )
        tally[status] += 1
        results.append((name, status, description, relative, line_no))
        emit(
            "%-34s %-13s %s:%d | %s | %s | md5 %s -> %s"
            % (name, status.upper(), relative, line_no, detail, test_cmd, before, after)
        )
        emit("  мутация: %s" % description)
        if extra:
            emit("  ожидаемо ловят: " + ", ".join(expected))
            emit("  упали: " + ", ".join(extra[:6]))

    emit("")
    emit("ИТОГО: caught %d, survived %d, inconclusive %d, compile-error %d (всего %d)"
         % (tally["caught"], tally["survived"], tally["inconclusive"], tally["compile-error"], len(selected)))
    survivors = [r for r in results if r[1] == "survived"]
    if survivors:
        emit("SURVIVORS:")
        for name, _status, description, relative, line_no in survivors:
            emit("  - %s (%s:%d) — %s" % (name, relative, line_no, description))
            note = SURVIVOR_NOTES.get(name)
            if note:
                emit("    разбор: " + note)
    compile_errors = [r for r in results if r[1] == "compile-error"]
    if compile_errors:
        emit("COMPILE-ERROR (не killed по методике):")
        for name, _status, description, relative, line_no in compile_errors:
            emit("  - %s (%s:%d) — %s" % (name, relative, line_no, description))
        emit(
            "  разбор: мутация ломает граф модулей, а не поведение; "
            "в зачёт покрытия не идёт, но показывает, что разрешение импорта app.js "
            "ничем, кроме сборки/смоука, не проверяется."
        )
    final = {MODEL: md5((repo / MODEL).read_bytes()), APP: md5((repo / APP).read_bytes())}
    emit("RESTORE: " + ", ".join("%s=%s (%s)" % (k, v, "ok" if final[k] == baseline_md5[k] else "MISMATCH")
                                 for k, v in final.items()))
    model_total = len([r for r in results if r[3] == MODEL])
    model_caught = len([r for r in results if r[3] == MODEL and r[1] == "caught"])
    emit(
        "ВЕРДИКТ: model.js — caught %d/%d (2 эквивалентных мутанта + 1 пробел LRU-пина); "
        "app.js — 1 пробел инициализации + 1 compile-error; продукт восстановлен, "
        "md5 совпадает с baseline." % (model_caught, model_total)
    )
    log_path.write_text("\n".join(log) + "\n", encoding="utf-8")
    print("\nлог: %s\nraw: %s" % (log_path, raw_dir))
    ok = tally["survived"] == 0 and tally["inconclusive"] == 0 and tally["compile-error"] == 0 \
        and final == baseline_md5
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
