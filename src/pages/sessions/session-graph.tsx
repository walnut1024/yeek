import { useState, useMemo, useCallback } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Controls,
  Background,
  MarkerType,
  Panel,
  useReactFlow,
  useNodesState,
  useEdgesState,
} from "@xyflow/react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { getSessionTranscript } from "@/lib/api";
import { GRAPH_MAX_NODES } from "@/lib/constants";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { buildTree } from "./graph/build-tree";
import { nodeTypes } from "./graph/nodes";
import NodeDetailPanel from "./graph/node-detail-panel";

import "@xyflow/react/dist/style.css";

function GraphCanvas({
  nodes: layoutedNodes,
  edges: layoutedEdges,
  onSelectNode,
}: {
  nodes: ReturnType<typeof buildTree>["nodes"];
  edges: ReturnType<typeof buildTree>["edges"];
  onSelectNode: (id: string) => void;
}) {
  const { fitView } = useReactFlow();
  const { t } = useTranslation();
  const [nodes, , onNodesChange] = useNodesState(layoutedNodes);
  const [edges, , onEdgesChange] = useEdgesState(layoutedEdges);

  const onNodeClick = useCallback((_: React.MouseEvent, node: { id: string }) => {
    onSelectNode(node.id);
  }, [onSelectNode]);

  const handleFitView = useCallback(() => {
    fitView({ padding: 0.15 });
  }, [fitView]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const onInit = useCallback((instance: any) => {
    instance.fitView({ padding: 0.15 });
  }, []);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onNodeClick={onNodeClick}
      onInit={onInit}
      nodeTypes={nodeTypes}
      defaultEdgeOptions={{
        type: "default",
        markerEnd: {
          type: MarkerType.ArrowClosed,
          width: 12,
          height: 12,
          color: "#8b9bb4",
        },
        style: { stroke: "#8b9bb4", strokeWidth: 2 },
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
        <Button
          variant="outline"
          size="sm"
          onClick={handleFitView}
          className="text-[11px]"
        >
          {t("graph.fitView")}
        </Button>
      </Panel>
    </ReactFlow>
  );
}

export default function SessionGraph({ sessionId }: { sessionId: string }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { t } = useTranslation();

  const { data: transcript, isLoading, error } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  const { nodes, edges, stats, truncated } = useMemo(() => {
    if (!transcript || transcript.main_path.length === 0)
      return { nodes: [], edges: [], stats: { total: 0, users: 0, assistants: 0, tools: 0 }, truncated: false };
    // Only include messages on the main path to keep graph manageable
    const mainSet = new Set(transcript.main_path);
    let mainMessages = transcript.messages.filter((m) => mainSet.has(m.id));
    let wasTruncated = false;
    if (mainMessages.length > GRAPH_MAX_NODES) {
      mainMessages = mainMessages.slice(0, GRAPH_MAX_NODES);
      wasTruncated = true;
    }
    const result = buildTree(mainMessages);
    return { ...result, truncated: wasTruncated };
  }, [transcript]);

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
    <div style={{ width: "100%", height: "100%", position: "relative", minHeight: 400 }}>
      {truncated && (
        <div className="absolute left-1/2 top-2 z-10 -translate-x-1/2 rounded-md border border-border bg-card/95 px-3 py-1.5 text-[12px] text-muted-foreground backdrop-blur-sm">
          {t("graph.truncated", { max: GRAPH_MAX_NODES, total: transcript!.main_path.length })}
        </div>
      )}
      <ReactFlowProvider>
        <GraphCanvas nodes={nodes} edges={edges} onSelectNode={setSelectedId} />
      </ReactFlowProvider>

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
