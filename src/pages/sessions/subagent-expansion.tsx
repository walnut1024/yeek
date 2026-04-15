import { useQuery } from "@tanstack/react-query";
import { getSubagentMessages } from "@/lib/api";
import type { MessageRecord } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import MessageCard from "./message-card";

export default function SubagentExpansion({
  sessionId,
  subagentId,
  agentType,
  description,
}: {
  sessionId: string;
  subagentId: string;
  agentType: string | null;
  description: string | null;
}) {
  const { data: messages, isLoading } = useQuery({
    queryKey: ["subagent-messages", sessionId, subagentId],
    queryFn: () => getSubagentMessages(sessionId, subagentId),
  });

  if (isLoading) {
    return (
      <div className="ml-3 mt-1 space-y-1 border-l-2 border-primary/20 pl-3">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
      </div>
    );
  }

  if (!messages || messages.length === 0) {
    return (
      <div className="ml-3 mt-1 border-l-2 border-primary/20 pl-3">
        <p className="text-[10px] text-muted-foreground/50">
          No subagent messages found
        </p>
      </div>
    );
  }

  return (
    <div className="ml-3 mt-1 space-y-1 border-l-2 border-primary/20 pl-3">
      {messages.map((msg) => (
        <MessageCard
          key={msg.id}
          msg={msg}
          depth={0}
          sessionId={`${sessionId}:${subagentId}`}
        />
      ))}
    </div>
  );
}
