#!/usr/bin/env node
// Structural analysis for the architecture-analyzer (Understand-Anything).
// Usage: node ua-arch-analyze.js <input.json> <output.json>
'use strict';

const fs = require('node:fs');
const path = require('node:path');

function fail(msg) {
  process.stderr.write(String(msg) + '\n');
  process.exit(1);
}

const [, , inPath, outPath] = process.argv;
if (!inPath || !outPath) fail('usage: node ua-arch-analyze.js <input.json> <output.json>');

let data;
try {
  data = JSON.parse(fs.readFileSync(inPath, 'utf8'));
} catch (e) {
  fail('cannot read input: ' + e.message);
}

const fileNodes = Array.isArray(data.fileNodes) ? data.fileNodes : [];
const importEdges = Array.isArray(data.importEdges) ? data.importEdges : [];
const allEdges = Array.isArray(data.allEdges) ? data.allEdges : [];

const byId = new Map(fileNodes.map((n) => [n.id, n]));

/* ---------- A. directory grouping ---------- */

function commonPrefix(paths) {
  if (paths.length === 0) return '';
  const split = paths.map((p) => p.split('/'));
  const first = split[0];
  let len = 0;
  outer: for (let i = 0; i < first.length - 1; i += 1) {
    for (const parts of split) {
      if (parts[i] !== first[i]) break outer;
    }
    len = i + 1;
  }
  return len ? first.slice(0, len).join('/') + '/' : '';
}

const prefix = commonPrefix(fileNodes.map((n) => n.filePath));

function groupKey(filePath) {
  const rel = prefix && filePath.startsWith(prefix) ? filePath.slice(prefix.length) : filePath;
  const parts = rel.split('/');
  if (parts.length === 1) return 'root';
  // Monorepo-aware grouping: crate as the unit for rust/, package dir for docs,
  // toolchain dir for .github.
  if (parts[0] === 'rust' && parts[1] === 'crates' && parts[2]) {
    return `rust/crates/${parts[2]}`;
  }
  if (parts[0] === 'docs' && parts[1] === 'rust-client' && parts[2] === 'verify') {
    return 'docs/rust-client/verify';
  }
  if (parts[0] === 'docs' && parts[1] === 'rust-client') return 'docs/rust-client';
  if (parts[0] === 'docs') return 'docs';
  if (parts[0] === '.github') return '.github';
  return parts[0];
}

const directoryGroups = {};
for (const n of fileNodes) {
  const g = groupKey(n.filePath);
  (directoryGroups[g] ||= []).push(n.id);
}

/* ---------- B. node type grouping ---------- */

const nodeTypeGroups = {};
for (const n of fileNodes) {
  (nodeTypeGroups[n.type] ||= []).push(n.id);
}

/* ---------- C. import adjacency ---------- */

const fanOut = {};
const fanIn = {};
for (const id of byId.keys()) {
  fanOut[id] = 0;
  fanIn[id] = 0;
}
for (const e of importEdges) {
  if (!byId.has(e.source) || !byId.has(e.target)) continue;
  fanOut[e.source] += 1;
  fanIn[e.target] += 1;
}

/* ---------- D. cross-category edges ---------- */

const crossMap = new Map();
for (const e of allEdges) {
  const s = byId.get(e.source);
  const t = byId.get(e.target);
  if (!s || !t) continue;
  const key = `${s.type}->${t.type}:${e.type}`;
  crossMap.set(key, (crossMap.get(key) || 0) + 1);
}
const crossCategoryEdges = [...crossMap.entries()]
  .map(([k, count]) => {
    const [types, edgeType] = k.split(':');
    const [fromType, toType] = types.split('->');
    return { fromType, toType, edgeType, count };
  })
  .sort((a, b) => b.count - a.count);

/* ---------- E/F. inter-group imports, intra-group density ---------- */

const groupOf = new Map();
for (const [g, ids] of Object.entries(directoryGroups)) {
  for (const id of ids) groupOf.set(id, g);
}

