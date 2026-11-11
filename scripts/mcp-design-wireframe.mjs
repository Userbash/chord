#!/usr/bin/env node
// @ts-check
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';

const ROOT = '/home/sanya/orca/project';
const WIREFRAME_DIR = path.join(ROOT, 'docs/specs/wireframes');

const DISCORD_DARK_TOKENS = {
  colors: {
    bg_primary: "#1e1f22",
    bg_secondary: "#2b2d31",
    bg_tertiary: "#313338",
    channel_sidebar: "#232428",
    user_panel: "#232428",
    text_normal: "#dbdee1",
    text_muted: "#949ba4",
    text_header: "#ffffff",
    accent_blurple: "#5865f2",
    accent_blurple_hover: "#4752c4",
    status_online: "#23a55a",
    status_idle: "#f0b232",
    status_dnd: "#f23f43",
    status_offline: "#80848e",
    border_subtle: "rgba(255, 255, 255, 0.08)",
    border_strong: "#3f4147",
    card_hover: "rgba(255, 255, 255, 0.04)"
  },
  typography: {
    font_family: "gg sans, Noto Sans, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica, Arial, sans-serif",
    sizes: {
      xs: "11px",
      sm: "12px",
      base: "14px",
      lg: "16px",
      xl: "20px",
      header: "24px"
    },
    weights: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700
    }
  },
  dimensions: {
    server_rail_width: "72px",
    channel_sidebar_width: "240px",
    member_sidebar_width: "240px",
    user_profile_bar_height: "52px",
    voice_panel_height: "52px",
    message_bar_height: "68px",
    context_menu_width: "220px",
    modal_create_guild_width: "440px",
    modal_screen_share_width: "600px",
    modal_settings_width: "740px"
  },
  radii: {
    xs: "3px",
    sm: "4px",
    md: "8px",
    lg: "12px",
    avatar_guild: "16px",
    pill: "9999px"
  }
};

/**
 * Генерация SVG wireframe для компонента
 * @param {string} component
 * @returns {string} SVG code
 */
