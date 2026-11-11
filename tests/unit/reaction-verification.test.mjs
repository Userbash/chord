// @ts-nocheck
/**
 * TASK-816 (G2 Frontend Lead): Интерактивное прохождение Reaction Verification в каналах правил
 * (`rust/gui-spike/ui/features/reaction-verification.js`,
 *  `rust/gui-spike/ui/components/reaction-verification.css`,
 *  `rust/gui-spike/ui/app.js:renderReactions`).
 *
 * Спецификация (waves/kanban-board.json):
 * - summary: "Обеспечить мгновенную установку реакции по клику на бейджи реакций под верификационным
 *   сообщением в канале #verify через существующий IPC add_reaction, автоматизируя получение роли участника."
 * - layout:
 *   - container: "Reactions Bar"
 *   - dimensions: "rust/gui-spike/ui/app.js:renderReactions"
 *   - padding: "4px 6px"
 *   - spacing: "4px"
 * - design_tokens:
 *   - background: "#2b2d31"
 *   - surface: "#35373c"
 *   - text_primary: "#f2f3f5"
 *   - accent: "#5865f2"
 * - interactive_states:
 *   - element: "Reaction Badge"
 *   - default: "border: 1px solid #3f4147"
 *   - hover: "border-color: #5865f2"
 *   - active: "PUT reaction"
 * - acceptance_criteria:
 *   1. "Клик по эмодзи в верификационном канале ставит реакцию пользователя"
 *   2. "Бот выдает роль участника и открываются остальные каналы сервера"
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  REACTION_VERIFICATION_TOKENS,
  VERIFICATION_CHANNEL_NAMES_RE,
  VERIFICATION_KEYWORDS_RE,
  isVerificationChannel,
  isVerificationMessage,
  createReactionBadge,
  handleReactionVerification,
  renderReactions,
  renderVerificationBanner,
} from '../../rust/gui-spike/ui/features/reaction-verification.js';

import {
  renderReactions as renderReactionsFromApp,
} from '../../rust/gui-spike/ui/app.js';

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для модульных тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {any} [ownerDocument]
   */
  constructor(tagName, ownerDocument = null) {
    this.tagName = String(tagName).toUpperCase();
    this.ownerDocument = ownerDocument;
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = '';
    this._listeners = new Map();
    /** @type {Record<string, string>} */
    this.style = {};
    this.className = '';
    this.id = '';
    this.type = '';
  }

  get textContent() {
    if (this.children.length === 0) return this._text;
    return this.children.map((c) => c.textContent).join('');
  }

  set textContent(val) {
    this.children = [];
    this._text = String(val ?? '');
  }

  get classList() {
    return {
      contains: (cls) => this.className.split(/\s+/).filter(Boolean).includes(cls),
      add: (cls) => {
        const parts = this.className ? this.className.split(/\s+/).filter(Boolean) : [];
        if (!parts.includes(cls)) parts.push(cls);
        this.className = parts.join(' ');
      },
      remove: (cls) => {
        const parts = this.className ? this.className.split(/\s+/).filter(Boolean) : [];
        this.className = parts.filter((c) => c !== cls).join(' ');
      },
      toggle: (cls, force) => {
        const parts = this.className ? this.className.split(/\s+/).filter(Boolean) : [];
        const has = parts.includes(cls);
        const shouldAdd = force !== undefined ? Boolean(force) : !has;
        if (shouldAdd && !has) parts.push(cls);
        if (!shouldAdd && has) {
          const idx = parts.indexOf(cls);
          parts.splice(idx, 1);
        }
        this.className = parts.join(' ');
        return shouldAdd;
      },
    };
  }

  setAttribute(name, value) {
    this._attrs.set(String(name), String(value));
  }

  getAttribute(name) {
    return this._attrs.has(String(name)) ? this._attrs.get(String(name)) : null;
  }

  removeAttribute(name) {
    this._attrs.delete(String(name));
  }

  hasAttribute(name) {
    return this._attrs.has(String(name));
  }

  appendChild(child) {
    if (!child) return child;
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  append(...items) {
    for (const item of items) {
      if (typeof item === 'string') {
        const span = new FakeElement('span', this.ownerDocument);
        span.textContent = item;
        this.appendChild(span);
      } else if (item instanceof FakeElement) {
        this.appendChild(item);
      }
    }
  }

  replaceChildren(...items) {
    this.children = [];
    this._text = '';
    this.append(...items);
  }

  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }

  querySelectorAll(selector) {
    const results = [];
    const walk = (el) => {
      for (const child of el.children) {
        if (matches(child, selector)) {
          results.push(child);
        }
        walk(child);
      }
    };
    walk(this);
    return results;
  }

  addEventListener(type, handler) {
    if (!this._listeners.has(type)) {
      this._listeners.set(type, []);
    }
    this._listeners.get(type).push(handler);
  }

  removeEventListener(type, handler) {
    const list = this._listeners.get(type);
    if (!list) return;
    const idx = list.indexOf(handler);
    if (idx >= 0) list.splice(idx, 1);
  }

  dispatchEvent(event) {
    const type = event?.type || 'click';
    const list = this._listeners.get(type) || [];
    for (const handler of list) {
      handler.call(this, event);
    }
    return true;
  }

  click() {
    this.dispatchEvent({ type: 'click', target: this, currentTarget: this, defaultPrevented: false });
  }

  remove() {
    if (this.parentNode) {
      const idx = this.parentNode.children.indexOf(this);
      if (idx >= 0) this.parentNode.children.splice(idx, 1);
      this.parentNode = null;
    }
  }
}

