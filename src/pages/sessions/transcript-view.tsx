import React, { useState, useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionTranscript } from "@/lib/api";
import type { MessageRecord } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import UserBubble from "./user-bubble";
import AIBubble from "./ai-bubble";
import ToolAccordion from "./tool-accordion";
import MetaLine from "./meta-line";
import SummarySection from "./summary-section";

// Grouped message types for rendering
type MessageGroup =
  | { type: "user"; msg: MessageRecord }
  | { type: "assistant"; msg: MessageRecord }
  | { type: "tools"; pairs: Array<{ call: MessageRecord; result?: MessageRecord }> }
  | { type: "meta"; msg: MessageRecord }
  | { type: "summary"; msg: MessageRecord };

export default function TranscriptView({
  sessionId,
}: {
  sessionId: string;
}) {
  const [visibleCount, setVisibleCount] = useState(100);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const { t } = useTranslation();

  const { data: transcript, error, isLoading } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  // Infinite scroll
  useEffect(() => {
    if (!sentinelRef.current) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          setVisibleCount((c) => c + 80);
        }
      },
      { rootMargin: "200px" }
    );
    observer.observe(sentinelRef.current);
    return () => observer.disconnect();
  }, [transcript]);

  if (isLoading) {
    return (
      <div className="px-4 space-y-3 py-3">
        <Skeleton className="h-5 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <Skeleton className="h-20 w-full" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="mx-4 rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3">
        <p className="text-[14px] text-destructive">{t("transcript.error", { message: String(error) })}</p>
      </div>
    );
  }

  if (!transcript || transcript.main_path.length === 0) {
    return (
      <p className="px-4 py-3 text-[14px] text-muted-foreground">{t("transcript.empty")}</p>
    );
  }

  // Build id→message lookup from all messages
  const msgMap = new Map<string, MessageRecord>();
  for (const m of transcript.messages) {
    msgMap.set(m.id, m);
  }

  // Extract main path messages (in order)
  const mainMessages: MessageRecord[] = transcript.main_path
    .map((id) => msgMap.get(id))
    .filter((m): m is MessageRecord => m != null);

  // Group into renderable segments
  const groups = groupMessages(mainMessages);
  const visibleGroups = groups.slice(0, visibleCount);
  const hasMore = groups.length > visibleCount;

  return (
    <div className="px-4">
      <div className="mb-3 text-[14px] text-muted-foreground">
        {t("transcript.messageCount", { count: mainMessages.length })}
        {transcript.branches.length > 0 &&
          ` · ${t("transcript.branchCount", { count: transcript.branches.length })}`}
      </div>

      <div className="space-y-1">
        {visibleGroups.map((group, i) => (
          <React.Fragment key={i}>
            {group.type === "user" && <UserBubble msg={group.msg} />}
            {group.type === "assistant" && <AIBubble msg={group.msg} />}
            {group.type === "tools" && (
              <ToolAccordion
                tools={group.pairs}
                sessionId={sessionId}
              />
            )}
            {group.type === "meta" && <MetaLine msg={group.msg} />}
            {group.type === "summary" && <SummarySection msg={group.msg} />}
          </React.Fragment>
        ))}
      </div>

      {hasMore && <div ref={sentinelRef} className="h-1" />}
    </div>
  );
}

/**
 * Group a flat message list into renderable segments.
 *
 * Rules:
 * - human (kind=message) → UserBubble
 * - assistant (kind=message, no tool_use) → AIBubble
 * - consecutive assistant(tool_use) + user(tool_result) pairs → ToolAccordion
 * - attachment / system → MetaLine
 * - summary → SummarySection
 */
function groupMessages(messages: MessageRecord[]): MessageGroup[] {
  const groups: MessageGroup[] = [];
  let i = 0;

  while (i < messages.length) {
    const msg = messages[i];

    // Summary
    if (msg.entry_type === "summary") {
      groups.push({ type: "summary", msg });
      i++;
      continue;
    }

    // Attachment / System → MetaLine
    if (msg.entry_type === "attachment" || msg.entry_type === "system") {
      groups.push({ type: "meta", msg });
      i++;
      continue;
    }

    // User text message → UserBubble
    if (msg.role === "human" && msg.kind === "message") {
      groups.push({ type: "user", msg });
      i++;
      continue;
    }

    // User tool_result without preceding tool_use → MetaLine
    if (msg.role === "human" && msg.kind === "tool_result") {
      // Orphan tool result — just show as meta
      groups.push({ type: "meta", msg });
      i++;
      continue;
    }

    // Assistant tool_use → collect consecutive tool_use + tool_result pairs
    if (msg.role === "assistant" && msg.kind === "tool_use") {
      const pairs: Array<{ call: MessageRecord; result?: MessageRecord }> = [];

      // If this tool_use also has text content, emit that as AIBubble first
      // Check if there's text before the tool mentions
      const textBeforeTool = msg.content_preview.indexOf("\nTool:");
      if (textBeforeTool > 0) {
        const textContent = msg.content_preview.slice(0, textBeforeTool).trim();
        if (textContent) {
          groups.push({
            type: "assistant",
            msg: { ...msg, content_preview: textContent },
          });
        }
      }

      // Collect this tool_use + next tool_result
      pairs.push({ call: msg });
      i++;

      // Look ahead for matching tool_result
      if (i < messages.length && messages[i].kind === "tool_result") {
        pairs[pairs.length - 1].result = messages[i];
        i++;
      }

      // Collect more consecutive tool_use + tool_result pairs
      while (i < messages.length) {
        if (messages[i].role === "assistant" && messages[i].kind === "tool_use") {
          pairs.push({ call: messages[i] });
          i++;
          if (i < messages.length && messages[i].kind === "tool_result") {
            pairs[pairs.length - 1].result = messages[i];
            i++;
          }
        } else {
          break;
        }
      }

      groups.push({ type: "tools", pairs });
      continue;
    }

    // Assistant text → AIBubble
    if (msg.role === "assistant" && msg.kind === "message") {
      groups.push({ type: "assistant", msg });
      i++;
      continue;
    }

    // Fallback: treat as meta
    groups.push({ type: "meta", msg });
    i++;
  }

  return groups;
}
