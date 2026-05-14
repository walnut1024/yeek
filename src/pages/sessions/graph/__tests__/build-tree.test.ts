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
    const sys = makeMsg({ id: "m2", parent_id: "m1", role: "system", kind: "message", entry_type: "system", subtype: "mcp_instructions_delta" });
    const assistant = makeMsg({ id: "m3", parent_id: "m2", role: "assistant", kind: "message" });
    const { nodes, edges } = buildTree([user, sys, assistant]);
    expect(nodes).toHaveLength(2);
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

  it("places user nodes in column 0 and assistant in column 1", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const asst = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const { nodes } = buildTree([user, asst]);
    const userNode = nodes.find((n) => n.type === "user")!;
    const asstNode = nodes.find((n) => n.type === "assistant")!;
    expect(userNode.position.x).toBe(0);
    expect(asstNode.position.x).toBe(220);
  });

  it("places tool nodes in column 2", () => {
    const tool = makeMsg({ id: "m1", role: "assistant", kind: "tool_use", tool_name: "Read" });
    const { nodes } = buildTree([tool]);
    expect(nodes[0].position.x).toBe(440);
  });

  it("places branch nodes in column 3+", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const asst = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const branchAsst = makeMsg({ id: "m3", role: "assistant", kind: "message", parent_id: "m1", content_preview: "alt path" });

    const { nodes, stats } = buildTree(
      [user, asst, branchAsst],
      {
        mainPath: ["m1", "m2"],
        branches: [{ parent_id: "m1", siblings: [{ message_id: "m3", label: "alt" }], active_index: 0 }],
      },
    );

    expect(stats.branches).toBeGreaterThan(0);
    const branchNode = nodes.find((n) => n.id === "m3");
    expect(branchNode).toBeDefined();
    expect(branchNode!.position.x).toBeGreaterThanOrEqual(3 * 220);
    expect(branchNode!.data.isBranch).toBe(true);
  });

  it("places branch chain children below the branch parent", () => {
    const user = makeMsg({ id: "m1", role: "human", kind: "message" });
    const mainAsst = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const branchAsst = makeMsg({ id: "m3", role: "assistant", kind: "message", parent_id: "m1", content_preview: "alt" });
    const branchTool = makeMsg({ id: "m4", role: "assistant", kind: "tool_use", tool_name: "Bash", parent_id: "m3" });

    const { nodes } = buildTree(
      [user, mainAsst, branchAsst, branchTool],
      {
        mainPath: ["m1", "m2"],
        branches: [{ parent_id: "m1", siblings: [{ message_id: "m3", label: "alt" }], active_index: 0 }],
      },
    );

    const branchNode = nodes.find((n) => n.id === "m3")!;
    const chainNode = nodes.find((n) => n.id === "m4")!;
    expect(chainNode).toBeDefined();
    expect(chainNode.position.y).toBeGreaterThan(branchNode.position.y);
    expect(chainNode.data.isBranch).toBe(true);
  });

  it("uses main_path ordering for y positions", () => {
    const m1 = makeMsg({ id: "m1", role: "human", kind: "message" });
    const m2 = makeMsg({ id: "m2", role: "assistant", kind: "message", parent_id: "m1" });
    const m3 = makeMsg({ id: "m3", role: "assistant", kind: "tool_use", tool_name: "Read", parent_id: "m2" });

    const { nodes } = buildTree([m1, m2, m3], { mainPath: ["m1", "m2", "m3"], branches: [] });

    const n1 = nodes.find((n) => n.id === "m1")!;
    const n2 = nodes.find((n) => n.id === "m2")!;
    const n3 = nodes.find((n) => n.id === "m3")!;
    expect(n2.position.y).toBeGreaterThan(n1.position.y);
    expect(n3.position.y).toBeGreaterThan(n2.position.y);
  });
});

describe("truncate", () => {
  it("returns empty for empty input", () => {
    expect(truncate("")).toBe("");
  });

  it("truncates at default length", () => {
    expect(truncate("a".repeat(60))).toHaveLength(49);
  });

  it("preserves short text", () => {
    expect(truncate("hello")).toBe("hello");
  });
});
