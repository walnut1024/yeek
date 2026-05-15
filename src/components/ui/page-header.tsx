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
      className="border-b border-border bg-card px-4 py-3 sm:px-5"
    >
      {kicker && <p className="zed-kicker">{kicker}</p>}
      <h2
        className={`${kicker ? "mt-1" : ""} text-[19px] font-semibold leading-[1.08] tracking-[-0.02em] text-foreground`}
      >
        {title}
      </h2>
      {description && (
        <p className="mt-1.5 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">
          {description}
        </p>
      )}
    </header>
  );
}
