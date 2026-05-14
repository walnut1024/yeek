import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getSessionDetail } from "@/lib/api";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";

export default function SourcesTab({ sessionId }: { sessionId: string }) {
  const { t } = useTranslation();

  const { data: detail, error } = useQuery({
    queryKey: ["session-detail", sessionId],
    queryFn: () => getSessionDetail(sessionId),
  });

  return (
    <section data-ai-region="sessions-sources">
      <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="zed-kicker">{t("sources.title")}</p>
          <p className="mt-0.5 truncate text-[12px] text-muted-foreground">{t("sources.help")}</p>
        </div>
        {detail && (
          <Badge variant="outline" className="bg-secondary px-2 py-0.5 text-[11px] text-muted-foreground">
            {t("sources.count", { count: detail.sources.length })}
          </Badge>
        )}
      </div>
      {!detail ? (
        error ? (
          <p className="text-[13px] text-destructive">
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
        <div className="space-y-2">
          <div className="space-y-1.5">
            {detail.sources.map((src) => (
              <div
                key={src.source_id}
                className="rounded-lg border border-border bg-secondary px-3 py-2"
              >
                <div className="mb-0.5 flex items-center justify-between gap-2">
                  <span className="zed-kicker">{src.source_type}</span>
                  <Badge variant="outline" className="bg-card px-1.5 py-0.5 text-[11px] text-muted-foreground">
                    {src.delete_policy}
                  </Badge>
                </div>
                <span className="break-all font-mono text-[12px] text-muted-foreground">
                  {src.path}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
