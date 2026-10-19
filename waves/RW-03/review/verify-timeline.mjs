// Ревью RW-03: независимая сверка timeline-95974_1389.tsv с сырьём (capture + msgs).
// Свой код (не запускаю скрипты воркера/координатора).
import fs from "node:fs";

const SOCK = "95974.1389";
const CAP = "/home/sanya/re-deep/capture-run2.jsonl";
const MSGS = "/home/sanya/re-deep/gw-proto/msgs-run2-95974_1389.jsonl";
const TSV = "/home/sanya/re-deep/gw-proto/timeline-95974_1389.tsv";

// 1) ts из захвата: все wsrecv этого сокета, в порядке файла.
const ts = [];
for (const line of fs.readFileSync(CAP, "utf8").split("\n")) {
  if (!line.includes('"wsrecv"')) continue;
  const o = JSON.parse(line);
  if (o.ev === "wsrecv" && o.requestId === SOCK) ts.push(o.ts);
}
// 2) op/s/t из распаковки.
const msgs = fs
  .readFileSync(MSGS, "utf8")
  .trim()
  .split("\n")
  .map((l) => JSON.parse(l));
// 3) строки tsv.
const rows = fs
  .readFileSync(TSV, "utf8")
  .split("\n")
  .filter((l) => l.length > 0 && !l.startsWith("#"))
  .map((l) => l.split("\t"));

console.log("tsv rows:", rows.length, "| capture ts:", ts.length, "| msgs:", msgs.length);

let mismatches = 0;
const max = Math.max(rows.length, ts.length, msgs.length);
for (let i = 0; i < max; i++) {
  const [ri, rts, rop, rs, rt] = rows[i] ?? [];
  const caps = ts[i];
  const m = msgs[i];
  const expS = m ? String(m.s) : "(no msg)";
  const expT = m ? String(m.t) : "(no msg)";
  const ok =
    Number(ri) === i + 1 &&
    Number(rts) === caps &&
    Number(rop) === m.op &&
    (rs === expS || (rs === "-" && expS === "nil")) &&
    (rt === expT || (rt === "-" && expT === "nil"));
  if (!ok) {
    mismatches++;
    if (mismatches <= 5)
      console.log(`MISMATCH row ${i + 1}: tsv=[${ri},${rts},${rop},${rs},${rt}] ts=${caps} msg=[${m.op},${m.s},${m.t}]`);
  }
}
console.log("mismatches:", mismatches);

// 4) ts монотонно неубывающая последовательность (факт таймлайна).
let mono = true;
for (let i = 1; i < ts.length; i++) if (ts[i] < ts[i - 1]) mono = false;
console.log("ts monotone non-decreasing:", mono, "| first:", ts[0], "| last:", ts[ts.length - 1]);

// 5) выборочная сверка "шумных" мест: строки 1, 80, 137 и все op11.
const sample = [1, 80, 137];
for (const i of sample) {
  console.log(
    `row ${i}: tsv=[${rows[i - 1].join(",")}] ts=${ts[i - 1]} msg=[op:${msgs[i - 1].op},s:${msgs[i - 1].s},t:${msgs[i - 1].t}]`,
  );
}
const ackIdx = msgs.map((m, i) => (m.op === 11 ? i + 1 : 0)).filter(Boolean);
console.log("op11 rows:", ackIdx.join(","));
