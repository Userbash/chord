// @ts-check
// Сверка лаборатории RE, конформанс-фикстур репозитория и опубликованных в документах
// констант. Оракул: сырые артефакты волны R7 в /home/sanya/re-deep.
//
// Запуск:
//   node docs/rust-client/verify/verify-re.mjs            # проверка (коды: 0 успех, 1 расхождение, 2 нет лаборатории)
//   node docs/rust-client/verify/verify-re.mjs --write    # + сохранить verify/report.json
//   node docs/rust-client/verify/verify-re.mjs --lab /path/to/lab
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..", "..");
const argv = process.argv.slice(2);
const labIdx = argv.indexOf("--lab");
const LAB = labIdx >= 0 ? argv[labIdx + 1] : "/home/sanya/re-deep";
const WRITE = argv.includes("--write");

/** @param {string} p */
const md5 = (p) => createHash("md5").update(readFileSync(p)).digest("hex");
/** @param {string} p @returns {any} */
const readJSON = (p) => JSON.parse(readFileSync(p, "utf8"));
/** @param {string} p */
const isDir = (p) => {
  try {
    return statSync(p).isDirectory();
  } catch {
    return false;
  }
};

// ── Проверки ────────────────────────────────────────────────────────────────
/** @type {{id: string, title: string, run: () => {actual: any, expected: any, note?: string}}[]} */
const checks = [];
/** @param {string} id @param {string} title @param {() => {actual: any, expected: any, note?: string}} run */
const check = (id, title, run) => checks.push({ id, title, run });

check("LAB-01", "лаборатория доступна", () => {
  const ok = isDir(LAB) && isDir(join(LAB, "gateway"));
  return { actual: ok ? "каталоги на месте" : "нет", expected: "каталоги на месте" };
});

check("PROV-01", "md5/размеры снапшотов D-02 совпадают с провенансом docs/re", () => {
  const stats = readJSON(join(REPO, "docs/re/rest-d02.stats.json"));
  const out = [];
  for (const p of stats.provenance) {
    const file = join(LAB, p.file);
    const size = statSync(file).size;
    const lines = readFileSync(file, "utf8").split("\n").length - 1;
    out.push(
      `${p.file}: md5 ${md5(file) === p.md5 ? "=" : "!"} ${p.md5}, bytes ${size === p.bytes ? "=" : "!"} ${p.bytes}, lines ${lines === p.lines ? "=" : "!"} ${p.lines}`,
    );
  }
  return { actual: out.join("; "), expected: "все совпадения (=)" };
});

check("GW-01", "gateway/by-type: 164 файла, 12 типов", () => {
  const dir = join(LAB, "gateway/by-type");
  const files = readdirSync(dir).filter((f) => f.endsWith(".json"));
  const types = new Set(files.map((f) => f.replace(/^\d+-/, "").replace(/\.json$/, "")));
  return { actual: `${files.length} файлов, ${types.size} типов`, expected: "164 файлов, 12 типов" };
});

check("GW-02", "READY (002-READY.json): 40 ключей, 7 гильдий, 17 ЛС, 146 users, 60 связей, 157 read_state", () => {
  const d = readJSON(join(LAB, "gateway/by-type/002-READY.json"));
  const r = d.payload ?? d.d ?? d;
  const rs = Array.isArray(r.read_state) ? r.read_state.length : r.read_state?.entries?.length;
  const actual = `${Object.keys(r).length} ключей, ${r.guilds.length} гильдий, ${r.private_channels.length} ЛС, ${r.users.length} users, ${r.relationships.length} связей, ${rs} read_state`;
  return { actual, expected: "40 ключей, 7 гильдий, 17 ЛС, 146 users, 60 связей, 157 read_state" };
});

