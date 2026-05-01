import { useState, useMemo } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  getProxyStatus,
  startProxy,
  stopProxy,
  restartProxy,
  getProxyConfig,
  updateProxyConfig,
  getProxyLogs,
  type ProxyConfig,
  type ProxyProviderConfig,
} from "@/lib/api";
import { useLocalStorage } from "@/lib/hooks";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";

const FILTERS = ["All", "DeepSeek", "OpenAI", "Anthropic", "Zhipu", "Ollama", "Custom"] as const;

export default function ProxyPage() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [launchAtStartup, setLaunchAtStartup] = useLocalStorage("yeek:proxy-launch-at-startup", false);
  const [editingProvider, setEditingProvider] = useState<string | null>(null);
  const [pendingRestart, setPendingRestart] = useState(false);
  const [filter, setFilter] = useState<string>("All");
  const [showLogs, setShowLogs] = useState(false);
  const [formName, setFormName] = useState("");
  const [formFormat, setFormFormat] = useState("chat_completions");
  const [formBaseUrl, setFormBaseUrl] = useState("");
  const [formApiKeyEnv, setFormApiKeyEnv] = useState("");
  const [formModels, setFormModels] = useState("");

  const { data: status } = useQuery({
    queryKey: ["proxy-status"],
    queryFn: getProxyStatus,
    refetchInterval: 3000,
    staleTime: 2000,
    gcTime: 5 * 60 * 1000,
  });

  const { data: config } = useQuery({
    queryKey: ["proxy-config"],
    queryFn: getProxyConfig,
    staleTime: 30 * 60 * 1000,
    gcTime: 60 * 60 * 1000,
  });

  const { data: logs, refetch: refetchLogs } = useQuery({
    queryKey: ["proxy-logs"],
    queryFn: () => getProxyLogs(50),
    enabled: showLogs,
    refetchInterval: showLogs ? 3000 : false,
  });

  const startMut = useMutation({
    mutationFn: startProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxy-status"] }),
  });

  const stopMut = useMutation({
    mutationFn: stopProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxy-status"] }),
  });

  const restartMut = useMutation({
    mutationFn: restartProxy,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["proxy-status"] });
      setPendingRestart(false);
    },
  });

  const saveConfigMut = useMutation({
    mutationFn: (cfg: ProxyConfig) => updateProxyConfig(cfg),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["proxy-config"] });
      setPendingRestart(true);
    },
  });

  const toggleProvider = (name: string, p: ProxyProviderConfig) => {
    if (!config || p.kind !== "builtin") return;
    const updated = {
      ...config,
      providers: {
        ...config.providers,
        [name]: { ...p, enabled: !p.enabled },
      },
    };
    saveConfigMut.mutate(updated);
  };

  const openEditProvider = (name: string, p: ProxyProviderConfig) => {
    setEditingProvider(name);
    setFormName(name);
    setFormFormat(p.format ?? "chat_completions");
    setFormBaseUrl(p.base_url);
    setFormApiKeyEnv(p.api_key_env ?? "");
    setFormModels((p.models ?? []).join(", "));
  };

  const openAddProvider = () => {
    setEditingProvider("__new__");
    setFormName("");
    setFormFormat("chat_completions");
    setFormBaseUrl("");
    setFormApiKeyEnv("");
    setFormModels("");
  };

  const saveProvider = () => {
    if (!config || !formName) return;
    const updated: ProxyConfig = {
      ...config,
      providers: {
        ...config.providers,
        [formName]: {
          kind: null,
          format: formFormat || null,
          base_url: formBaseUrl,
          api_key_env: formApiKeyEnv || null,
          models: formModels.split(",").map((s) => s.trim()).filter(Boolean),
          enabled: true,
        },
      },
    };
    if (editingProvider && editingProvider !== "__new__" && editingProvider !== formName) {
      delete updated.providers[editingProvider];
    }
    saveConfigMut.mutate(updated);
    setEditingProvider(null);
  };

  const deleteProvider = (name: string) => {
    if (!config) return;
    const updated = { ...config };
    delete updated.providers[name];
    saveConfigMut.mutate(updated);
  };

  // Filter providers
  const filteredProviders = useMemo(() => {
    if (!config || !config.providers) return [];
    const entries = Object.entries(config.providers);
    if (filter === "All") return entries;
    if (filter === "Custom") return entries.filter(([, p]) => !p.kind || p.kind !== "builtin");
    return entries.filter(([name]) => name.toLowerCase().includes(filter.toLowerCase()));
  }, [config, filter]);

  const isRunning = status?.running ?? false;
  const isPending = startMut.isPending || stopMut.isPending || restartMut.isPending;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <div className="flex flex-col gap-2 border-b border-border px-3 pb-3">
        <h2 className="text-[14px] font-medium leading-none text-foreground">{t("proxy.title")}</h2>
        <p className="mt-2 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">{t("proxy.description")}</p>
      </div>
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <Badge
            variant={isRunning ? "default" : "secondary"}
            className="h-5 px-1.5 text-[11px] font-medium uppercase tracking-[0.06em]"
          >
            {isRunning ? "● Running" : "○ Stopped"}
          </Badge>
          {isRunning ? (
            <Button variant="outline" size="sm" className="h-7 rounded-md px-2.5 text-[13px]"
              disabled={isPending} onClick={() => stopMut.mutate()}>Stop</Button>
          ) : (
            <Button size="sm" className="h-7 rounded-md px-2.5 text-[13px]"
              disabled={isPending} onClick={() => startMut.mutate()}>Start</Button>
          )}
          {pendingRestart && (
            <Button variant="outline" size="sm"
              className="h-7 rounded-md px-2.5 text-[13px] border-chart-3/50 text-chart-3"
              onClick={() => restartMut.mutate()}>Restart to apply</Button>
          )}
        </div>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-1 p-2">
          {/* Status card */}
          <div className="surface-card overflow-hidden">
            <div className="flex items-center gap-4 px-3 py-2.5 text-[13px]">
              <span className="text-muted-foreground">Listen</span>
              <span className="font-mono text-[12px] text-foreground">{status?.listen_addr ?? "—"}</span>
              <Separator orientation="vertical" className="h-4" />
              <span className="text-muted-foreground">Version</span>
              <span className="font-mono text-[12px] text-foreground">{status?.version ?? "—"}</span>
              <Separator orientation="vertical" className="h-4" />
              <label className="flex items-center gap-1.5 text-[13px] cursor-pointer">
                <input type="checkbox" checked={launchAtStartup}
                  onChange={(e) => setLaunchAtStartup(e.target.checked)}
                  className="h-3.5 w-3.5 accent-primary" />
                Launch at startup
              </label>
            </div>
            {status?.unexpected_exit && (
              <div className="border-t border-border px-3 py-1.5 text-[12px] text-destructive">
                Proxy exited unexpectedly — check logs below.
              </div>
            )}
          </div>

          {/* Provider section header */}
          <div className="flex items-center justify-between px-1 py-1.5">
            <span className="zed-kicker">Providers</span>
            <Button variant="outline" size="sm" className="h-6 rounded-md px-2 text-[12px]"
              onClick={openAddProvider}>+ Custom</Button>
          </div>

          {/* Filter chips */}
          <div className="flex items-center gap-1 pb-1">
            {FILTERS.map((f) => {
              const active = filter === f;
              return (
                <button key={f} type="button" onClick={() => setFilter(f)}
                  className={`rounded-sm px-2 py-0.5 text-[11px] font-medium uppercase tracking-[0.06em] transition-colors ${
                    active
                      ? "bg-secondary text-foreground"
                      : "text-muted-foreground hover:bg-secondary/50 hover:text-foreground"
                  }`}>
                  {f}
                </button>
              );
            })}
          </div>

          {/* Provider cards */}
          {filteredProviders.map(([name, p]) => {
            const isBuiltin = p.kind === "builtin";
            const isEnabled = p.enabled !== false;
            return (
              <div key={name} className={`surface-card overflow-hidden ${!isEnabled ? "opacity-50" : ""}`}>
                <div className="flex items-center justify-between px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-medium text-foreground">{name}</span>
                      {isBuiltin && (
                        <Badge variant="secondary" className="h-4 px-1 text-[10px] uppercase tracking-[0.06em]">
                          built-in
                        </Badge>
                      )}
                      {name === config?.default_provider && (
                        <Badge variant="default" className="h-4 px-1 text-[10px] uppercase tracking-[0.06em]">
                          default
                        </Badge>
                      )}
                      {!isEnabled && (
                        <Badge variant="secondary" className="h-4 px-1 text-[10px] uppercase tracking-[0.06em]">
                          disabled
                        </Badge>
                      )}
                    </div>
                    <div className="mt-0.5 flex items-center gap-2 text-[12px] text-muted-foreground">
                      <span className="font-mono">{p.base_url}</span>
                      {p.format && (
                        <Badge variant="secondary" className="h-3.5 px-1 text-[10px] uppercase tracking-[0.06em]">
                          {p.format.replace("_", " ")}
                        </Badge>
                      )}
                    </div>
                    {(p.models?.length ?? 0) > 0 && (
                      <div className="mt-0.5 text-[12px] text-muted-foreground">{p.models.join(", ")}</div>
                    )}
                    {p.api_key_env && (
                      <div className="mt-0.5 text-[12px] text-muted-foreground">key: ${p.api_key_env}</div>
                    )}
                  </div>
                  <div className="flex items-center gap-1 ml-2 shrink-0">
                    {isBuiltin && (
                      <Button variant="outline" size="sm"
                        className={`h-6 rounded-md px-2 text-[11px] ${isEnabled ? "" : "border-chart-2/50 text-chart-2"}`}
                        onClick={() => toggleProvider(name, p)}>
                        {isEnabled ? "Disable" : "Enable"}
                      </Button>
                    )}
                    <Button variant="outline" size="sm" className="h-6 w-6 rounded-md p-0 text-[14px]"
                      onClick={() => openEditProvider(name, p)}>✎</Button>
                    {!isBuiltin && (
                      <Button variant="outline" size="sm"
                        className="h-6 w-6 rounded-md p-0 text-[14px] border-destructive/30 text-destructive"
                        onClick={() => deleteProvider(name)}>✕</Button>
                    )}
                  </div>
                </div>
              </div>
            );
          })}

          {/* Log panel toggle */}
          <div className="surface-card overflow-hidden mt-2">
            <button
              type="button"
              className="flex w-full items-center justify-between px-3 py-1.5 text-[12px] text-muted-foreground hover:text-foreground"
              onClick={() => { setShowLogs(!showLogs); if (!showLogs) refetchLogs(); }}
            >
              <span>Logs</span>
              <span className="text-[10px]">{showLogs ? "▲" : "▼"}</span>
            </button>
            {showLogs && (
              <pre className="border-t border-border px-3 py-2 text-[11px] font-mono text-muted-foreground whitespace-pre-wrap max-h-48 overflow-auto select-text">
                {logs || "No log output yet."}
              </pre>
            )}
          </div>
        </div>
      </ScrollArea>

      {/* Provider edit dialog */}
      <AlertDialog open={!!editingProvider} onOpenChange={(open) => !open && setEditingProvider(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {editingProvider === "__new__" ? "Add Custom Provider" : `Edit: ${editingProvider}`}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {config?.providers[editingProvider ?? ""]?.kind === "builtin"
                ? "Built-in provider — base URL and format are pre-set."
                : "Configure the provider connection details."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="space-y-3 py-2">
            <div>
              <label className="text-[13px] text-muted-foreground">Name</label>
              <input className="zed-input mt-0.5 w-full text-[13px]"
                value={formName} onChange={(e) => setFormName(e.target.value)}
                placeholder="e.g. My Provider" />
            </div>
            <div>
              <label className="text-[13px] text-muted-foreground">Format</label>
              <select className="zed-input mt-0.5 w-full text-[13px]"
                value={formFormat} onChange={(e) => setFormFormat(e.target.value)}>
                <option value="chat_completions">Chat Completions</option>
                <option value="anthropic_messages">Anthropic Messages</option>
              </select>
            </div>
            <div>
              <label className="text-[13px] text-muted-foreground">Base URL</label>
              <input className="zed-input mt-0.5 w-full font-mono text-[12px]"
                value={formBaseUrl} onChange={(e) => setFormBaseUrl(e.target.value)}
                placeholder="https://api.example.com" />
            </div>
            <div>
              <label className="text-[13px] text-muted-foreground">API Key Env Var</label>
              <input className="zed-input mt-0.5 w-full font-mono text-[12px]"
                value={formApiKeyEnv} onChange={(e) => setFormApiKeyEnv(e.target.value)}
                placeholder="e.g. MY_API_KEY" />
            </div>
            <div>
              <label className="text-[13px] text-muted-foreground">Models (comma-separated)</label>
              <input className="zed-input mt-0.5 w-full text-[13px]"
                value={formModels} onChange={(e) => setFormModels(e.target.value)}
                placeholder="e.g. gpt-4, gpt-3.5" />
            </div>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={saveProvider}>Save</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
