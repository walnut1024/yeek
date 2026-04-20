import React, { useState, useEffect, useRef, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionTranscript } from "@/lib/api";
import { TRANSCRIPT_INITIAL_COUNT, TRANSCRIPT_LOAD_MORE } from "@/lib/constants";
import type { MessageRecord } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { ChevronUpIcon, ChevronDownIcon } from "@/components/icons";
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
  const [visibleCount, setVisibleCount] = useState(TRANSCRIPT_INITIAL_COUNT);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const topAnchorRef = useRef<HTMLDivElement>(null);
  const bottomAnchorRef = useRef<HTMLDivElement>(null);
  const pendingJumpBottomRef = useRef(false);
  const { t } = useTranslation();

  const { data: transcript, error, isLoading } = useQuery({
    queryKey: ["session-transcript", sessionId],
    queryFn: () => getSessionTranscript(sessionId),
  });

  // Compute groups from transcript (memoized, always available)
  const { groups, mainCount } = useMemo(() => {
    if (!transcript || transcript.main_path.length === 0)
      return { groups: [], mainCount: 0 };
    const msgMap = new Map<string, MessageRecord>();
    for (const m of transcript.messages) {
      msgMap.set(m.id, m);
    }
    const mainMessages: MessageRecord[] = transcript.main_path
      .map((id) => msgMap.get(id))
      .filter((m): m is MessageRecord => m != null);
    return { groups: groupMessages(mainMessages), mainCount: mainMessages.length };
  }, [transcript]);

  // Infinite scroll
  useEffect(() => {
    if (!sentinelRef.current) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          setVisibleCount((c) => c + TRANSCRIPT_LOAD_MORE);
        }
      },
      { rootMargin: "200px" }
    );
    observer.observe(sentinelRef.current);
    return () => observer.disconnect();
  }, [transcript]);

  // Pending jump to bottom after loading more messages
  useEffect(() => {
    if (!pendingJumpBottomRef.current) return;
    if (visibleCount < groups.length) return;
    pendingJumpBottomRef.current = false;
    requestAnimationFrame(() => {
      bottomAnchorRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "end",
      });
    });
  }, [visibleCount, groups.length]);

  // --- Early returns (after all hooks) ---
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

  if (groups.length === 0) {
    return (
      <p className="px-4 py-3 text-[14px] text-muted-foreground">{t("transcript.empty")}</p>
    );
  }

  const visibleGroups = groups.slice(0, visibleCount);
  const hasMore = groups.length > visibleCount;
  const branchCount = transcript?.branches.length ?? 0;

  const scrollToTop = () => {
    topAnchorRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const scrollToBottom = () => {
    if (visibleCount < groups.length) {
      pendingJumpBottomRef.current = true;
      setVisibleCount(groups.length);
      return;
    }
    bottomAnchorRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  };

  return (
    <div className="relative px-4">
      <div ref={topAnchorRef} />
      <div className="mb-3 text-[14px] text-muted-foreground">
        {t("transcript.messageCount", { count: mainCount })}
        {branchCount > 0 &&
          ` · ${t("transcript.branchCount", { count: branchCount })}`}
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
      <div ref={bottomAnchorRef} />
      <div className="fixed bottom-6 right-6 z-30 flex flex-col gap-1.5 rounded-md border border-border bg-card/95 p-1.5 backdrop-blur-sm">
        <Button
          variant="outline"
          size="icon-sm"
          onClick={scrollToTop}
          aria-label="Scroll to top"
          title="Scroll to top"
        >
          <ChevronUpIcon />
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          onClick={scrollToBottom}
          aria-label="Scroll to bottom"
          title="Scroll to bottom"
        >
          <ChevronDownIcon />
        </Button>
      </div>
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
