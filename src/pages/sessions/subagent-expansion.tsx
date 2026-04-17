import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSubagentMessages } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import UserBubble from "./user-bubble";
import AIBubble from "./ai-bubble";
import MetaLine from "./meta-line";

export default function SubagentExpansion(props: {
  sessionId: string;
  subagentId: string;
  agentType: string | null;
  description: string | null;
}) {
  const { sessionId, subagentId } = props;
  const { t } = useTranslation();
  const { data: messages, isLoading } = useQuery({
    queryKey: ["subagent-messages", sessionId, subagentId],
    queryFn: () => getSubagentMessages(sessionId, subagentId),
  });

  if (isLoading) {
    return (
      <div className="relative ml-[7px] mt-1 border-l border-border/40 pl-3.5 space-y-1">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-8 w-full" />
      </div>
    );
  }

  if (!messages || messages.length === 0) {
    return (
      <div className="relative ml-[7px] mt-1 border-l border-border/40 pl-3.5">
        <p className="text-[14px] text-muted-foreground/50">
          {t("subagent.empty")}
        </p>
      </div>
    );
  }

  return (
    <div className="relative ml-[7px] mt-1 border-l border-border/40 pl-3.5 space-y-1">
      {messages.map((msg) => {
        if (msg.role === "human" && msg.kind === "message") {
          return <UserBubble key={msg.id} msg={msg} />;
        }
        if (msg.role === "assistant" && msg.kind === "message") {
          return <AIBubble key={msg.id} msg={msg} />;
        }
        // tool_use, tool_result, attachment, system → MetaLine
        return <MetaLine key={msg.id} msg={msg} />;
      })}
    </div>
  );
}