const interMap = new Map();
const intra = {};
for (const [g, ids] of Object.entries(directoryGroups)) {
  intra[g] = { internalEdges: 0, totalEdges: 0 };
}
for (const e of importEdges) {
  const gs = groupOf.get(e.source);
  const gt = groupOf.get(e.target);
  if (!gs || !gt) continue;
  intra[gs].totalEdges += 1;
  if (gt !== gs) intra[gt].totalEdges += 1;
  if (gs === gt) {
    intra[gs].internalEdges += 1;
  } else {
    const key = `${gs}=>${gt}`;
    interMap.set(key, (interMap.get(key) || 0) + 1);
  }
}
const interGroupImports = [...interMap.entries()]
  .map(([k, count]) => {
    const [from, to] = k.split('=>');
    return { from, to, count };
  })
  .sort((a, b) => b.count - a.count);

const intraGroupDensity = {};
for (const [g, v] of Object.entries(intra)) {
  intraGroupDensity[g] = {
    internalEdges: v.internalEdges,
    totalEdges: v.totalEdges,
    density: v.totalEdges ? +(v.internalEdges / v.totalEdges).toFixed(3) : 0,
  };
}

/* ---------- G. pattern matching ---------- */

const patterns = [
  [/^(routes|api|controllers|endpoints|handlers)$/, 'api'],
  [/^(services|core|lib|domain|logic)$/, 'service'],
  [/^(models|db|data|persistence|repository|entities)$/, 'data'],
  [/^(components|views|pages|ui|layouts|screens)$/, 'ui'],
  [/^(middleware|plugins|interceptors|guards)$/, 'middleware'],
  [/^(utils|helpers|common|shared|tools)$/, 'utility'],
  [/^(config|constants|env|settings)$/, 'config'],
  [/^(__tests__|test|tests|spec|specs)$/, 'test'],
  [/^(types|interfaces|schemas|contracts|dtos)$/, 'types'],
  [/^hooks$/, 'hooks'],
  [/^(store|state|reducers|actions|slices)$/, 'state'],
  [/^(assets|static|public)$/, 'assets'],
  [/^migrations$/, 'data'],
  [/^(management|commands)$/, 'config'],
  [/^serializers$/, 'api'],
  [/^cmd$/, 'entry'],
  [/^internal$/, 'service'],
  [/^pkg$/, 'utility'],
  [/^docs$/, 'documentation'],
  [/^(deploy|deployment|infra|infrastructure)$/, 'infrastructure'],
  [/^\.github$/, 'ci-cd'],
  [/^(sql|database|schema)$/, 'data'],
];

function matchPattern(name) {
  const base = name.split('/').pop();
  for (const [re, label] of patterns) if (re.test(base)) return label;
  if (/^discord-(core|voice|media|platform|i18n|store)$/.test(base)) return 'service';
  if (/^discord-(codec|conformance)$/.test(base)) return 'service';
  if (base === 'discord-ui') return 'ui';
  if (base === 'discord-app') return 'entry';
  return 'unknown';
}

const patternMatches = {};
for (const g of Object.keys(directoryGroups)) patternMatches[g] = matchPattern(g);

