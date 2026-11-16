#!/usr/bin/env node
// Proves that a freshly built binary works the way a user gets it: packed
// into npm packages, installed into an empty project, and run through npx.
//
//   node scripts/pack-smoke.mjs <path to a fogline binary built for this machine>
//
// CI runs this on each of the five platforms (docs/specs/phase-1.md,
// section 8). It needs the npm registry: npm looks there for the four
// platform packages that were not built here. They are optional, so not
// finding them is fine.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

import { PackError, pack, removeScratch } from "./npm-pack.mjs";

/** A check that did not hold, reported as one line. */
class SmokeFailure extends Error {}

/**
 * Runs npm or npx in a folder and returns what it printed.
 *
 * The arguments go as a list, so no shell reads them. Windows is the
 * exception it forces: npm and npx are .cmd files there, and those can only
 * be started through cmd.exe. Every argument here is a fixed name or a file
 * this script just created, so there is nothing for cmd.exe to misread.
 */
function run(program, args, cwd) {
  console.log(`$ ${program} ${args.join(" ")}`);
  const options = { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] };
  return process.platform === "win32"
    ? execFileSync("cmd.exe", ["/d", "/s", "/c", program, ...args], options)
    : execFileSync(program, args, options);
}

function smoke(binary) {
  if (!fs.existsSync(binary)) {
    throw new SmokeFailure(
      `${binary} does not exist. Build it with: cargo build --release -p fogline`,
    );
  }

  // What the binary says about itself when run directly. The installed copy
  // has to say exactly the same.
  const direct = execFileSync(binary, ["version", "--json"], { encoding: "utf8" });
  const { target } = JSON.parse(direct);

  const work = fs.mkdtempSync(path.join(os.tmpdir(), "fogline-smoke-"));
  try {
    // 1. Lay the binary out like a downloaded build artifact, and pack it
    //    together with the launcher.
    const artifacts = path.join(work, "artifacts");
    fs.mkdirSync(path.join(artifacts, `fogline-${target}`), { recursive: true });
    fs.copyFileSync(binary, path.join(artifacts, `fogline-${target}`, path.basename(binary)));
    const packed = pack({ artifacts, out: path.join(work, "packages"), allowMissing: true });
    console.log(`${packed.report}\n`);

    // 2. Make the tarballs that `npm publish` would upload.
    const folders = packed.packages.filter((pkg) => !pkg.skipped).map((pkg) => pkg.dir);
    const tarballs = path.join(work, "tarballs");
    fs.mkdirSync(tarballs);
    run("npm", ["pack", ...folders.map((dir) => `../packages/${dir}`)], tarballs);
    const files = fs.readdirSync(tarballs);
    if (files.length !== folders.length) {
      throw new SmokeFailure(`expected ${folders.length} tarballs, found: ${files.join(", ")}`);
    }

    // 3. Install them into an empty project. Scripts are turned off to hold
    //    the promise that fogline needs none.
    const project = path.join(work, "project");
    fs.mkdirSync(project);
    fs.writeFileSync(
      path.join(project, "package.json"),
      '{ "name": "fogline-smoke", "private": true }\n',
    );
    const paths = files.map((file) => `../tarballs/${file}`);
    run("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund", ...paths], project);

    // 4. Run it the way a user does. `--no` stops npx from fetching some
    //    other fogline from the registry if the install above went wrong.
    const installed = run("npx", ["--no", "fogline", "version", "--json"], project);
    if (installed !== direct) {
      throw new SmokeFailure(
        `the installed fogline does not match the binary.\n  binary:    ${direct.trim()}\n  installed: ${installed.trim()}`,
      );
    }
    console.log(
      `\npack-smoke: ok. Installed from tarballs and run through npx:\n  ${installed.trim()}`,
    );
  } finally {
    removeScratch(work);
  }
}

const [binary, ...extra] = process.argv.slice(2);
if (!binary || extra.length > 0) {
  console.error("Usage: node scripts/pack-smoke.mjs <path to the fogline binary>");
  process.exitCode = 2;
} else {
  try {
    smoke(path.resolve(binary));
  } catch (error) {
    // A failed check or a packing problem is reported as one line. A command
    // that failed has already printed its own output, under the `$` line
    // that shows what was run.
    if (error instanceof SmokeFailure || error instanceof PackError) {
      console.error(`pack-smoke: ${error.message}`);
    } else if (error.status !== undefined) {
      console.error(`pack-smoke: a command failed with exit code ${error.status}.`);
    } else {
      throw error;
    }
    process.exitCode = 1;
  }
}
