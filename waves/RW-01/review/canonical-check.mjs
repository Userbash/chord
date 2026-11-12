#!/usr/bin/env node
// Независимая проверка канонических байтов из canonical_encoder_matches_fixed_bytes
// (etf_properties.rs): свой мини-энкодер на BigInt по правилам ТЗ RW-01, сверка
// с hex-строками, присутствующими в тест-файле (или с рантайм-значениями — для format!-кейсов).
import { readFileSync } from "node:fs";

const src = readFileSync("/home/user/orca/project/rust/crates/discord-codec/tests/etf_properties.rs", "utf8");
let bad = 0, total = 0;
function check(name, expected, computed, { inTest = null } = {}) {
  total++;
  const testOk = inTest === null ? true : src.includes(`"${expected}"`) === inTest;
  const ok = expected === computed && testOk;
  if (!ok) bad++;
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}: ${computed}${inTest === null ? " (format!-кейс, рантайм-сверка — exttest.rs)" : ""}`);
}

function intHex(v) {
  const n = BigInt(v);
  if (n >= 0n && n <= 255n) return "61" + n.toString(16).padStart(2, "0");
  if (n >= -2147483648n && n <= 2147483647n) return "62" + BigInt.asUintN(32, n).toString(16).padStart(8, "0");
  const mag = n < 0n ? -n : n;
  const digits = mag === 0n ? 1 : Math.ceil(mag.toString(2).length / 8);
  let le = "", m = mag;
  for (let i = 0; i < digits; i++) { le += Number(m & 0xffn).toString(16).padStart(2, "0"); m >>= 8n; }
  return "6e" + digits.toString(16).padStart(2, "0") + (n < 0n ? "01" : "00") + le;
}
const f64Hex = (x) => "46" + Buffer.from(new Float64Array([x]).buffer).reverse().toString("hex");
const binHex = (s) => { const b = Buffer.from(s, "utf8"); return "6d" + b.length.toString(16).padStart(8, "0") + b.toString("hex"); };
const atomHex = (s) => {
  const b = Buffer.from(s, "utf8");
  const ascii = [...b].every((c) => c < 0x80);
  if (ascii) return "64" + b.length.toString(16).padStart(4, "0") + b.toString("hex");
  return (b.length <= 255 ? "77" + b.length.toString(16).padStart(2, "0") : "76" + b.length.toString(16).padStart(4, "0")) + b.toString("hex");
};
const listHex = (count, inner) => "6c" + count.toString(16).padStart(8, "0") + inner + "6a";
const tupleHex = (count, inner) => count <= 255 ? "68" + count.toString(16).padStart(2, "0") + inner : "69" + count.toString(16).padStart(8, "0") + inner;
const mapHex = (count, inner) => "74" + count.toString(16).padStart(8, "0") + inner;

check("int 10", "610a", intHex(10), { inTest: true });
check("int 41250", "620000a122", intHex(41250), { inTest: true });
check("int 255", "61ff", intHex(255), { inTest: true });
check("int -1", "62ffffffff", intHex(-1), { inTest: true });
check("int 2^31", "6e040000000080", intHex(2_147_483_648), { inTest: true });
check("int 1792391313326", "6e0600aef7d852a101", intHex(1_792_391_313_326), { inTest: true });
check("int i64::MAX", "6e0800ffffffffffffff7f", intHex(9_223_372_036_854_775_807n), { inTest: true });
check("int i64::MIN", "6e08010000000000000080", intHex(-9_223_372_036_854_775_808n), { inTest: true });
check("float 1.5", "463ff8000000000000", f64Hex(1.5), { inTest: true });
check("atom op", "6400026f70", atomHex("op"), { inTest: true });
check("text abc", "6d00000003616263", binHex("abc"), { inTest: true });
check("list [1,2]", "6c00000002610161026a", listHex(2, "6101" + "6102"), { inTest: true });
check("tuple [1]", "68016101", tupleHex(1, "6101"), { inTest: true });
check("map {a:1}", "7400000001640001616101", mapHex(1, atomHex("a") + "6101"), { inTest: true });
check("nil", "6a", "6a", { inTest: true });
check("text empty", "6d00000000", binHex(""), { inTest: true });
check("list []", "6c000000006a", listHex(0, ""), { inTest: true });
check("tuple []", "6800", tupleHex(0, ""), { inTest: true });
check("map {}", "7400000000", mapHex(0, ""), { inTest: true });
check("guild_id 1014537851418980372", "6e0800143004f5cc5c140e", intHex(1_014_537_851_418_980_372n), { inTest: true });

// format!-кейсы (в тест-файле нет литерала; сверено с рантайм-выводом экзотики — exttest.rs)
const uni = "Проверка";
check("atom unicode «Проверка»", "7710d09fd180d0bed0b2d0b5d180d0bad0b0", atomHex(uni));
const longAtom = uni.repeat(40);
check("atom 640B (>255 → 0x76)", "760280" + Buffer.from(longAtom).toString("hex"), atomHex(longAtom));
check("tuple 255 prefix", "68ff", tupleHex(255, "").slice(0, 4), { inTest: true });
check("tuple 256 prefix", "6900000100", tupleHex(256, "").slice(0, 10), { inTest: true });
check("list 256 prefix", "6c00000100", listHex(256, "").slice(0, 10), { inTest: true });

console.log(`\nИтог: ${total - bad}/${total} PASS, FAIL=${bad}`);
process.exit(bad === 0 ? 0 : 1);
