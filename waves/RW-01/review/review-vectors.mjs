#!/usr/bin/env node
// Независимая перепроверка оракулов RW-01 (ревьюер, 2026-10-19).
// Сверяет hex-константы тестов с СВЕЖЕЙ вырезкой gw-slice-terms.mjs, длины — с полем
// `bytes` в msgs-run2-95974_1389.jsonl, ожидаемые значения — с JSON-деревьями того же
// файла, а big-int — независимым пересчётом BigInt (правильный и ошибочный порядок).
// Запуск: node review-vectors.mjs
import { readFileSync } from "node:fs";

const TEST_VECTORS = "/home/sanya/orca/project/rust/crates/discord-codec/tests/etf_vectors.rs";
const TEST_PROPS = "/home/sanya/orca/project/rust/crates/discord-codec/tests/etf_properties.rs";
const FRESH = "/tmp/opencode/rw-01/review/oracle-fresh.txt";
const JSONL = "/home/sanya/re-deep/gw-proto/msgs-run2-95974_1389.jsonl";
const SPEC = "/home/sanya/orca/project/docs/re/gateway-protocol.md";

let failures = 0;
const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok, detail });
  const mark = ok ? "PASS" : "FAIL";
  console.log(`${mark}  ${name}${detail ? "  — " + detail : ""}`);
  if (!ok) failures++;
}

// ---------- 1. hex-константы из тестов ----------
function parseHexConsts(file) {
  const src = readFileSync(file, "utf8");
  const out = {};
  const re = /const\s+(\w+_HEX)\s*:\s*&str\s*=\s*((?:"[0-9a-fA-F]*"\s*)+);/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    out[m[1]] = [...m[2].matchAll(/"([0-9a-fA-F]*)"/g)].map((x) => x[1]).join("");
  }
  return out;
}
const vectors = {
  HELLO_HEX: { idx: 1, bytes: 137 },
  ACK_HEX: { idx: 6, bytes: 42 },
  PRESENCE_HEX: { idx: 18, bytes: 209 },
  MESSAGE_ACK_HEX: { idx: 46, bytes: 147 },
  MEMBER_LIST_HEX: { idx: 75, bytes: 256 },
};

