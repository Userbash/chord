#!/usr/bin/env node
// Независимый поиск сырых big-int-векторов §2.5 в живом захвате (сокет 95974.1389).
// Своя минимальная распаковка (zstd-stream: один контекст на сокет, без end()), поиск байтовых
// шаблонов по всему распакованному потоку. Запуск: node raw-search.mjs
import { readFileSync } from "node:fs";
import zlib from "node:zlib";

const lines = readFileSync("/home/sanya/re-deep/capture-run2.jsonl", "utf8")
  .split("\n").map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);

const frames = lines.filter((r) => r.ev === "wsrecv" && r.requestId === "95974.1389");
console.log(`сокет 95974.1389: wsrecv-кадров ${frames.length}`);

const dec = zlib.createZstdDecompress();
const chunks = [];
dec.on("data", (c) => chunks.push(c));
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
console.log(`распаковано ${raw.length} Б`);

const raws = {
  "guild.id 1014537851418980372": "6e0800143004f5cc5c140e",
  "user.id 384299970632876032": "6e08000000c484c94e5505",
  "channel.id 1176785373989445703": "6e08004720841110c85410",
};
let bad = 0;
for (const [name, hex] of Object.entries(raws)) {
  const pat = Buffer.from(hex, "hex");
  let count = 0, pos = 0;
  while ((pos = raw.indexOf(pat, pos)) !== -1) { count++; pos += pat.length; }
  console.log(`${count > 0 ? "PASS" : "FAIL"}  ${name}: найдено ${count}× (шаблон ${hex})`);
  if (count === 0) bad++;
}
console.log(`Итог: ${Object.keys(raws).length - bad}/${Object.keys(raws).length} шаблонов найдено`);
process.exit(bad === 0 ? 0 : 1);
