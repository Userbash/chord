// @ts-check
// Historical verification records retain the namespace used when accepted.
// Resolve only the source rename already declared by the current Cargo files;
// release artifact names and clean-room evidence identifiers stay unchanged.
const crateRoles = new Set([
  "app", "codec", "conformance", "core", "i18n", "json", "media", "perf",
  "platform", "store", "ui", "updater", "voice",
]);

/** @param {string} reference @returns {string} */
export function canonicalRustSourcePath(reference) {
  const path = reference.replaceAll("\\", "/");
  const crate = /^rust\/crates\/discord-([^/]+)(\/.*)?$/.exec(path);
  if (crate?.[1] && crateRoles.has(crate[1])) {
    return `rust/crates/chord-${crate[1]}${crate[2] ?? ""}`;
  }
  const gui = /^rust\/discord-(gpui|voice)(\/.*)?$/.exec(path);
  if (gui?.[1]) return `rust/chord-${gui[1]}${gui[2] ?? ""}`;
  return reference;
}
