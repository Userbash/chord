#!/usr/bin/env node
// Вырезка эталонных термов ETF из живого захвата (запуск: node gw-slice-terms.mjs [capture.jsonl]).
// Логика Reader/parseTerm — копия проверенного gw-decode.mjs (сверка 0 расхождений с gateway/messages.jsonl),
// отличие: вместо записи JSON печатаются границы термов и hex — эталонные векторы для Rust-тестов.
// Лаборатория: файл рабочий, в git-репозиторий не попадает.
import { readFileSync } from "node:fs";
import zlib from "node:zlib";

const FILE = process.argv[2] ?? "/home/sanya/re-deep/capture-run2.jsonl";
const lines = readFileSync(FILE, "utf8")
  .split("\n")
  .map((l) => {
    try {
      return JSON.parse(l);
    } catch {
      return null;
    }
  })
  .filter(Boolean);

// Сокеты: все requestId, у которых есть wsrecv-кадры (в run2 их шесть).
const bySock = new Map();
for (const r of lines) {
  if (r.ev !== "wsrecv") continue;
  if (!bySock.has(r.requestId)) bySock.set(r.requestId, []);
  bySock.get(r.requestId).push(r);
}

class Reader {
  constructor(buf) {
    this.b = buf;
    this.i = 0;
  }
  u8() {
    return this.b[this.i++];
  }
  u16() {
    const v = this.b.readUInt16BE(this.i);
    this.i += 2;
    return v;
  }
  u32() {
    const v = this.b.readUInt32BE(this.i);
    this.i += 4;
    return v;
  }
  bytes(n) {
    const v = this.b.subarray(this.i, this.i + n);
    this.i += n;
    return v;
  }
}
function parseTerm(r) {
  const tag = r.u8();
  switch (tag) {
    case 70: {
      const v = r.b.readDoubleBE(r.i);
      r.i += 8;
      return v;
    }
    case 97:
      return r.u8();
    case 98: {
      const v = r.b.readInt32BE(r.i);
      r.i += 4;
      return v;
    }
    case 99:
      return parseFloat(r.bytes(31).toString("latin1"));
    case 100: {
      const n = r.u16();
      return r.bytes(n).toString("latin1");
    }
    case 104: {
      const n = r.u8();
      const a = [];
      for (let k = 0; k < n; k++) a.push(parseTerm(r));
      return a;
    }
    case 105: {
      const n = r.u32();
      const a = [];
      for (let k = 0; k < n; k++) a.push(parseTerm(r));
      return a;
    }
    case 106:
      return null;
    case 107: {
      const n = r.u16();
      return r.bytes(n).toString("latin1");
    }
    case 108: {
      const n = r.u32();
      const a = [];
      for (let k = 0; k < n; k++) a.push(parseTerm(r));
      parseTerm(r); // NIL в хвосте списка
      return a;
    }
    case 109: {
      const n = r.u32();
      return r.bytes(n).toString("utf8");
    }
    case 110: {
      const n = r.u8();
      const sign = r.u8();
      const b = r.bytes(n);
      let v = 0n;
      for (let k = n - 1; k >= 0; k--) v = (v << 8n) | BigInt(b[k]);
      return sign ? -v : v;
    }
    case 111: {
      const n = r.u32();
      const sign = r.u8();
      const b = r.bytes(n);
      let v = 0n;
      for (let k = n - 1; k >= 0; k--) v = (v << 8n) | BigInt(b[k]);
      return sign ? -v : v;
    }
    case 115: {
      const n = r.u8();
      return r.bytes(n).toString("latin1");
    }
    case 116: {
      const n = r.u32();
      const m = {};
      for (let k = 0; k < n; k++) {
        const key = parseTerm(r);
        m[String(key)] = parseTerm(r);
      }
      return m;
    }
    case 118: {
      const n = r.u16();
      return r.bytes(n).toString("utf8");
    }
    case 119: {
      const n = r.u8();
      return r.bytes(n).toString("utf8");
    }
    default:
      throw new Error(`неизвестный тег ${tag} (0x${tag.toString(16)}) на позиции ${r.i - 1}`);
  }
}

const J = (v) => JSON.stringify(v, (k, x) => (typeof x === "bigint" ? x.toString() : x));

for (const [sock, frames] of bySock) {
  const total = frames.reduce((a, f) => a + Buffer.from(f.payload ?? "", "base64").length, 0);
  const dec = zlib.createZstdDecompress();
  const chunks = [];
  let zErr = null;
  dec.on("data", (c) => chunks.push(c));
  dec.on("error", (e) => (zErr = e));
  await new Promise((resolve) => {
    let i = 0;
    const step = () => {
      if (i >= frames.length) return setTimeout(resolve, 400);
      dec.write(Buffer.from(frames[i].payload ?? "", "base64"));
      i++;
      setTimeout(step, 5);
    };
    step();
  });
  const raw = Buffer.concat(chunks);
  if (zErr) {
    console.log(`=== сокет ${sock}: ошибка zstd ${zErr.message}`);
    continue;
  }
  // Границы термов
  const r = new Reader(raw);
  const terms = [];
  let idx = 0;
  while (r.i < raw.length) {
    idx++;
    const start = r.i;
    let framed = false;
    if (raw[r.i] === 0x83) {
      r.i++;
      framed = true;
    }
    let term = null;
    let err = null;
    try {
      term = parseTerm(r);
    } catch (e) {
      err = e.message;
    }
    const end = r.i;
    terms.push({ idx, start, end, len: end - start, framed, hex: raw.subarray(start, end).toString("hex"), json: err ? null : J(term), err });
    if (err) break;
  }
  console.log(`=== сокет ${sock}: кадров ${frames.length}, сжато ${total} Б, распаковано ${raw.length} Б, термов ${terms.length}`);
  for (const t of terms) {
    if (sock.includes("95974.1389") && [1,6,18,46,75].includes(t.idx)) {
      console.log(`  терм #${t.idx}: ${t.start}..${t.end} (${t.len} Б, framed=${t.framed})`);
      console.log(`    hex: ${t.hex}`);
      console.log(`    json: ${t.json}`);
    }
  }
}
