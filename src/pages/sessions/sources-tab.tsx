import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionDetail } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { FileText, ShieldCheck } from "lucide-react";

export default function SourcesTab({ sessionId }: { sessionId: string }) {
  const { t } = useTranslation();

  const { data: detail, error } = useQuery({
    queryKey: ["session-detail", sessionId],
    queryFn: () => getSessionDetail(sessionId),
  });

  return (
    <section data-ai-region="sessions-sources" className="w-full">
      {!detail ? (
        error ? (
          <p className="text-[14px] text-destructive">
            {t("sources.error")}
          </p>
        ) : (
          <div className="space-y-2">
            {Array.from({ length: 2 }).map((_, i) => (
              <Skeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        )
      ) : detail.sources.length === 0 ? (
        <p className="rounded-lg border border-border bg-secondary px-3 py-2.5 text-[12px] text-muted-foreground">
          {t("sources.empty")}
        </p>
      ) : (
        <div className="w-full space-y-1.5">
          {detail.sources.map((src) => (
            <div
              key={src.source_id}
              className="flex w-full items-center gap-2 rounded-lg border border-border bg-secondary px-3 py-2"
            >
              <FileText size={14} className="shrink-0 text-muted-foreground" />
              <span className="shrink-0 text-[12px] font-medium text-foreground">{src.source_type}</span>
              <Badge variant="outline" className="shrink-0 gap-1 bg-card px-1.5 py-0.5 text-[12px] text-muted-foreground">
                <ShieldCheck size={10} />
                {src.delete_policy}
              </Badge>
              <span className="min-w-0 truncate font-mono text-[12px] text-muted-foreground">
                {src.path}
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
