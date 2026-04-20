import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionDetail } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";

export default function SourcesTab({ sessionId }: { sessionId: string }) {
  const { t } = useTranslation();

  const { data: detail, error } = useQuery({
    queryKey: ["session-detail", sessionId],
    queryFn: () => getSessionDetail(sessionId),
  });

  if (!detail) {
    if (error) {
      return (
        <p className="text-[13px] text-destructive">
          {t("sources.error")}
        </p>
      );
    }
    return (
      <div className="space-y-2">
        {Array.from({ length: 2 }).map((_, i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    );
  }

  if (detail.sources.length === 0) {
    return (
      <p className="text-[13px] text-muted-foreground/50">
        {t("sources.empty")}
      </p>
    );
  }

  return (
    <div className="space-y-3">
      <div className="space-y-2">
        {detail.sources.map((src) => (
          <div
            key={src.source_id}
            className="rounded-sm border border-border px-2 py-1.5"
          >
            <span className="break-all font-mono text-[13px] text-muted-foreground">
              {src.path}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
