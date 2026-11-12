// Независимая проверка оракулов zstd-потока 95974.1389 (ревью RW-02).
// 1) инкрементальная распаковка Node createZstdDecompress БЕЗ end();
// 2) window descriptor из кадра 0;
// 3) сверка с msgs-run2-95974_1389.jsonl (Σbytes, границы термов);
// 4) HELLO: константа из тест-файла репо vs независимая распаковка capture.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import zlib from "node:zlib";

const framesPath = process.argv[2] ?? "/home/user/clean-room-oracle/gw-proto/zstd-frames-95974_1389.hexl";
const msgsPath = process.argv[3] ?? "/home/user/clean-room-oracle/gw-proto/msgs-run2-95974_1389.jsonl";
const testPath = "/home/user/orca/project/rust/crates/discord-codec/tests/zstd_stream.rs";

const frames = readFileSync(framesPath, "utf8").trim().split("\n").map((l) => Buffer.from(l, "hex"));
console.log(`кадров=${frames.length}, сжато=${frames.reduce((a, f) => a + f.length, 0)} Б`);

// window descriptor кадра 0
const f0 = frames[0];
const fhd = f0[4];
const singleSegment = (fhd >> 5) & 1;
let windowSize = null;
if (!singleSegment) {
  const wd = f0[5];
  const exp = wd >> 3;
  const mant = wd & 7;
  const base = 1 << (10 + exp);
  windowSize = base + (base / 8) * mant;
  console.log(`FHD=0x${fhd.toString(16)}, window descriptor=0x${wd.toString(16)}, exponent=${exp} → окно ${windowSize} Б`);
} else {
  console.log(`FHD=0x${fhd.toString(16)}, Single_Segment=1`);
}

// инкрементально, end() НЕ вызывается
const dec = zlib.createZstdDecompress();
const chunks = [];
dec.on("data", (c) => chunks.push(c));
dec.on("error", (e) => console.error("zstd error:", e.message));
for (const f of frames) dec.write(f);
await new Promise((r) => setTimeout(r, 1500));
const out = Buffer.concat(chunks);
console.log(`распаковано Node (без end())=${out.length} Б`);
console.log(`md5(out)=${createHash("md5").update(out).digest("hex")}`);
const hello = out.subarray(0, 138).toString("hex");
console.log(`out[0:138] hex=${hello}`);

// HELLO из тест-файла
const testSrc = readFileSync(testPath, "utf8");
const m = testSrc.match(/const HELLO_HEX: &str = "([0-9a-f]+)"/);
console.log(`HELLO из zstd_stream.rs: ${m ? "найден" : "НЕ НАЙДЕН"}`);
if (m) console.log(`  совпадает с out[0:138]: ${m[1] === hello}`);

if (windowSize !== null) {
  console.log(`окно: ${out.length} − ${windowSize} = ${out.length - windowSize}; == 759280: ${out.length - windowSize === 759280}`);
}
dec.destroy();

// msgs: Σbytes и границы термов в префиксе 759280
const recs = readFileSync(msgsPath, "utf8").trim().split("\n").map((l) => JSON.parse(l));
const sumBytes = recs.reduce((a, r) => a + r.bytes, 0);
console.log(`msgs: термов=${recs.length}, Σbytes=${sumBytes}, Σ(bytes+1)=${sumBytes + recs.length}`);
let off = 0, terms = 0;
for (const r of recs) {
  const end = off + 1 + r.bytes;
  if (end > 759280) break;
  off = end;
  terms++;
}
console.log(`префикс 759280: полных термов=${terms}, смещение=${off}, остаток=${759280 - off}`);
console.log(`первый терм bytes=${recs[0].bytes}`);
