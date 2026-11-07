import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = '  "THREAD_CREATE",\n';
if (!s.includes(from)) throw new Error("m5 anchor missing");
writeFileSync(p, s.replace(from, '  "THREAD_CREATED",\n'));
