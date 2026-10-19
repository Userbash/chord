// Независимая проверка синтетического кадра (ревью RW-02).
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import zlib from "node:zlib";

const repoHexl = "/home/sanya/orca/project/rust/crates/discord-codec/tests/data/synth-zstd-frame.hexl";
const reHexl = "/home/sanya/re-deep/gw-proto/synth-zstd-frame.hexl";
const termHexl = "/home/sanya/re-deep/gw-proto/synth-zstd-payload-term.hexl";

const repoText = readFileSync(repoHexl, "utf8");
const reText = readFileSync(reHexl, "utf8");
console.log(`repo hexl == re-deep hexl: ${repoText === reText}`);
console.log(`md5(repo)=${createHash("md5").update(repoText).digest("hex")}`);

const frame = Buffer.from(repoText.trim(), "hex");
console.log(`кадр=${frame.length} Б, первая строка без \n: ${repoText.trim().length === repoText.replace(/\n$/, "").length}`);

const termHex = readFileSync(termHexl, "utf8").trim();
const term = Buffer.from(termHex, "hex");
console.log(`терм=${term.length} Б (из synth-zstd-payload-term.hexl)`);
const payload = Buffer.concat(Array(4096).fill(term));
console.log(`payload=${payload.length} Б (терм × 4096)`);
console.log(`md5(payload)=${createHash("md5").update(payload).digest("hex")}`);

// полная распаковка кадра
const out = zlib.zstdDecompressSync(frame);
console.log(`распаковка 67-Б кадра=${out.length} Б; == payload: ${out.equals(payload)}`);
// целостность кадра по компонентам
console.log(`магия кадра: ${frame.subarray(0, 4).toString("hex")}`);
console.log(`FHD=0x${frame[4].toString(16)} (FCS-размер по битам 7-6; Single_Segment=${(frame[4] >> 5) & 1})`);

// сверка терма с PAYLOAD_TERM_HEX из тест-файла
const testSrc = readFileSync("/home/sanya/orca/project/rust/crates/discord-codec/tests/zstd_stream.rs", "utf8");
const m = testSrc.match(/const PAYLOAD_TERM_HEX: &str =\s*\n?\s*"([0-9a-f]+)"/);
console.log(`PAYLOAD_TERM_HEX найден: ${!!m}; совпадает с re-deep термом: ${m ? m[1] === termHex : false}`);
