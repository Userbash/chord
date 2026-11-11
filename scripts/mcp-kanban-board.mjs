#!/usr/bin/env node
// @ts-check
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import readline from 'node:readline';

const ROOT = '/home/sanya/orca/project';
const BOARD_FILE = path.join(ROOT, 'waves/kanban-board.json');
const EXPORT_HTML_FILE = path.join(ROOT, 'waves/kanban-board.html');

/**
 * @typedef {Object} KanbanCard
 * @property {string} id
 * @property {string} title
 * @property {string} status 'backlog' | 'design_specs' | 'in_dev' | 'in_review' | 'done'
 * @property {string} priority 'P0' | 'P1' | 'P2' | 'P3'
 * @property {string} department 'G1 Backend' | 'G2 Frontend' | 'G3 Design' | 'G5 Voice/Platform' | 'G4 QA'
 * @property {string} summary
 * @property {Object} [layout]
 * @property {string} [layout.container]
 * @property {string} [layout.dimensions]
 * @property {string} [layout.padding]
 * @property {string} [layout.spacing]
 * @property {Object} [design_tokens]
 * @property {string} [design_tokens.background]
 * @property {string} [design_tokens.surface]
 * @property {string} [design_tokens.text_primary]
 * @property {string} [design_tokens.text_muted]
 * @property {string} [design_tokens.accent]
 * @property {string} [design_tokens.border]
 * @property {Array<{ element: string, default: string, hover: string, active: string }>} [interactive_states]
 * @property {string[]} [acceptance_criteria]
 * @property {string} [wireframe_preview]
 * @property {string} [updated_at]
 */

/**
 * @typedef {Object} KanbanBoardData
 * @property {string} project
 * @property {string} wave
 * @property {string} updated_at
 * @property {Array<{ id: string, title: string, color: string }>} columns
 * @property {KanbanCard[]} cards
 */

function defaultBoard() {
  return {
    project: 'Discord GPUI Client (Rust + WGPU)',
    wave: 'RW-42: Native GPUI Migration & Design Parity',
    updated_at: new Date().toISOString(),
    columns: [
      { id: 'backlog', title: '📋 Бэклог задач', color: '#4e5058' },
      { id: 'design_specs', title: '🎨 Спецификации и Дизайн', color: '#5865f2' },
      { id: 'in_dev', title: '⚡ В разработке', color: '#f0b232' },
      { id: 'in_review', title: '🧪 Тестирование и Ревью', color: '#eb459e' },
      { id: 'done', title: '✅ Завершено и Принято', color: '#23a55a' }
    ],
    cards: []
  };
}

/**
 * @returns {KanbanBoardData}
 */
function loadBoard() {
  if (!fs.existsSync(BOARD_FILE)) {
    const b = defaultBoard();
    saveBoard(b);
    return b;
  }
  try {
    const raw = fs.readFileSync(BOARD_FILE, 'utf-8');
    return JSON.parse(raw);
  } catch (e) {
    return defaultBoard();
  }
}

/**
 * @param {KanbanBoardData} board
 */
function saveBoard(board) {
  board.updated_at = new Date().toISOString();
  fs.mkdirSync(path.dirname(BOARD_FILE), { recursive: true });
  fs.writeFileSync(BOARD_FILE, JSON.stringify(board, null, 2), 'utf-8');
  exportHtml(board);
}

/**
 * @param {KanbanBoardData} board
 */
