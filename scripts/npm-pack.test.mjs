// Tests for scripts/npm-pack.mjs. Run them with
// `node --test scripts/npm-pack.test.mjs`. They pack stand-in binaries into
// scratch folders and never run a real one: `ask` is replaced where the
// script would start the host's binary.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import platforms from "../npm/fogline/lib/platforms.js";
import {
  launcherManifest,
  PackError,
  pack,
  platformManifest,
  removeScratch,
  workspaceVersion,
} from "./npm-pack.mjs";

const { PLATFORMS, binaryName, findPlatform, packageName } = platforms;

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const VERSION = workspaceVersion(fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8"));
const HOST = findPlatform(process.platform, process.arch);
const ON_WINDOWS = process.platform === "win32";

// Windows cannot pack the Unix binaries (it would lose their executable
// bit), so there the tests pack the Windows package alone.
const PACKABLE = ON_WINDOWS ? PLATFORMS.filter((platform) => platform.os === "win32") : PLATFORMS;
const ONE = HOST && PACKABLE.includes(HOST) ? HOST : PACKABLE[0];

const TEMPLATE = {
  name: "fogline",
  private: true,
  homepage: "https://example.test/fogline",
  repository: { type: "git", url: "git+https://example.test/fogline.git" },
  license: "MIT",
  bin: { fogline: "bin/fogline.js" },
  scripts: { test: "node --test" },
};

/** A scratch folder that is removed when the test ends. */
function scratch(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "fogline-pack-test-"));
  t.after(() => removeScratch(dir));
  return dir;
}

/**
 * Writes a stand-in binary for each platform, the way a downloaded build
 * artifact arrives: in a folder named after the target, and not executable.
 */
function fakeArtifacts(dir, wanted) {
  for (const platform of wanted) {
    const folder = path.join(dir, `fogline-${platform.target}`);
    fs.mkdirSync(folder, { recursive: true });
    fs.writeFileSync(path.join(folder, binaryName(platform)), `binary for ${platform.target}\n`, {
      mode: 0o644,
    });
  }
  return dir;
}

/** Stands in for running the host's binary, and reports what a good one would. */
const asExpected = () => ({ version: VERSION, target: HOST?.target });

const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
const isExecutable = (file) => (fs.statSync(file).mode & 0o111) === 0o111;

test("the version comes from [workspace.package] in Cargo.toml", () => {
  assert.match(VERSION, /^\d+\.\d+\.\d+/);

  const cargoToml = [
    "[workspace]",
    'members = ["a"]',
    "",
    "[workspace.package]",
    'edition = "2024"',
    'version = "1.2.3"',
    "",
    "[workspace.dependencies]",
    'version = "9.9.9"',
  ].join("\n");
  assert.equal(workspaceVersion(cargoToml), "1.2.3");

  assert.throws(() => workspaceVersion('[package]\nversion = "1.0.0"\n'), PackError);
  assert.throws(
    () => workspaceVersion('[workspace.package]\nedition = "2024"\n[other]\nversion = "1.0.0"\n'),
    PackError,
  );
});

test("the published launcher pins every platform package to its own version", () => {
  const manifest = launcherManifest(TEMPLATE, "1.2.3");

  assert.equal(manifest.name, "fogline");
  assert.equal(manifest.version, "1.2.3");
  assert.deepEqual(manifest.bin, TEMPLATE.bin);
  assert.deepEqual(manifest.optionalDependencies, {
    "@fogline/cli-darwin-arm64": "1.2.3",
    "@fogline/cli-darwin-x64": "1.2.3",
    "@fogline/cli-linux-x64": "1.2.3",
    "@fogline/cli-linux-arm64": "1.2.3",
    "@fogline/cli-win32-x64": "1.2.3",
  });
  // What only the copy in the repository needs is left out.
  assert.equal("private" in manifest, false);
  assert.equal("scripts" in manifest, false);
});

