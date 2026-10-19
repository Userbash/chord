// Ревью RW-03: мутация h' — «лишний» ACK без pending переносит расписание.
import { execFileSync } from "node:child_process";
import fs from "node:fs";

const MUT_DIR = "/tmp/opencode/rw-03/review/mut";
const SRC = `${MUT_DIR}/crates/discord-core/src/gateway.rs`;
const ORIG = fs.readFileSync(`${MUT_DIR}/gateway.rs.orig`, "utf8");

const from = "let (Some(sent), Some(interval)) = (self.pending_beat_ms, self.interval_ms()) else {";
const to = "let (Some(sent), Some(interval)) = (self.pending_beat_ms.or(Some(now_ms)), self.interval_ms()) else {";
if (ORIG.split(from).length - 1 !== 1) throw new Error("фрагмент не уникален");
fs.writeFileSync(SRC, ORIG.replace(from, to));

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
console.log("статус:", /test result: ok\./.test(out) ? "НЕ ПОЙМАНА" : "ПОЙМАНА");
console.log("exit:", exit, "| упали:", failed.join(", ") || "(нет)");
console.log([...out.matchAll(/panicked at ([^\n]+)/g)].map((x) => x[1]).slice(0, 3).join("\n"));
fs.writeFileSync(SRC, ORIG);