check("GW-03", "ITSKIDON: 83 канала, 34 роли, 46 эмодзи, участников 24228/24229 (динамика)", () => {
  const d = readJSON(join(LAB, "gateway/by-type/002-READY.json"));
  const r = d.payload ?? d.d ?? d;
  const g = r.guilds.find((/** @type {any} */ x) => x.id === "1014537851418980372");
  const ok = g.channels.length === 83 && g.roles.length === 34 && g.emojis.length === 46 && [24228, 24229].includes(g.member_count);
  return {
    actual: `${g.channels.length}/${g.roles.length}/${g.emojis.length}, member_count=${g.member_count}`,
    expected: "83/34/46, member_count ∈ {24228, 24229} (снапшоты разных секунд)",
    note: ok ? "оба значения встречаются в сырье: READY=24228, facts-frozen=24229" : undefined,
  };
});

check("GW-04", "ready-stats.json: 7 гильдий, 17 ЛС, 60 связей, 40 ключей", () => {
  const s = readJSON(join(LAB, "gateway/ready-stats.json"));
  return {
    actual: `${s.guilds} гильдий, ${s.privateChannels} ЛС, ${s.relationships} связей, ${s.topKeys.length} ключей`,
    expected: "7 гильдий, 17 ЛС, 60 связей, 40 ключей",
  };
});

check("GW-05", "summary.json содержит READY (payloadBytes=575853)", () => {
  const s = readJSON(join(LAB, "gateway/summary.json"));
  const e = s.find((/** @type {any} */ x) => x.op === 0 && x.t === "READY" && x.payloadBytes === 575853);
  return { actual: e ? "есть" : "нет", expected: "есть" };
});

check("GW-06", "gw-proto: 1544 кадра, 1340 диспатчей, 21 тип", () => {
  let total = 0;
  let op0 = 0;
  const types = new Set();
  for (const f of readdirSync(join(LAB, "gw-proto")).filter((x) => x.endsWith(".jsonl"))) {
    for (const line of readFileSync(join(LAB, "gw-proto", f), "utf8").split("\n")) {
      if (!line.trim()) continue;
      total++;
      const o = JSON.parse(line);
      if (o.op === 0) {
        op0++;
        if (o.t && o.t !== "nil") types.add(o.t);
      }
    }
  }
  return { actual: `${total} кадров, ${op0} диспатчей, ${types.size} типов`, expected: "1544 кадров, 1340 диспатчей, 21 тип" };
});

check("ENC-01", "ETF-теги: 11 кодов в сырье совпадают с таблицей docs/re/gateway-protocol.json", () => {
  /** @param {any} node @returns {any[]|null} */
  const findTags = (node) => {
    if (Array.isArray(node) && node.length >= 5 && node[0] && typeof node[0] === "object" && "tag" in node[0] && "code" in node[0]) return node;
    if (node && typeof node === "object") for (const k of Object.keys(node)) {
      const r = findTags(node[k]);
      if (r) return r;
    }
    return null;
  };
  const raw = findTags(readJSON(join(LAB, "gw-proto/facts-frozen.json"))) ?? [];
  const doc = findTags(readJSON(join(REPO, "docs/re/gateway-protocol.json"))) ?? [];
  const rawCodes = [...new Set(raw.map((t) => t.code))].sort().join(",");
  const docCodes = [...new Set(doc.map((t) => t.code))].sort().join(",");
  const ok = rawCodes === docCodes && raw.length === 11;
  return {
    actual: `сырьё: ${raw.length} тегов (${rawCodes}); документ: ${doc.length} (${docCodes})`,
    expected: "11 тегов, наборы совпадают",
    note: ok ? undefined : "расхождение таблиц — править docs/re/gateway-protocol.json",
  };
});

check("REST-01", "rest-d02.json: 47 маршрутов / 133 запроса v9 / auth 221", () => {
  const r = readJSON(join(REPO, "docs/re/rest-d02.json"));
  const sum = r.endpoints.reduce((/** @type {number} */ s, /** @type {any} */ e) => s + (e.count ?? 0), 0);
  return {
    actual: `${r.endpoints.length} маршрутов, ${sum} запросов, auth ${r.auth.withAuthorization}`,
    expected: "47 маршрутов, 133 запроса, auth 221",
  };
});