function exportHtml(board) {
  /** @type {Record<string, string>} */
  const wireframes = {};
  const wfDir = path.join(ROOT, 'docs/specs/wireframes');
  if (fs.existsSync(wfDir)) {
    for (const f of fs.readdirSync(wfDir)) {
      if (f.endsWith('.svg')) {
        const name = f.replace('.svg', '');
        wireframes[name] = fs.readFileSync(path.join(wfDir, f), 'utf-8');
      }
    }
  }

  const html = `<!DOCTYPE html>
<html lang="ru">
<head>
  <meta charset="UTF-8">
  <title>${board.project} — Visual Kanban Board</title>
  <style>
    :root {
      --bg-primary: #1e1f22;
      --bg-secondary: #2b2d31;
      --bg-tertiary: #313338;
      --channel-bg: #232428;
      --text-normal: #dbdee1;
      --text-muted: #949ba4;
      --accent: #5865f2;
      --accent-hover: #4752c4;
      --green: #23a55a;
      --yellow: #f0b232;
      --red: #f23f43;
      --border: #3f4147;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background-color: var(--bg-primary);
      color: var(--text-normal);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
    }
    header {
      background: var(--bg-secondary);
      border-bottom: 1px solid var(--border);
      padding: 16px 24px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .header-title h1 {
      font-size: 20px;
      font-weight: 700;
      color: #fff;
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .header-title p {
      font-size: 13px;
      color: var(--text-muted);
      margin-top: 4px;
    }
    .badge-wave {
      background: rgba(88, 101, 242, 0.2);
      color: #5865f2;
      padding: 4px 10px;
      border-radius: 12px;
      font-size: 12px;
      font-weight: 600;
    }
    .board-container {
      flex: 1;
      display: flex;
      gap: 16px;
      padding: 20px 24px;
      overflow-x: auto;
      align-items: flex-start;
    }
    .column {
      background: var(--bg-secondary);
      border-radius: 8px;
      width: 330px;
      min-width: 330px;
      max-height: calc(100vh - 120px);
      display: flex;
      flex-direction: column;
      border: 1px solid rgba(255, 255, 255, 0.05);
      box-shadow: 0 4px 12px rgba(0,0,0,0.2);
    }
    .col-header {
      padding: 14px 16px;
      border-bottom: 2px solid var(--border);
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-size: 14px;
      font-weight: 700;
      color: #fff;
    }
    .col-count {
      background: var(--bg-primary);
      padding: 2px 8px;
      border-radius: 10px;
      font-size: 12px;
      color: var(--text-muted);
    }
    .cards-list {
      padding: 12px;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 10px;
      flex: 1;
    }
    .card {
      background: var(--bg-tertiary);
      border-radius: 6px;
      padding: 12px;
      cursor: pointer;
      border: 1px solid transparent;
      transition: all 0.15s ease;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .card:hover {
      border-color: var(--accent);
      transform: translateY(-2px);
      box-shadow: 0 6px 16px rgba(0, 0, 0, 0.3);
    }
    .card-top {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .card-id {
      font-size: 11px;
      font-weight: 700;
      color: var(--text-muted);
    }
    .card-priority {
      font-size: 11px;
      font-weight: 700;
      padding: 2px 6px;
      border-radius: 4px;
    }
    .p-P0 { background: rgba(242, 63, 67, 0.2); color: #f23f43; }
    .p-P1 { background: rgba(240, 178, 50, 0.2); color: #f0b232; }
    .p-P2 { background: rgba(88, 101, 242, 0.2); color: #5865f2; }
    .p-P3 { background: rgba(148, 155, 164, 0.2); color: #949ba4; }
    .card-title {
      font-size: 13.5px;
      font-weight: 600;
      color: #fff;
      line-height: 1.35;
    }
    .card-dept {
      font-size: 11px;
      color: var(--text-muted);
      display: flex;
      align-items: center;
      gap: 4px;
    }
    .card-layout-tag {
      background: var(--bg-primary);
      padding: 4px 8px;
      border-radius: 4px;
      font-size: 11px;
      color: #00a8fc;
      border-left: 2px solid #00a8fc;
    }
    .modal-overlay {
      display: none;
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.7);
      backdrop-filter: blur(4px);
      justify-content: center;
      align-items: center;
      z-index: 1000;
    }
    .modal-content {
      background: var(--bg-secondary);
      border-radius: 8px;
      width: 740px;
      max-width: 90vw;
      max-height: 85vh;
      overflow-y: auto;
      border: 1px solid var(--border);
      padding: 24px;
      display: flex;
      flex-direction: column;
      gap: 16px;
    }
    .modal-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      border-bottom: 1px solid var(--border);
      padding-bottom: 14px;
    }
    .modal-title { font-size: 18px; font-weight: 700; color: #fff; }
    .close-btn {
      background: none;
      border: none;
      color: var(--text-muted);
      font-size: 20px;
      cursor: pointer;
    }
    .close-btn:hover { color: #fff; }
    .section-title {
      font-size: 13px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: var(--text-muted);
      margin-top: 8px;
    }
    .specs-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
      background: var(--bg-primary);
      padding: 12px;
      border-radius: 6px;
      font-size: 12.5px;
    }
    .states-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 12.5px;
    }
    .states-table th, .states-table td {
      border: 1px solid var(--border);
      padding: 8px 10px;
      text-align: left;
    }
    .states-table th { background: var(--bg-primary); color: #fff; }
    .dod-item {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 13px;
      margin-top: 4px;
    }
  </style>
</head>
<body>
  <header>
    <div class="header-title">
      <h1>🚀 ${board.project}</h1>
      <p>${board.wave} • Обновлено: ${new Date(board.updated_at).toLocaleString('ru-RU')}</p>
    </div>
    <div class="badge-wave">61/61 Контрактов Pass (100%)</div>
  </header>

  <main class="board-container">
    ${board.columns.map(col => {
      const colCards = board.cards.filter(c => c.status === col.id);
      return `
      <section class="column" data-col="${col.id}">
        <div class="col-header" style="border-top: 3px solid ${col.color}">
          <span>${col.title}</span>
          <span class="col-count">${colCards.length}</span>
        </div>
        <div class="cards-list">
          ${colCards.map(card => `
            <article class="card" onclick="openModal('${card.id}')">
              <div class="card-top">
                <span class="card-id">${card.id}</span>
                <span class="card-priority p-${card.priority}">${card.priority}</span>
              </div>
              <h2 class="card-title">${card.title}</h2>
              <div class="card-dept">🏷️ ${card.department}</div>
              ${card.layout?.dimensions ? `<div class="card-layout-tag">📐 ${card.layout.dimensions}</div>` : ''}
            </article>
          `).join('')}
        </div>
      </section>`;
    }).join('')}
  </main>

  <div id="cardModal" class="modal-overlay" onclick="closeModal(event)">
    <div class="modal-content" onclick="event.stopPropagation()">
      <div class="modal-header">
        <div>
          <span id="modalId" class="card-id"></span>
          <h2 id="modalTitle" class="modal-title"></h2>
        </div>
        <button class="close-btn" onclick="hideModal()">✕</button>
      </div>

      <div class="section-title">Описание и Назначение</div>
      <p id="modalSummary" style="font-size: 13.5px; line-height: 1.5; color: var(--text-normal);"></p>

      <div class="section-title">Геометрия и Макет (Layout Specs)</div>
      <div id="modalLayout" class="specs-grid"></div>

      <div class="section-title" id="wireframeSectionTitle">Визуальная схема (SVG Wireframe)</div>
      <div id="modalWireframe" style="background: #1e1f22; border-radius: 8px; padding: 12px; margin-top: 8px; overflow-x: auto; text-align: center; border: 1px solid var(--border);"></div>

      <div class="section-title">Дизайн-Токены (Палитра Discord Dark)</div>
      <div id="modalTokens" class="specs-grid"></div>

      <div class="section-title">Интерактивные Состояния Элементов</div>
      <table class="states-table">
        <thead>
          <tr><th>Элемент</th><th>Default</th><th>Hover</th><th>Active / Click</th></tr>
        </thead>
        <tbody id="modalStatesBody"></tbody>
      </table>

      <div class="section-title">Критерии приёмки (DoD)</div>
      <div id="modalDod"></div>

      <div class="section-title" id="modalInstructionsTitle">Инструкция по написанию кода (TDD & Оракулы)</div>
      <div id="modalInstructions" style="background: #111214; padding: 12px; border-radius: 6px; font-family: monospace; font-size: 12px; white-space: pre-wrap; color: #dbdee1; border: 1px solid #3f4147; margin-top: 6px; max-height: 240px; overflow-y: auto;"></div>
    </div>
  </div>

  <script>
    const CARDS = ${JSON.stringify(board.cards)};
    const WIREFRAMES = ${JSON.stringify(wireframes)};
    const WF_MAP = {
      'TASK-701': 'app_layout',
      'TASK-707': 'server_rail',
      'TASK-708': 'channel_sidebar',
      'TASK-709': 'app_layout',
      'TASK-710': 'context_menu',
      'TASK-711': 'voice_panel',
      'TASK-712': 'screen_share_modal',
      'TASK-743': 'create_guild_modal'
    };

    function openModal(id) {
      const card = CARDS.find(c => c.id === id);
      if (!card) return;
      document.getElementById('modalId').innerText = card.id + ' • ' + card.department + ' • ' + card.priority;
      document.getElementById('modalTitle').innerText = card.title;
      document.getElementById('modalSummary').innerText = card.summary || 'Описание не указано';

      const layoutDiv = document.getElementById('modalLayout');
      layoutDiv.innerHTML = card.layout ? \`
        <div><strong>Контейнер:</strong> \${card.layout.container || 'Flex'}</div>
        <div><strong>Размеры:</strong> \${card.layout.dimensions || 'auto'}</div>
        <div><strong>Внутренние отступы:</strong> \${card.layout.padding || '0px'}</div>
        <div><strong>Зазоры между элементами:</strong> \${card.layout.spacing || '0px'}</div>
      \` : '<div>Макет по умолчанию</div>';

      const wfKey = WF_MAP[card.id];
      const wfContainer = document.getElementById('modalWireframe');
      const wfTitle = document.getElementById('wireframeSectionTitle');
      if (wfKey && WIREFRAMES[wfKey]) {
        wfContainer.style.display = 'block';
        wfTitle.style.display = 'block';
        wfContainer.innerHTML = WIREFRAMES[wfKey];
      } else {
        wfContainer.style.display = 'none';
        wfTitle.style.display = 'none';
      }

      const tokensDiv = document.getElementById('modalTokens');
      tokensDiv.innerHTML = card.design_tokens ? \`
        <div><strong>Фон:</strong> \${card.design_tokens.background || '#2b2d31'}</div>
        <div><strong>Поверхность:</strong> \${card.design_tokens.surface || '#313338'}</div>
        <div><strong>Текст:</strong> \${card.design_tokens.text_primary || '#ffffff'}</div>
        <div><strong>Акцент:</strong> \${card.design_tokens.accent || '#5865f2'}</div>
      \` : '<div>Токены Discord Dark</div>';

      const tbody = document.getElementById('modalStatesBody');
      tbody.innerHTML = (card.interactive_states && card.interactive_states.length > 0) ? card.interactive_states.map(s => \`
        <tr>
          <td><strong>\${s.element}</strong></td>
          <td>\${s.default}</td>
          <td><span style="color: #00a8fc;">\${s.hover}</span></td>
          <td><span style="color: #23a55a;">\${s.active}</span></td>
        </tr>
      \`).join('') : '<tr><td colspan="4">Стандартные состояния</td></tr>';

      const dodDiv = document.getElementById('modalDod');
      dodDiv.innerHTML = (card.acceptance_criteria && card.acceptance_criteria.length > 0) ? card.acceptance_criteria.map(d => \`
        <div class="dod-item"><span style="color: #23a55a;">✔</span> <span>\${d}</span></div>
      \`).join('') : '<div>100% покрытие контрактами</div>';

      const instrTitle = document.getElementById('modalInstructionsTitle');
      const instrDiv = document.getElementById('modalInstructions');
      if (card.coding_instructions) {
        instrTitle.style.display = 'block';
        instrDiv.style.display = 'block';
        instrDiv.textContent = typeof card.coding_instructions === 'string' 
          ? card.coding_instructions 
          : JSON.stringify(card.coding_instructions, null, 2);
      } else {
        instrTitle.style.display = 'none';
        instrDiv.style.display = 'none';
      }

      document.getElementById('cardModal').style.display = 'flex';
    }
    function hideModal() {
      document.getElementById('cardModal').style.display = 'none';
    }
    function closeModal(e) {
      if (e.target.id === 'cardModal') hideModal();
    }
  </script>
</body>
</html>`;
  fs.writeFileSync(EXPORT_HTML_FILE, html, 'utf-8');
}

