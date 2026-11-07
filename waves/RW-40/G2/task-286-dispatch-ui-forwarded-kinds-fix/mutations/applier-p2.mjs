import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = "if (!FORWARDED.has(kind)) {";
if (!s.includes(from)) throw new Error("p2 anchor missing");
writeFileSync(p, s.replace(from, "if (false && !FORWARDED.has(kind)) {"));