// ---------- 2. свежая вырезка из живого захвата ----------
const freshText = readFileSync(FRESH, "utf8");
const freshSockLines = freshText.split("\n");
const fresh = {}; // idx -> hex
{
  let inSock = false;
  let curIdx = null;
  let mode = null;
  for (const line of freshSockLines) {
    if (line.startsWith("=== сокет ")) {
      inSock = line.includes("95974.1389");
      continue;
    }
    if (!inSock) continue;
    const tm = line.match(/терм #(\d+):/);
    if (tm) {
      curIdx = Number(tm[1]);
      mode = null;
      continue;
    }
    const hm = line.match(/^\s+hex: ([0-9a-f]+)$/);
    if (hm && curIdx !== null) {
      fresh[curIdx] = hm[1];
      mode = "hex";
      continue;
    }
    if (/^\s+json: /.test(line) && curIdx !== null) mode = "json";
  }
}

// ---------- 3. JSONL с ожидаемыми деревьями ----------
const jsonl = {};
for (const line of readFileSync(JSONL, "utf8").split("\n")) {
  if (!line) continue;
  let obj;
  try { obj = JSON.parse(line); } catch { continue; }
  if ([1, 6, 18, 46, 75].includes(obj.idx)) jsonl[obj.idx] = obj;
}

// ---------- 4. сверка hex: тесты ↔ свежая вырезка ----------
for (const [constName, meta] of Object.entries(vectors)) {
  for (const [label, file] of [["etf_vectors.rs", TEST_VECTORS], ["etf_properties.rs", TEST_PROPS]]) {
    const hexFromTest = parseHexConsts(file)[constName];
    if (!hexFromTest) { check(`${label}:${constName}: константа найдена`, false); continue; }
    const hexFresh = fresh[meta.idx];
    if (!hexFresh) { check(`${label}:${constName}: терм #${meta.idx} в свежей вырезке`, false); continue; }
    check(`hex ${label}:${constName} == gw-slice-terms #${meta.idx}`, hexFromTest === hexFresh,
      hexFromTest === hexFresh ? `${hexFromTest.length / 2} Б` : `test=${hexFromTest.slice(0, 40)}… fresh=${hexFresh.slice(0, 40)}…`);
  }
}

// ---------- 5. длины: тест ↔ поле bytes в JSONL ----------
for (const [constName, meta] of Object.entries(vectors)) {
  const hexFromTest = parseHexConsts(TEST_VECTORS)[constName];
  const lenWithVersion = hexFromTest.length / 2;
  const j = jsonl[meta.idx];
  check(`длина ${constName}: ${lenWithVersion}-1 == bytes(${j.bytes})`,
    lenWithVersion - 1 === j.bytes && j.bytes === meta.bytes,
    `${lenWithVersion}-1=${lenWithVersion - 1}, jsonl.bytes=${j.bytes}, ТЗ=${meta.bytes}`);
  check(`${constName}: ведущий 0x83 в тесте`, hexFromTest.startsWith("83"), hexFromTest.slice(0, 2));
}

// ---------- 6. ожидаемые значения тестов ↔ JSON-деревья JSONL ----------
function get(obj, path) {
  return path.split(".").reduce((acc, k) => {
    if (acc === null || acc === undefined) return acc;
    const m = k.match(/^(.*)\[(\d+)\]$/);
    if (m) return acc[m[1]]?.[Number(m[2])];
    return acc[k];
  }, obj);
}
const expectedValues = {
  1: [
    ["op", 10], ["s", "nil"], ["t", "nil"],
    ["d.heartbeat_interval", 41250],
    ["d._trace[0]", '["gateway-prd-arm-us-east1-d-04m5",{"micros":0.0}]'],
  ],
  6: [["op", 11], ["s", "nil"], ["t", "nil"], ["d", "nil"]],
  18: [
    ["op", 0], ["s", 13], ["t", "PRESENCE_UPDATE"],
    ["d.guild_id", "1014537851418980372"],
    ["d.user.id", "702520807447855184"],
    ["d.activities", null], ["d.status", "online"], ["d.processed_at_timestamp", 0],
    ["d.client_status.mobile", "online"], ["d.client_status.vr", "idle"],
  ],
  46: [
    ["op", 0], ["s", 38], ["t", "MESSAGE_ACK"],
    ["d.channel_id", "1293192721237938187"], ["d.flags", 0], ["d.last_viewed", 4310],
    ["d.message_id", "1561348877573947515"], ["d.version", 128950],
  ],
  75: [
    ["op", 0], ["s", 58], ["t", "GUILD_MEMBER_LIST_UPDATE"],
    ["d.id", "1659606647"], ["d.guild_id", "1014537851418980372"],
    ["d.member_count", 24228], ["d.online_count", 708],
    ["d.groups[0].id", "1062745847512715385"], ["d.groups[0].count", 3],
    ["d.groups[1].id", "online"], ["d.groups[1].count", 12],
    ["d.ops[0].index", 11], ["d.ops[0].op", "DELETE"],
  ],
};
for (const [idx, pairs] of Object.entries(expectedValues)) {
  for (const [path, want] of pairs) {
    const got = get(jsonl[idx].term, path);
    check(`JSONL #${idx}.${path} == ${JSON.stringify(want)}`, JSON.stringify(got) === JSON.stringify(want), `got=${JSON.stringify(got)}`);
  }
}

// ---------- 7. типовые различения на уровне байтов (JSON их не видит) ----------
{
  const hello = parseHexConsts(TEST_VECTORS).HELLO_HEX;
  const ack = parseHexConsts(TEST_VECTORS).ACK_HEX;
  const presence = parseHexConsts(TEST_VECTORS).PRESENCE_HEX;
  const mack = parseHexConsts(TEST_VECTORS).MESSAGE_ACK_HEX;
  // #6: d -> ATOM "nil" (0x64 00 03 "nil"), НЕ NIL_EXT
  check("ACK: d — ATOM_EXT nil (0x64…)", ack.includes("640001646400036e696c"), ack.match(/64000164[0-9a-f]+/)?.[0]);
  // #18: activities -> NIL_EXT (0x6a), не атом
  check("PRESENCE: activities — NIL_EXT (0x6a)", presence.includes("64000a616374697669746965736a"), presence.slice(presence.indexOf("61637469766974696573"), presence.indexOf("61637469766974696573") + 28));
  // #46: ключи d — BINARY_EXT (0x6d), не атомы
  for (const key of ["channel_id", "flags", "last_viewed", "message_id", "version"]) {
    const keyHex = Buffer.from(key, "utf8").toString("hex");
    check(`MESSAGE_ACK: ключ ${key} — BINARY_EXT`, mack.includes(`6d000000${key.length.toString(16).padStart(2, "0")}${keyHex}`), `6d…${keyHex}`);
  }
  // #1: t — атом "nil" (не NIL_EXT)
  check("HELLO: t — ATOM_EXT nil", hello.endsWith("640001746400036e696c"), hello.slice(-22));
}

// ---------- 8. big-int: независимый пересчёт BigInt ----------
function leToBigInt(bytes) {
  let v = 0n;
  for (let k = bytes.length - 1; k >= 0; k--) v = (v << 8n) | BigInt(bytes[k]);
  return v;
}
function parseRight(hex) { // [6e][n][sign][n LE]
  const b = Buffer.from(hex, "hex");
  const n = b[1];
  const sign = b[2];
  const mag = leToBigInt(b.subarray(3, 3 + n));
  return sign ? -mag : mag;
}
function parseWrong(hex) { // ошибочный порядок: [6e][n][n байт LE][sign]
  const b = Buffer.from(hex, "hex");
  const n = b[1];
  const mag = leToBigInt(b.subarray(2, 2 + n));
  const sign = b[2 + n];
  return sign ? -mag : mag;
}

// три вектора из docs/re/gateway-protocol.md §2.5
const specRaws = [
  "6e0800143004f5cc5c140e",
  "6e08000000c484c94e5505",
  "6e08004720841110c85410",
];
const spec = readFileSync(SPEC, "utf8");
const specCorrect = [...spec.matchAll(/→ \*\*(\d+)\*\*/g)].map((m) => m[1]);
const specWrong = [...spec.matchAll(/ошибочный[^`]{0,80}`(-?\d+)`/g)].map((m) => m[1]);
check("спека: найдены 3 правильных и 3 ошибочных значения", specCorrect.length === 3 && specWrong.length === 3,
  `correct=${specCorrect.length} [${specCorrect}], wrong=${specWrong.length} [${specWrong}]`);

const testSrc = readFileSync(TEST_VECTORS, "utf8");
const testTriples = [...testSrc.matchAll(/\(\s*(-?[\d_]+),\s*"(6e[0-9a-f]+)",\s*(-[\d_]+),\s*\)/g)]
  .map((m) => ({ expected: BigInt(m[1].replaceAll("_", "")), src: m[2], wrong: BigInt(m[3].replaceAll("_", "")) }));
check("тест: найдены 3 big-int тройки", testTriples.length === 3, `n=${testTriples.length}`);

for (let i = 0; i < 3; i++) {
  const t = testTriples[i];
  const right = parseRight(t.src);
  const wrong = parseWrong(t.src);
  check(`bigint[${i}] ${t.src}: независимый пересчёт == тест`, right === t.expected, `${right} == ${t.expected}`);
  check(`bigint[${i}] ${t.src}: тест == спека §2.5`, t.expected.toString() === specCorrect[i], `test=${t.expected} spec=${specCorrect[i]}`);
  check(`bigint[${i}] ${t.src}: «ошибочный» порядок == тест-негатив`, wrong === t.wrong, `${wrong} == ${t.wrong}`);
  check(`bigint[${i}] ${t.src}: негатив == значение в спеке`, t.wrong.toString() === specWrong[i], `test=${t.wrong} spec=${specWrong[i]}`);
  check(`bigint[${i}] ${t.src}: правильное != ошибочное`, right !== wrong);
  check(`bigint[${i}] ${t.src}: правильное > 0 (снежинка)`, right > 0n);
}

// граница: i64::MIN и i64::MAX по канону теста
const minHex = "6e08010000000000000080";
const maxHex = "6e0800ffffffffffffff7f";
check("bigint i64::MIN независимо", parseRight(minHex) === -9223372036854775808n, `${parseRight(minHex)}`);
check("bigint i64::MAX независимо", parseRight(maxHex) === 9223372036854775807n, `${parseRight(maxHex)}`);

// ---------- 6b. свежий декод gw-slice ↔ JSONL term (два независимых декодера одного сырья) ----------
function deepEqual(a, b) {
  if (a === b) return true;
  if (typeof a !== typeof b || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) return a.length === b.length && a.every((x, i) => deepEqual(x, b[i]));
  if (typeof a === "object") {
    const ka = Object.keys(a).sort(), kb = Object.keys(b).sort();
    return ka.length === kb.length && ka.every((k, i) => k === kb[i] && deepEqual(a[k], b[k]));
  }
  return false;
}
const freshJson = {};
{
  let inSock = false, curIdx = null;
  for (const line of freshSockLines) {
    if (line.startsWith("=== сокет ")) { inSock = line.includes("95974.1389"); continue; }
    if (!inSock) continue;
    const tm = line.match(/терм #(\d+):/);
    if (tm) { curIdx = Number(tm[1]); continue; }
    const jm = line.match(/^\s+json: (.+)$/);
    if (jm && curIdx !== null) { try { freshJson[curIdx] = JSON.parse(jm[1]); } catch {} }
  }
}
for (const idx of [1, 6, 18, 46, 75]) {
  check(`свежий декод #${idx} == JSONL term (whitespace-insensitive)`,
    deepEqual(freshJson[idx], jsonl[idx].term), `idx=${idx}`);
}

// ---------- итог ----------
console.log("\n===============================");
console.log(`Итог: ${results.length - failures}/${results.length} PASS, FAIL=${failures}`);
process.exit(failures === 0 ? 0 : 1);