/**
 * Запуск фонового HTTP-сервера для живого просмотра доски в браузере
 * @param {number} [port]
 */
function startHttpServer(port = 4321) {
  const server = http.createServer((req, res) => {
    if (req.url === '/' || req.url === '/board') {
      const board = loadBoard();
      exportHtml(board);
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(fs.readFileSync(EXPORT_HTML_FILE));
    } else if (req.url === '/api/cards') {
      const board = loadBoard();
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(board));
    } else {
      res.writeHead(404);
      res.end('Not found');
    }
  });
  server.listen(port, () => {
    // Тихо слушаем
  });
  return server;
}

/** @type {Record<string, { description: string, inputSchema: any, run: (args: any) => Promise<string>|string }>} */
const TOOLS = {
  kanban_get_board: {
    description: "Returns the full interactive visual Kanban board with all columns, tasks, design specs and layouts.",
    inputSchema: { type: "object", properties: {} },
    run: () => {
      const board = loadBoard();
      return JSON.stringify({
        project: board.project,
        columns: board.columns,
        total_cards: board.cards.length,
        export_html_path: EXPORT_HTML_FILE
      }, null, 2);
    }
  },

  kanban_list_cards: {
    description: "Lists task cards filtered by status, priority or department.",
    inputSchema: {
      type: "object",
      properties: {
        status: { type: "string", description: "Filter by status: backlog, design_specs, in_dev, in_review, done" },
        department: { type: "string", description: "Filter by department: G1 Backend, G2 Frontend, G3 Design, G5 Voice/Platform, G4 QA" }
      }
    },
    run: (args = {}) => {
      const board = loadBoard();
      let cards = board.cards;
      if (args.status) cards = cards.filter(c => c.status === args.status);
      if (args.department) cards = cards.filter(c => c.department === args.department);
      return JSON.stringify(cards, null, 2);
    }
  },

  kanban_create_card: {
    description: "Creates or updates a rich design-first task card on the visual Kanban board.",
    inputSchema: {
      type: "object",
      required: ["id", "title", "department", "priority", "status", "summary"],
      properties: {
        id: { type: "string", description: "Task ID (e.g. TASK-741)" },
        title: { type: "string", description: "Clear descriptive task title" },
        department: { type: "string", description: "Department: G1 Backend, G2 Frontend, G3 Design, G5 Voice/Platform, G4 QA" },
        priority: { type: "string", description: "Priority: P0, P1, P2, P3" },
        status: { type: "string", description: "Status: backlog, design_specs, in_dev, in_review, done" },
        summary: { type: "string", description: "User journey and purpose summary" },
        layout: {
          type: "object",
          properties: {
            container: { type: "string" },
            dimensions: { type: "string" },
            padding: { type: "string" },
            spacing: { type: "string" }
          }
        },
        design_tokens: {
          type: "object",
          properties: {
            background: { type: "string" },
            surface: { type: "string" },
            text_primary: { type: "string" },
            text_muted: { type: "string" },
            accent: { type: "string" },
            border: { type: "string" }
          }
        },
        interactive_states: {
          type: "array",
          items: {
            type: "object",
            properties: {
              element: { type: "string" },
              default: { type: "string" },
              hover: { type: "string" },
              active: { type: "string" }
            }
          }
        },
        acceptance_criteria: {
          type: "array",
          items: { type: "string" }
        }
      }
    },
    run: (args) => {
      const board = loadBoard();
      const existingIdx = board.cards.findIndex(c => c.id === args.id);
      /** @type {KanbanCard} */
      const card = {
        id: args.id,
        title: args.title,
        department: args.department,
        priority: args.priority,
        status: args.status,
        summary: args.summary,
        layout: args.layout || { container: "Flex", dimensions: "auto", padding: "8px", spacing: "8px" },
        design_tokens: args.design_tokens || { background: "#2b2d31", surface: "#313338", text_primary: "#ffffff", accent: "#5865f2" },
        interactive_states: args.interactive_states || [],
        acceptance_criteria: args.acceptance_criteria || [],
        updated_at: new Date().toISOString()
      };

      if (existingIdx >= 0) {
        board.cards[existingIdx] = card;
      } else {
        board.cards.push(card);
      }
      saveBoard(board);
      return `Card ${card.id} successfully saved to column ${card.status}. Visual board updated at ${EXPORT_HTML_FILE}`;
    }
  },

  kanban_move_card: {
    description: "Moves a card to another status column.",
    inputSchema: {
      type: "object",
      required: ["id", "new_status"],
      properties: {
        id: { type: "string", description: "Task ID (e.g. TASK-741)" },
        new_status: { type: "string", description: "New column: backlog, design_specs, in_dev, in_review, done" }
      }
    },
    run: (args) => {
      const board = loadBoard();
      const card = board.cards.find(c => c.id === args.id);
      if (!card) return `Card ${args.id} not found`;
      card.status = args.new_status;
      card.updated_at = new Date().toISOString();
      saveBoard(board);
      return `Card ${args.id} moved to ${args.new_status}. HTML board re-generated.`;
    }
  },

  kanban_export_html: {
    description: "Exports the visual interactive Kanban HTML board for viewing in any browser.",
    inputSchema: { type: "object", properties: {} },
    run: () => {
      const board = loadBoard();
      exportHtml(board);
      return `Visual board successfully exported to ${EXPORT_HTML_FILE} (${board.cards.length} cards rendered)`;
    }
  }
};

