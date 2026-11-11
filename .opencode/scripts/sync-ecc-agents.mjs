#!/usr/bin/env node
// @ts-check
/**
 * Синхронизация курируемого набора ECC-агентов в формат OpenCode.
 *
 * Источник: /home/sanya/orca/projects/ECC/agents/*.md (Claude Code: tools = "Read, Grep, ...").
 * Приёмник: /home/sanya/orca/project/.opencode/agent/*.md (OpenCode: tools = объект с boolean).
 *
 * Запуск:  node .opencode/scripts/sync-ecc-agents.mjs [--check]
 *   --check — только отчёт, без записи файлов.
 *
 * Правки руками в сгенерированные файлы не выживают: правьте список AGENTS ниже
 * или тело агента в репозитории ECC.
 */
import fs from 'node:fs';
import path from 'node:path';

const ECC_AGENTS = '/home/sanya/orca/projects/ECC/agents';
const OUT_DIR = path.resolve(import.meta.dirname, '..', 'agent');

/** Курируемый набор под стек discord-lab: Rust core + Tauri BFF, WebView UI, QA. */
const AGENTS = [
  'planner',
  'architect',
  'code-reviewer',
  'rust-reviewer',
  'rust-build-resolver',
  'security-reviewer',
  'tdd-guide',
  'e2e-runner',
  'pr-test-analyzer',
  'silent-failure-hunter',
  'refactor-cleaner',
  'performance-optimizer',
  'docs-lookup',
  'doc-updater',
];

/** Claude tool name -> OpenCode tool id. */
const TOOL_MAP = {
  Read: 'read',
  Grep: 'grep',
  Glob: 'glob',
  Bash: 'bash',
  Write: 'write',
  Edit: 'edit',
  MultiEdit: 'edit',
  NotebookEdit: 'edit',
  WebFetch: 'webfetch',
  WebSearch: 'websearch',
  Task: 'task',
  TodoWrite: 'todowrite',
};

/** Инструменты OpenCode, которые явно выключаем, если агент их не запросил. */
const ALL_TOOLS = ['read', 'grep', 'glob', 'list', 'bash', 'write', 'edit', 'patch', 'webfetch', 'websearch', 'task', 'todowrite'];

/**
 * `docs-lookup` в ECC опирается на MCP context7, которого в этом окружении нет:
 * подменяем его на webfetch/websearch, иначе агент бесполезен.
 */
const TOOL_OVERRIDES = {
  'docs-lookup': ['read', 'grep', 'webfetch', 'websearch'],
};

/**
 * @param {string} raw
 * @returns {{meta: Record<string, string>, body: string}}
 */
function splitFrontmatter(raw) {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) throw new Error('нет YAML-frontmatter');
  /** @type {Record<string, string>} */
  const meta = {};
  for (const line of match[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (kv) meta[kv[1]] = kv[2].trim();
  }
  return { meta, body: raw.slice(match[0].length) };
}

/**
 * @param {string} name
 * @returns {{name: string, toolIds: string[]}|null}
 */
function convert(name) {
  const src = path.join(ECC_AGENTS, `${name}.md`);
  if (!fs.existsSync(src)) {
    console.error(`НЕТ ИСХОДНИКА: ${src}`);
    return null;
  }
  const { meta, body } = splitFrontmatter(fs.readFileSync(src, 'utf8'));

  const declared = (meta.tools ?? '')
    .split(',')
    .map((t) => t.trim())
    .filter(Boolean)
    .map((t) => TOOL_MAP[t] ?? (t.startsWith('mcp__') ? t : null))
    .filter((t) => typeof t === 'string');

  const toolIds = TOOL_OVERRIDES[name] ?? declared;
  if (toolIds.length === 0) throw new Error(`${name}: не удалось разобрать tools="${meta.tools}"`);

  // `list` — только чтение каталогов, оставляем всем агентам: в Claude-версиях он
  // неявно доступен через Glob/Bash, а его запрет ломает обзор дерева проекта.
  const tools = ALL_TOOLS.map((t) => `  ${t}: ${t === 'list' || toolIds.includes(t)}`).join('\n');

  const preamble = [
    `> ECC-агент \`${name}\`, адаптирован под OpenCode и проект discord-lab.`,
    '> Правила проекта в `AGENTS.md`, `PROCESS.md`, `TEST-POLICY.md` приоритетнее этого промпта.',
    '',
  ].join('\n');

  const out = [
    '---',
    `description: ${meta.description ?? name}`,
    'mode: subagent',
    'tools:',
    tools,
    '---',
    '',
    preamble,
    body.trimStart(),
  ].join('\n');

  return { name, toolIds, content: out };
}

const checkOnly = process.argv.includes('--check');
fs.mkdirSync(OUT_DIR, { recursive: true });

let written = 0;
for (const name of AGENTS) {
  const needOverride = name in TOOL_OVERRIDES;
  let result;
  try {
    result = convert(name);
  } catch (error) {
    console.error(`ОШИБКА ${name}: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
    continue;
  }
  if (!result) {
    process.exitCode = 1;
    continue;
  }
  const dest = path.join(OUT_DIR, `${name}.md`);
  const note = needOverride ? ' [tools переопределены]' : '';
  if (checkOnly) {
    const same = fs.existsSync(dest) && fs.readFileSync(dest, 'utf8') === result.content;
    console.log(`${same ? 'OK  ' : 'DIFF'} ${name}${note}`);
    continue;
  }
  fs.writeFileSync(dest, result.content);
  written += 1;
  console.log(`записан ${path.relative(process.cwd(), dest)}${note}`);
}

if (!checkOnly) console.log(`\nвсего агентов: ${AGENTS.length}, записано: ${written}`);
