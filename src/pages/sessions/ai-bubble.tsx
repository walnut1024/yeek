import React from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { Sparkles } from "lucide-react";

const AIBubble = React.memo(function AIBubble({
  msg,
}: {
  msg: MessageRecord;
}) {
  const { t } = useTranslation();
  return (
    <article data-ai-item="assistant-message" className="rounded-xl border border-border bg-[var(--editor)] px-3 py-3 transition-colors hover:bg-element-hover">
      <div className="mb-2 flex items-center gap-1.5">
        <span className="inline-flex size-6 items-center justify-center rounded-full border border-border bg-card">
          <Sparkles size={14} className="text-muted-foreground" />
        </span>
        <span className="text-[13px] font-medium text-muted-foreground">
          {t("ai.role")}
        </span>
        {msg.model && (
          <span className="rounded-full border border-border bg-secondary px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
            {msg.model}
          </span>
        )}
        {msg.timestamp && (
          <span className="text-[12px] text-muted-foreground/80">
            {new Date(msg.timestamp).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
              hour12: false,
            })}
          </span>
        )}
      </div>
      <div className="max-h-[520px] max-w-[72ch] overflow-y-auto custom-scrollbar pr-1 text-[14px] leading-[1.6] text-foreground">
        <Markdown
          remarkPlugins={[remarkGfm]}
          components={{
            pre: ({ children }) => (
              <pre
                className="my-2 overflow-x-auto rounded-md border border-border bg-secondary p-2.5"
              >
                {children}
              </pre>
            ),
            code: ({ className, children, ...props }) => {
              const isInline = !className;
              return isInline ? (
                <code
                  className="rounded-sm border border-border bg-secondary px-1 py-0.5 font-mono text-[13px] text-muted-foreground"
                  {...props}
                >
                  {children}
                </code>
              ) : (
                <code className={`${className} font-mono text-[13px]`} {...props}>
                  {children}
                </code>
              );
            },
            p: ({ children }) => <p className="mb-1.5 last:mb-0">{children}</p>,
            ul: ({ children }) => (
              <ul className="mb-1.5 list-disc pl-5">{children}</ul>
            ),
            ol: ({ children }) => (
              <ol className="mb-1.5 list-decimal pl-5">{children}</ol>
            ),
          }}
        >
          {msg.content_preview}
        </Markdown>
      </div>
    </article>
  );
});

export default AIBubble;
