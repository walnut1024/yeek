import { useTranslation } from "react-i18next";
import { LayoutGrid, MessageSquare, ShoppingBag, Code, Settings } from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

type Section = "dashboard" | "sessions" | "marketplace" | "settings" | "proxy";

interface AppSidebarProps {
  section: Section;
  onSectionChange: (section: Section) => void;
  totalSessions?: number;
}

export function AppSidebar({ section, onSectionChange, totalSessions }: AppSidebarProps) {
  const { t } = useTranslation();

  const navItems: { key: Section; label: string; icon: React.ComponentType<{ size?: number }> }[] = [
    { key: "dashboard", label: t("nav.dashboard"), icon: LayoutGrid },
    { key: "sessions", label: t("nav.sessions"), icon: MessageSquare },
    { key: "marketplace", label: t("nav.marketplace"), icon: ShoppingBag },
    { key: "proxy", label: t("nav.proxy"), icon: Code },
    { key: "settings", label: t("nav.settings"), icon: Settings },
  ];

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <p className="zed-kicker">{t("app.title")}</p>
        <p className="mt-0.5 truncate text-[11px] leading-[1.4] text-muted-foreground group-data-[collapsible=icon]:hidden">
          {t("app.sessionBrowser")}
        </p>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map(({ key, label, icon: Icon }) => (
                <SidebarMenuItem key={key}>
                  <SidebarMenuButton
                    isActive={section === key}
                    tooltip={label}
                    onClick={() => onSectionChange(key)}
                  >
                    <Icon size={16} />
                    <span>{label}</span>
                    {key === "sessions" && totalSessions != null && (
                      <SidebarMenuBadge>{totalSessions}</SidebarMenuBadge>
                    )}
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