function matches(el, selector) {
  const parts = selector.split(/\s+/);
  const target = parts[parts.length - 1];
  if (target.startsWith('.')) {
    const cls = target.slice(1);
    if (cls.includes('[')) {
      const className = cls.split('[')[0];
      const attr = cls.slice(className.length);
      return el.classList.contains(className) && matchesAttr(el, attr);
    }
    return el.classList.contains(cls);
  }
  if (target.startsWith('#')) {
    return el.id === target.slice(1);
  }
  if (target.startsWith('[')) {
    return matchesAttr(el, target);
  }
  return el.tagName.toLowerCase() === target.toLowerCase();
}

function matchesAttr(el, attrExpr) {
  const m = attrExpr.match(/\[([a-zA-Z0-9_-]+)(?:=(["]?)([^"\]]*)\2)?\]/);
  if (!m) return false;
  const attr = m[1];
  const val = m[3];
  if (val === undefined) return el.hasAttribute(attr);
  return el.getAttribute(attr) === val;
}

class FakeDocument {
  createElement(tagName) {
    return new FakeElement(tagName, this);
  }

  getElementById(id) {
    if (!this.body) return null;
    return this.body.querySelector(`#${id}`);
  }
}

describe('TASK-816: Интерактивное прохождение Reaction Verification в каналах правил', () => {
  let doc;

  beforeEach(() => {
    doc = new FakeDocument();
    doc.body = new FakeElement('body', doc);
  });

  describe('1. Дизайн-токены и валидаторы правил/верификации', () => {
    it('экспортирует точные дизайн-токены Discord по спецификации TASK-816', () => {
      assert.equal(REACTION_VERIFICATION_TOKENS.background, '#2b2d31');
      assert.equal(REACTION_VERIFICATION_TOKENS.surface, '#35373c');
      assert.equal(REACTION_VERIFICATION_TOKENS.text_primary, '#f2f3f5');
      assert.equal(REACTION_VERIFICATION_TOKENS.accent, '#5865f2');
      assert.equal(REACTION_VERIFICATION_TOKENS.border_default, '1px solid #3f4147');
      assert.equal(REACTION_VERIFICATION_TOKENS.border_hover, '#5865f2');
      assert.equal(REACTION_VERIFICATION_TOKENS.padding, '4px 6px');
      assert.equal(REACTION_VERIFICATION_TOKENS.spacing, '4px');
    });

    it('корректно распознает каналы правил и верификации (isVerificationChannel)', () => {
      assert.equal(isVerificationChannel({ name: 'verify' }), true);
      assert.equal(isVerificationChannel({ name: 'verification' }), true);
      assert.equal(isVerificationChannel({ name: 'rules' }), true);
      assert.equal(isVerificationChannel({ name: 'правила' }), true);
      assert.equal(isVerificationChannel({ name: 'добро-пожаловать' }), true);
      assert.equal(isVerificationChannel({ name: 'welcome' }), true);
      assert.equal(isVerificationChannel({ name: 'general' }), false);
      assert.equal(isVerificationChannel({ name: 'chat' }), false);
      assert.equal(isVerificationChannel({ name: 'voice' }), false);
      assert.equal(isVerificationChannel({ name: 'custom', isVerification: true }), true);
    });

    it('корректно распознает верификационные сообщения с правилами (isVerificationMessage)', () => {
      const msg1 = { content: 'Нажмите на реакцию ✅ ниже, чтобы согласиться с правилами сервера' };
      const msg2 = { content: 'Please react with ✅ to verify and get access' };
      const msg3 = { content: 'Обычное сообщение участника в чате' };
      assert.equal(isVerificationMessage(msg1), true);
      assert.equal(isVerificationMessage(msg2), true);
      assert.equal(isVerificationMessage(msg3), false);
    });
  });

  describe('2. Рендеринг и интерактивность Reaction Badge (createReactionBadge)', () => {
    it('рендерит бейдж реакции с классами, счетчиком и доступностью (A11y)', () => {
      const badge = createReactionBadge(
        { emoji: '✅', count: 42, me: false },
        { document: doc, channelId: '101', messageId: '202' }
      );

      assert.equal(badge.tagName, 'BUTTON');
      assert.equal(badge.getAttribute('type'), 'button');
      assert.equal(badge.classList.contains('reaction-badge'), true);
      assert.equal(badge.getAttribute('data-emoji'), '✅');
      assert.equal(badge.getAttribute('data-me'), 'false');
      assert.equal(badge.getAttribute('aria-pressed'), 'false');
      assert.match(badge.getAttribute('aria-label') || '', /Реакция ✅, 42/);

      const emojiEl = badge.querySelector('.reaction-emoji');
      const countEl = badge.querySelector('.reaction-count');
      assert.ok(emojiEl, 'отсутствует .reaction-emoji');
      assert.ok(countEl, 'отсутствует .reaction-count');
      assert.equal(emojiEl.textContent, '✅');
      assert.equal(countEl.textContent, '42');
    });

    it('клик по бейджу реакции оптимистично обновляет счетчик, класс active и вызывает IPC add_reaction', async () => {
      /** @type {Array<{ cmd: string, args: any }>} */
      const calls = [];
      const invokeMock = async (cmd, args) => {
        calls.push({ cmd, args });
        return true;
      };

      const badge = createReactionBadge(
        { emoji: '✅', count: 10, me: false },
        {
          document: doc,
          channelId: 'ch-verify',
          messageId: 'msg-rules',
          invoke: invokeMock,
        }
      );

      assert.equal(badge.getAttribute('data-me'), 'false');
      assert.equal(badge.classList.contains('active'), false);
      assert.equal(badge.querySelector('.reaction-count')?.textContent, '10');

      // Клик пользователя (PUT reaction)
      badge.click();

      // Оптимистичное обновление
      assert.equal(badge.getAttribute('data-me'), 'true');
      assert.equal(badge.getAttribute('aria-pressed'), 'true');
      assert.equal(badge.classList.contains('active'), true);
      assert.equal(badge.querySelector('.reaction-count')?.textContent, '11');

      // Проверка вызова IPC
      assert.equal(calls.length, 1);
      assert.equal(calls[0].cmd, 'add_reaction');
      assert.deepEqual(calls[0].args, {
        channelId: 'ch-verify',
        messageId: 'msg-rules',
        emoji: '✅',
      });
    });

    it('повторный клик снимает реакцию через toggle_reaction active=false и декрементирует счетчик', async () => {
      /** @type {Array<{ cmd: string, args: any }>} */
      const calls = [];
      const invokeMock = async (cmd, args) => {
        calls.push({ cmd, args });
        return true;
      };

      const badge = createReactionBadge(
        { emoji: '✅', count: 5, me: true },
        {
          document: doc,
          channelId: 'ch-verify',
          messageId: 'msg-rules',
          invoke: invokeMock,
        }
      );

      assert.equal(badge.getAttribute('data-me'), 'true');
      assert.equal(badge.classList.contains('active'), true);

      badge.click();

      assert.equal(badge.getAttribute('data-me'), 'false');
      assert.equal(badge.classList.contains('active'), false);
      assert.equal(badge.querySelector('.reaction-count')?.textContent, '4');

      assert.equal(calls.length, 1);
      assert.equal(calls[0].cmd, 'toggle_reaction');
      assert.deepEqual(calls[0].args, {
        channelId: 'ch-verify',
        messageId: 'msg-rules',
        emoji: '✅',
        active: false,
      });
    });
  });

  describe('3. Автоматизация Reaction Verification и разблокировка каналов', () => {
    it('при клике на эмодзи в канале правил выдает роль участника и открывает остальные каналы сервера', async () => {
      let verifiedRoleId = null;
      let unlocked = false;

      const guild = {
        id: 'guild-100',
        name: 'Сообщество разработчиков',
        pending: true,
        roles: [
          { id: 'guild-100', name: '@everyone', permissions: '0' },
          { id: 'role-member', name: 'Member', permissions: '1024' },
        ],
        channels: [
          { id: 'ch-rules', name: 'rules', type: 0 },
          { id: 'ch-general', name: 'general', type: 0, permission_overwrites: [{ id: 'guild-100', deny: '1024' }, { id: 'role-member', allow: '1024' }] },
          { id: 'ch-dev', name: 'dev', type: 0, permission_overwrites: [{ id: 'guild-100', deny: '1024' }, { id: 'role-member', allow: '1024' }] },
        ],
      };

      const badge = createReactionBadge(
        { emoji: '✅', count: 120, me: false },
        {
          document: doc,
          guildId: 'guild-100',
          guild,
          channelId: 'ch-rules',
          messageId: 'msg-verification',
          isVerificationChannel: true,
          invoke: async () => true,
          onVerified: (gId, rId) => {
            verifiedRoleId = rId;
            guild.pending = false;
          },
          onUnlockChannels: () => {
            unlocked = true;
          },
        }
      );

      badge.click();

      // Проверяем выдачу роли
      assert.equal(verifiedRoleId, 'role-member');
      assert.equal(guild.pending, false);
      assert.equal(unlocked, true);
    });

    it('handleReactionVerification находит и назначает подходящую роль участника', () => {
      const guild = {
        id: 'guild-200',
        roles: [
          { id: 'guild-200', name: '@everyone' },
          { id: 'role-verified', name: 'Верифицирован' },
        ],
      };

      let assignedRole = null;
      const res = handleReactionVerification({
        guildId: 'guild-200',
        channelId: 'verify',
        messageId: '1',
        emoji: '✅',
        guild,
        onVerified: (gId, rId) => {
          assignedRole = rId;
        },
      });

      assert.equal(res.verified, true);
      assert.equal(res.roleId, 'role-verified');
      assert.equal(assignedRole, 'role-verified');
    });
  });

  describe('4. Рендеринг панели реакций (renderReactions) в app.js и CSS разметка', () => {
    it('renderReactions рендерит Reactions Bar с правильными отступами, фоном и бейджами', () => {
      const container = doc.createElement('div');
      const reactions = [
        { emoji: '✅', count: 15, me: false },
        { emoji: '👍', count: 7, me: true },
      ];

      const bar = renderReactions(container, reactions, {
        document: doc,
        channelId: 'ch-rules',
        messageId: 'msg-01',
      });

      assert.ok(bar);
      assert.equal(bar.classList.contains('reactions-bar'), true);
      assert.equal(bar.getAttribute('role'), 'group');
      assert.equal(bar.getAttribute('aria-label'), 'Реакции');

      const badges = bar.querySelectorAll('.reaction-badge');
      assert.equal(badges.length, 2);

      assert.equal(badges[0].getAttribute('data-emoji'), '✅');
      assert.equal(badges[0].getAttribute('data-me'), 'false');

      assert.equal(badges[1].getAttribute('data-emoji'), '👍');
      assert.equal(badges[1].getAttribute('data-me'), 'true');
      assert.equal(badges[1].classList.contains('active'), true);
    });

    it('renderReactions в app.js экспортируется и идентичен функционалу модуля', () => {
      assert.equal(typeof renderReactionsFromApp, 'function');
      const container = doc.createElement('div');
      const bar = renderReactionsFromApp(container, [{ emoji: '🎉', count: 1, me: true }], {
        document: doc,
        channelId: '1',
        messageId: '2',
      });
      assert.ok(bar);
      assert.equal(bar.querySelectorAll('.reaction-badge').length, 1);
    });

    it('renderVerificationBanner рендерит плашку правила сервера с подсказкой прохождения', () => {
      const banner = renderVerificationBanner({
        document: doc,
        channelName: 'verify',
        guildName: 'Test Guild',
      });

      assert.ok(banner);
      assert.equal(banner.classList.contains('reaction-verification-banner'), true);
      assert.match(banner.textContent, /правил|верификац/i);
    });
  });
});
