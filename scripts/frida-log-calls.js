// @ts-check
'use strict';

// Frida agent for the Discord function lab (Frida 17 module API).
//
// Driven through rpc.exports from scripts/logger.mjs:
//   inventory(options) -> per-module symbol/export inventory for discord*.node
//   hook(options)      -> intercept matching functions and report calls
//   unhook()           -> detach every installed interceptor
//
// Discord ships stripped dynamic exports but readable symbol tables, so the
// default hook source is `symbols` (name + address), not `exports`.

/**
 * Опции RPC inventory (приходят из scripts/logger.mjs).
 * @typedef {object} InventoryOptions
 * @property {string[]} [modules]   ограничить модули (по умолчанию discord*.node + updater)
 * @property {number} [maxSymbols]  лимит символов на модуль (по умолчанию 50000)
 */
/**
 * Опции RPC hook.
 * @typedef {object} HookOptions
 * @property {string[]} [modules]
 * @property {"symbols" | "exports"} [kind]
 * @property {string} [pattern]
 * @property {number} [limit]
 * @property {number} [argCount]
 * @property {boolean} [backtrace]
 */
/**
 * @typedef {object} SymbolEntry
 * @property {string} name
 * @property {string} address
 * @property {string} offset
 */
/**
 * @typedef {object} ExportEntry
 * @property {string} name
 * @property {string} type
 * @property {string} address
 * @property {string} offset
 */
/**
 * @typedef {object} ModuleInfo
 * @property {string} name
 * @property {string} path
 * @property {string} base
 * @property {number} size
 * @property {SymbolEntry[]} symbols
 * @property {ExportEntry[]} exports
 * @property {number} [symbolsTruncated]  исходное число символов при обрезке
 * @property {string | null} error
 */
/** @typedef {{ module: string, name: string, address: NativePointer }} Candidate */

/** @type {InvocationListener[]} */
const activeHooks = [];

/** @returns {string[]} */
function discordModuleNames() {
  return Process.enumerateModules()
    .filter(function (m) {
      return /\.node$/i.test(m.path) && /discord|updater/i.test(m.path);
    })
    .map(function (m) {
      return m.name;
    });
}

/** @param {string} name @returns {ModuleInfo} */
function moduleEntry(name) {
  /** @type {Module} */
  let module_;
  try {
    module_ = Process.getModuleByName(name);
  } catch (e) {
    return {
      name: name,
      path: '',
      base: '0',
      size: 0,
      symbols: [],
      exports: [],
      error: String(e),
    };
  }
  /** @type {ModuleSymbolDetails[]} */
  let symbols = [];
  /** @type {ModuleExportDetails[]} */
  let exports = [];
  /** @type {string | null} */
  let error = null;
  try {
    symbols = module_.enumerateSymbols().filter(function (symbol) {
      return symbol.type === 'function';
    });
  } catch (e) {
    error = String(e);
  }
  try {
    exports = module_.enumerateExports();
  } catch (e) {
    error = (error ? error + ' | ' : '') + String(e);
  }
  return {
    name: module_.name,
    path: module_.path,
    base: module_.base.toString(),
    size: module_.size,
    symbols: symbols.map(function (symbol) {
      return {
        name: symbol.name,
        address: symbol.address.toString(),
        offset: symbol.address.sub(module_.base).toString(),
      };
    }),
    exports: exports.map(function (entry) {
      return {
        name: entry.name,
        type: entry.type,
        address: entry.address.toString(),
        offset: entry.address.sub(module_.base).toString(),
      };
    }),
    error: error,
  };
}

/**
 * @param {string[]} moduleNames
 * @param {"symbols" | "exports"} kind
 * @param {RegExp} pattern
 * @param {number} limit
 * @returns {Candidate[]}
 */
