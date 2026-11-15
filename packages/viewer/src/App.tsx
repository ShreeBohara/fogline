import { useEffect, useState } from "react";
import { type LoadState, loadManifest, SUPPORTED_SCHEMA } from "./bundle/load";

interface AppProps {
  /** The bundle folder, ending with a slash. The viewer reads `manifest.json` from it. */
  bundleUrl: URL;
}

export function App({ bundleUrl }: AppProps) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });

  useEffect(() => {
    let current = true;
    loadManifest(bundleUrl).then((result) => {
      if (current) {
        setState(result);
      }
    });
    return () => {
      current = false;
    };
  }, [bundleUrl]);

  return (
    <main className="fogline-shell">
      <Notice state={state} />
    </main>
  );
}

function Notice({ state }: { state: LoadState }) {
  switch (state.kind) {
    case "loading":
      return <p>Loading…</p>;
    case "none":
      return (
        <>
          <h1>No bundle loaded</h1>
          <p>
            Run <code>npx fogline</code> inside a git repository to scan it and open its map.
          </p>
        </>
      );
    case "unsupported":
      return (
        <>
          <h1>This bundle needs a different viewer</h1>
          <p>
            It uses the format <code>{state.schema}</code>. This viewer reads{" "}
            <code>{SUPPORTED_SCHEMA}</code>.
          </p>
        </>
      );
    case "error":
      return (
        <>
          <h1>The bundle could not be loaded</h1>
          <p>{state.message}</p>
        </>
      );
    case "ready":
      return (
        <>
          <h1>Bundle loaded</h1>
          <p>The map is not built yet. It arrives later in Phase 1.</p>
        </>
      );
  }
}
