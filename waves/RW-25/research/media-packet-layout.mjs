#!/usr/bin/env node
// RE-исследование для карточки P4-03 (SRTP): разбор медиапакетов из strace-дампа.
// Только чтение: /home/sanya/re-deep/voice/strace-media-s3.jsonl.
//
// usage: node media-packet-layout.mjs [jsonl]
import { readFileSync } from "node:fs";

const file = process.argv[2] ?? "/home/sanya/re-deep/voice/strace-media-s3.jsonl";
const rows = readFileSync(file, "utf8").trim().split("\n").map((l) => JSON.parse(l));

const media = rows.filter((r) => r.len > 12 && !(r.len === 74 && r.hex.startsWith("0001")));
console.log(`всего пакетов: ${rows.length}, медиа (>12 Б): ${media.length}`);

const byLen = new Map();
for (const r of media) {
  const k = `${r.len}`;
  byLen.set(k, (byLen.get(k) ?? 0) + 1);
}
console.log("размеры:", [...byLen.entries()].sort((a, b) => b[0] - a[0]).map(([k, v]) => `${k}Б×${v}`).join(", "));

console.log("\n=== первые 14 медиапакетов (hex | последние 8 Б) ===");
for (const r of media.slice(0, 14)) {
  const b = Buffer.from(r.hex, "hex");
  const tail = b.subarray(b.length - 8).toString("hex");
  const tail4 = b.readUInt32LE(b.length - 4);
  console.log(
    `${r.ts} ${r.dir.padEnd(3)} len=${String(r.len).padStart(3)} tail8=${tail} ` +
      `tail4le=0x${tail4.toString(16).padStart(8, "0")} hi=${tail4 >>> 31}`,
  );
}

console.log("\n=== разбор 47-байтового пакета (с валидным расширением) ===");
for (const r of media.filter((x) => x.len === 47).slice(0, 3)) {
  const b = Buffer.from(r.hex, "hex");
  console.log(`\n${r.ts} ${r.dir} len=47`);
  console.log(`  полный hex: ${b.toString("hex")}`);
  for (let off = 0; off < b.length; off += 16) {
    console.log(
      `  ${String(off).padStart(2)}: ${b.subarray(off, off + 16).toString("hex").replace(/(..)/g, "$1 ")}`,
    );
  }
  // хвост: tag16 + counter4
  const tag = b.subarray(b.length - 20, b.length - 4);
  const ctr = b.readUInt32LE(b.length - 4);
  console.log(`  tag16  = ${tag.toString("hex")}`);
  console.log(`  ctr4le = 0x${ctr.toString(16).padStart(8, "0")}  (hi-bit ${ctr >>> 31}, n=0x${(ctr & 0x7fffffff).toString(16)})`);
  console.log(`  шифротекст (${b.length - 20} Б) = ${b.subarray(0, b.length - 20).toString("hex")}`);
}

console.log("\n=== 43-байтовые пакеты (аномалия: без валидного расширения) ===");
for (const r of media.filter((x) => x.len === 43).slice(0, 4)) {
  const b = Buffer.from(r.hex, "hex");
  console.log(`\n${r.ts} ${r.dir} len=43`);
  console.log(`  hex: ${b.toString("hex")}`);
  console.log(`  ctr4le=0x${b.readUInt32LE(b.length - 4).toString(16).padStart(8, "0")}`);
}

console.log("\n=== хвосты всех медиапакетов: значение 4-го LE-счётчика ===");
for (const r of media) {
  const b = Buffer.from(r.hex, "hex");
  const ctr = b.readUInt32LE(b.length - 4);
  console.log(
    `${r.ts} ${r.dir.padEnd(3)} len=${String(r.len).padStart(3)} ctr=0x${ctr.toString(16).padStart(8, "0")} hi=${ctr >>> 31}`,
  );
}
