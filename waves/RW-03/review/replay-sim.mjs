// Ревью RW-03: собственная симуляция семантики ТЗ п.1–4 на реальном таймлайне (не sim.mjs воркера).
// Семантика: HELLO → первый удар now + seed%interval; ACK с pending → rtt=now-sent, next=now+interval;
// иначе ACK игнорируется; удар по next_due; зомби = pending + interval; op7/op9 → смерть.
import fs from "node:fs";

const rows = fs
  .readFileSync("/home/sanya/re-deep/gw-proto/timeline-95974_1389.tsv", "utf8")
  .split("\n")
  .filter((l) => l && !l.startsWith("#"))
  .map((l) => {
    const [i, ts, op, s] = l.split("\t");
    return { i: +i, ts: +ts, op: +op, s: s === "nil" ? null : +s };
  });

const IV = 41250;

function run(seed) {
  let hello = false;
  let nextDue = null;
  let pending = null;
  let dead = false;
  let beats = 0;
  let lastSeq = null;
  let rtt = null;
  let zombies = 0;

  const timer = (now) => {
    if (dead) return;
    if (pending !== null && now >= pending + IV) {
      dead = true;
      zombies++;
      return;
    }
    if (nextDue !== null && now >= nextDue) {
      beats++;
      pending = now;
      nextDue = null;
    }
  };
  const value = (op, s, now) => {
    if (dead) return;
    if (op === 10) {
      hello = true;
      if (beats === 0) nextDue = now + (seed % IV);
    } else if (op === 11) {
      if (pending !== null) {
        rtt = now - pending;
        nextDue = now + IV;
        pending = null;
      }
    } else if (op === 0) {
      lastSeq = s;
    } else if (op === 7 || op === 9) {
      dead = true;
    }
  };

  for (const r of rows) {
    timer(r.ts);
    value(r.op, r.s, r.ts);
  }
  return { seed, beats, lastSeq, rtt, dead, zombies, hello };
}

const r = run(1214);
console.log("seed 1214:", JSON.stringify(r));

let zTotal = 0;
let minB = Infinity;
let maxB = -Infinity;
let bad = [];
for (let seed = 0; seed < IV; seed++) {
  const x = run(seed);
  zTotal += x.zombies;
  if (x.beats < minB) minB = x.beats;
  if (x.beats > maxB) maxB = x.beats;
  if (x.dead && bad.length < 5) bad.push(seed);
}
console.log(`все сиды 0..${IV - 1}: зомби=${zTotal}, ударов min=${minB} max=${maxB}, мертвые сиды: ${bad.join(",") || "нет"}`);
