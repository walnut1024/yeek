// Warm-toned icon system following DESIGN.md palette
// All icons are 12x12 inline SVGs

export function ChevronIcon({ className, expanded }: { className?: string; expanded?: boolean }) {
  return (
    <svg
      width={12}
      height={12}
      viewBox="0 0 16 16"
      fill="none"
      className={`${className} transition-transform duration-200 ${expanded ? "rotate-90" : ""}`}
    >
      <path d="M6 4l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function ChevronUpIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M4 10l4-4 4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function ChevronDownIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function UserIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <circle cx="8" cy="5" r="3" stroke="currentColor" strokeWidth="1.5" />
      <path d="M2 15c0-3.3 2.7-6 6-6s6 2.7 6 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

export function AssistantIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M8 1l1.5 3.5L13 5.5l-2.5 2.5L11 12l-3-2-3 2 .5-4L3 5.5l3.5-1z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

export function ToolIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M4.5 2L2 4.5l2 2a5.5 5.5 0 008 3l-2-2-1.5 1.5-1-1L11 6.5l-2-2L7.5 6l-1-1L8 3.5 6 1.5 4.5 2z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
    </svg>
  );
}

export function SystemIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 5v3M8 10.5v.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

export function FileIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M4 2h5l4 4v8a1 1 0 01-1 1H4a1 1 0 01-1-1V3a1 1 0 011-1z" stroke="currentColor" strokeWidth="1.2" />
      <path d="M9 2v4h4" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

export function SummaryIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <rect x="3" y="2" width="10" height="12" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M6 5h4M6 8h4M6 11h2" stroke="currentColor" strokeWidth="1" strokeLinecap="round" />
    </svg>
  );
}

export function ErrorIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M8 2L1.5 14h13L8 2z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M8 6v3M8 11.5v.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

export function PlanIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M2 14L6 6l4 4 4-8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx="6" cy="6" r="1.2" fill="currentColor" />
      <circle cx="10" cy="10" r="1.2" fill="currentColor" />
      <circle cx="14" cy="2" r="1.2" fill="currentColor" />
    </svg>
  );
}

export function CompactIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <path d="M3 6h10M3 10h10" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
      <path d="M6 3l-3 3 3 3M10 7l3 3-3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function ScheduleIcon({ className }: { className?: string }) {
  return (
    <svg width={12} height={12} viewBox="0 0 16 16" fill="none" className={className}>
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 4v4l3 2" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function WriteIcon({ className }: { className?: string }) {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
      <path d="m15 5 4 4" />
    </svg>
  );
}

export function BashIcon({ className }: { className?: string }) {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  );
}

export function ReadIcon({ className }: { className?: string }) {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
      <path d="M14 2v4a2 2 0 0 0 2 2h4" />
      <path d="M10 9H8" />
      <path d="M16 13H8" />
      <path d="M16 17H8" />
    </svg>
  );
}

const TOOL_ICONS: Record<string, React.FC<{ className?: string }>> = {
  Write: WriteIcon,
  Bash: BashIcon,
  Read: ReadIcon,
  Edit: WriteIcon,
  MultiEdit: WriteIcon,
  Glob: ReadIcon,
  Grep: ReadIcon,
};

export function getToolIcon(toolName: string): React.FC<{ className?: string }> | null {
  return TOOL_ICONS[toolName] ?? null;
}
