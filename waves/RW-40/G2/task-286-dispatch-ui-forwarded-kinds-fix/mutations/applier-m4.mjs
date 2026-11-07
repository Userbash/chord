import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = '  "TYPING_START",\n';
if (!s.includes(from)) throw new Error("m4 anchor missing");
writeFileSync(p, s.replace(from, '  "TYPING_START",\n  "TYPING_START",\n'));
