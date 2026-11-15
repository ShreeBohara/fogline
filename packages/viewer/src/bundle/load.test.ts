import { describe, expect, it } from "vitest";
import { type FetchManifest, loadManifest, SUPPORTED_SCHEMA } from "./load";

const bundleUrl = new URL("http://127.0.0.1:4000/token/bundle/");

function answering(status: number, body: unknown): FetchManifest {
  return async () => ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  });
}

describe("loadManifest", () => {
  it("asks for manifest.json inside the bundle folder", async () => {
    const requested: string[] = [];
    await loadManifest(bundleUrl, (url) => {
      requested.push(url.href);
      return answering(404, null)(url);
    });
    expect(requested).toEqual(["http://127.0.0.1:4000/token/bundle/manifest.json"]);
  });

  it("reports that there is no bundle when the manifest is missing", async () => {
    expect(await loadManifest(bundleUrl, answering(404, null))).toEqual({ kind: "none" });
  });

  it("is ready when the bundle uses the format it reads", async () => {
    const result = await loadManifest(bundleUrl, answering(200, { schema: SUPPORTED_SCHEMA }));
    expect(result).toEqual({ kind: "ready", manifest: { schema: SUPPORTED_SCHEMA } });
  });

  it("refuses a bundle in another format and names that format", async () => {
    const result = await loadManifest(bundleUrl, answering(200, { schema: "fogline.bundle/9" }));
    expect(result).toEqual({ kind: "unsupported", schema: "fogline.bundle/9" });
  });

  it("turns a server failure into a message that carries the status", async () => {
    const result = await loadManifest(bundleUrl, answering(500, null));
    expect(result).toEqual({
      kind: "error",
      message: "The server answered 500 for manifest.json.",
    });
  });

  it("turns an unreachable server into a message", async () => {
    const result = await loadManifest(bundleUrl, async () => {
      throw new TypeError("fetch failed");
    });
    expect(result).toEqual({
      kind: "error",
      message: "The bundle could not be reached: fetch failed.",
    });
  });

  it("turns a body that is not JSON into a message", async () => {
    const result = await loadManifest(bundleUrl, async () => ({
      ok: true,
      status: 200,
      json: async () => {
        throw new SyntaxError("Unexpected token <");
      },
    }));
    expect(result).toEqual({ kind: "error", message: "manifest.json is not valid JSON." });
  });

  it.each([
    ["an empty object", {}],
    ["null", null],
    ["a string", "fogline.bundle/0"],
    ["a schema that is not text", { schema: 0 }],
  ])("rejects a manifest that is %s", async (_label, body) => {
    const result = await loadManifest(bundleUrl, answering(200, body));
    expect(result).toEqual({
      kind: "error",
      message: "manifest.json does not say which format it uses.",
    });
  });
});
