#!/usr/bin/env node
// Turns built fogline binaries into the folders that are published to npm:
// the launcher, and one package per platform holding that platform's binary.
//
//   node scripts/npm-pack.mjs --artifacts <dir> --out <dir>
//
// It publishes nothing. `npm pack` or `npm publish` runs on each folder
// afterwards. The contract is in docs/specs/phase-1.md, section 5.8.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

// The launcher's own table of platforms. Reading it here means the launcher
// and the packages cannot disagree about names.
import platforms from "../npm/fogline/lib/platforms.js";

const { PLATFORMS, binaryName, findPlatform, packageName } = platforms;

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const LAUNCHER = path.join(ROOT, "npm", "fogline");

const USAGE = `\
Usage: node scripts/npm-pack.mjs --artifacts <dir> --out <dir> [options]

  --artifacts <dir>  Folder with one sub-folder per target, named
                     fogline-<target>, each holding that target's binary.
  --out <dir>        Where the publishable folders go. Must be new or empty.
  --allow-missing    Pack only the platforms whose binary is present.
                     Without it, a missing binary is an error.
  --dry-run          Run every check and list what would be written, but
                     write nothing. Works without --artifacts and --out.
  --expect-version <version>
                     Fail unless Cargo.toml has this version. A release
                     passes the version from its tag.
`;

/** A problem with the input, reported as one line and without a stack trace. */
export class PackError extends Error {}

/** The version every package gets: the workspace version in Cargo.toml. */
export function workspaceVersion(cargoToml) {
  const lines = cargoToml.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === "[workspace.package]");
  if (start === -1) {
    throw new PackError("Cargo.toml has no [workspace.package] section.");
  }
  for (const line of lines.slice(start + 1)) {
    if (line.startsWith("[")) {
      break;
    }
    const match = /^version\s*=\s*"([^"]+)"/.exec(line);
    if (match) {
      return match[1];
    }
  }
  throw new PackError("Cargo.toml has no version under [workspace.package].");
}

/**
 * The launcher's package.json as it is published.
 *
 * The file in the repository is a template. It is marked private, so nobody
 * can publish that folder by mistake: it has no version and no binaries.
 */
export function launcherManifest(template, version) {
  const { name, private: _private, scripts: _scripts, ...rest } = template;
  return {
    name,
    version,
    ...rest,
    // Pinned to the exact version, so a launcher always runs the binary it
    // was released with.
    optionalDependencies: Object.fromEntries(
      PLATFORMS.map((platform) => [packageName(platform), version]),
    ),
  };
}

/** The package.json of one platform package. */
export function platformManifest(platform, template, version) {
  return {
    name: packageName(platform),
    version,
    description: `The fogline binary for ${platform.os}-${platform.cpu}. Install "${template.name}" instead; it picks the package that fits the machine.`,
    homepage: template.homepage,
    bugs: template.bugs,
    repository: template.repository,
    license: template.license,
    // npm installs the package only where both of these match.
    os: [platform.os],
    cpu: [platform.cpu],
    // Yarn's Plug'n'Play keeps packages zipped, and a binary cannot run from
    // inside a zip. This asks Yarn to unpack the package.
    preferUnplugged: true,
  };
}

function platformReadme(platform, template) {
  const name = packageName(platform);
  return [
    `# ${name}`,
    "",
    `The [${template.name}](https://www.npmjs.com/package/${template.name}) binary for ${platform.os}-${platform.cpu} (\`${platform.target}\`).`,
    "",
    `Do not install this package directly. Install \`${template.name}\`, and npm adds the package that fits the machine.`,
    "",
  ].join("\n");
}

/** Every file under `dir`, as sorted paths relative to it, with forward slashes. */
function filesUnder(dir) {
  return fs
    .readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.relative(dir, path.join(entry.parentPath, entry.name)))
    .map((file) => file.split(path.sep).join("/"))
    .sort();
}

/**
 * Works out what to write, and writes nothing.
 *
 * Returns one entry per package: `{ name, dir, manifest, files }`, or
 * `{ name, skipped }` for a platform whose binary is absent. A file is
 * `{ to, from }` to copy or `{ to, text }` to write, with `executable` set
 * where the result has to be runnable.
 */
