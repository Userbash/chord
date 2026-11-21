#!/usr/bin/env node
// @ts-check
/**
 * R4-09: Store Metadata Linter (Flathub metainfo.xml + Windows Winget 1.6.0 manifests).
 * Validates metadata manifests for stable and canary release channels.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_METAINFO = join(
  ROOT,
  "packaging",
  "flatpak",
  "com.discord_lab.DiscordGpui.metainfo.xml",
);
const DEFAULT_WINGET_DIR = join(ROOT, "packaging", "windows", "winget");
const VALID_CHANNELS = new Set(["stable", "canary", "all"]);

const CHANNEL_PACKAGE_IDS = {
  stable: "DiscordLab.DiscordGpui",
  canary: "DiscordLab.DiscordGpui.Canary",
};

/**
 * Parse CLI arguments.
 * @param {string[]} argv
 */
function parseArgs(argv) {
  let channel = "all";
  let metainfoPath = DEFAULT_METAINFO;
  let wingetDir = DEFAULT_WINGET_DIR;
  let json = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--channel") {
      channel = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--metainfo") {
      metainfoPath = argv[i + 1] ?? metainfoPath;
      i += 1;
    } else if (arg === "--winget-dir") {
      wingetDir = argv[i + 1] ?? wingetDir;
      i += 1;
    } else if (arg === "--json") {
      json = true;
    } else if (arg === "-h" || arg === "--help") {
      console.log(
        "Usage: node scripts/lint-store-metadata.mjs [--channel stable|canary|all] [--metainfo PATH] [--winget-dir DIR] [--json]",
      );
      process.exit(0);
    } else {
      console.error(`lint-store-metadata: unknown option: ${arg}`);
      process.exit(1);
    }
  }

  if (!VALID_CHANNELS.has(channel)) {
    console.error(
      `lint-store-metadata: invalid channel '${channel}'. Expected: stable, canary, all`,
    );
    process.exit(1);
  }

  return { channel, metainfoPath, wingetDir, json };
}

/**
 * Parse YAML file using Python PyYAML oracle.
 * @param {string} yamlPath
 * @returns {{ ok: boolean, data?: any, error?: string }}
 */
function parseYaml(yamlPath) {
  const py = [
    "import json, sys, yaml",
    "with open(sys.argv[1], 'r', encoding='utf-8') as f:",
    "    print(json.dumps(yaml.safe_load(f)))",
  ].join("\n");
  const res = spawnSync("python3", ["-c", py, yamlPath], { encoding: "utf8" });
  if (res.status !== 0) {
    return { ok: false, error: res.stderr.trim() || `YAML parse failed for ${yamlPath}` };
  }
  try {
    return { ok: true, data: JSON.parse(res.stdout) };
  } catch (err) {
    return { ok: false, error: String(err) };
  }
}

/**
 * Validate Flathub metainfo.xml using xmllint, appstreamcli (when present), and structural checks.
 * @param {string} metainfoPath
 */
export function lintFlathubMetainfo(metainfoPath = DEFAULT_METAINFO) {
  /** @type {string[]} */
  const errors = [];

  if (!existsSync(metainfoPath)) {
    return { ok: false, path: metainfoPath, errors: [`Missing metainfo file: ${metainfoPath}`] };
  }

  const xmlRes = spawnSync("xmllint", ["--noout", metainfoPath], { encoding: "utf8" });
  if (xmlRes.status !== 0 && !xmlRes.error) {
    errors.push(`xmllint failed: ${xmlRes.stderr.trim()}`);
  }

  const appstreamRes = spawnSync(
    "appstreamcli",
    ["validate", "--no-net", "--pedantic", metainfoPath],
    { encoding: "utf8" },
  );
  if (appstreamRes.status !== 0 && !appstreamRes.error) {
    errors.push(
      `appstreamcli validate failed: ${(appstreamRes.stdout + "\n" + appstreamRes.stderr).trim()}`,
    );
  }

  const raw = readFileSync(metainfoPath, "utf8");
  const requiredTags = [
    /<component\s+type="desktop-application"/,
    /<id>com\.discord_lab\.DiscordGpui<\/id>/,
    /<metadata_license>[^<]+<\/metadata_license>/,
    /<project_license>[^<]+<\/project_license>/,
    /<name>[^<]+<\/name>/,
    /<summary>[^<]+<\/summary>/,
    /<launchable\s+type="desktop-id">com\.discord_lab\.DiscordGpui\.desktop<\/launchable>/,
    /<content_rating\s+type="oars-1\.1">/,
    /<releases>/,
  ];
  for (const re of requiredTags) {
    if (!re.test(raw)) {
      errors.push(`metainfo.xml missing required pattern: ${re}`);
    }
  }

  return { ok: errors.length === 0, path: metainfoPath, errors };
}

/**
 * Validate Windows Winget 1.6.0 manifests for a given channel.
 * @param {"stable" | "canary"} channel
 * @param {string} wingetDir
 */
