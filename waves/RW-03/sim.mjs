// Независимая симуляция семантики ТЗ RW-03 на реальном таймлайне (проверка ожиданий тестов).
import fs from 'node:fs';

const path = '/home/sanya/re-deep/gw-proto/timeline-95974_1389.tsv';
const rows = fs
  .readFileSync(path, 'utf8')
  .split('\n')
  .filter((l) => l.length > 0 && !l.startsWith('#'))
  .map((l) => {
    const [i, ts, op, s, t] = l.split('\t');
    return { i: +i, ts: +ts, op: +op, s: s === '-' ? null : +s, t: t === '-' ? null : t };
  });

const INTERVAL = 41250;

function simulate(seed) {
  let hello = null;
  let nextDue = null;
  let pending = null;
  let rtt = null;
  let lastSeq = null;
  let dead = false;
  let beats = 0;
  const effects = [];

  const onTimer = (now) => {
    if (dead) return;
    if (pending !== null && now >= pending + INTERVAL) {
      dead = true;
      effects.push('zombie');
      return;
    }
    if (nextDue !== null && now >= nextDue) {
      beats += 1;
      effects.push('beat:' + lastSeq);
      pending = now;
      nextDue = null;
    }
  };
  const onValue = (op, now, s) => {
    if (dead) return;
    if (op === 10) {
      hello = INTERVAL;
      // До первого удара — пере-планировать; после — игнорировать.
      if (beats === 0) nextDue = now + (seed % INTERVAL);
      return;
    }
    if (op === 11) {
      if (pending !== null) {
        rtt = now - pending;
        nextDue = now + INTERVAL;
        pending = null;
      }
      return;
    }
    if (op === 0) {
      lastSeq = s;
      return;
    }
    if (op === 7 || op === 9) dead = true;
  };

  for (const row of rows) {
    onTimer(row.ts);
    onValue(row.op, row.ts, row.s);
  }
  return { beats, lastSeq, rtt, dead, hello, effects: effects.filter((e) => e === 'zombie').length };
}

const r = simulate(1214);
console.log('seed=1214 ->', r);

let min = Infinity;
let max = -Infinity;
let zombies = 0;
const badSeeds = [];
for (let seed = 0; seed < INTERVAL; seed++) {
  const x = simulate(seed);
  if (x.dead) {
    zombies++;
    if (badSeeds.length < 5) badSeeds.push(seed);
  }
  if (x.beats < min) min = x.beats;
  if (x.beats > max) max = x.beats;
}
console.log('all seeds: zombies', zombies, 'beats min/max', min, max, 'badSeeds', badSeeds);
