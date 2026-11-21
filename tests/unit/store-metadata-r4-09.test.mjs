// @ts-check
/**
 * R4-09: Store Metadata — Release Pipeline Contract Tests.
 * Covers:
 *   1. Flathub AppStream metainfo.xml (validates with appstreamcli + xmllint + Flatpak integration)
 *   2. Windows Winget 1.6.0 manifests for stable and canary channels (YAML + schema validation)
 *   3. Store metadata linter script (scripts/lint-store-metadata.mjs) positive & mutation tests
 *   4. Build-time release channel selection (scripts/release.sh, scripts/package-dist.mjs)
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const FLATPAK_MANIFEST = join(ROOT, "packaging", "flatpak", "com.discord_lab.DiscordGpui.yml");
const FLATHUB_METAINFO = join(
  ROOT,
  "packaging",
  "flatpak",
  "com.discord_lab.DiscordGpui.metainfo.xml",
);
const WINGET_DIR = join(ROOT, "packaging", "windows", "winget");
const LINT_SCRIPT = join(ROOT, "scripts", "lint-store-metadata.mjs");
const RELEASE_SCRIPT = join(ROOT, "scripts", "release.sh");
const PACKAGE_DIST_SCRIPT = join(ROOT, "scripts", "package-dist.mjs");

/** @type {string} */
let workDir = "";

before(() => {
  workDir = mkdtempSync(join(tmpdir(), "r4-09-store-meta-"));
});

after(() => {
  if (workDir && existsSync(workDir)) {
    rmSync(workDir, { recursive: true, force: true });
  }
});

/**
 * Parse YAML via Python PyYAML oracle.
 * @param {string} yamlPath
 * @returns {any}
 */
function parseYamlOracle(yamlPath) {
  const py = [
    "import json, sys, yaml",
    "with open(sys.argv[1], 'r', encoding='utf-8') as f:",
    "    print(json.dumps(yaml.safe_load(f)))",
  ].join("\n");
  const res = spawnSync("python3", ["-c", py, yamlPath], { encoding: "utf8" });
  assert.equal(res.status, 0, `PyYAML failed on ${yamlPath}:\n${res.stderr}`);
  return JSON.parse(res.stdout);
}

