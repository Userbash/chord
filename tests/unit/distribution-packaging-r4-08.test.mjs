// @ts-check
/**
 * R4-08: Distribution Packaging — Cross-platform release pipeline contract tests.
 * Covers: Debian/Ubuntu .deb, RPM, AppImage, Flatpak, Windows WiX MSI, macOS DMG;
 * desktop entries, icons, MIME types, and release pipeline verification.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const PACKAGE_DIST_SCRIPT = join(ROOT, "scripts", "package-dist.mjs");
const SHA256SUMS_PATH = join(ROOT, "rust", "dist", "SHA256SUMS");
const FLATPAK_MANIFEST = join(ROOT, "packaging", "flatpak", "com.discord_lab.DiscordGpui.yml");
const WIX_MANIFEST = join(ROOT, "packaging", "windows", "wix.xml");
const DMG_SCRIPT = join(ROOT, "packaging", "macos", "build-dmg.sh");
const DEB_DESKTOP = join(ROOT, "packaging", "deb", "usr", "share", "applications", "discord-gpui.desktop");
const DEB_MIME = join(ROOT, "packaging", "deb", "usr", "share", "mime", "packages", "discord-gpui.xml");
const DEB_ICON = join(ROOT, "packaging", "deb", "usr", "share", "icons", "hicolor", "scalable", "apps", "discord-gpui.svg");
const RPM_SPEC = join(ROOT, "packaging", "rpm", "discord-gpui.spec");
const APPIMAGE_DESKTOP = join(ROOT, "packaging", "appimage", "discord-gpui.desktop");
const APPIMAGE_ICON = join(ROOT, "packaging", "appimage", "discord-gpui.svg");

/** @param {string} content */
function parseSha256Sums(content) {
  /** @type {Map<string, string>} */
  const map = new Map();
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const match = /^([a-f0-9]{64})\s+\*?(.+)$/.exec(trimmed);
    if (match?.[1] && match?.[2]) {
      map.set(match[2].trim(), match[1]);
    }
  }
  return map;
}

/** @type {string} */
let workDir = "";

before(() => {
  workDir = mkdtempSync(join(tmpdir(), "r4-08-dist-"));
});

after(() => {
  if (workDir && existsSync(workDir)) {
    rmSync(workDir, { recursive: true, force: true });
  }
});

