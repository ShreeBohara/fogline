# fogline

Maps which parts of a codebase arrived in agent-attributed commits.

**Pre-alpha.** `fogline version` is the only command that works so far. The plan and the current state are in the [repository](https://github.com/ShreeBohara/fogline).

## Run it

```bash
npx fogline version
```

## How this package works

This package is a small launcher with no dependencies. The program itself is a native binary, and npm installs it from one of five platform packages, chosen by your operating system and CPU:

| Platform | Package |
|---|---|
| macOS on Apple silicon | `@fogline/cli-darwin-arm64` |
| macOS on Intel | `@fogline/cli-darwin-x64` |
| Linux on x64 | `@fogline/cli-linux-x64` |
| Linux on Arm | `@fogline/cli-linux-arm64` |
| Windows on x64 | `@fogline/cli-win32-x64` |

The Linux binaries are static, so they run on any distribution, Alpine included.

No install script runs, so `--ignore-scripts` is safe. The platform packages are optional dependencies: an install that leaves those out (`--omit=optional`) has no binary to run, and the launcher says so.

To run a binary you built yourself, set `FOGLINE_BINARY` to its path.

## Licence

MIT