const filePatternMatches = {};
for (const n of fileNodes) {
  const p = n.filePath;
  let label = null;
  if (/(^|\/)(test|tests|__tests__)\//.test(p) || /(\.test\.|\.spec\.|_test\.rs$|Tests?\.rs$)/.test(p)) label = 'test';
  else if (/\.(md|rst)$/.test(p)) label = 'documentation';
  else if (/\.github\/workflows\//.test(p)) label = 'ci-cd';
  else if (/Cargo\.toml$|(^|\/)config\.json$/.test(p)) label = 'config';
  else if (/src\/main\.rs$|src\/lib\.rs$/.test(p)) label = 'entry';
  if (label) filePatternMatches[n.id] = label;
}

/* ---------- H. deployment topology ---------- */

const infraFiles = fileNodes
  .filter((n) => /Dockerfile|docker-compose|\.tf$|\.tfvars$|k8s|helm|Makefile/.test(n.filePath))
  .map((n) => n.filePath);
const ciFiles = fileNodes
  .filter((n) => /\.github\/workflows\//.test(n.filePath))
  .map((n) => n.filePath);
const deploymentTopology = {
  hasDockerfile: infraFiles.some((f) => /Dockerfile/.test(f)),
  hasCompose: infraFiles.some((f) => /docker-compose/.test(f)),
  hasK8s: infraFiles.some((f) => /k8s|helm/.test(f)),
  hasTerraform: infraFiles.some((f) => /\.tf(vars)?$/.test(f)),
  hasCI: ciFiles.length > 0,
  infraFiles: [...infraFiles, ...ciFiles],
};

/* ---------- I. data pipeline ---------- */

const dataPipeline = {
  schemaFiles: fileNodes.filter((n) => /\.(sql|graphql|gql|proto|prisma)$/.test(n.filePath)).map((n) => n.filePath),
  migrationFiles: fileNodes.filter((n) => /migrations?\//.test(n.filePath)).map((n) => n.filePath),
  dataModelFiles: fileNodes
    .filter((n) => /(src\/(ready|ready_data)|store\/src)/.test(n.filePath))
    .map((n) => n.filePath),
  apiHandlerFiles: fileNodes.filter((n) => /src\/rest\//.test(n.filePath)).map((n) => n.filePath),
};

/* ---------- J. documentation coverage ---------- */

const docIds = fileNodes.filter((n) => n.type === 'document').map((n) => n.id);
const docPathByGroup = {};
for (const [g, ids] of Object.entries(directoryGroups)) {
  docPathByGroup[g] = ids.filter((id) => byId.get(id).type === 'document').length;
}
const groupsWithDocs = Object.values(docPathByGroup).filter((c) => c > 0).length;
const totalGroups = Object.keys(directoryGroups).length;
const docCoverage = {
  groupsWithDocs,
  totalGroups,
  coverageRatio: totalGroups ? +(groupsWithDocs / totalGroups).toFixed(2) : 0,
  undocumentedGroups: Object.entries(docPathByGroup)
    .filter(([, c]) => c === 0)
    .map(([g]) => g),
  totalDocumentNodes: docIds.length,
};

/* ---------- K. dependency direction ---------- */

const dependencyDirection = [];
const seenPairs = new Set();
for (const { from, to, count } of interGroupImports) {
  const reverse = interGroupImports.find((x) => x.from === to && x.to === from);
  const forward = count;
  const backward = reverse ? reverse.count : 0;
  if (forward > backward && !seenPairs.has(`${to}->${from}`)) {
    seenPairs.add(`${from}->${to}`);
    dependencyDirection.push({ dependent: from, dependsOn: to, forward, backward });
  }
}
dependencyDirection.sort((a, b) => b.forward - a.forward);

/* ---------- stats & output ---------- */

const filesPerGroup = {};
for (const [g, ids] of Object.entries(directoryGroups)) filesPerGroup[g] = ids.length;
const nodeTypeCounts = {};
for (const [t, ids] of Object.entries(nodeTypeGroups)) nodeTypeCounts[t] = ids.length;

const fanInSorted = Object.fromEntries(Object.entries(fanIn).sort((a, b) => b[1] - a[1]).slice(0, 15));
const fanOutSorted = Object.fromEntries(Object.entries(fanOut).sort((a, b) => b[1] - a[1]).slice(0, 15));

const result = {
  scriptCompleted: true,
  commonPrefix: prefix || null,
  directoryGroups,
  nodeTypeGroups,
  crossCategoryEdges,
  interGroupImports,
  intraGroupDensity,
  patternMatches,
  filePatternMatches,
  deploymentTopology,
  dataPipeline,
  docCoverage,
  dependencyDirection,
  fileStats: {
    totalFileNodes: fileNodes.length,
    filesPerGroup,
    nodeTypeCounts,
  },
  fileFanIn: fanInSorted,
  fileFanOut: fanOutSorted,
};

try {
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, JSON.stringify(result, null, 2));
} catch (e) {
  fail('cannot write output: ' + e.message);
}

process.stdout.write(
  `ok: ${fileNodes.length} nodes, ${Object.keys(directoryGroups).length} groups, ` +
    `${importEdges.length} import edges, ${allEdges.length} total edges\n`
);
process.exit(0);
