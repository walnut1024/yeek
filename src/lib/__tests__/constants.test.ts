import { describe, it, expect } from "vitest";
import { truncate } from "../../pages/sessions/graph/build-tree";

describe("constants", () => {
  it("GRAPH_MAX_NODES is 300", async () => {
    const { GRAPH_MAX_NODES } = await import("../constants");
    expect(GRAPH_MAX_NODES).toBe(300);
  });

  it("TITLE_TRUNCATE_LEN is 80", async () => {
    const { TITLE_TRUNCATE_LEN } = await import("../constants");
    expect(TITLE_TRUNCATE_LEN).toBe(80);
  });
});

describe("truncate helper", () => {
  it("handles newline collapse", () => {
    expect(truncate("line1\nline2\nline3")).toBe("line1 line2 line3");
  });
});