describe("R4-09: Store Metadata & Build-Time Release Channels", () => {
  it("SUB-R409-1: Flathub metainfo.xml exists, passes xmllint and appstreamcli validate, and is installed in Flatpak manifest", () => {
    assert.ok(existsSync(FLATHUB_METAINFO), `Missing Flathub metainfo: ${FLATHUB_METAINFO}`);

    const xmlRes = spawnSync("xmllint", ["--noout", FLATHUB_METAINFO], { encoding: "utf8" });
    assert.equal(xmlRes.status, 0, `xmllint failed:\n${xmlRes.stderr}`);

    const appstreamRes = spawnSync(
      "appstreamcli",
      ["validate", "--no-net", "--pedantic", FLATHUB_METAINFO],
      { encoding: "utf8" },
    );
    assert.equal(
      appstreamRes.status,
      0,
      `appstreamcli validate failed:\n${appstreamRes.stdout}\n${appstreamRes.stderr}`,
    );

    const flatpakRaw = readFileSync(FLATPAK_MANIFEST, "utf8");
    assert.match(
      flatpakRaw,
      /\/app\/share\/metainfo\/com\.discord_lab\.DiscordGpui\.metainfo\.xml/,
      "Flatpak manifest must install metainfo.xml to /app/share/metainfo/",
    );
  });

  it("SUB-R409-2: Windows Winget 1.6.0 manifests exist for both stable and canary channels and pass PyYAML + schema checks", () => {
    const channelSpecs = [
      { channel: "stable", id: "DiscordLab.DiscordGpui" },
      { channel: "canary", id: "DiscordLab.DiscordGpui.Canary" },
    ];

    for (const { channel, id } of channelSpecs) {
      const dir = join(WINGET_DIR, channel);
      const versionFile = join(dir, `${id}.yaml`);
      const installerFile = join(dir, `${id}.installer.yaml`);
      const localeFile = join(dir, `${id}.locale.en-US.yaml`);

      for (const f of [versionFile, installerFile, localeFile]) {
        assert.ok(existsSync(f), `Missing Winget ${channel} manifest: ${f}`);
      }

      const ver = parseYamlOracle(versionFile);
      const inst = parseYamlOracle(installerFile);
      const loc = parseYamlOracle(localeFile);

      assert.equal(ver.PackageIdentifier, id);
      assert.equal(inst.PackageIdentifier, id);
      assert.equal(loc.PackageIdentifier, id);

      assert.ok(typeof ver.PackageVersion === "string" && ver.PackageVersion.length > 0);
      assert.equal(inst.PackageVersion, ver.PackageVersion);
      assert.equal(loc.PackageVersion, ver.PackageVersion);

      assert.equal(ver.ManifestType, "version");
      assert.equal(inst.ManifestType, "installer");
      assert.equal(loc.ManifestType, "defaultLocale");

      assert.equal(ver.ManifestVersion, "1.6.0");
      assert.equal(inst.ManifestVersion, "1.6.0");
      assert.equal(loc.ManifestVersion, "1.6.0");

      assert.ok(Array.isArray(inst.Installers) && inst.Installers.length > 0);
      for (const entry of inst.Installers) {
        assert.match(String(entry.InstallerUrl), /^https:\/\//);
        assert.match(String(entry.InstallerSha256), /^[A-Fa-f0-9]{64}$/);
      }
    }
  });

  it("SUB-R409-3: scripts/lint-store-metadata.mjs passes on repo manifests and rejects mutated invalid manifests", () => {
    assert.ok(existsSync(LINT_SCRIPT), `Missing linter script: ${LINT_SCRIPT}`);

    const passRes = spawnSync("node", [LINT_SCRIPT, "--channel", "all", "--json"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(
      passRes.status,
      0,
      `lint-store-metadata.mjs failed:\n${passRes.stderr}\n${passRes.stdout}`,
    );
    const report = JSON.parse(passRes.stdout);
    assert.equal(report.ok, true);
    assert.equal(report.channels.stable.ok, true);
    assert.equal(report.channels.canary.ok, true);
    assert.equal(report.flathub.ok, true);

    // Negative / mutation test: invalid SHA-256 in Winget installer manifest must fail linter
    const mutatedWinget = join(workDir, "winget-mutated");
    cpSync(WINGET_DIR, mutatedWinget, { recursive: true });
    const badInstallerPath = join(
      mutatedWinget,
      "stable",
      "DiscordLab.DiscordGpui.installer.yaml",
    );
    const mutatedText = readFileSync(badInstallerPath, "utf8").replace(
      /InstallerSha256:\s*[A-Fa-f0-9]{64}/,
      "InstallerSha256: not-a-valid-sha256",
    );
    writeFileSync(badInstallerPath, mutatedText, "utf8");

    const failRes = spawnSync(
      "node",
      [LINT_SCRIPT, "--channel", "stable", "--winget-dir", mutatedWinget],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(failRes.status, 1, "Linter must reject invalid InstallerSha256 with exit code 1");

    // Negative test: invalid channel argument must fail
    const badChanRes = spawnSync("node", [LINT_SCRIPT, "--channel", "invalid-chan"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(badChanRes.status, 1);
  });

  it("SUB-R409-4: release channel (stable, canary) is selectable at build time in scripts/release.sh and scripts/package-dist.mjs", () => {
    for (const channel of ["stable", "canary"]) {
      const relRes = spawnSync("bash", [RELEASE_SCRIPT, "--dry-run", "--channel", channel], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(
        relRes.status,
        0,
        `release.sh --dry-run --channel ${channel} failed:\n${relRes.stderr}`,
      );
      assert.match(
        relRes.stdout,
        new RegExp(`CHORD_RELEASE_CHANNEL=${channel}`),
        `release.sh must export CHORD_RELEASE_CHANNEL=${channel} for build-time selection`,
      );

      const reportPath = join(workDir, `pkg-${channel}.json`);
      const pkgRes = spawnSync(
        "node",
        [
          PACKAGE_DIST_SCRIPT,
          "--verify-only",
          "--channel",
          channel,
          "--report-json",
          reportPath,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        pkgRes.status,
        0,
        `package-dist.mjs --channel ${channel} failed:\n${pkgRes.stderr}\n${pkgRes.stdout}`,
      );
      const pkgReport = JSON.parse(readFileSync(reportPath, "utf8"));
      assert.equal(pkgReport.ok, true);
      assert.equal(pkgReport.channel, channel);
      assert.equal(pkgReport.storeMetadata?.ok, true);
    }

    // Invalid channel rejection
    const badRel = spawnSync("bash", [RELEASE_SCRIPT, "--dry-run", "--channel", "nightly"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(badRel.status, 1, "release.sh must reject invalid release channel");

    const badPkg = spawnSync(
      "node",
      [PACKAGE_DIST_SCRIPT, "--verify-only", "--channel", "nightly"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(badPkg.status, 1, "package-dist.mjs must reject invalid release channel");
  });
});
