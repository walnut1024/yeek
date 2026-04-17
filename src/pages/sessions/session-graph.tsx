import { useState, useMemo, useCallback, useRef } from "react";
import { ReactFlow, Controls, Background, MarkerType, Panel } from "@xyflow/react";
import type { Node } from "@xyflow/react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { getSessionTranscript } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import { buildTree, type TreeNodeData } from "./graph/build-tree";
import { nodeTypes } from "./graph/nodes";
import NodeDetailPanel from "./graph/node-detail-panel";

import "@xyflow/react/dist/style.css";

export default function SessionGraph({ sessionId }: { sessionId: string }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const flowRef = useRef<{ fitView: (opts?: { padding?: number }) => void }>(null);
  const { t } = useTranslation();

  const { data: transcript, isLoading, error } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  const { nodes, edges, stats } = useMemo(() => {
    if (!transcript) return { nodes: [], edges: [], stats: { total: 0, users: 0, assistants: 0, tools: 0 } };
    return buildTree(transcript.messages);
  }, [transcript]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node<TreeNodeData>) => {
    setSelectedId(node.id);
  }, []);

  const handleFitView = useCallback(() => {
    flowRef.current?.fitView({ padding: 0.15 });
  }, []);

  if (isLoading) {
    return (
      <div className="space-y-2 p-3">
        <Skeleton className="h-5 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
      </div>
    );
  }

  if (error) {
    return (
      <p className="px-4 py-3 text-[14px] text-destructive">
        {t("transcript.error", { message: String(error) })}
      </p>
    );
  }

  if (nodes.length === 0) {
    return (
      <p className="px-4 py-3 text-[14px] text-muted-foreground">
        {t("graph.noData")}
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
        style={{ background: "var(--background, #3b414d)" }}
      >
        <Controls
          position="bottom-left"
          style={{
            background: "var(--card, #2f343e)",
            border: "1px solid var(--border, #464b57)",
            borderRadius: 6,
          }}
        />
        <Background color="var(--border, #464b57)" gap={24} size={1} />
        <Panel position="top-right" style={{ padding: 0 }}>
          <button
            onClick={handleFitView}
            style={{
              padding: "5px 12px",
              borderRadius: 4,
              fontSize: 11,
              cursor: "pointer",
              background: "var(--card, #2f343e)",
              border: "1px solid var(--border, #464b57)",
              color: "var(--muted-foreground, #a9afbc)",
            }}
          >
            {t("graph.fitView")}
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
          color: "var(--muted-foreground, #a9afbc)",
          fontFamily: "var(--font-data, monospace)",
          zIndex: 5,
        }}
      >
        <span>{t("graph.statsNodes", { count: stats.total })}</span>
        <span>{t("graph.statsUsers", { count: stats.users })}</span>
        <span>{t("graph.statsAssistants", { count: stats.assistants })}</span>
        <span>{t("graph.statsTools", { count: stats.tools })}</span>
      </div>

      {selectedId && transcript && (
        <NodeDetailPanel
          nodeId={selectedId}
          messages={transcript.messages}
          onClose={() => setSelectedId(null)}
        />
      )}
    </div>
  );
}
