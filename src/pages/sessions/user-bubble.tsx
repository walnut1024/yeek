import React from "react";
import { useTranslation } from "react-i18next";
import type { MessageRecord } from "@/lib/api";
import { UserIcon } from "@/components/icons";

const UserBubble = React.memo(function UserBubble({
  msg,
}: {
  msg: MessageRecord;
}) {
  const { t } = useTranslation();
  return (
    <div className="-mx-1 rounded-md border border-border/60 bg-[var(--editor)] px-2.5 py-2 transition-colors hover:bg-accent/40">
      <div className="mb-0.5 flex items-center gap-1.5">
        <UserIcon className="text-muted-foreground" />
        <span className="text-[13px] font-medium text-foreground">
          {t("user.role")}
        </span>
        {msg.timestamp && (
          <span className="text-[12px] text-muted-foreground">
            {new Date(msg.timestamp).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
              hour12: false,
            })}
          </span>
        )}
      </div>
      <div className="max-h-[300px] overflow-y-auto custom-scrollbar pr-1">
        <p className="whitespace-pre-wrap text-[14px] leading-[1.55] text-foreground">
          {msg.content_preview}
        </p>
      </div>
    </div>
  );
});

export default UserBubble;
