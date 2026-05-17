import { describe, it, expect } from "vitest";
import { buildMapTree, TOOL_COLLAPSE_THRESHOLD } from "../build-session-tree";
import type { MessageRecord } from "@/lib/api";

function makeMsg(
  overrides: Partial<MessageRecord> & { id: string },
): MessageRecord {
  return {
    session_id: "s1",
    parent_id: null,
    role: "human",
    kind: "message",
    content_preview: "test",
    timestamp: "2026-05-17T14:02:00Z",
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

describe("buildMapTree", () => {
  it("returns empty result for empty input", () => {
    const { turns, stats } = buildMapTree([], []);
    expect(turns).toHaveLength(0);
    expect(stats.users).toBe(0);
  });

  it("creates a Turn with a user node", () => {
    const msgs = [makeMsg({ id: "m1", role: "human", kind: "message" })];
    const { turns, stats } = buildMapTree(msgs, ["m1"]);
    expect(turns).toHaveLength(1);
    expect(turns[0].user).toHaveLength(1);
    expect(turns[0].user[0].type).toBe("user");
    expect(stats.users).toBe(1);
  });

  it("groups user and assistant into the same Turn", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
    ];
    const { turns, stats } = buildMapTree(msgs, ["m1", "m2"]);
    expect(turns).toHaveLength(1);
    expect(turns[0].user).toHaveLength(1);
    expect(turns[0].assistants).toHaveLength(1);
    expect(stats.assistants).toBe(1);
  });

  it("places tool_use in the tools bucket", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
      makeMsg({
        id: "m3",
        role: "assistant",
        kind: "tool_use",
        tool_name: "Read",
        parent_id: "m2",
        content_preview: "Tool: Read\nfile.ts",
      }),
    ];
    const { turns, stats } = buildMapTree(msgs, ["m1", "m2", "m3"]);
    expect(stats.tools).toBe(1);
    expect(turns[0].tools).toHaveLength(1);
    expect(turns[0].tools[0].toolName).toBe("Read");
  });

  it("places subagent in the subagents bucket", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
      makeMsg({
        id: "m3",
        role: "assistant",
        kind: "tool_use",
        tool_name: "Agent",
        parent_id: "m2",
      }),
    ];
    const { turns, stats } = buildMapTree(msgs, ["m1", "m2", "m3"]);
    expect(stats.subagents).toBe(1);
    expect(turns[0].subagents).toHaveLength(1);
    expect(turns[0].subagents[0].type).toBe("subagent");
  });

  it("places tool_result in tools bucket", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
      makeMsg({
        id: "m3",
        role: "assistant",
        kind: "tool_use",
        tool_name: "Read",
        parent_id: "m2",
      }),
      makeMsg({
        id: "m4",
        role: "assistant",
        kind: "tool_result",
        parent_id: "m3",
        content_preview: "file content...",
      }),
    ];
    const { turns } = buildMapTree(msgs, ["m1", "m2", "m3", "m4"]);
    expect(turns[0].tools).toHaveLength(2);
    expect(turns[0].tools[1].type).toBe("toolResult");
  });

  it("marks main_path nodes", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
    ];
    const { turns } = buildMapTree(msgs, ["m1", "m2"]);
    expect(turns[0].user[0].isMainPath).toBe(true);
    expect(turns[0].assistants[0].isMainPath).toBe(true);
  });

  it("splits into multiple turns at each human message", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
      makeMsg({ id: "m3", role: "human", kind: "message", parent_id: "m2" }),
      makeMsg({
        id: "m4",
        role: "assistant",
        kind: "message",
        parent_id: "m3",
      }),
    ];
    const { turns } = buildMapTree(msgs, ["m1", "m2", "m3", "m4"]);
    expect(turns).toHaveLength(2);
    expect(turns[0].user[0].id).toBe("m1");
    expect(turns[1].user[0].id).toBe("m3");
  });

  it("uses first user message ID as Turn ID", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
    ];
    const { turns } = buildMapTree(msgs, ["m1", "m2"]);
    expect(turns[0].id).toBe("m1");
  });

  it("extracts timestamp from first user message", () => {
    const msgs = [
      makeMsg({
        id: "m1",
        role: "human",
        kind: "message",
        timestamp: "2026-05-17T14:05:00Z",
      }),
    ];
    const { turns } = buildMapTree(msgs, ["m1"]);
    expect(turns[0].timestamp).toBe("2026-05-17T14:05:00Z");
  });

  it("skips verbose system subtypes", () => {
    const msgs = [
      makeMsg({
        id: "m1",
        role: "system",
        kind: "message",
        entry_type: "system",
        subtype: "skill_listing",
      }),
    ];
    const { turns } = buildMapTree(msgs, []);
    expect(turns).toHaveLength(0);
  });

  it("keeps all tools in data regardless of collapse threshold", () => {
    const msgs = [
      makeMsg({ id: "m1", role: "human", kind: "message" }),
      makeMsg({
        id: "m2",
        role: "assistant",
        kind: "message",
        parent_id: "m1",
      }),
      ...Array.from({ length: 10 }, (_, i) =>
        makeMsg({
          id: `tool-${i}`,
          role: "assistant",
          kind: "tool_use",
          tool_name: ["Read", "Edit", "Bash", "Write", "Grep"][i % 5],
          parent_id: i === 0 ? "m2" : `tool-${i - 1}`,
          content_preview: `Tool: action ${i}`,
        }),
      ),
    ];
    const { turns } = buildMapTree(msgs, msgs.map((m) => m.id));
    expect(turns[0].tools).toHaveLength(10);
    expect(turns[0].tools.length).toBeGreaterThan(TOOL_COLLAPSE_THRESHOLD);
  });

  it("handles prelude Turn (messages before first user)", () => {
    const msgs = [
      makeMsg({
        id: "m1",
        role: "assistant",
        kind: "message",
        parent_id: null,
      }),
      makeMsg({ id: "m2", role: "human", kind: "message", parent_id: "m1" }),
    ];
    const { turns } = buildMapTree(msgs, ["m1", "m2"]);
    // First turn: assistant only (prelude)
    expect(turns).toHaveLength(2);
    expect(turns[0].user).toHaveLength(0);
    expect(turns[0].assistants).toHaveLength(1);
    expect(turns[1].user).toHaveLength(1);
  });
});
