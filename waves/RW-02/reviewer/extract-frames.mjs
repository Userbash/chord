// Независимое извлечение WS-кадров сокета из capture-run2.jsonl (ревью RW-02).
// Своя реализация: ev=="wsrecv", requestId==SOCK, opcode==2, payload base64 → hex.
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";

const FILE = process.argv[2] ?? "/home/user/clean-room-oracle/capture-run2.jsonl";
const SOCK = process.argv[3] ?? "95974.1389";
const OUT = process.argv[4] ?? `/tmp/opencode/review-rw-02/frames-${SOCK.replace(".", "_")}.hexl`;

const text = readFileSync(FILE, "utf8");
const frames = [];
let seen = 0;
for (const line of text.split("\n")) {
  if (!line) continue;
  if (!line.includes('"wsrecv"')) continue;
  const o = JSON.parse(line);
  if (o.ev !== "wsrecv" || o.requestId !== SOCK || o.opcode !== 2) continue;
  seen++;
  frames.push(Buffer.from(o.payload, "base64"));
}
const joined = frames.map((f) => f.toString("hex")).join("\n") + "\n";
writeFileSync(OUT, joined);
const md5 = createHash("md5").update(joined).digest("hex");
const total = frames.reduce((a, f) => a + f.length, 0);
const magic = (f) => f.length >= 4 && f[0] === 0x28 && f[1] === 0xb5 && f[2] === 0x2f && f[3] === 0xfd;
console.log(`socket ${SOCK}: wsrecv(opcode=2) кадров=${seen}, сжато=${total} Б`);
console.log(`frame0=${frames[0].length} Б, магия frame0=${magic(frames[0])}, магия frames[1:]=${frames.slice(1).filter(magic).length}`);
console.log(`hexl=/tmp...: ${OUT}`);
console.log(`md5(hexl, своя выгрузка)=${md5}`);
