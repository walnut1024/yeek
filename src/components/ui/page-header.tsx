import type { ReactNode } from "react";

export function PageHeader({
  title,
  description,
  kicker,
  region,
}: {
  title: ReactNode;
  description?: ReactNode;
  kicker?: ReactNode;
  region?: string;
}) {
  return (
    <header
      {...(region ? { "data-ai-region": region } : {})}
      className="border-b border-border px-3 py-3"
    >
      {kicker && <p className="zed-kicker">{kicker}</p>}
      <h2
        className={`${kicker ? "mt-1" : ""} text-[14px] font-medium leading-none text-foreground`}
      >
        {title}
      </h2>
      {description && (
        <p className="mt-2 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">
          {description}
        </p>
      )}
    </header>
  );
}
