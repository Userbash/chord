// @ts-check
// TR-194A: мутационный прогон scripts/test-impact.mjs (PR-165 / task-283).
// Каждая мутация: точная замена одной конструкции → прогон контракта селектора →
// восстановление из бэкапа с проверкой MD5. Артефакты: mutations/*.log, mutations/results.json.
import { readFileSync, writeFileSync, copyFileSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const root = "/home/user/orca/workspaces/project/qa-task-283-tsconfig-and-test-impact";
const artifacts = "/home/user/orca/project/waves/RW-40/QUALITY/task-283-tsconfig-and-test-impact";
const target = join(root, "scripts/test-impact.mjs");
const backup = join(artifacts, "mutations/test-impact.orig.mjs");
const testLogs = join(artifacts, "mutations");
const TEST = "tests/unit/test-impact.test.mjs";
const ORIGINAL_MD5 = "330caeeac8e6f553df0ecfd25eea84cc";

/** @param {string} text */
const md5 = (text) => createHash("md5").update(text).digest("hex");

/** @type {{id: string, scope: string, mutation: string, from: string, to: string}[]} */
const MUTANTS = [
  {
    id: "M1",
    scope: "RUST_MODULE_CONTRACTS",
    mutation: `значение automod_contract → automod_contracts (суффикс-опечатка)`,
    from: `"rust/gui-spike/src/automod.rs": "automod_contract",`,
    to: `"rust/gui-spike/src/automod.rs": "automod_contracts",`,
  },
  {
    id: "M2",
    scope: "RUST_MODULE_CONTRACTS",
    mutation: `удалено правило bff_domains_router (уходит в полный прогон крейта)`,
    from: `  "rust/gui-spike/src/bff_domains_router.rs": "bff_domains_router_contract",\n`,
    to: ``,
  },
  {
    id: "M3",
    scope: "RUST_MODULE_CONTRACTS",
    mutation: `stage_instances_contract подменён на soundboard_contract`,
    from: `"rust/gui-spike/src/stage_instances.rs": "stage_instances_contract",`,
    to: `"rust/gui-spike/src/stage_instances.rs": "soundboard_contract",`,
  },
  {
    id: "M4",
    scope: "UI_MODULE_TESTS",
    mutation: `удалено правило app-shell-wiring.js (фоллбэк frontend()+geometry)`,
    from: `  "rust/gui-spike/ui/features/app-shell-wiring.js": [
    "tests/unit/app-shell-wiring.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],\n`,
    to: ``,
  },
  {
    id: "M5",
    scope: "UI_MODULE_TESTS",
    mutation: `у composer-popovers-bridge.js отрезан профильный тест (остались общие)`,
    from: `  "rust/gui-spike/ui/features/composer-popovers-bridge.js": [
    "tests/unit/composer-popovers-bridge.test.mjs",`,
    to: `  "rust/gui-spike/ui/features/composer-popovers-bridge.js": [`,
  },
  {
    id: "M6",
    scope: "UI_MODULE_TESTS",
    mutation: `удалено правило shell-controllers-registry.js`,
    from: `  "rust/gui-spike/ui/features/shell-controllers-registry.js": [
    "tests/unit/shell-controllers-registry.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],\n`,
    to: ``,
  },
  {
    id: "M7",
    scope: "UI_MODULE_TESTS",
    mutation: `chat-helpers-bridge.js сшит с чужим профилем app-shell-wiring.test.mjs`,
    from: `  "rust/gui-spike/ui/features/chat-helpers-bridge.js": [
    "tests/unit/chat-helpers-bridge.test.mjs",`,
    to: `  "rust/gui-spike/ui/features/chat-helpers-bridge.js": [
    "tests/unit/app-shell-wiring.test.mjs",`,
  },
];

/** @param {string[]} args */
const runTests = () => {
  try {
    const stdout = execFileSync(process.execPath, ["--test", TEST], {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    });
    return { killed: false, log: stdout };
  } catch (error) {
    const e = /** @type {{stdout?: string, stderr?: string}} */ (error);
    return { killed: true, log: `${e.stdout ?? ""}${e.stderr ?? ""}` };
  }
};

mkdirSync(testLogs, { recursive: true });
copyFileSync(target, backup);
const original = readFileSync(backup, "utf8");
if (md5(original) !== ORIGINAL_MD5) {
  throw new Error(`исходник не совпал с ТЗ: ${md5(original)}`);
}

const baseline = runTests();
writeFileSync(join(testLogs, "baseline.log"), baseline.log);
console.log(`baseline: killed=${baseline.killed} (ожидается false)`);

/** @type {{id: string, scope: string, mutation: string, mutantMd5: string, exitKilled: boolean, killedBy: string}[]} */
const results = [];
for (const mutant of MUTANTS) {
  if (!original.includes(mutant.from)) throw new Error(`${mutant.id}: якорь не найден`);
  const mutated = original.replace(mutant.from, mutant.to);
  if (md5(mutated) === ORIGINAL_MD5) throw new Error(`${mutant.id}: файл не изменился`);
  writeFileSync(target, mutated);
  const { killed, log } = runTests();
  writeFileSync(target, original);
  if (md5(readFileSync(target, "utf8")) !== ORIGINAL_MD5) throw new Error(`${mutant.id}: восстановление не сошлось`);
  writeFileSync(join(testLogs, `${mutant.id}.log`), log);
  const killedBy = killed
    ? (log.split("\n").filter((line) => /^\s*✖ /.test(line)).map((line) => line.trim()).slice(0, 2).join("; ") ||
      "тест упал")
    : "";
  results.push({
    id: mutant.id,
    scope: mutant.scope,
    mutation: mutant.mutation,
    mutantMd5: md5(mutated),
    exitKilled: killed,
    killedBy,
  });
  console.log(`${mutant.id} ${mutant.scope}: ${killed ? "KILLED" : "SURVIVED"} :: ${mutant.mutation} :: ${killedBy}`);
}

writeFileSync(
  join(testLogs, "results.json"),
  `${JSON.stringify(
    {
      task: "task-283 / PR-165 / TR-194A",
      file: "scripts/test-impact.mjs",
      test: TEST,
      baselineKilled: baseline.killed,
      originalMd5: md5(readFileSync(target, "utf8")),
      expectedMd5: ORIGINAL_MD5,
      killed: results.filter((r) => r.exitKilled).length,
      total: results.length,
      results,
    },
    null,
    2,
  )}\n`,
);
console.log(`\nитог: убито ${results.filter((r) => r.exitKilled).length}/${results.length}; MD5 после цикла ${md5(readFileSync(target, "utf8"))}`);
