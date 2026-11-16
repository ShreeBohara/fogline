"use strict";

// Tests for the launcher. Run them with `npm test` in this package. They
// need no install and no fogline binary: where a binary is needed, Node runs
// fake-fogline.js in its place.

const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");

const { locate } = require("../lib/launcher.js");
const { PLATFORMS, binaryName, findPlatform, packageName } = require("../lib/platforms.js");

const LAUNCHER = path.join(__dirname, "..", "bin", "fogline.js");
const FAKE = path.join(__dirname, "fake-fogline.js");
const NO_SIGNALS = process.platform === "win32" ? "Windows has no signals" : false;

// ---- The platform table ----

test("the table lists five platforms, each with its own package", () => {
  assert.deepEqual(PLATFORMS.map(packageName), [
    "@fogline/cli-darwin-arm64",
    "@fogline/cli-darwin-x64",
    "@fogline/cli-linux-x64",
    "@fogline/cli-linux-arm64",
    "@fogline/cli-win32-x64",
  ]);
  assert.equal(new Set(PLATFORMS.map((platform) => platform.target)).size, 5);
});

test("each target triple agrees with its os and cpu", () => {
  const cpus = { aarch64: "arm64", x86_64: "x64" };
  const systems = {
    darwin: "apple-darwin",
    linux: "unknown-linux-musl",
    win32: "pc-windows-msvc",
  };
  for (const platform of PLATFORMS) {
    const [arch, ...rest] = platform.target.split("-");
    assert.equal(cpus[arch], platform.cpu, platform.target);
    assert.equal(rest.join("-"), systems[platform.os], platform.target);
  }
});

test("only the Windows binary has an extension", () => {
  assert.equal(binaryName(findPlatform("win32", "x64")), "fogline.exe");
  assert.equal(binaryName(findPlatform("linux", "arm64")), "fogline");
  assert.equal(binaryName(findPlatform("darwin", "x64")), "fogline");
});

test("platforms outside the table are not found", () => {
  const others = [
    ["win32", "arm64"],
    ["linux", "riscv64"],
    ["freebsd", "x64"],
    ["android", "arm64"],
  ];
  for (const [system, cpu] of others) {
    assert.equal(findPlatform(system, cpu), undefined, `${system}-${cpu}`);
  }
});

// ---- Choosing the binary ----

const mustNotResolve = () => {
  throw new Error("resolve was called");
};

const notInstalled = (request) => {
  const error = new Error(`Cannot find module '${request}'`);
  error.code = "MODULE_NOT_FOUND";
  throw error;
};

function assertListsTheWaysOut(problem) {
  assert.match(problem, /darwin-arm64, darwin-x64, linux-x64, linux-arm64, win32-x64/);
  assert.match(
    problem,
    /cargo install --locked --git https:\/\/github\.com\/ShreeBohara\/fogline fogline\n/,
  );
  assert.match(problem, /Set FOGLINE_BINARY to the path/);
}

test("FOGLINE_BINARY wins, even where no package exists", () => {
  const found = locate({
    env: { FOGLINE_BINARY: "/opt/fogline" },
    platform: "freebsd",
    arch: "x64",
    resolve: mustNotResolve,
  });
  assert.deepEqual(found, { binary: "/opt/fogline" });
});

test("a supported platform resolves the binary inside its package", () => {
  const asked = [];
  const resolve = (request) => {
    asked.push(request);
    return `/modules/${request}`;
  };

  const onLinux = locate({ env: {}, platform: "linux", arch: "x64", resolve });
  const onWindows = locate({ env: {}, platform: "win32", arch: "x64", resolve });

  assert.deepEqual(onLinux, { binary: "/modules/@fogline/cli-linux-x64/bin/fogline" });
  assert.deepEqual(onWindows, { binary: "/modules/@fogline/cli-win32-x64/bin/fogline.exe" });
  assert.deepEqual(asked, [
    "@fogline/cli-linux-x64/bin/fogline",
    "@fogline/cli-win32-x64/bin/fogline.exe",
  ]);
});