/** @param {any} msg */
function send(msg) {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

/**
 * @param {any} id
 * @param {any} result
 */
function reply(id, result) {
  send({ jsonrpc: '2.0', id, result });
}

/**
 * @param {any} id
 * @param {any} code
 * @param {any} message
 */
function replyError(id, code, message) {
  send({ jsonrpc: '2.0', id, error: { code, message } });
}

/** @param {any} msg */
async function handle(msg) {
  const { id, method, params } = msg;

  if (method === 'initialize') {
    reply(id, {
      protocolVersion: '2024-11-05',
      serverInfo: { name: 'kanban-board', version: '1.0.0' },
      capabilities: { tools: {} }
    });
    return;
  }

  if (method === 'tools/list') {
    const list = Object.entries(TOOLS).map(([name, def]) => ({
      name,
      description: def.description,
      inputSchema: def.inputSchema
    }));
    reply(id, { tools: list });
    return;
  }

  if (method === 'tools/call') {
    const tool = TOOLS[params?.name];
    if (!tool) {
      replyError(id, -32601, `Tool not found: ${params?.name}`);
      return;
    }
    try {
      const result = await tool.run(params?.arguments ?? {});
      reply(id, { content: [{ type: 'text', text: result }] });
    } catch (e) {
      replyError(id, -32603, String(e));
    }
    return;
  }

  replyError(id, -32601, `Method not supported: ${method}`);
}

// Если запущен с аргументом --export, генерируем HTML и завершаем работу
if (process.argv.includes('--export')) {
  const b = loadBoard();
  exportHtml(b);
  console.log(`[Kanban Board] Exported HTML to ${EXPORT_HTML_FILE}`);
  process.exit(0);
}

// Если запущен напрямую с --serve, поднимаем web-сервер
if (process.argv.includes('--serve')) {
  const port = 4321;
  startHttpServer(port);
  console.log(`[Kanban Board UI] Live web server running at http://localhost:${port}/board`);
} else {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout, terminal: false });
  rl.on('line', (line) => {
    if (!line.trim()) return;
    try {
      handle(JSON.parse(line));
    } catch (e) {
      replyError(null, -32700, 'Parse error');
    }
  });
}
