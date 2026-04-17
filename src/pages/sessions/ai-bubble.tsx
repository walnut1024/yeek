import React from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { AssistantIcon } from "@/components/icons";

const AIBubble = React.memo(function AIBubble({
  msg,
}: {
  msg: MessageRecord;
}) {
  const { t } = useTranslation();
  return (
    <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2 transition-colors hover:bg-accent/40">
      <div className="mb-0.5 flex items-center gap-1.5">
        <AssistantIcon className="text-muted-foreground" />
        <span className="text-[13px] font-medium text-muted-foreground">
          {t("ai.role")}
        </span>
        {msg.model && (
          <span className="font-mono text-[12px] text-muted-foreground">
            {msg.model}
          </span>
        )}
        {msg.timestamp && (
          <span className="text-[12px] text-muted-foreground">
            {new Date(msg.timestamp).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
              hour12: false,
            })}
          </span>
        )}
        <span className="font-mono text-[11px] text-muted-foreground/50">
          uuid:{msg.id}
        </span>
      </div>
      <div className="max-h-[500px] overflow-y-auto custom-scrollbar pr-1 text-[14px] leading-[1.55] text-foreground">
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
    </div>
  );
});

export default AIBubble;
