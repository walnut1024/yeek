import { describe, it, expect } from "vitest";
import { buildTree, truncate } from "../build-tree";
import type { MessageRecord } from "@/lib/api";

function makeMsg(overrides: Partial<MessageRecord> & { id: string }): MessageRecord {
  return {
    session_id: "s1",
    parent_id: null,
    role: "human",
    kind: "message",
    content_preview: "test",
    timestamp: null,
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: null,
    subagent_id: null,
    model: null,
    metadata: null,
    ...overrides,
  };
}

describe("buildTree", () => {
  it("returns empty result for empty input", () => {
    const { nodes, edges, stats } = buildTree([]);
    expect(nodes).toHaveLength(0);
    expect(edges).toHaveLength(0);
    expect(stats.total).toBe(0);
  });

  it("creates a user node for human messages", () => {
    const msg = makeMsg({ id: "m1", role: "human", kind: "message", content_preview: "Hello" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("user");
    expect(stats.users).toBe(1);
  });

  it("creates an assistant node", () => {
    const msg = makeMsg({ id: "m1", role: "assistant", kind: "message", content_preview: "Hi" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("assistant");
    expect(stats.assistants).toBe(1);
  });

  it("creates an edge from parent to child", () => {
    const parent = makeMsg({ id: "m1", role: "human", kind: "message" });
    const child = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const { edges } = buildTree([parent, child]);
    expect(edges).toHaveLength(1);
    expect(edges[0].source).toBe("m1");
    expect(edges[0].target).toBe("m2");
  });

  it("re-parents across skipped nodes", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    // System message with skipped subtype
    const sys = makeMsg({ id: "m2", parent_id: "m1", role: "system", kind: "message", entry_type: "system", subtype: "mcp_instructions_delta" });
    const assistant = makeMsg({ id: "m3", parent_id: "m2", role: "assistant", kind: "message" });
    const { nodes, edges } = buildTree([user, sys, assistant]);
    // System node should be skipped
    expect(nodes).toHaveLength(2);
    // Edge should go directly from user to assistant
    expect(edges).toHaveLength(1);
    expect(edges[0].source).toBe("m1");
    expect(edges[0].target).toBe("m3");
  });

  it("creates tool_use nodes", () => {
    const msg = makeMsg({ id: "m1", role: "assistant", kind: "tool_use", tool_name: "Read", content_preview: "Tool: Read\nfile.ts" });
    const { nodes, stats } = buildTree([msg]);
    expect(nodes).toHaveLength(1);
    expect(nodes[0].type).toBe("toolUse");
    expect(stats.tools).toBe(1);
  });

  it("skips verbose system subtypes", () => {
    const msg = makeMsg({ id: "m1", role: "system", kind: "message", entry_type: "system", subtype: "skill_listing" });
    const { nodes } = buildTree([msg]);
    expect(nodes).toHaveLength(0);
  });
});

describe("truncate", () => {
  it("returns empty for empty input", () => {
    expect(truncate("")).toBe("");
  });

  it("truncates at default length", () => {
    expect(truncate("a".repeat(60))).toHaveLength(49); // 48 + ellipsis
  });

  it("preserves short text", () => {
    expect(truncate("hello")).toBe("hello");
  });
});
