import { SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import claudeIcon from "@/assets/claude-icon.svg";
import openaiIcon from "@/assets/openai-icon.svg";

export function SiteHeader({
  section,
  agentFilter,
  onAgentFilterChange,
}: {
  section?: string;
  agentFilter?: string;
  onAgentFilterChange?: (v: string) => void;
}) {
  return (
    <header className="flex h-[--header-height] shrink-0 items-center gap-2 border-b border-border bg-card px-2 transition-[width,height] ease-linear">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-2 h-4" />
      {section === "sessions" && agentFilter !== undefined && onAgentFilterChange && (
        <Tabs value={agentFilter} onValueChange={onAgentFilterChange}>
          <TabsList variant="line" className="h-6 gap-0.5 border-0 bg-transparent p-0">
            <TabsTrigger value="claude_code" className="h-6 rounded-md px-2 text-[12px]">
              <img src={claudeIcon} alt="Claude" className="h-3 w-auto" />
              Claude Code
            </TabsTrigger>
            <TabsTrigger value="codex" className="h-6 rounded-md px-2 text-[12px]">
              <img src={openaiIcon} alt="Codex" className="h-4 w-auto" />
              Codex
            </TabsTrigger>
          </TabsList>
        </Tabs>
      )}
    </header>
  );
}
