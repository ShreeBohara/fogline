"use strict";

// What `npx fogline` runs. It finds the fogline binary for this machine, runs
// it with the same arguments and the same terminal, and ends the way the
// binary ended. It has no dependencies and runs on Node 18 or newer.

const { spawn } = require("node:child_process");
const os = require("node:os");
const manifest = require("../package.json");
const { PLATFORMS, binaryName, findPlatform, packageName } = require("./platforms.js");

// "git+https://github.com/owner/name.git" becomes "https://github.com/owner/name".
const REPOSITORY = manifest.repository.url.replace(/^git\+/, "").replace(/\.git$/, "");

// The signals that ask a program to stop. The launcher passes them on to the
// binary.
const STOP_SIGNALS = ["SIGINT", "SIGTERM", "SIGHUP"];

// The signals the launcher ends itself with when they ended the binary.
const RERAISED = new Set([...STOP_SIGNALS, "SIGKILL"]);

/**
 * Decides which binary to run.
 *
 * Returns `{ binary }` with a path, or `{ problem }` with the text to show
 * when there is nothing to run. `resolve` is `require.resolve`; the tests
 * pass their own.
 */
function locate({ env, platform, arch, resolve }) {
  // An explicit path always wins. It is how a binary built from source is
  // used, on any platform.
  if (env.FOGLINE_BINARY) {
    return { binary: env.FOGLINE_BINARY };
  }

  const match = findPlatform(platform, arch);
  if (!match) {
    return { problem: explain(`there is no prebuilt binary for ${platform}-${arch}.`) };
  }

  const name = packageName(match);
  try {
    return { binary: resolve(`${name}/bin/${binaryName(match)}`) };
  } catch {
    // The only way this fails is that the package, or the file in it, is
    // not there.
    return {
      problem: explain(
        `the package ${name} is not installed.`,
        "It holds the binary for this machine. It is an optional dependency of",
        "fogline, so an install that leaves out optional dependencies skips it",
        "(for example `npm install --omit=optional`). Install fogline again with",
        "optional dependencies turned on.",
      ),
    };
  }
}

/** Builds the text shown when there is no binary to run. */
function explain(summary, ...details) {
  const supported = PLATFORMS.map((platform) => `${platform.os}-${platform.cpu}`).join(", ");
  const lines = [`fogline: ${summary}`, ""];
  if (details.length > 0) {
    lines.push(...details, "");
  }
  lines.push(
    "Prebuilt binaries exist for:",
    `  ${supported}`,
    "",
    "Other ways to run fogline:",
    "  - Build it from source:",
    `      cargo install --locked --git ${REPOSITORY} fogline`,
    "  - Set FOGLINE_BINARY to the path of a fogline binary you already have.",
  );
  return lines.join("\n");
}

/**
 * Runs the binary on this process's terminal and waits for it to end.
 *
 * Resolves with `{ code }` for a normal exit, `{ signal }` when a signal
 * ended the binary, or `{ error }` when it could not be started.
 */
function run(binary, args) {
  return new Promise((resolve) => {
    const child = spawn(binary, args, { stdio: "inherit" });

    // A signal sent to the launcher alone, by `kill` or by a supervisor,
    // would otherwise end the launcher and leave the binary running. Having
    // a listener also stops Node from exiting on these signals, so the
    // launcher stays until the binary has ended.
    //
    // Ctrl-C in a terminal already reaches both processes, so the binary
    // sees that one twice. Windows has no signals to pass on: the console
    // delivers Ctrl-C to the binary itself, and child.kill() there would end
    // it with no chance to clean up.
    const passOn = (signal) => {
      if (process.platform !== "win32") {
        child.kill(signal);
      }
    };
    for (const signal of STOP_SIGNALS) {
      process.on(signal, passOn);
    }

    const finish = (outcome) => {
      for (const signal of STOP_SIGNALS) {
        process.removeListener(signal, passOn);
      }
      resolve(outcome);
    };
    child.on("error", (error) => finish({ error }));
    child.on("exit", (code, signal) => finish(signal ? { signal } : { code }));
  });
}

/**
 * Ends this process the way a signal ended the binary.
 *
 * For a signal that only stops a program, the launcher sends itself the same
 * one, so a shell or a CI runner sees what really happened. Any other signal
 * means the binary crashed. Raising one of those here would make Node write
 * a core dump or a crash report about itself, so it becomes the exit code
 * shells use for a signal: 128 plus its number.
 */
function endBySignal(signal) {
  process.exitCode = 128 + (os.constants.signals[signal] || 0);
  if (!RERAISED.has(signal)) {
    console.error(`fogline: the binary was stopped by ${signal}.`);
    return;
  }
  process.kill(process.pid, signal);
  // The signal ends this process at once. The timer only matters if it does
  // not: it keeps Node from exiting with the code above before the signal
  // has had time to arrive.
  setTimeout(() => {}, 1000);
}

/** The whole program. `bin/fogline.js` calls this and nothing else. */
async function main() {
  const found = locate({
    env: process.env,
    platform: process.platform,
    arch: process.arch,
    resolve: require.resolve,
  });
  if (found.problem) {
    console.error(found.problem);
    process.exitCode = 1;
    return;
  }

  const outcome = await run(found.binary, process.argv.slice(2));
  if (outcome.error) {
    console.error(`fogline: could not run ${found.binary}: ${outcome.error.message}`);
    process.exitCode = 1;
  } else if (outcome.signal) {
    endBySignal(outcome.signal);
  } else {
    process.exitCode = outcome.code;
  }
}

module.exports = { locate, main };