export function plan({ version, template, artifacts, allowMissing }) {
  const licence = { to: "LICENSE", from: path.join(ROOT, "LICENSE") };
  const binEntries = new Set(Object.values(template.bin));

  const launcherFiles = template.files.flatMap((entry) =>
    filesUnder(path.join(LAUNCHER, entry)).map((file) => {
      const to = path.posix.join(entry, file);
      return { to, from: path.join(LAUNCHER, to), executable: binEntries.has(to) };
    }),
  );
  // The folders are sorted into `platform/` and `launcher/`, so that whatever
  // publishes them can take the platform packages first without knowing any
  // package's name.
  const packages = [
    {
      name: template.name,
      dir: `launcher/${template.name}`,
      manifest: launcherManifest(template, version),
      files: [
        ...launcherFiles,
        { to: "README.md", from: path.join(LAUNCHER, "README.md") },
        licence,
      ],
    },
  ];

  const missing = [];
  for (const platform of PLATFORMS) {
    const name = packageName(platform);
    const binary = binaryName(platform);
    const folder = `fogline-${platform.target}`;
    const source = artifacts ? path.join(artifacts, folder, binary) : null;

    if (source && !fs.existsSync(source)) {
      missing.push(`${folder}/${binary}`);
      packages.push({ name, skipped: `no ${folder}/${binary}` });
      continue;
    }
    // Windows cannot record the executable bit, so a Unix binary packed
    // there would install as a file that cannot be run.
    if (source && process.platform === "win32" && platform.os !== "win32") {
      throw new PackError(
        `${name} holds a Unix binary and cannot be packed on Windows, which would lose its executable bit. Pack it on macOS or Linux.`,
      );
    }
    packages.push({
      name,
      dir: `platform/cli-${platform.os}-${platform.cpu}`,
      platform,
      manifest: platformManifest(platform, template, version),
      files: [
        { to: `bin/${binary}`, from: source, executable: true },
        { to: "README.md", text: platformReadme(platform, template) },
        licence,
      ],
    });
  }

  if (missing.length > 0 && !allowMissing) {
    throw new PackError(
      `these binaries are missing from ${artifacts}: ${missing.join(", ")}. Pass --allow-missing to pack the rest.`,
    );
  }
  if (artifacts && missing.length === PLATFORMS.length) {
    throw new PackError(`${artifacts} holds no fogline binary at all.`);
  }
  return packages;
}

/**
 * Removes a scratch folder, and never fails the caller over it.
 *
 * On Windows a file can stay locked for a moment after the program that used
 * it has ended. The removal is retried, and a folder that still cannot be
 * removed is left behind with a warning: tidying up must not turn a run that
 * passed into one that failed.
 */
export function removeScratch(dir) {
  try {
    fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  } catch (error) {
    console.warn(`Could not remove ${dir}: ${error.message}`);
  }
}

/** Runs `version --json` on a binary and returns what it reports. */
function askBinary(binary) {
  // Downloaded build artifacts lose their executable bit, so run a private
  // copy and leave the input as it was.
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "fogline-pack-"));
  try {
    const copy = path.join(scratch, path.basename(binary));
    fs.copyFileSync(binary, copy);
    fs.chmodSync(copy, 0o755);
    const result = spawnSync(copy, ["version", "--json"], { encoding: "utf8" });
    if (result.error) {
      throw new PackError(`could not run ${binary}: ${result.error.message}`);
    }
    if (result.status !== 0) {
      throw new PackError(`\`${binary} version --json\` failed: ${result.stderr.trim()}`);
    }
    return JSON.parse(result.stdout);
  } finally {
    removeScratch(scratch);
  }
}

/**
 * Runs the one binary this machine can run and checks what it says.
 *
 * The other four were built for other machines, so this is the only place
 * where the version in Cargo.toml and a real binary are compared.
 */
function checkHostBinary(packages, version, ask) {
  const host = findPlatform(process.platform, process.arch);
  const pkg = host && packages.find((candidate) => candidate.platform === host);
  const binary = pkg?.files.find((file) => file.executable)?.from;
  if (!binary) {
    return "Host binary: none among the artifacts, so none was run.";
  }

  const report = ask(binary);
  if (report.target !== host.target) {
    throw new PackError(
      `the binary in fogline-${host.target} says it was built for ${report.target}.`,
    );
  }
  if (report.version !== version) {
    throw new PackError(
      `the binary for ${host.target} reports version ${report.version}, but Cargo.toml says ${version}. Rebuild it.`,
    );
  }
  return `Host binary: version ${report.version} for ${report.target}, as expected.`;
}