function candidates(moduleNames, kind, pattern, limit) {
  /** @type {Candidate[]} */
  const found = [];
  for (const moduleName of moduleNames) {
    /** @type {Module} */
    let module_;
    try {
      module_ = Process.getModuleByName(moduleName);
    } catch (e) {
      continue;
    }
    /** @type {{ name: string, address: NativePointer, type: string }[]} */
    let entries = [];
    if (kind === 'exports') {
      try {
        entries = module_.enumerateExports().map(function (entry) {
          return { name: entry.name, address: entry.address, type: entry.type };
        });
      } catch (e) {
        continue;
      }
    } else {
      try {
        entries = module_
          .enumerateSymbols()
          .filter(function (symbol) {
            return symbol.type === 'function' || symbol.type === 'unknown';
          })
          .map(function (symbol) {
            return { name: symbol.name, address: symbol.address, type: symbol.type };
          });
      } catch (e) {
        continue;
      }
    }
    for (const entry of entries) {
      if (!pattern.test(entry.name)) continue;
      found.push({ module: moduleName, name: entry.name, address: entry.address });
      if (found.length >= limit) return found;
    }
  }
  return found;
}

/** Снимает все ранее установленные хуки; сбои detach не мешают остальным. */
function detachAll() {
  while (activeHooks.length > 0) {
    const hook = activeHooks.pop();
    if (!hook) break;
    try {
      hook.detach();
    } catch (e) {
      // Already detached.
    }
  }
}

rpc.exports = {
  /** @param {InventoryOptions} [options] */
  inventory: function (options) {
    /** @type {InventoryOptions} */
    const opts = options || {};
    const maxSymbols = typeof opts.maxSymbols === 'number' ? opts.maxSymbols : 50000;
    const names =
      opts.modules && opts.modules.length > 0 ? opts.modules : discordModuleNames();
    return names.map(function (name) {
      const entry = moduleEntry(name);
      if (entry.symbols.length > maxSymbols) {
        entry.symbolsTruncated = entry.symbols.length;
        entry.symbols = entry.symbols.slice(0, maxSymbols);
      }
      return entry;
    });
  },

  /** @param {HookOptions} [options] */
  hook: function (options) {
    detachAll();
    /** @type {HookOptions} */
    const opts = options || {};
    const limit = typeof opts.limit === 'number' ? opts.limit : 64;
    const argCount = typeof opts.argCount === 'number' ? Math.min(opts.argCount, 6) : 2;
    const pattern = new RegExp(opts.pattern || '.');
    const kind = opts.kind === 'exports' ? 'exports' : 'symbols';
    const names =
      opts.modules && opts.modules.length > 0 ? opts.modules : discordModuleNames();

    const targets = candidates(names, kind, pattern, limit);
    for (const target of targets) {
      try {
        activeHooks.push(
          Interceptor.attach(target.address, {
            onEnter: function (args) {
              /** @type {(string | null)[]} */
              const preview = [];
              for (let index = 0; index < argCount; index += 1) {
                try {
                  const arg = args[index];
                  preview.push(arg === undefined ? null : arg.toString());
                } catch (e) {
                  preview.push(null);
                }
              }
              /** @type {Record<string, unknown>} */
              const message = {
                type: 'call',
                module: target.module,
                name: target.name,
                address: target.address.toString(),
                threadId: this.threadId,
                at: Date.now(),
                args: preview,
              };
              if (opts.backtrace === true) {
                /** @type {string[] | null} */
                let stack = null;
                try {
                  stack = Thread.backtrace(this.context, Backtracer.FUZZY)
                    .slice(0, 8)
                    .map(function (address) {
                      const module_ = Process.findModuleByAddress(address);
                      if (!module_) return String(address);
                      const offset = address.sub(module_.base).toString().replace(/^0x/i, '');
                      return module_.name + '+0x' + offset;
                    });
                } catch (e) {
                  // Backtrace is optional and may be unavailable for a given context.
                }
                message.stack = stack;
              }
              send(message);
            },
          }),
        );
      } catch (e) {
        // Unsupported prologue or unreachable target: keep the rest hooked.
      }
    }
    return { hooked: activeHooks.length, targets: targets.length, kind: kind };
  },

  unhook: function () {
    const count = activeHooks.length;
    detachAll();
    return count;
  },
};
