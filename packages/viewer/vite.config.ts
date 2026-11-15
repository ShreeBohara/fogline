import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  // Relative asset URLs: `fogline open` serves the viewer under a per-run
  // token path and a static export can sit in any folder.
  base: "./",
  // Answer 404 for files that do not exist instead of falling back to
  // index.html, so a missing bundle looks the same here as it does in
  // production.
  appType: "mpa",
  plugins: [react()],
  build: {
    target: "es2022",
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