function writePackage(pkg, out) {
  const dir = path.join(out, pkg.dir);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, "package.json"), `${JSON.stringify(pkg.manifest, null, 2)}\n`);
  for (const file of pkg.files) {
    const to = path.join(dir, file.to);
    fs.mkdirSync(path.dirname(to), { recursive: true });
    if (file.from) {
      fs.copyFileSync(file.from, to);
    } else {
      fs.writeFileSync(to, file.text);
    }
    if (file.executable) {
      fs.chmodSync(to, 0o755);
    }
  }
}

/**
 * Checks everything, then writes the folders unless this is a dry run.
 *
 * `ask` runs a binary and returns its `version --json` report. The tests
 * replace it.
 */
export function pack({
  artifacts,
  out,
  allowMissing = false,
  dryRun = false,
  expectVersion,
  ask = askBinary,
}) {
  if (!dryRun && !(artifacts && out)) {
    throw new PackError("--artifacts and --out are both needed, unless this is a --dry-run.");
  }
  if (out && fs.existsSync(out) && fs.readdirSync(out).length > 0) {
    throw new PackError(`${out} is not empty. Choose a new folder, so nothing stale is published.`);
  }

  const version = workspaceVersion(fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8"));
  if (expectVersion && expectVersion !== version) {
    throw new PackError(
      `version ${expectVersion} was expected, but Cargo.toml says ${version}. A release tag has to be "v" followed by the version in Cargo.toml.`,
    );
  }
  const template = JSON.parse(fs.readFileSync(path.join(LAUNCHER, "package.json"), "utf8"));
  const packages = plan({ version, template, artifacts, allowMissing });
  const hostLine = artifacts
    ? checkHostBinary(packages, version, ask)
    : "Host binary: no artifacts given, so none was run.";

  const lines = [`fogline ${version}`, ""];
  const width = Math.max(...packages.map((pkg) => pkg.name.length));
  for (const pkg of packages) {
    const contents = pkg.skipped
      ? `skipped: ${pkg.skipped}`
      : ["package.json", ...pkg.files.map((file) => file.to)].join(", ");
    lines.push(`  ${pkg.name.padEnd(width)}  ${contents}`);
  }
  lines.push("", hostLine);

  const written = packages.filter((pkg) => !pkg.skipped);
  if (dryRun) {
    lines.push("Dry run: nothing was written.");
  } else {
    for (const pkg of written) {
      writePackage(pkg, out);
    }
    lines.push(`Wrote ${written.length} packages to ${out}.`);
  }
  return { version, packages, report: lines.join("\n") };
}

function main() {
  const { values } = parseArgs({
    options: {
      artifacts: { type: "string" },
      out: { type: "string" },
      "allow-missing": { type: "boolean", default: false },
      "dry-run": { type: "boolean", default: false },
      "expect-version": { type: "string" },
      help: { type: "boolean", short: "h", default: false },
    },
  });
  if (values.help) {
    process.stdout.write(USAGE);
    return;
  }
  const { report } = pack({
    artifacts: values.artifacts && path.resolve(values.artifacts),
    out: values.out && path.resolve(values.out),
    allowMissing: values["allow-missing"],
    dryRun: values["dry-run"],
    expectVersion: values["expect-version"],
  });
  console.log(report);
}

// Run only when this file is the program, not when a test imports it. Real
// paths are compared, because Node follows symlinks for a module's own
// address and not for the path in argv. Comparing them as given would make
// a checkout reached through a symlink do nothing here and still exit with 0.
const startedAs = process.argv[1] ? fs.realpathSync(process.argv[1]) : "";
if (startedAs === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    // A PackError, or a mistyped option, is the user's to fix: one line.
    // Anything else is a bug in this script, and keeps its stack trace.
    if (error instanceof PackError || error.code?.startsWith("ERR_PARSE_ARGS")) {
      console.error(`npm-pack: ${error.message}`);
      process.exitCode = 1;
    } else {
      throw error;
    }
  }
}