function generateSvgWireframe(component) {
  switch (component.toLowerCase()) {
    case 'app_layout':
    case 'layout':
      return `<svg width="800" height="450" viewBox="0 0 800 450" xmlns="http://www.w3.org/2000/svg">
  <rect width="800" height="450" fill="#1e1f22" />
  <!-- Server Rail (72px) -->
  <rect x="0" y="0" width="72" height="450" fill="#1e1f22" stroke="#3f4147" stroke-width="1"/>
  <circle cx="36" cy="36" r="24" fill="#5865f2" />
  <text x="36" y="41" fill="#fff" font-size="12" text-anchor="middle" font-family="sans-serif">🎮</text>
  <line x1="16" y1="72" x2="56" y2="72" stroke="#3f4147" stroke-width="2" />
  <circle cx="36" cy="104" r="22" fill="#2b2d31" />
  <text x="36" y="109" fill="#dbdee1" font-size="11" text-anchor="middle" font-family="sans-serif">RD</text>
  <circle cx="36" cy="160" r="22" fill="#23a55a" />
  <text x="36" y="165" fill="#fff" font-size="16" text-anchor="middle" font-family="sans-serif">+</text>
  
  <!-- Channel Sidebar (240px) -->
  <rect x="72" y="0" width="240" height="450" fill="#2b2d31" stroke="#3f4147" stroke-width="1"/>
  <rect x="72" y="0" width="240" height="48" fill="#2b2d31" border-bottom="1px solid #1e1f22"/>
  <text x="88" y="30" fill="#fff" font-size="14" font-weight="bold" font-family="sans-serif">Rust Developers</text>
  <text x="88" y="70" fill="#949ba4" font-size="11" font-weight="bold" font-family="sans-serif">▾ ТЕКСТОВЫЕ КАНАЛЫ</text>
  <rect x="80" y="80" width="224" height="34" rx="4" fill="#35373c"/>
  <text x="94" y="102" fill="#fff" font-size="13" font-family="sans-serif">#  general</text>
  <text x="94" y="136" fill="#949ba4" font-size="13" font-family="sans-serif">#  architecture</text>
  
  <!-- Voice Panel (52px) -->
  <rect x="72" y="346" width="240" height="52" fill="#232428" stroke="#3f4147" stroke-width="1"/>
  <circle cx="88" cy="372" r="5" fill="#23a55a" />
  <text x="100" y="368" fill="#23a55a" font-size="11" font-weight="bold" font-family="sans-serif">Голос подключен</text>
  <text x="100" y="384" fill="#949ba4" font-size="11" font-family="sans-serif">🔊 Voice 1 / 18ms</text>
  <text x="286" y="376" fill="#f23f43" font-size="14" text-anchor="middle" font-family="sans-serif">📞✕</text>

  <!-- User Profile Bar (52px) -->
  <rect x="72" y="398" width="240" height="52" fill="#232428" stroke="#3f4147" stroke-width="1"/>
  <circle cx="94" cy="424" r="16" fill="#5865f2"/>
  <circle cx="106" cy="436" r="5" fill="#23a55a" stroke="#232428" stroke-width="2"/>
  <text x="118" y="420" fill="#fff" font-size="13" font-weight="bold" font-family="sans-serif">Rustery</text>
  <text x="118" y="436" fill="#949ba4" font-size="11" font-family="sans-serif">В сети</text>
  <text x="250" y="428" fill="#dbdee1" font-size="14" font-family="sans-serif">🎙️ 🎧 ⚙️</text>

  <!-- Chat View (flex-1) -->
  <rect x="312" y="0" width="488" height="450" fill="#313338" />
  <rect x="312" y="0" width="488" height="48" fill="#313338" stroke="#3f4147" stroke-width="1"/>
  <text x="330" y="30" fill="#fff" font-size="15" font-weight="bold" font-family="sans-serif"># general</text>
  <text x="410" y="30" fill="#949ba4" font-size="12" font-family="sans-serif">Общий чат сообщества</text>

  <!-- Messages -->
  <circle cx="340" cy="80" r="18" fill="#5865f2"/>
  <text x="368" y="74" fill="#fff" font-size="13" font-weight="bold" font-family="sans-serif">ChiefArchitect</text>
  <text x="470" y="74" fill="#949ba4" font-size="11" font-family="sans-serif">Сегодня в 10:45</text>
  <text x="368" y="96" fill="#dbdee1" font-size="13" font-family="sans-serif">Клиент собран на чистом GPUI 120 FPS без WebKit!</text>

  <!-- Message Bar -->
  <rect x="328" y="394" width="456" height="44" rx="8" fill="#383a40"/>
  <text x="344" y="421" fill="#949ba4" font-size="13" font-family="sans-serif">Написать в #general...</text>
  <text x="750" y="421" fill="#dbdee1" font-size="16" font-family="sans-serif">😊 🎁</text>
</svg>`;

    case 'context_menu':
      return `<svg width="260" height="200" viewBox="0 0 260 200" xmlns="http://www.w3.org/2000/svg">
  <rect x="10" y="10" width="240" height="180" rx="8" fill="#2b2d31" stroke="#3f4147" stroke-width="1" filter="drop-shadow(0 8px 16px rgba(0,0,0,0.4))"/>
  <!-- Item 1: Pin (Hover State) -->
  <rect x="16" y="16" width="228" height="34" rx="4" fill="#5865f2"/>
  <text x="28" y="38" fill="#fff" font-size="14" font-family="sans-serif">📌</text>
  <text x="52" y="38" fill="#fff" font-size="13" font-weight="bold" font-family="sans-serif">Закрепить сообщение</text>
  <!-- Item 2: Reply -->
  <rect x="16" y="52" width="228" height="34" rx="4" fill="transparent"/>
  <text x="28" y="74" fill="#dbdee1" font-size="14" font-family="sans-serif">💬</text>
  <text x="52" y="74" fill="#dbdee1" font-size="13" font-family="sans-serif">Ответить</text>
  <!-- Item 3: Copy -->
  <rect x="16" y="88" width="228" height="34" rx="4" fill="transparent"/>
  <text x="28" y="110" fill="#dbdee1" font-size="14" font-family="sans-serif">📋</text>
  <text x="52" y="110" fill="#dbdee1" font-size="13" font-family="sans-serif">Копировать текст</text>
  <line x1="20" y1="126" x2="240" y2="126" stroke="#3f4147" stroke-width="1"/>
  <!-- Item 4: Delete -->
  <rect x="16" y="132" width="228" height="34" rx="4" fill="transparent"/>
  <text x="28" y="154" fill="#f23f43" font-size="14" font-family="sans-serif">🗑️</text>
  <text x="52" y="154" fill="#f23f43" font-size="13" font-weight="bold" font-family="sans-serif">Удалить сообщение</text>
</svg>`;

    case 'channel_sidebar':
      return `<svg width="240" height="500" viewBox="0 0 240 500" xmlns="http://www.w3.org/2000/svg">
  <rect width="240" height="500" fill="#2b2d31"/>
  <!-- Guild Header Dropdown -->
  <rect x="0" y="0" width="240" height="48" fill="#2b2d31" stroke="#1f2023" stroke-width="1"/>
  <text x="16" y="28" fill="#ffffff" font-size="14" font-weight="bold" font-family="sans-serif">Rust Developers</text>
  <text x="216" y="28" fill="#dbdee1" font-size="14" font-family="sans-serif">▾</text>

  <!-- Category: TEXT CHANNELS -->
  <text x="16" y="74" fill="#949ba4" font-size="11" font-weight="bold" font-family="sans-serif">▾ ТЕКСТОВЫЕ КАНАЛЫ</text>
  <!-- Channel Item: Active #general -->
  <rect x="8" y="86" width="224" height="34" rx="4" fill="#35373c"/>
  <text x="18" y="108" fill="#ffffff" font-size="15" font-family="sans-serif">#</text>
  <text x="36" y="108" fill="#ffffff" font-size="13" font-weight="bold" font-family="sans-serif">general</text>
  <!-- Channel Item: Hover #architecture -->
  <rect x="8" y="124" width="224" height="34" rx="4" fill="rgba(255,255,255,0.04)"/>
  <text x="18" y="146" fill="#949ba4" font-size="15" font-family="sans-serif">#</text>
  <text x="36" y="146" fill="#dbdee1" font-size="13" font-family="sans-serif">architecture</text>
  <!-- Channel Item: Unread #announcements -->
  <rect x="8" y="162" width="224" height="34" rx="4" fill="transparent"/>
  <circle cx="4" cy="179" r="3" fill="#ffffff"/>
  <text x="18" y="184" fill="#949ba4" font-size="15" font-family="sans-serif">#</text>
  <text x="36" y="184" fill="#ffffff" font-size="13" font-weight="bold" font-family="sans-serif">announcements</text>

  <!-- Category: VOICE CHANNELS -->
  <text x="16" y="230" fill="#949ba4" font-size="11" font-weight="bold" font-family="sans-serif">▾ ГОЛОСОВЫЕ КАНАЛЫ</text>
  <!-- Voice Channel: Lounge -->
  <rect x="8" y="242" width="224" height="34" rx="4" fill="transparent"/>
  <text x="18" y="264" fill="#949ba4" font-size="14" font-family="sans-serif">🔊</text>
  <text x="38" y="264" fill="#dbdee1" font-size="13" font-family="sans-serif">Voice 1 (Lounge)</text>
  <!-- Connected Voice Users -->
  <circle cx="36" cy="292" r="10" fill="#5865f2"/>
  <text x="54" y="296" fill="#23a55a" font-size="12" font-weight="bold" font-family="sans-serif">Rustery (Говорит)</text>
  <circle cx="216" cy="292" r="4" fill="#23a55a"/>

  <!-- Voice Control Panel (Connected State) -->
  <rect x="0" y="396" width="240" height="52" fill="#232428" stroke="#1f2023" stroke-width="1"/>
  <circle cx="16" cy="418" r="4" fill="#23a55a"/>
  <text x="26" y="415" fill="#23a55a" font-size="11" font-weight="bold" font-family="sans-serif">RTC Подключено</text>
  <text x="26" y="432" fill="#949ba4" font-size="11" font-family="sans-serif">Voice 1 / 18ms</text>
  <rect x="196" y="408" width="32" height="28" rx="4" fill="rgba(242,63,67,0.15)"/>
  <text x="206" y="427" fill="#f23f43" font-size="14" font-family="sans-serif">✕</text>

  <!-- User Bar -->
  <rect x="0" y="448" width="240" height="52" fill="#232428"/>
  <circle cx="24" cy="474" r="16" fill="#5865f2"/>
  <circle cx="36" cy="486" r="5" fill="#23a55a" stroke="#232428" stroke-width="2"/>
  <text x="48" y="470" fill="#ffffff" font-size="13" font-weight="bold" font-family="sans-serif">Rustery</text>
  <text x="48" y="486" fill="#949ba4" font-size="11" font-family="sans-serif">#0001</text>
  <text x="160" y="478" fill="#b5bac1" font-size="15" font-family="sans-serif">🎙️ 🎧 ⚙️</text>
</svg>`;

    case 'server_rail':
      return `<svg width="72" height="500" viewBox="0 0 72 500" xmlns="http://www.w3.org/2000/svg">
  <rect width="72" height="500" fill="#1e1f22"/>
  <!-- Home/DM Icon (Active) -->
  <rect x="0" y="14" width="4" height="40" rx="2" fill="#ffffff"/>
  <circle cx="36" cy="34" r="24" fill="#5865f2"/>
  <text x="36" y="40" fill="#fff" font-size="16" text-anchor="middle" font-family="sans-serif">💬</text>
  <!-- Separator -->
  <line x1="20" y1="70" x2="52" y2="70" stroke="#35373c" stroke-width="2"/>
  <!-- Guild 1: Rust Developers (Hovered) -->
  <rect x="0" y="90" width="4" height="20" rx="2" fill="#ffffff"/>
  <rect x="12" y="80" width="48" height="48" rx="16" fill="#313338"/>
  <text x="36" y="110" fill="#ffffff" font-size="14" font-weight="bold" text-anchor="middle" font-family="sans-serif">RD</text>
  <!-- Guild 2: Tokyo Async -->
  <circle cx="36" cy="160" r="24" fill="#2b2d31"/>
  <text x="36" y="165" fill="#dbdee1" font-size="13" text-anchor="middle" font-family="sans-serif">TA</text>
  <!-- Add Guild Button -->
  <circle cx="36" cy="220" r="24" fill="#313338" stroke="#23a55a" stroke-dasharray="4 2"/>
  <text x="36" y="226" fill="#23a55a" font-size="20" font-weight="bold" text-anchor="middle" font-family="sans-serif">+</text>
  <!-- Explore Public Guilds -->
  <circle cx="36" cy="280" r="24" fill="#313338"/>
  <text x="36" y="286" fill="#23a55a" font-size="16" text-anchor="middle" font-family="sans-serif">🧭</text>
</svg>`;

    case 'voice_panel':
      return `<svg width="300" height="120" viewBox="0 0 300 120" xmlns="http://www.w3.org/2000/svg">
  <rect width="300" height="120" fill="#232428" rx="8" stroke="#3f4147" stroke-width="1"/>
  <!-- Voice Status Banner -->
  <circle cx="20" cy="28" r="5" fill="#23a55a"/>
  <text x="34" y="25" fill="#23a55a" font-size="12" font-weight="bold" font-family="sans-serif">Голос подключен</text>
  <text x="34" y="42" fill="#949ba4" font-size="11" font-family="sans-serif">Voice 1 / RTC 18ms (Opus 48kHz)</text>
  <!-- Disconnect Button -->
  <rect x="250" y="16" width="34" height="30" rx="4" fill="rgba(242,63,67,0.2)"/>
  <text x="267" y="36" fill="#f23f43" font-size="16" text-anchor="middle" font-family="sans-serif">📞✕</text>
  
  <line x1="16" y1="58" x2="284" y2="58" stroke="#35373c" stroke-width="1"/>

  <!-- Voice Control Buttons: Video, Screen Share, Mute, Deafen -->
  <rect x="16" y="68" width="60" height="38" rx="4" fill="#2b2d31"/>
  <text x="46" y="92" fill="#dbdee1" font-size="14" text-anchor="middle" font-family="sans-serif">📹 Камера</text>
  <rect x="84" y="68" width="60" height="38" rx="4" fill="#5865f2"/>
  <text x="114" y="92" fill="#ffffff" font-size="14" text-anchor="middle" font-family="sans-serif">🖥️ Экран</text>
  <rect x="152" y="68" width="60" height="38" rx="4" fill="#2b2d31"/>
  <text x="182" y="92" fill="#dbdee1" font-size="14" text-anchor="middle" font-family="sans-serif">🎙️ Микр</text>
  <rect x="220" y="68" width="64" height="38" rx="4" fill="#2b2d31"/>
  <text x="252" y="92" fill="#dbdee1" font-size="14" text-anchor="middle" font-family="sans-serif">🎧 Звук</text>
</svg>`;

    case 'screen_share_modal':
      return `<svg width="600" height="420" viewBox="0 0 600 420" xmlns="http://www.w3.org/2000/svg">
  <rect width="600" height="420" rx="12" fill="#313338" stroke="#3f4147" stroke-width="1"/>
  <!-- Modal Header -->
  <text x="32" y="44" fill="#ffffff" font-size="20" font-weight="bold" font-family="sans-serif">Демонстрация экрана</text>
  <text x="32" y="68" fill="#949ba4" font-size="13" font-family="sans-serif">Выберите монитор или окно приложения для трансляции через PipeWire</text>
  <!-- Tabs: Screens / Applications -->
  <rect x="32" y="92" width="120" height="32" rx="4" fill="#3f4147"/>
  <text x="92" y="113" fill="#ffffff" font-size="13" font-weight="bold" text-anchor="middle" font-family="sans-serif">Экраны (1)</text>
  <text x="200" y="113" fill="#949ba4" font-size="13" font-weight="bold" text-anchor="middle" font-family="sans-serif">Приложения (4)</text>

  <!-- Screen 1 Card (Selected) -->
  <rect x="32" y="140" width="250" height="150" rx="8" fill="#2b2d31" stroke="#5865f2" stroke-width="2"/>
  <rect x="42" y="150" width="230" height="100" rx="4" fill="#1e1f22"/>
  <text x="157" y="205" fill="#5865f2" font-size="24" text-anchor="middle" font-family="sans-serif">🖥️</text>
  <text x="42" y="274" fill="#ffffff" font-size="13" font-weight="bold" font-family="sans-serif">Экран 1: 1920x1080 (60 FPS)</text>

  <!-- Stream Quality Settings -->
  <text x="32" y="324" fill="#dbdee1" font-size="12" font-weight="bold" font-family="sans-serif">КАЧЕСТВО СТРИМА</text>
  <rect x="32" y="336" width="70" height="28" rx="4" fill="#2b2d31"/>
  <text x="67" y="355" fill="#949ba4" font-size="12" text-anchor="middle" font-family="sans-serif">720p</text>
  <rect x="110" y="336" width="70" height="28" rx="4" fill="#5865f2"/>
  <text x="145" y="355" fill="#ffffff" font-size="12" font-weight="bold" text-anchor="middle" font-family="sans-serif">1080p</text>
  <rect x="188" y="336" width="70" height="28" rx="4" fill="#2b2d31"/>
  <text x="223" y="355" fill="#949ba4" font-size="12" text-anchor="middle" font-family="sans-serif">1440p</text>

  <!-- Modal Footer Actions -->
  <rect x="0" y="372" width="600" height="48" fill="#2b2d31" rx="0 0 12 12"/>
  <text x="430" y="402" fill="#ffffff" font-size="14" font-family="sans-serif">Отмена</text>
  <rect x="490" y="380" width="94" height="34" rx="4" fill="#5865f2"/>
  <text x="537" y="402" fill="#ffffff" font-size="13" font-weight="bold" text-anchor="middle" font-family="sans-serif">В эфир</text>
</svg>`;

    case 'create_guild_modal':
      return `<svg width="440" height="360" viewBox="0 0 440 360" xmlns="http://www.w3.org/2000/svg">
  <rect width="440" height="360" rx="12" fill="#313338" stroke="#3f4147" stroke-width="1"/>
  <text x="220" y="44" fill="#ffffff" font-size="20" font-weight="bold" text-anchor="middle" font-family="sans-serif">Создайте свой сервер</text>
  <text x="220" y="68" fill="#949ba4" font-size="13" text-anchor="middle" font-family="sans-serif">Сервер — это место, где собираются ваши друзья</text>
  
  <!-- Server Name Input Field -->
  <text x="32" y="112" fill="#b5bac1" font-size="12" font-weight="bold" font-family="sans-serif">НАЗВАНИЕ СЕРВЕРА</text>
  <rect x="32" y="124" width="376" height="42" rx="4" fill="#1e1f22" stroke="#5865f2" stroke-width="1"/>
  <text x="44" y="150" fill="#ffffff" font-size="14" font-family="sans-serif">Сервер Rustery</text>

  <!-- Guild Icon Upload Placeholder -->
  <circle cx="220" cy="220" r="36" fill="#2b2d31" stroke="#3f4147" stroke-dasharray="4 2"/>
  <text x="220" y="226" fill="#949ba4" font-size="24" text-anchor="middle" font-family="sans-serif">📷</text>
  <text x="220" y="272" fill="#949ba4" font-size="11" text-anchor="middle" font-family="sans-serif">Загрузить иконку</text>

  <!-- Footer Actions -->
  <rect x="0" y="304" width="440" height="56" fill="#2b2d31" rx="0 0 12 12"/>
  <text x="48" y="338" fill="#ffffff" font-size="14" font-family="sans-serif">Назад</text>
  <rect x="310" y="316" width="98" height="34" rx="4" fill="#5865f2"/>
  <text x="359" y="338" fill="#ffffff" font-size="13" font-weight="bold" text-anchor="middle" font-family="sans-serif">Создать</text>
</svg>`;

    default:
      return `<svg width="400" height="200" xmlns="http://www.w3.org/2000/svg">
  <rect width="400" height="200" fill="#2b2d31" rx="8" stroke="#3f4147"/>
  <text x="200" y="100" fill="#fff" font-size="16" text-anchor="middle" font-family="sans-serif">Wireframe: ${component}</text>
</svg>`;
  }
}