test("a missing package is explained, with the ways out", () => {
  const found = locate({ env: {}, platform: "darwin", arch: "arm64", resolve: notInstalled });
  assert.equal(found.binary, undefined);
  assert.match(
    found.problem,
    /^fogline: the package @fogline\/cli-darwin-arm64 is not installed\.\n/,
  );
  assert.match(found.problem, /npm install --omit=optional/);
  assertListsTheWaysOut(found.problem);
});

test("an unsupported platform is named, with the ways out", () => {
  const found = locate({ env: {}, platform: "freebsd", arch: "x64", resolve: mustNotResolve });
  assert.equal(found.binary, undefined);
  assert.match(found.problem, /^fogline: there is no prebuilt binary for freebsd-x64\.\n/);
  assertListsTheWaysOut(found.problem);
});

// ---- Running it, the way a user does ----

/**
 * Runs `bin/fogline.js` and collects what it did.
 *
 * By default the binary is Node running fake-fogline.js, so `args` start
 * with one of the fake's modes. Pass `binary: null` to run with no
 * FOGLINE_BINARY at all, or a path to point it somewhere else. With
 * `signalWhenReady`, that signal is sent to the launcher, and only to the
 * launcher, once the fake has printed "ready".
 */
function launch(args, { binary = process.execPath, input = "", signalWhenReady } = {}) {
  const env = { ...process.env };
  delete env.FOGLINE_BINARY;
  if (binary !== null) {
    env.FOGLINE_BINARY = binary;
  }
  const launcherArgs = binary === process.execPath ? [FAKE, ...args] : args;

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [LAUNCHER, ...launcherArgs], { env });
    let stdout = "";
    let stderr = "";
    let signalled = false;
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      if (signalWhenReady && !signalled && stdout.includes("ready")) {
        signalled = true;
        child.kill(signalWhenReady);
      }
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    // "close" comes after the binary has let go of the pipes too, so all of
    // its output has been read by then.
    child.on("close", (code, signal) => resolve({ code, signal, stdout, stderr }));
    child.stdin.end(input);
  });
}

test("arguments, output and the exit code pass through", async () => {
  const echoed = await launch(["echo", "scan", "--json", "a b"]);
  assert.equal(echoed.stdout, `${JSON.stringify(["scan", "--json", "a b"])}\n`);
  assert.equal(echoed.stderr, "fake stderr\n");
  assert.equal(echoed.code, 0);

  const failed = await launch(["exit", "7"]);
  assert.equal(failed.code, 7);
  assert.equal(failed.signal, null);
});

test("standard input passes through", async () => {
  const result = await launch(["stdin"], { input: "one\ntwo\n" });
  assert.equal(result.stdout, "one\ntwo\n");
  assert.equal(result.code, 0);
});

test("the launcher ends by the signal that ended the binary", { skip: NO_SIGNALS }, async () => {
  const result = await launch(["signal", "SIGTERM"]);
  assert.equal(result.signal, "SIGTERM");
  assert.equal(result.code, null);
});

test("a signal sent to the launcher reaches the binary", { skip: NO_SIGNALS }, async () => {
  const result = await launch(["wait"], { signalWhenReady: "SIGTERM" });
  assert.equal(result.stdout, "ready\ngot SIGTERM\n");
  assert.equal(result.code, 42);
});

test("a crash signal becomes an exit code and a message", { skip: NO_SIGNALS }, async () => {
  // SIGUSR2 stands in for a crash: nothing here handles it, and unlike
  // SIGSEGV it leaves no core dump behind.
  const result = await launch(["signal", "SIGUSR2"]);
  assert.equal(result.code, 128 + os.constants.signals.SIGUSR2);
  assert.equal(result.signal, null);
  assert.equal(result.stderr, "fogline: the binary was stopped by SIGUSR2.\n");
});

test("a binary that cannot be started is reported", async () => {
  const result = await launch(["version"], { binary: path.join(__dirname, "no-such-binary") });
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /^fogline: could not run .*no-such-binary: /);
});

test("with nothing to run, the launcher says what to do and exits with 1", async () => {
  // No platform package is installed in this repository. On a supported
  // platform the launcher reports the missing package, and anywhere else the
  // unsupported platform. Both end the same way.
  const result = await launch(["version"], { binary: null });
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.match(result.stderr, /^fogline: /);
  assert.match(result.stderr, /Prebuilt binaries exist for:/);
});
