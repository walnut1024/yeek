import type { ReactNode } from "react";

export function PageToolbar({
  children,
  region,
}: {
  children: ReactNode;
  region?: string;
}) {
  return (
    <div
      {...(region ? { "data-ai-region": region } : {})}
      className="flex min-h-11 flex-wrap items-center justify-between gap-2.5 border-b border-border bg-card px-4 py-2.5 sm:px-5"
    >
      {children}
    </div>
  );
}