describe("R4-08: Cross-platform Distribution Packaging", () => {
  it("SUB-R408-1: scripts/package-dist.mjs exists and verifies all 6 platform targets (--verify-only)", () => {
    assert.ok(existsSync(PACKAGE_DIST_SCRIPT), "scripts/package-dist.mjs must exist");
    const reportPath = join(workDir, "dist-report.json");
    const res = spawnSync(
      "node",
      [PACKAGE_DIST_SCRIPT, "--verify-only", "--report-json", reportPath],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(res.status, 0, `package-dist.mjs --verify-only failed:\n${res.stderr}\n${res.stdout}`);
    assert.ok(existsSync(reportPath), "dist-report.json must be written");
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    assert.equal(report.ok, true);
    for (const target of ["deb", "rpm", "appimage", "flatpak", "wix-msi", "macos-dmg"]) {
      assert.equal(report.targets?.[target]?.ok, true, `Target ${target} must pass verification`);
    }
  });

  it("SUB-R408-2: Flatpak manifest syncs sbom.spdx.json SHA-256 and installs desktop entry, icon, and MIME XML", () => {
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const expectedSbomHash = sums.get("sbom.spdx.json");
    assert.ok(expectedSbomHash, "sbom.spdx.json must exist in rust/dist/SHA256SUMS");

    const flatpakRaw = readFileSync(FLATPAK_MANIFEST, "utf8");
    assert.ok(
      flatpakRaw.includes(expectedSbomHash),
      `Flatpak manifest must reference current sbom.spdx.json sha256 (${expectedSbomHash})`,
    );
    assert.match(
      flatpakRaw,
      /\/app\/share\/applications\/com\.discord_lab\.DiscordGpui\.desktop/,
      "Flatpak must install /app/share/applications/com.discord_lab.DiscordGpui.desktop",
    );
    assert.match(
      flatpakRaw,
      /\/app\/share\/icons\/hicolor\/scalable\/apps\/com\.discord_lab\.DiscordGpui\.svg/,
      "Flatpak must install /app/share/icons/hicolor/scalable/apps/com.discord_lab.DiscordGpui.svg",
    );
    assert.match(
      flatpakRaw,
      /\/app\/share\/mime\/packages\/com\.discord_lab\.DiscordGpui\.xml/,
      "Flatpak must install /app/share/mime/packages/com.discord_lab.DiscordGpui.xml",
    );
  });

  it("SUB-R408-3: Windows WiX MSI syncs SBOM_SHA256 and registers URI schemes, .discord-invite MIME, icon, and shortcut", () => {
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const expectedSbomHash = sums.get("sbom.spdx.json");
    assert.ok(expectedSbomHash, "sbom.spdx.json must exist in rust/dist/SHA256SUMS");

    const wixRaw = readFileSync(WIX_MANIFEST, "utf8");
    assert.ok(
      wixRaw.includes(`Id="SBOM_SHA256" Value="${expectedSbomHash}"`),
      `wix.xml SBOM_SHA256 must match rust/dist/SHA256SUMS (${expectedSbomHash})`,
    );
    assert.match(wixRaw, /Software\\Classes\\discord/, "wix.xml must register discord:// URI scheme");
    assert.match(wixRaw, /Software\\Classes\\disclip/, "wix.xml must register disclip:// URI scheme");
    assert.match(wixRaw, /\.discord-invite/, "wix.xml must register .discord-invite file association");
    assert.match(wixRaw, /application\/x-discord-gpui-invite/, "wix.xml must register application/x-discord-gpui-invite MIME type");
    assert.match(wixRaw, /DiscordGpuiIcon\.ico/, "wix.xml must configure ARPPRODUCTICON / shortcut icon");
  });

  it("SUB-R408-4: macOS DMG script generates Info.plist with CFBundleIconFile, CFBundleURLTypes, and CFBundleDocumentTypes", () => {
    const dmgRaw = readFileSync(DMG_SCRIPT, "utf8");
    assert.match(dmgRaw, /CFBundleIconFile/, "build-dmg.sh must set CFBundleIconFile");
    assert.match(dmgRaw, /CFBundleURLTypes/, "build-dmg.sh must set CFBundleURLTypes");
    assert.match(dmgRaw, /CFBundleDocumentTypes/, "build-dmg.sh must set CFBundleDocumentTypes for .discord-invite");
    assert.match(dmgRaw, /discord-invite/, "build-dmg.sh must register discord-invite extension");
    assert.match(dmgRaw, /application\/x-discord-gpui-invite/, "build-dmg.sh must register application/x-discord-gpui-invite MIME type");

    const macOut = join(workDir, "macos-out");
    const res = spawnSync("bash", [DMG_SCRIPT, "--output", macOut, "--arch", "universal"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(res.status, 0, `build-dmg.sh failed:\n${res.stderr}\n${res.stdout}`);

    const verify = spawnSync("bash", [DMG_SCRIPT, "--output", macOut, "--arch", "universal", "--verify"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(verify.status, 0, `build-dmg.sh --verify failed:\n${verify.stderr}\n${verify.stdout}`);
  });

  it("SUB-R408-5: Linux desktop entries, icons, and MIME types are consistent across .deb, .rpm, and AppImage", () => {
    for (const file of [DEB_DESKTOP, DEB_MIME, DEB_ICON, RPM_SPEC, APPIMAGE_DESKTOP, APPIMAGE_ICON]) {
      assert.ok(existsSync(file), `Required Linux packaging asset must exist: ${file}`);
    }
    const debDesktop = readFileSync(DEB_DESKTOP, "utf8");
    const debMime = readFileSync(DEB_MIME, "utf8");
    const rpmSpec = readFileSync(RPM_SPEC, "utf8");
    const appImageDesktop = readFileSync(APPIMAGE_DESKTOP, "utf8");

    assert.match(debDesktop, /MimeType=.*x-scheme-handler\/discord/);
    assert.match(debDesktop, /MimeType=.*x-scheme-handler\/disclip/);
    assert.match(debDesktop, /MimeType=.*application\/x-discord-gpui-invite/);
    assert.match(debMime, /application\/x-discord-gpui-invite/);
    assert.match(debMime, /\*\.discord-invite/);
    assert.match(rpmSpec, /MimeType=x-scheme-handler\/discord;x-scheme-handler\/disclip/);
    assert.match(appImageDesktop, /MimeType=x-scheme-handler\/discord/);
  });

  it("SUB-R408-6: scripts/package-dist.mjs rejects invalid platform flags with exit code 1", () => {
    assert.ok(existsSync(PACKAGE_DIST_SCRIPT), "scripts/package-dist.mjs must exist");
    const res = spawnSync("node", [PACKAGE_DIST_SCRIPT, "--platform", "invalid-os"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(res.status, 1);
  });
});
