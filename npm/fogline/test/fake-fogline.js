"use strict";

// A stand-in for the fogline binary. The tests set FOGLINE_BINARY to Node
// itself and pass this file as the first argument, so the launcher ends up
// running `node fake-fogline.js <mode> ...`. That works on every platform,
// Windows included, where a script cannot be started directly.

const [mode, ...rest] = process.argv.slice(2);

switch (mode) {
  // Print the remaining arguments as one JSON line, and a line on stderr.
  case "echo":
    process.stdout.write(`${JSON.stringify(rest)}\n`);
    process.stderr.write("fake stderr\n");
    break;

  // Exit with the given code.
  case "exit":
    process.exit(Number(rest[0]));
    break;

  // Copy standard input to standard output.
  case "stdin":
    process.stdin.pipe(process.stdout);
    break;

  // End by sending itself the given signal.
  case "signal":
    process.kill(process.pid, rest[0]);
    setTimeout(() => {}, 5000);
    break;

  // Say when ready, then wait. SIGTERM makes it say so and exit with 42,
  // which nothing else in the tests produces.
  case "wait":
    process.on("SIGTERM", () => {
      process.stdout.write("got SIGTERM\n");
      process.exit(42);
    });
    process.stdout.write("ready\n");
    setInterval(() => {}, 1000);
    break;

  default:
    process.stderr.write(`fake-fogline: unknown mode ${mode}\n`);
    process.exit(99);
}
