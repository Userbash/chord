// Быстрый сканер ETF-термов (только для планирования, не в репозиторий).
import { readFileSync } from "node:fs";

const buf = readFileSync(process.argv[2]);
const limit = process.argv[3] ? Number(process.argv[3]) : buf.length;
const data = buf.subarray(0, limit);

function readTerm(pos, depth = 1) {
  if (depth > 64) throw new Error("depth");
  const tag = data[pos];
  if (tag === undefined) throw new Error(`eof at ${pos}`);
  switch (tag) {
    case 0x46: return [pos + 9, { kind: "float" }];
    case 0x61: return [pos + 2, { kind: "int", value: data[pos + 1] }];
    case 0x62: return [pos + 5, { kind: "int", value: data.readInt32BE(pos + 1) }];
    case 0x63: return [pos + 32, { kind: "float" }];
    case 0x64: {
      const len = data.readUInt16BE(pos + 1);
      return [pos + 3 + len, { kind: "atom", value: data.toString("latin1", pos + 3, pos + 3 + len) }];
    }
    case 0x73: {
      const len = data[pos + 1];
      return [pos + 2 + len, { kind: "atom", value: data.toString("latin1", pos + 2, pos + 2 + len) }];
    }
    case 0x68: {
      const count = data[pos + 1];
      let p = pos + 2; const items = [];
      for (let i = 0; i < count; i += 1) { const [np, v] = readTerm(p, depth + 1); items.push(v); p = np; }
      return [p, { kind: "tuple", items }];
    }
    case 0x6a: return [pos + 1, { kind: "nil" }];
    case 0x6b: {
      const len = data.readUInt16BE(pos + 1);
      return [pos + 3 + len, { kind: "str", value: data.toString("latin1", pos + 3, pos + 3 + len) }];
    }
    case 0x6c: {
      const count = data.readUInt32BE(pos + 1);
      let p = pos + 5; const items = [];
      for (let i = 0; i < count; i += 1) { const [np, v] = readTerm(p, depth + 1); items.push(v); p = np; }
      if (data[p] !== 0x6a) throw new Error(`list tail not nil at ${p}`);
      return [p + 1, { kind: "list", items }];
    }
    case 0x6d: {
      const len = data.readUInt32BE(pos + 1);
      return [pos + 5 + len, { kind: "bin", len }];
    }
    case 0x6e: {
      const n = data[pos + 1];
      let mag = 0n;
      for (let i = n; i >= 1; i -= 1) mag = (mag << 8n) | BigInt(data[pos + 2 + i]);
      const sign = data[pos + 2];
      return [pos + 3 + n, { kind: "int", value: Number(sign ? -mag : mag) }];
    }
    case 0x74: {
      const count = data.readUInt32BE(pos + 1);
      let p = pos + 5; const pairs = [];
      for (let i = 0; i < count; i += 1) {
        const [kp, k] = readTerm(p, depth + 1);
        const [vp, v] = readTerm(kp, depth + 1);
        pairs.push([k, v]); p = vp;
      }
      return [p, { kind: "map", pairs }];
    }
    default: throw new Error(`unknown tag 0x${tag.toString(16)} at ${pos}`);
  }
}

let pos = 0; let terms = 0; const events = [];
while (true) {
  let start = pos;
  if (data[start] === 0x83) start += 1;
  let result;
  try { result = readTerm(start); } catch (error) {
    console.log(`термов: ${terms}, стоп на смещении ${pos}, остаток ${data.length - pos} Б: ${error.message}`);
    break;
  }
  const [next, value] = result;
  terms += 1;
  if (value.kind === "map") {
    const get = (key) => value.pairs.find(([k]) => (k.kind === "atom" || k.kind === "str") && k.value === key)?.[1];
    const op = get("op"); const s = get("s"); const t = get("t");
    events.push({ op: op?.value, s: s?.value, t: t?.value, len: next - pos });
  } else {
    events.push({ op: null, len: next - pos });
  }
  pos = next;
  if (pos >= data.length) { console.log(`термов: ${terms}, конец данных, остаток ${data.length - pos} Б`); break; }
}
const ops = {}; for (const e of events) ops[e.op] = (ops[e.op] ?? 0) + 1;
console.log("опкоды:", JSON.stringify(ops));
const dispatches = events.filter((e) => e.op === 0).map((e) => e.s);
console.log("диспатчи:", dispatches.length, "первый:", dispatches[0], "последний:", dispatches.at(-1));
console.log("последние 5 термов:", JSON.stringify(events.slice(-5).map((e) => ({ op: e.op, s: e.s, len: e.len }))));
console.log("крупнейшие термы:", JSON.stringify(events.map((e) => e.len).sort((a, b) => b - a).slice(0, 5)));
