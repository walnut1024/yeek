import { useQuery } from "@tanstack/react-query";
import { browseSessions } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { formatRelativeTime } from "@/lib/formatters";

export default function MemoryPage() {
  const { data, isLoading } = useQuery({
    queryKey: ["sessions", "pinned"],
    queryFn: () =>
      browseSessions({
        limit: 100,
        visibility: "visible",
        pinned_only: true,
      }),
  });

  const sessions = data?.sessions ?? [];

  return (
    <div className="h-full overflow-auto p-6">
      <h2 className="text-sm font-semibold mb-4">Memory</h2>

      {isLoading ? (
        <div className="space-y-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-16 w-full" />
          ))}
        </div>
      ) : sessions.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20">
          <p className="text-sm text-muted-foreground">No pinned sessions</p>
          <p className="mt-1 text-xs text-muted-foreground/50">
            Pin sessions from the Sessions page to curate your memory
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <div
              key={s.id}
              className="rounded-lg border border-border p-3 space-y-1.5"
            >
              <p className="text-sm font-medium leading-tight">
                {s.title || s.id.slice(0, 12)}
              </p>
              <div className="flex flex-wrap items-center gap-1.5 text-[10px] text-muted-foreground">
                <Badge variant="outline" className="px-1 py-0 text-[9px] font-normal">
                  {s.agent === "claude_code" ? "Claude" : s.agent}
                </Badge>
                {s.project_path && (
                  <span>{s.project_path.split("/").pop()}</span>
                )}
                <span>{s.message_count} msgs</span>
                <span>{formatRelativeTime(s.updated_at)}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