/** @type {Record<string, { description: string, inputSchema: any, run: (args: any) => Promise<string>|string }>} */
const TOOLS = {
  wireframe_inspect_design_tokens: {
    description: "Returns official Discord Dark theme design tokens (colors, typography, dimensions, border-radii).",
    inputSchema: { type: "object", properties: {} },
    run: () => {
      return JSON.stringify(DISCORD_DARK_TOKENS, null, 2);
    }
  },

  wireframe_generate_component: {
    description: "Generates an SVG wireframe and layout blueprint for a Discord GPUI component.",
    inputSchema: {
      type: "object",
      required: ["component_name"],
      properties: {
        component_name: {
          type: "string",
          description: "Component name: app_layout, context_menu, server_rail, channel_sidebar, chat_view, create_guild_modal, screen_share_modal, voice_panel"
        }
      }
    },
    run: (args) => {
      fs.mkdirSync(WIREFRAME_DIR, { recursive: true });
      const svg = generateSvgWireframe(args.component_name);
      const filePath = path.join(WIREFRAME_DIR, `${args.component_name}.svg`);
      fs.writeFileSync(filePath, svg, 'utf-8');
      return JSON.stringify({
        component: args.component_name,
        svg_file: filePath,
        preview_svg: svg
      }, null, 2);
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
      serverInfo: { name: 'design-wireframe', version: '1.0.0' },
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

if (process.argv.includes('--generate-all')) {
  fs.mkdirSync(WIREFRAME_DIR, { recursive: true });
  const components = [
    'app_layout',
    'context_menu',
    'server_rail',
    'channel_sidebar',
    'voice_panel',
    'screen_share_modal',
    'create_guild_modal'
  ];
  for (const c of components) {
    const svg = generateSvgWireframe(c);
    const fp = path.join(WIREFRAME_DIR, `${c}.svg`);
    fs.writeFileSync(fp, svg, 'utf-8');
    console.log(`Generated: ${fp}`);
  }
  process.exit(0);
}

const rl = readline.createInterface({ input: process.stdin, output: process.stdout, terminal: false });
rl.on('line', (line) => {
  if (!line.trim()) return;
  try {
    handle(JSON.parse(line));
  } catch (e) {
    replyError(null, -32700, 'Parse error');
  }
});
