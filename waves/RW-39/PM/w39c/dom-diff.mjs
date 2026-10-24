// Сверка текущего ui/index.html с замороженным DOM-CONTRACT §3 (списки §9.2).
// Ничего не правит: только печатает, каких id/классов контракта нет в html
// и какие есть в html, но контрактом не предусмотрены.
// Запуск из корня репозитория: node waves/RW-39/PM/w39c/dom-diff.mjs
import { readFileSync } from "node:fs";

const contract = readFileSync("waves/RW-39/DESIGN/DOM-CONTRACT.md", "utf8");
const html = readFileSync("rust/gui-spike/ui/index.html", "utf8");

const ticks = (text) => [...text.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
const slice = (from, to) => {
  const i = contract.indexOf(from);
  const j = contract.indexOf(to, i);
  if (i < 0 || j < 0) throw new Error(`не найден блок «${from}»…«${to}» в §9.2`);
  return contract.slice(i, j);
};

const contractIds = ticks(slice("**id (29):**", "**Классы (76):**"));
const contractClasses = ticks(slice("**Классы (76):**", "Правила правки"));

const htmlIds = new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
const htmlClasses = new Set(
  [...html.matchAll(/\sclass="([^"]+)"/g)].flatMap((m) => m[1].trim().split(/\s+/)),
);

const missingIds = contractIds.filter((id) => !htmlIds.has(id));
const missingClasses = contractClasses.filter((c) => !htmlClasses.has(c));
const extraIds = [...htmlIds].filter((id) => !contractIds.includes(id));
const extraClasses = [...htmlClasses].filter((c) => !contractClasses.includes(c));

console.log(`контракт: id=${contractIds.length}, классов=${contractClasses.length}`);
console.log(`index.html: id=${htmlIds.size}, классов=${htmlClasses.size}`);
console.log(`\nнет id (${missingIds.length}): ${missingIds.join(", ") || "—"}`);
console.log(`\nнет классов (${missingClasses.length}): ${missingClasses.join(", ") || "—"}`);
console.log(`\nлишние id (${extraIds.length}): ${extraIds.join(", ") || "—"}`);
console.log(`\nлишние классы (${extraClasses.length}): ${extraClasses.join(", ") || "—"}`);
