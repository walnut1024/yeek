import React from "react";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { User } from "lucide-react";

const UserBubble = React.memo(function UserBubble({
  msg,
}: {
  msg: MessageRecord;
}) {
  const { t } = useTranslation();
  return (
    <article data-ai-item="user-message" className="rounded-xl border border-border bg-secondary px-3 py-3 transition-colors hover:bg-element-hover">
      <div className="mb-2 flex items-center gap-1.5">
        <span className="inline-flex size-6 items-center justify-center rounded-full border border-border bg-card">
          <User size={14} className="text-foreground" />
        </span>
        <span className="text-[13px] font-medium text-foreground">
          {t("user.role")}
        </span>
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
      <div className="max-h-[320px] overflow-y-auto custom-scrollbar pr-1">
        <p className="whitespace-pre-wrap text-[14px] leading-[1.55] text-foreground">
          {msg.content_preview}
        </p>
      </div>
    </article>
  );
});

export default UserBubble;