export function lintWingetChannel(channel, wingetDir = DEFAULT_WINGET_DIR) {
  const id = CHANNEL_PACKAGE_IDS[channel];
  const dir = join(wingetDir, channel);
  const versionPath = join(dir, `${id}.yaml`);
  const installerPath = join(dir, `${id}.installer.yaml`);
  const localePath = join(dir, `${id}.locale.en-US.yaml`);
  /** @type {string[]} */
  const errors = [];

  for (const p of [versionPath, installerPath, localePath]) {
    if (!existsSync(p)) {
      errors.push(`Missing Winget manifest: ${p}`);
    }
  }
  if (errors.length > 0) {
    return { ok: false, channel, id, errors };
  }

  const verParsed = parseYaml(versionPath);
  const instParsed = parseYaml(installerPath);
  const locParsed = parseYaml(localePath);
  /** @type {Array<[string, { ok: boolean, data?: any, error?: string }]>} */
  const manifests = [
    ["version", verParsed],
    ["installer", instParsed],
    ["locale", locParsed],
  ];
  for (const [label, parsed] of manifests) {
    if (!parsed || !parsed.ok || !parsed.data || typeof parsed.data !== "object") {
      errors.push(`Invalid ${label} YAML: ${parsed?.error ?? "not an object"}`);
    }
  }
  if (errors.length > 0) {
    return { ok: false, channel, id, errors };
  }

  const ver = verParsed.data;
  const inst = instParsed.data;
  const loc = locParsed.data;

  if (ver.PackageIdentifier !== id) errors.push(`version PackageIdentifier != ${id}`);
  if (inst.PackageIdentifier !== id) errors.push(`installer PackageIdentifier != ${id}`);
  if (loc.PackageIdentifier !== id) errors.push(`locale PackageIdentifier != ${id}`);

  if (typeof ver.PackageVersion !== "string" || !ver.PackageVersion.trim()) {
    errors.push("version PackageVersion must be a non-empty string");
  }
  if (inst.PackageVersion !== ver.PackageVersion) {
    errors.push("installer PackageVersion must match version PackageVersion");
  }
  if (loc.PackageVersion !== ver.PackageVersion) {
    errors.push("locale PackageVersion must match version PackageVersion");
  }

  if (ver.ManifestType !== "version") errors.push("version ManifestType must be 'version'");
  if (inst.ManifestType !== "installer") errors.push("installer ManifestType must be 'installer'");
  if (loc.ManifestType !== "defaultLocale") {
    errors.push("locale ManifestType must be 'defaultLocale'");
  }

  for (const [label, doc] of [
    ["version", ver],
    ["installer", inst],
    ["locale", loc],
  ]) {
    if (doc.ManifestVersion !== "1.6.0") {
      errors.push(`${label} ManifestVersion must be '1.6.0'`);
    }
  }

  if (!["wix", "msi", "msix", "inno", "nullsoft", "exe", "burn", "zip", "portable"].includes(inst.InstallerType)) {
    errors.push(`Invalid InstallerType: ${inst.InstallerType}`);
  }
  if (!["user", "machine"].includes(inst.Scope)) {
    errors.push(`Invalid Scope: ${inst.Scope}`);
  }
  if (!Array.isArray(inst.Installers) || inst.Installers.length === 0) {
    errors.push("installer Installers must be a non-empty array");
  } else {
    for (const entry of inst.Installers) {
      if (!["x64", "x86", "arm64"].includes(entry.Architecture)) {
        errors.push(`Invalid installer Architecture: ${entry.Architecture}`);
      }
      if (typeof entry.InstallerUrl !== "string" || !/^https:\/\/.+/.test(entry.InstallerUrl)) {
        errors.push(`Invalid InstallerUrl: ${entry.InstallerUrl}`);
      }
      if (typeof entry.InstallerSha256 !== "string" || !/^[A-Fa-f0-9]{64}$/.test(entry.InstallerSha256)) {
        errors.push(`Invalid InstallerSha256: ${entry.InstallerSha256}`);
      }
    }
  }

  for (const field of ["PackageLocale", "Publisher", "PackageName", "License", "ShortDescription"]) {
    if (typeof loc[field] !== "string" || !loc[field].trim()) {
      errors.push(`locale missing required field: ${field}`);
    }
  }

  return { ok: errors.length === 0, channel, id, version: ver.PackageVersion, errors };
}

/**
 * Lint store metadata for selected channel(s).
 * @param {"stable" | "canary" | "all"} channel
 * @param {string} metainfoPath
 * @param {string} wingetDir
 */
export function lintStoreMetadata(
  channel = "all",
  metainfoPath = DEFAULT_METAINFO,
  wingetDir = DEFAULT_WINGET_DIR,
) {
  const flathub = lintFlathubMetainfo(metainfoPath);
  /** @type {Record<string, any>} */
  const channels = {};
  const targetChannels = channel === "all" ? ["stable", "canary"] : [channel];
  for (const ch of targetChannels) {
    if (ch === "stable" || ch === "canary") {
      channels[ch] = lintWingetChannel(ch, wingetDir);
    }
  }
  const ok = flathub.ok && Object.values(channels).every((c) => Boolean(c.ok));
  return { ok, channel, flathub, channels };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const report = lintStoreMetadata(
    /** @type {"stable" | "canary" | "all"} */ (args.channel),
    args.metainfoPath,
    args.wingetDir,
  );
  if (args.json) {
    console.log(JSON.stringify(report, null, 2));
  } else if (report.ok) {
    console.log(
      `lint-store-metadata: PASS (flathub + winget [${Object.keys(report.channels).join(", ")}])`,
    );
  } else {
    console.error("lint-store-metadata: FAIL", JSON.stringify(report, null, 2));
  }
  if (!report.ok) {
    process.exit(1);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