test("a platform package installs only on its own os and cpu", () => {
  const manifest = platformManifest(findPlatform("linux", "arm64"), TEMPLATE, "1.2.3");

  assert.equal(manifest.name, "@fogline/cli-linux-arm64");
  assert.equal(manifest.version, "1.2.3");
  assert.deepEqual(manifest.os, ["linux"]);
  assert.deepEqual(manifest.cpu, ["arm64"]);
  assert.deepEqual(manifest.repository, TEMPLATE.repository);
  assert.equal(manifest.license, "MIT");
  // The launcher owns the `fogline` command. A second `bin` would fight it.
  assert.equal("bin" in manifest, false);
});

test("packs the launcher and all five platform packages", { skip: ON_WINDOWS }, (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), PLATFORMS);
  const out = path.join(dir, "out");

  const result = pack({ artifacts, out, ask: asExpected });

  // Two groups, so a publisher can take the platform packages first.
  assert.deepEqual(fs.readdirSync(out).sort(), ["launcher", "platform"]);
  assert.deepEqual(fs.readdirSync(path.join(out, "launcher")), ["fogline"]);
  assert.deepEqual(fs.readdirSync(path.join(out, "platform")).sort(), [
    "cli-darwin-arm64",
    "cli-darwin-x64",
    "cli-linux-arm64",
    "cli-linux-x64",
    "cli-win32-x64",
  ]);
  assert.match(result.report, /Wrote 6 packages to /);

  for (const platform of PLATFORMS) {
    const folder = path.join(out, "platform", `cli-${platform.os}-${platform.cpu}`);
    const binary = path.join(folder, "bin", binaryName(platform));
    assert.deepEqual(readJson(path.join(folder, "package.json")).name, packageName(platform));
    assert.equal(readJson(path.join(folder, "package.json")).version, VERSION);
    assert.equal(fs.readFileSync(binary, "utf8"), `binary for ${platform.target}\n`);
    assert.ok(isExecutable(binary), `${binary} should be executable`);
    assert.ok(fs.existsSync(path.join(folder, "README.md")));
    assert.ok(fs.existsSync(path.join(folder, "LICENSE")));
  }

  const launcher = path.join(out, "launcher", "fogline");
  const manifest = readJson(path.join(launcher, "package.json"));
  assert.equal(manifest.version, VERSION);
  assert.equal(Object.keys(manifest.optionalDependencies).length, 5);
  assert.ok(isExecutable(path.join(launcher, "bin", "fogline.js")));
  assert.ok(fs.existsSync(path.join(launcher, "lib", "launcher.js")));
  assert.ok(fs.existsSync(path.join(launcher, "lib", "platforms.js")));
  assert.ok(fs.existsSync(path.join(launcher, "README.md")));
  assert.deepEqual(
    fs.readFileSync(path.join(launcher, "LICENSE")),
    fs.readFileSync(path.join(ROOT, "LICENSE")),
  );
  // The tests stay in the repository.
  assert.equal(fs.existsSync(path.join(launcher, "test")), false);

  // The input is left as it arrived.
  const input = path.join(artifacts, `fogline-${PLATFORMS[0].target}`, binaryName(PLATFORMS[0]));
  assert.equal(isExecutable(input), false);
});

test("packs only what is present when missing binaries are allowed", (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), [ONE]);
  const out = path.join(dir, "out");

  const result = pack({ artifacts, out, allowMissing: true, ask: asExpected });

  assert.deepEqual(fs.readdirSync(path.join(out, "platform")), [`cli-${ONE.os}-${ONE.cpu}`]);
  assert.deepEqual(fs.readdirSync(path.join(out, "launcher")), ["fogline"]);
  assert.match(result.report, /Wrote 2 packages to /);
  assert.match(result.report, /skipped: no fogline-/);
  // The launcher still names all five, so it is the same file a release ships.
  const manifest = readJson(path.join(out, "launcher", "fogline", "package.json"));
  assert.equal(Object.keys(manifest.optionalDependencies).length, 5);
});

test("a missing binary is an error unless it is allowed", (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), [ONE]);
  const out = path.join(dir, "out");
  const absent = PLATFORMS.find((platform) => platform !== ONE);

  assert.throws(
    () => pack({ artifacts, out, ask: asExpected }),
    (error) =>
      error instanceof PackError &&
      error.message.includes("are missing") &&
      error.message.includes(`fogline-${absent.target}/${binaryName(absent)}`),
  );
  assert.equal(fs.existsSync(out), false);
});

