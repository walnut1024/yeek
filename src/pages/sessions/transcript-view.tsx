import React, { useState, useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { getSessionDetail } from "@/lib/api";
import MessageCard from "./message-card";

export default function TranscriptView({ sessionId }: { sessionId: string }) {
  const [visibleCount, setVisibleCount] = useState(100);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const { data: detail, error, isLoading } = useQuery({
    queryKey: ["session-detail", sessionId],
    queryFn: () => getSessionDetail(sessionId),
  });

  const mainMessages = detail?.messages?.filter((m) => !m.is_sidechain) ?? [];

  // Infinite scroll via IntersectionObserver
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
  }, [detail]);

  if (isLoading) {
    return <p className="px-4 py-3 text-sm text-muted-foreground">Loading transcript...</p>;
  }

  if (error) {
    return (
      <div className="mx-4 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3">
        <p className="text-sm text-destructive">Error: {String(error)}</p>
      </div>
    );
  }

  if (mainMessages.length === 0) {
    return <p className="px-4 py-3 text-sm text-muted-foreground">No messages</p>;
  }

  const visibleMessages = mainMessages.slice(0, visibleCount);
  const hasMore = mainMessages.length > visibleCount;

  return (
    <div className="px-4">
      <div className="mb-3 text-[11px] text-muted-foreground/50">
        {mainMessages.length} messages
      </div>

      <div>
        {visibleMessages.map((msg, i) => {
          const isTopLevel = msg.role === "human" || msg.role === "assistant" || msg.entry_type === "summary";
          return (
            <React.Fragment key={msg.id}>
              {i > 0 && isTopLevel && (
                <div className="my-2 border-t border-border/40" />
              )}
              <MessageCard msg={msg} sessionId={sessionId} />
            </React.Fragment>
          );
        })}
      </div>

      {hasMore && <div ref={sentinelRef} className="h-1" />}
    </div>
  );
}