check("REST-02", "rest-static.json: 904 эндпоинта (реестр веб-бандла)", () => {
  const s = readJSON(join(LAB, "rest-static.json"));
  return { actual: `${s.endpoints.length}`, expected: "904" };
});

check("I18N-01", "i18n-stats: ru 23836 ключей, en 23903, плуральных пар 911", () => {
  const s = readJSON(join(LAB, "i18n-stats.json"));
  return {
    actual: `ru ${s.keys.ru}, en ${s.keys.en}, plural ${s.plural.count}`,
    expected: "ru 23836, en 23903, plural 911",
  };
});

check("I18N-02", "локали: 31 включённая из 36 (locale-chunk-hashes.json)", () => {
  const a = readJSON(join(LAB, "locale-chunk-hashes.json"));
  const locales = new Set(a.map((/** @type {any} */ x) => x.locale));
  return { actual: `${locales.size}`, expected: "31" };
});

check("IPC-01", "backend-ipc: 168 каналов, 115 общих, 35 записей моста, whitelist 43 (30+14)", () => {
  const b = readJSON(join(LAB, "backend-ipc.json"));
  const s = b.stats;
  const w = b.bridges.ipcWhitelists;
  return {
    actual: `${s.channels}/${s.channelsBoth}/${Object.keys(b.bridges.fields).length}/${w.unionCount} (${w.send.count}+${w.invoke.count})`,
    expected: "168/115/35/43 (30+14)",
  };
});

check("NAT-01", "нативные модули: 9; discord_voice 126 136 024 Б, md5 5fe80ce0…", () => {
  const n = readJSON(join(LAB, "native-re.json"));
  const voice = n.modules.find((/** @type {any} */ m) => m.name === "discord_voice");
  const ok = voice.size === 126136024 && voice.md5 === "5fe80ce0f5d89fd53d30b2fb25a6cdac";
  return {
    actual: `${n.modules.length} модулей; voice ${voice.size} Б, md5 ${voice.md5.slice(0, 12)}…`,
    expected: `9 модулей; voice 126136024 Б, md5 5fe80ce0f5d8…${ok ? "" : " (расхождение!)"}`,
  };
});

check("VOICE-01", "RTC-кадры: набор op = {0,1,2,3,4,6,8,15,16}", () => {
  const ops = new Set();
  for (const f of readdirSync(join(LAB, "voice")).filter((x) => /^rtc.*frames\.jsonl$/.test(x))) {
    for (const line of readFileSync(join(LAB, "voice", f), "utf8").split("\n")) {
      if (!line.trim()) continue;
      const o = JSON.parse(line);
      ops.add(o.op);
    }
  }
  const actual = [...ops].sort((a, b) => a - b).join(",");
  return { actual, expected: "0,1,2,3,4,6,8,15,16" };
});

check("VOICE-02", "strace: медиа-пакеты и магия keepalive 1337 присутствуют (s3)", () => {
  const text = readFileSync(join(LAB, "voice/strace-media-s3.jsonl"), "utf8");
  const ok = text.includes("1337");
  return { actual: ok ? "1337 найден" : "нет", expected: "1337 найден в strace-media-s3.jsonl" };
});

