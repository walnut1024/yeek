import { useState, useMemo, useCallback, useRef } from "react";
import { ReactFlow, Controls, Background, MarkerType, Panel } from "@xyflow/react";
import type { Node } from "@xyflow/react";
import { buildTree, type TreeNodeData } from "./graph/build-tree";
import { nodeTypes } from "./graph/nodes";
import NodeDetailPanel from "./graph/node-detail-panel";

import "@xyflow/react/dist/style.css";

interface RawMessage {
  id: string;
  parent_id: string | null;
  role: string;
  kind: string;
  content_preview?: string | null;
  timestamp?: string | null;
  entry_type?: string | null;
  subtype?: string | null;
  tool_name?: string | null;
  model?: string | null;
  is_sidechain?: boolean;
  subagent_id?: string | null;
}

interface SessionGraphProps {
  messages: RawMessage[];
}

export default function SessionGraph({ messages }: SessionGraphProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const flowRef = useRef<{ fitView: (opts?: { padding?: number }) => void }>(null);

  const { nodes, edges, stats } = useMemo(() => {
    if (!messages) return { nodes: [], edges: [], stats: { total: 0, users: 0, assistants: 0, tools: 0 } };
    return buildTree(messages);
  }, [messages]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node<TreeNodeData>) => {
    setSelectedId(node.id);
  }, []);

  const handleFitView = useCallback(() => {
    flowRef.current?.fitView({ padding: 0.15 });
  }, []);

  if (nodes.length === 0) {
    return (
      <p style={{ padding: "12px 16px", fontSize: 14, color: "var(--muted-foreground, #666)" }}>
        No data to display.
      </p>
    );
  }

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <ReactFlow
        ref={flowRef}
        nodes={nodes}
        edges={edges}
        onNodeClick={onNodeClick}
        nodeTypes={nodeTypes}
        defaultEdgeOptions={{
          type: "smoothstep",
          markerEnd: {
            type: MarkerType.ArrowClosed,
            width: 10,
            height: 10,
            color: "#555",
          },
        }}
        fitView
        minZoom={0.05}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
        style={{ background: "var(--background, #141418)" }}
      >
        <Controls
          position="bottom-left"
          style={{
            background: "var(--card, #1e1e2e)",
            border: "1px solid var(--border, #2a2a3a)",
            borderRadius: 6,
          }}
        />
        <Background color="var(--border, #252530)" gap={24} size={1} />
        <Panel position="top-right" style={{ padding: 0 }}>
          <button
            onClick={handleFitView}
            style={{
              padding: "5px 12px",
              borderRadius: 4,
              fontSize: 11,
              cursor: "pointer",
              background: "var(--card, #1e1e2e)",
              border: "1px solid var(--border, #2a2a3a)",
              color: "var(--muted-foreground, #888)",
            }}
          >
            Fit View
          </button>
        </Panel>
      </ReactFlow>

      {/* Stats bar */}
      <div
        style={{
          position: "absolute",
          bottom: 8,
          right: 8,
          display: "flex",
          gap: 12,
          fontSize: 10,
          color: "var(--muted-foreground, #555)",
          fontFamily: "monospace",
          zIndex: 5,
        }}
      >
        <span>{stats.total} nodes</span>
        <span>{stats.users} user</span>
        <span>{stats.assistants} assistant</span>
        <span>{stats.tools} tools</span>
      </div>

      {selectedId && messages && (
        <NodeDetailPanel
          nodeId={selectedId}
          messages={messages}
          onClose={() => setSelectedId(null)}
        />
      )}
    </div>
  );
}
