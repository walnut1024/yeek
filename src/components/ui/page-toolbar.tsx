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
      className="flex items-center justify-between border-b border-border px-3 py-2"
    >
      {children}
    </div>
  );
}