test("a folder with no binary at all is an error even when missing ones are allowed", (t) => {
  const dir = scratch(t);
  const artifacts = path.join(dir, "artifacts");
  fs.mkdirSync(artifacts);

  assert.throws(
    () => pack({ artifacts, out: path.join(dir, "out"), allowMissing: true, ask: asExpected }),
    (error) => error instanceof PackError && error.message.includes("no fogline binary at all"),
  );
});

test("a dry run lists the packages and writes nothing", (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), [ONE]);
  const out = path.join(dir, "out");

  const withArtifacts = pack({ artifacts, out, allowMissing: true, dryRun: true, ask: asExpected });
  assert.match(withArtifacts.report, /Dry run: nothing was written\./);
  assert.equal(fs.existsSync(out), false);

  // With no artifacts it still checks the manifests and lists all six.
  const bare = pack({ dryRun: true });
  for (const name of ["fogline", ...PLATFORMS.map(packageName)]) {
    assert.ok(bare.report.includes(`  ${name} `), `${name} should be listed`);
  }
  assert.match(bare.report, /no artifacts given/);
});

test("a release whose tag is not the version in Cargo.toml is refused", () => {
  assert.match(pack({ dryRun: true, expectVersion: VERSION }).report, /Dry run/);
  assert.throws(
    () => pack({ dryRun: true, expectVersion: "99.0.0" }),
    (error) => error instanceof PackError && error.message.includes("version 99.0.0 was expected"),
  );
});

test("without a dry run, both folders have to be named", () => {
  assert.throws(() => pack({}), PackError);
  assert.throws(() => pack({ artifacts: "somewhere" }), PackError);
});

test("an output folder that already holds files is refused", (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), [ONE]);
  const out = path.join(dir, "out");
  fs.mkdirSync(out);
  fs.writeFileSync(path.join(out, "stale.txt"), "left over\n");

  assert.throws(
    () => pack({ artifacts, out, allowMissing: true, ask: asExpected }),
    (error) => error instanceof PackError && error.message.includes("is not empty"),
  );
  assert.deepEqual(fs.readdirSync(out), ["stale.txt"]);
});

test("a host binary with the wrong version or target stops the pack", { skip: !HOST }, (t) => {
  const dir = scratch(t);
  const artifacts = fakeArtifacts(path.join(dir, "artifacts"), [HOST]);
  const out = path.join(dir, "out");
  const packWith = (report) => () =>
    pack({ artifacts, out, allowMissing: true, ask: () => report });

  assert.throws(
    packWith({ version: "9.9.9", target: HOST.target }),
    (error) => error instanceof PackError && error.message.includes("reports version 9.9.9"),
  );
  assert.throws(
    packWith({ version: VERSION, target: "wasm32-unknown-unknown" }),
    (error) =>
      error instanceof PackError && error.message.includes("built for wasm32-unknown-unknown"),
  );
  assert.equal(fs.existsSync(out), false);
});

test("the script still runs when the checkout is reached through a symlink", {
  skip: ON_WINDOWS,
}, (t) => {
  // Node follows symlinks for a module's own address and not for the path it
  // was started with. A guard that compared the two as given would decide
  // this file was only being imported, do nothing, and exit with 0.
  const dir = scratch(t);
  const link = path.join(dir, "checkout");
  fs.symlinkSync(ROOT, link);

  const output = execFileSync(
    process.execPath,
    [path.join(link, "scripts", "npm-pack.mjs"), "--dry-run"],
    { encoding: "utf8" },
  );

  assert.match(output, /Dry run: nothing was written\./);
});

test("the workflows and deny.toml list the same five targets as the platform table", () => {
  // These files cannot read the table, so they repeat it. This test is what
  // keeps the copies from drifting.
  const expected = PLATFORMS.map((platform) => platform.target).sort();
  const triple = /\b(?:aarch64|x86_64)-[a-z0-9_]+-[a-z0-9_]+(?:-[a-z0-9_]+)?\b/g;

  for (const file of [".github/workflows/ci.yml", ".github/workflows/release.yml", "deny.toml"]) {
    const text = fs.readFileSync(path.join(ROOT, file), "utf8");
    assert.deepEqual([...new Set(text.match(triple))].sort(), expected, file);
  }
});
