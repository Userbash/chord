// Независимая сверка оракульных чисел ТЗ RW-07 по lab-файлам (L1/L2).
import fs from "node:fs";
import path from "node:path";

const root = "/home/user/clean-room-oracle/gw-proto";
const files = [
  "msgs-run1-57992_5112.jsonl",
  "msgs-run2-95974_1389.jsonl",
  "msgs-run2-95974_5532.jsonl",
  "msgs-run2-95974_8269.jsonl",
  "msgs-run2-95974_8608.jsonl",
  "msgs-run2-95974_10029.jsonl",
  "msgs-run2-95974_13072.jsonl",
];

// L1: union 7 файлов.
const counts = new Map();
let total = 0;
for (const name of files) {
  const text = fs.readFileSync(path.join(root, name), "utf8");
  for (const line of text.split("\n")) {
    if (!line.includes('"op":0')) continue;
    let obj;
    try { obj = JSON.parse(line); } catch { console.log("SKIP bad json", line.slice(0, 80)); continue; }
    if (obj.op !== 0) continue;
    total += 1;
    counts.set(obj.t, (counts.get(obj.t) ?? 0) + 1);
  }
}
console.log("L1: total op0 =", total, "types =", counts.size);
const sorted = [...counts.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
for (const [t, c] of sorted) console.log(`  ${t} ${c}`);

// L2: сокет 95974_1389: op0 c s<=36.
const text2 = fs.readFileSync(path.join(root, "msgs-run2-95974_1389.jsonl"), "utf8");
const c2 = new Map();
let n2 = 0;
let dispatchTotal = 0;
for (const line of text2.split("\n")) {
  if (!line.trim()) continue;
  let obj;
  try { obj = JSON.parse(line); } catch { continue; }
  if (obj.op !== 0) continue;
  dispatchTotal += 1;
  if (typeof obj.s !== "number" || obj.s > 36) continue;
  n2 += 1;
  c2.set(obj.t, (c2.get(obj.t) ?? 0) + 1);
}
console.log("L2: op0 всего =", dispatchTotal, "; s<=36 =", n2);
const sorted2 = [...c2.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
for (const [t, c] of sorted2) console.log(`  ${t} ${c}`);

// Байтовые факты hexl: 137 кадров, 104066 сжатых байт.
const hexl = fs.readFileSync(path.join(root, "zstd-frames-95974_1389.hexl"), "utf8");
const lines = hexl.split("\n").filter((l) => l.trim() !== "");
let bytes = 0;
for (const l of lines) bytes += l.trim().length / 2;
console.log("L2: frames =", lines.length, "compressed bytes =", bytes);
