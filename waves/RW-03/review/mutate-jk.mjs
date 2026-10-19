// Ревью RW-03: дополнительные граничные мутации (j) зомби `>` вместо `>=`, (k) удар `>` вместо `>=`.
import { execFileSync } from "node:child_process";
import fs from "node:fs";

const MUT_DIR = "/tmp/opencode/rw-03/review/mut";
const SRC = `${MUT_DIR}/crates/discord-core/src/gateway.rs`;
const ORIG = fs.readFileSync(`${MUT_DIR}/gateway.rs.orig`, "utf8");

const mutations = [
  {
    id: "j",
    name: "зомби строго после дедлайна (now > deadline вместо >=)",
    from: ".is_some_and(|deadline| now_ms >= deadline)",
    to: ".is_some_and(|deadline| now_ms > deadline)",
  },
  {
    id: "k",
    name: "удар строго после срока (now > due вместо >=)",
    from: ".is_some_and(|due| now_ms >= due)",
    to: ".is_some_and(|due| now_ms > due)",
  },
];

for (const m of mutations) {
  if (ORIG.split(m.from).length - 1 !== 1) throw new Error(`фрагмент "${m.from}" не уникален`);
  fs.writeFileSync(SRC, ORIG.replace(m.from, m.to));
  let out = "";
  let exit = 0;
  try {
    out = execFileSync("cargo", ["test", "--offline", "-p", "discord-core", "--test", "gateway_session"], {
      cwd: MUT_DIR,
      env: { ...process.env, CARGO_TARGET_DIR: "/tmp/opencode/rw-03/review/mut-target" },
      encoding: "utf8",
    });
  } catch (e) {
    exit = e.status ?? -1;
    out = (e.stdout ?? "") + (e.stderr ?? "");
  }
  const failed = [...out.matchAll(/^test ([^\s]+) \.\.\. FAILED$/gm)].map((x) => x[1]);
  console.log(
    `${m.id}) ${m.name}: ${/test result: ok\./.test(out) ? "НЕ ПОЙМАНА" : "ПОЙМАНА"}; exit=${exit}; упали: ${failed.join(", ") || "(нет)"}`,
  );
  fs.writeFileSync(SRC, ORIG);
}
console.log("оригинал восстановлен; md5:", fs.readFileSync(SRC).toString().length, "байт");
