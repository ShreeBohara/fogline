"use strict";

// The platforms fogline ships a binary for.
//
// This table is the one place that knows them. The launcher reads it to pick
// a package, and scripts/npm-pack.mjs reads it to build the packages.
//
// `os` and `cpu` use Node's names (process.platform and process.arch). npm
// compares the same names against a package's "os" and "cpu" fields, which
// is how it installs only the package that matches the machine.
//
// The Linux binaries are static musl builds, so one binary per CPU runs on
// every distribution and no "libc" field is needed.

/** The npm scope that holds the platform packages. */
const SCOPE = "@fogline";

/** The binary's file name, without the Windows extension. */
const BINARY = "fogline";

const PLATFORMS = [
  { target: "aarch64-apple-darwin", os: "darwin", cpu: "arm64" },
  { target: "x86_64-apple-darwin", os: "darwin", cpu: "x64" },
  { target: "x86_64-unknown-linux-musl", os: "linux", cpu: "x64" },
  { target: "aarch64-unknown-linux-musl", os: "linux", cpu: "arm64" },
  { target: "x86_64-pc-windows-msvc", os: "win32", cpu: "x64" },
];

/** The npm package that holds the binary for a platform. */
function packageName(platform) {
  return `${SCOPE}/cli-${platform.os}-${platform.cpu}`;
}

/** The binary's file name on a platform. */
function binaryName(platform) {
  return platform.os === "win32" ? `${BINARY}.exe` : BINARY;
}

/** The table row for an operating system and a CPU, or undefined. */
function findPlatform(os, cpu) {
  return PLATFORMS.find((platform) => platform.os === os && platform.cpu === cpu);
}

module.exports = { PLATFORMS, binaryName, findPlatform, packageName };
