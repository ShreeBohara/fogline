/** The bundle format this viewer reads (spec section 5.5). */
export const SUPPORTED_SCHEMA = "fogline.bundle/0";

/** The part of `manifest.json` the viewer relies on so far. */
export interface Manifest {
  schema: string;
}

/** What loading a bundle can end in. */
export type LoadResult =
  /** There is no bundle next to the viewer. */
  | { kind: "none" }
  /** There is a bundle, in a format this viewer does not read. */
  | { kind: "unsupported"; schema: string }
  | { kind: "error"; message: string }
  | { kind: "ready"; manifest: Manifest };

export type LoadState = { kind: "loading" } | LoadResult;

/** The slice of `fetch` the loader uses, so tests can stand in for the network. */
export type FetchManifest = (url: URL) => Promise<Pick<Response, "ok" | "status" | "json">>;

/**
 * Loads `manifest.json` from a bundle folder and decides what the viewer can
 * do with it. `bundleUrl` must end with a slash.
 *
 * It never rejects: every failure becomes a result the UI can show.
 */
export async function loadManifest(
  bundleUrl: URL,
  fetchManifest: FetchManifest = (url) => fetch(url),
): Promise<LoadResult> {
  let response: Awaited<ReturnType<FetchManifest>>;
  try {
    response = await fetchManifest(new URL("manifest.json", bundleUrl));
  } catch (cause) {
    return { kind: "error", message: `The bundle could not be reached: ${describe(cause)}.` };
  }

  if (response.status === 404) {
    return { kind: "none" };
  }
  if (!response.ok) {
    return { kind: "error", message: `The server answered ${response.status} for manifest.json.` };
  }

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return { kind: "error", message: "manifest.json is not valid JSON." };
  }

  const schema = readSchema(body);
  if (schema === undefined) {
    return { kind: "error", message: "manifest.json does not say which format it uses." };
  }
  if (schema !== SUPPORTED_SCHEMA) {
    return { kind: "unsupported", schema };
  }
  return { kind: "ready", manifest: { schema } };
}

function readSchema(body: unknown): string | undefined {
  if (typeof body !== "object" || body === null || !("schema" in body)) {
    return undefined;
  }
  return typeof body.schema === "string" ? body.schema : undefined;
}

function describe(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