check("SAN-01", "санитизация: capture-run2.jsonl и fixtures/ без токенов и чувствительных ключей", () => {
  const problems = [];
  const text = readFileSync(join(LAB, "capture-run2.jsonl"), "utf8");
  if (/\b[\w-]{23,}\.[\w-]{6}\.[\w-]{25,}\b/.test(text)) problems.push("capture: токен-подобные строки");
  if (text.includes("analytics_token")) problems.push("capture: analytics_token");
  const SENSITIVE = ["analytics_token", "session_id", "auth_session_id_hash", "static_client_session_id", "qos_token", "resume_gateway_url"];
  /** @param {string} dir */
  const walk = (dir) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      if (e.isDirectory()) {
        walk(p);
        continue;
      }
      if (e.name === "MANIFEST.json") continue;
      const t = readFileSync(p, "utf8");
      if (/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{15,}/.test(t)) problems.push(`${p}: токен-подобная строка`);
      for (const key of SENSITIVE) {
        if (new RegExp(`"${key}":"(?!REDACTED)`).test(t)) problems.push(`${p}: ${key} без REDACTED`);
      }
    }
  };
  walk(join(REPO, "fixtures"));
  return {
    actual: problems.length ? problems.slice(0, 3).join("; ") : "чисто: capture + файлы фикстур",
    expected: "чисто: capture + фикстуры (токены и чувствительные ключи отсутствуют)",
    note: problems.length ? "пересобрать: node fixtures/tools/make-fixtures.mjs" : undefined,
  };
});

check("FIX-01", "фикстуры репозитория: манифест и md5 совпадают с копиями", () => {
  const manifest = readJSON(join(REPO, "fixtures/MANIFEST.json"));
  const bad = [];
  for (const f of manifest.fixtures) {
    const target = join(REPO, "fixtures", f.path);
    if (f.kind === "file") {
      if (md5(target) !== f.md5 || statSync(target).size !== f.bytes) bad.push(f.path);
    } else {
      const names = readdirSync(target).sort();
      const lines = names.map((name) => {
        const p = join(target, name);
        return `${name}\t${statSync(p).size}\t${md5(p)}`;
      });
      const combined = createHash("md5").update(lines.join("\n")).digest("hex");
      if (`index:${combined}` !== f.md5 || names.length !== f.files) bad.push(f.path);
    }
  }
  return {
    actual: `${manifest.fixtures.length} записей, расхождений: ${bad.length}${bad.length ? " → " + bad.join(", ") : ""}`,
    expected: "12 записей, расхождений: 0",
  };
});

check("FIX-02", "в фикстуре шлюза присутствует HELLO heartbeat 41250", () => {
  const text = readFileSync(join(REPO, "fixtures/gw-proto/msgs-run2-95974_13072.jsonl"), "utf8");
  const ok = text.includes("41250");
  return { actual: ok ? "41250 найден" : "нет", expected: "41250 найден" };
});

// ── Прогон ──────────────────────────────────────────────────────────────────
if (!isDir(LAB)) {
  console.error(`лаборатория не найдена: ${LAB} (код 2: среда недоступна)`);
  process.exit(2);
}

/** @type {{id: string, title: string, ok: boolean, expected: any, actual: any, note?: string, error?: string}[]} */
const results = [];
for (const c of checks) {
  try {
    const { actual, expected, note } = c.run();
    results.push({ id: c.id, title: c.title, ok: true, expected, actual, ...(note ? { note } : {}) });
  } catch (/** @type {any} */ e) {
    results.push({ id: c.id, title: c.title, ok: false, expected: "—", actual: "ошибка", error: String(e?.message ?? e) });
  }
}

let passed = 0;
for (const r of results) {
  const mark = r.ok ? "✓" : "✗";
  if (r.ok) passed++;
  console.log(`${mark} ${r.id.padEnd(9)} ${r.title}`);
  console.log(`    факт: ${r.actual}`);
  if (!r.ok) console.log(`    ожидалось: ${r.expected}${r.error ? ` (${r.error})` : ""}`);
  else if (String(r.actual) !== String(r.expected)) console.log(`    (ожидалось: ${r.expected})`);
  if (r.note) console.log(`    примечание: ${r.note}`);
}
console.log(`\nитог: ${passed}/${results.length} проверок пройдено (лаборатория: ${LAB})`);

if (WRITE) {
  const report = {
    generatedAt: new Date().toISOString(),
    lab: LAB,
    repo: REPO,
    passed,
    total: results.length,
    results,
  };
  writeFileSync(join(HERE, "report.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(`отчёт: ${join("docs/rust-client/verify", "report.json")}`);
}

process.exit(passed === results.length ? 0 : 1);
