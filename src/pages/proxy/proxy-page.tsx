import { useMemo, useState, type ReactNode } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  getProxyStatus,
  startProxy,
  stopProxy,
  getProxyConfig,
  updateProxyConfig,
  type ProxyBridgeConfig,
  type ProxyConfig,
  type ProxyProviderConfig,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";

const API_FORMATS = ["anthropic_messages", "chat_completions", "responses"] as const;

const DEFAULT_PROXY_CONFIG: ProxyConfig = {
  server: { listen_addr: "127.0.0.1:8787" },
  bridges: {
    claude_desktop_deepseek: {
      agent: { base_url: "/deepseek_anthropic", api_format: "anthropic_messages" },
      provider: { name: "deepseek_anthropic" },
      models: {
        "claude-sonnet": "deepseek-v4-pro[1m]",
        "claude-haiku": "deepseek-v4-flash",
        "claude-opus": "deepseek-v4-pro[1m]",
      },
    },
    claude_desktop_zhipu: {
      agent: { base_url: "/zhipu_anthropic", api_format: "anthropic_messages" },
      provider: { name: "zhipu_anthropic" },
      models: {
        "claude-sonnet": "glm-5.1",
        "claude-haiku": "glm-5.1",
        "claude-opus": "glm-5.1",
      },
    },
  },
  providers: {
    deepseek_anthropic: {
      base_url: "https://api.deepseek.com/anthropic",
      api_format: "anthropic_messages",
      api_key_env: "DEEPSEEK_API_KEY",
    },
    zhipu_anthropic: {
      base_url: "https://open.bigmodel.cn/api/anthropic",
      api_format: "anthropic_messages",
      api_key_env: "ZHIPU_API_KEY",
    },
  },
};

type ProxyMode = "cards" | "toml";

interface ValidationIssue {
  scope: string;
  message: string;
}

export default function ProxyPage() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const [mode, setMode] = useState<ProxyMode>("cards");
  const [draft, setDraft] = useState<ProxyConfig | null>(null);

  const { data: status } = useQuery({
    queryKey: ["proxy-status"],
    queryFn: getProxyStatus,
    refetchInterval: 3000,
    staleTime: 2000,
    gcTime: 5 * 60 * 1000,
  });

  const { data: savedConfig } = useQuery({
    queryKey: ["proxy-config"],
    queryFn: getProxyConfig,
    staleTime: 30 * 60 * 1000,
    gcTime: 60 * 60 * 1000,
  });

  const config = draft ?? savedConfig ?? DEFAULT_PROXY_CONFIG;
  const dirty = draft !== null;
  const isRunning = status?.running ?? false;
  const issues = useMemo(() => validateConfig(config), [config]);
  const toml = useMemo(() => serializeProxyConfig(config), [config]);
  const lineCount = toml.split("\n").length;

  const startMut = useMutation({
    mutationFn: startProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxy-status"] }),
  });

  const stopMut = useMutation({
    mutationFn: stopProxy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["proxy-status"] }),
  });

  const saveConfigMut = useMutation({
    mutationFn: (cfg: ProxyConfig) => updateProxyConfig(cfg),
    onSuccess: () => {
      setDraft(null);
      queryClient.invalidateQueries({ queryKey: ["proxy-config"] });
      queryClient.invalidateQueries({ queryKey: ["proxy-status"] });
    },
  });

  const updateDraft = (mutator: (next: ProxyConfig) => void) => {
    const next = cloneConfig(config);
    mutator(next);
    setDraft(next);
  };

  const saveDraft = () => saveConfigMut.mutate(config);
  const resetDraft = () => setDraft(cloneConfig(DEFAULT_PROXY_CONFIG));
  const isBusy = startMut.isPending || stopMut.isPending || saveConfigMut.isPending;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header data-ai-region="proxy-header" className="border-b border-border px-3 py-3">
        <h2 className="text-[14px] font-medium leading-none text-foreground">{t("proxy.title")}</h2>
        <p className="mt-2 max-w-2xl text-[14px] leading-[1.5] text-muted-foreground">
          Route local Agent requests through Provider-specific formats, credentials, and explicit model mappings.
        </p>
      </header>

      <div data-ai-region="proxy-toolbar" className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <div className="tabs">
            <Button type="button" variant="secondary" size="sm" className={`chip ${mode === "cards" ? "active" : ""}`} onClick={() => setMode("cards")}>Cards</Button>
            <Button type="button" variant="secondary" size="sm" className={`chip ${mode === "toml" ? "active" : ""}`} onClick={() => setMode("toml")}>TOML</Button>
          </div>
          <Badge variant={isRunning ? "default" : "secondary"} className="h-5 px-2 text-[11px] uppercase tracking-[0.04em]">
            {isRunning ? "Running" : "Stopped"}
          </Badge>
        </div>
        <div className="flex items-center gap-1.5">
          <Button variant={isRunning ? "destructive" : "primary"} size="sm" disabled={isBusy}
            onClick={() => isRunning ? stopMut.mutate() : startMut.mutate()}>
            {isRunning ? "Stop" : "Run"}
          </Button>
        </div>
      </div>

      <section data-ai-region="proxy-config" className={`proxy-config-workspace ${mode === "toml" ? "toml-mode" : ""}`}>
        {mode === "cards" ? (
          <ScrollArea className="min-h-0">
            <div className="proxy-config-cards">
              <ConfigCards
                config={config}
                updateDraft={updateDraft}
                saveDraft={saveDraft}
                resetDraft={resetDraft}
                dirty={dirty}
                issues={issues}
                isBusy={isBusy}
              />
            </div>
          </ScrollArea>
        ) : (
          <TomlPreview toml={toml} lineCount={lineCount} dirty={dirty} issues={issues} saveDraft={saveDraft} resetDraft={resetDraft} isBusy={isBusy} />
        )}
      </section>
    </div>
  );
}

function ConfigCards({
  config,
  updateDraft,
  saveDraft,
  resetDraft,
  dirty,
  issues,
  isBusy,
}: {
  config: ProxyConfig;
  updateDraft: (mutator: (next: ProxyConfig) => void) => void;
  saveDraft: () => void;
  resetDraft: () => void;
  dirty: boolean;
  issues: ValidationIssue[];
  isBusy: boolean;
}) {
  return (
    <>
      <div className="proxy-panel-head">
        <span className="zed-kicker">TOML Cards</span>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-muted-foreground">
            {Object.keys(config.bridges).length + Object.keys(config.providers).length + 1} cards
          </span>
          {dirty && (
            <Button variant="primary" size="sm" disabled={isBusy || issues.length > 0} onClick={saveDraft}>
              Save
            </Button>
          )}
          <Button variant="outline" size="sm" disabled={isBusy} onClick={resetDraft}>Reset</Button>
        </div>
      </div>

      <div className="toml-card-groups">
        <section data-ai-region="proxy-server" className="toml-group">
          <div className="toml-group-head"><span className="zed-kicker">Server</span><span className="font-mono text-[11px] text-muted-foreground">1</span></div>
          <article className="toml-card server-card">
            <div className="toml-card-head"><span className="toml-card-title">Server: server</span></div>
            <div className="toml-card-body">
              <Field label="Listen Address">
                <input className="zed-input font-mono text-[12px]" value={config.server.listen_addr}
                  onChange={(e) => updateDraft((next) => { next.server.listen_addr = e.target.value; })} />
              </Field>
            </div>
          </article>
        </section>

        <div className="toml-collections">
          <section data-ai-region="proxy-providers" className="toml-group">
            <div className="toml-group-head">
              <span className="zed-kicker">Providers</span>
              <Button variant="outline" size="sm" onClick={() => updateDraft((next) => addProvider(next))}>Add Provider</Button>
            </div>
            {Object.entries(config.providers).map(([name, provider]) => (
              <ProviderCard key={name} name={name} provider={provider} config={config} updateDraft={updateDraft} />
            ))}
          </section>

          <section data-ai-region="proxy-bridges" className="toml-group">
            <div className="toml-group-head">
              <span className="zed-kicker">Bridges</span>
              <Button variant="outline" size="sm" onClick={() => updateDraft((next) => addBridge(next))}>Add Bridge</Button>
            </div>
            {Object.entries(config.bridges).map(([name, bridge]) => (
              <BridgeCard key={name} name={name} bridge={bridge} config={config} updateDraft={updateDraft} />
            ))}
          </section>
        </div>
      </div>
    </>
  );
}

function ProviderCard({
  name,
  provider,
  config,
  updateDraft,
}: {
  name: string;
  provider: ProxyProviderConfig;
  config: ProxyConfig;
  updateDraft: (mutator: (next: ProxyConfig) => void) => void;
}) {
  const inUse = Object.values(config.bridges).some((bridge) => bridge.provider.name === name);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const deleteProvider = () => updateDraft((next) => { delete next.providers[name]; });

  return (
    <article className={`toml-card provider-card ${inUse ? "is-active" : "is-muted"}`}>
      <div className="toml-card-head">
        <span className="toml-card-title">Provider: {name}</span>
        {confirmDelete ? (
          <ConfirmDelete onCancel={() => setConfirmDelete(false)} onConfirm={deleteProvider} />
        ) : (
          <IconButton
            label="Delete provider"
            disabled={inUse}
            danger
            onClick={() => setConfirmDelete(true)}
          >
            ×
          </IconButton>
        )}
      </div>
      <div className="toml-card-body">
        <Field label="Provider Name">
          <input className="zed-input font-mono text-[12px]" value={name}
            onChange={(e) => updateDraft((next) => renameProvider(next, name, e.target.value))} />
        </Field>
        <Field label="Base URL">
          <input className="zed-input font-mono text-[12px]" value={provider.base_url}
            onChange={(e) => updateDraft((next) => { next.providers[name].base_url = e.target.value; })} />
        </Field>
        <Field label="API Format">
          <FormatSelect value={provider.api_format}
            onChange={(value) => updateDraft((next) => { next.providers[name].api_format = value; })} />
        </Field>
        <Field label="API Key Env">
          <input className="zed-input font-mono text-[12px]" value={provider.api_key_env ?? ""}
            onChange={(e) => updateDraft((next) => { next.providers[name].api_key_env = e.target.value || null; })} />
        </Field>
      </div>
    </article>
  );
}

function BridgeCard({
  name,
  bridge,
  config,
  updateDraft,
}: {
  name: string;
  bridge: ProxyBridgeConfig;
  config: ProxyConfig;
  updateDraft: (mutator: (next: ProxyConfig) => void) => void;
}) {
  const provider = config.providers[bridge.provider.name];
  const [confirmDelete, setConfirmDelete] = useState(false);
  const deleteBridge = () => updateDraft((next) => { delete next.bridges[name]; });

  return (
    <article className={`toml-card bridge-card ${provider ? "is-active" : "is-muted"}`}>
      <div className="toml-card-head">
        <span className="toml-card-title">Bridge: {name}</span>
        {confirmDelete ? (
          <ConfirmDelete onCancel={() => setConfirmDelete(false)} onConfirm={deleteBridge} />
        ) : (
          <IconButton
            label="Delete bridge"
            danger
            onClick={() => setConfirmDelete(true)}
          >
            ×
          </IconButton>
        )}
      </div>
      <div className="toml-card-body">
        <Field label="Bridge Name">
          <input className="zed-input font-mono text-[12px]" value={name}
            onChange={(e) => updateDraft((next) => renameBridge(next, name, e.target.value))} />
        </Field>
      </div>
      <div className="bridge-card-grid">
        <div className="bridge-fields">
          <Field label="Agent Base URL">
            <input className="zed-input font-mono text-[12px]" value={bridge.agent.base_url}
              onChange={(e) => updateDraft((next) => { next.bridges[name].agent.base_url = normalizeBaseUrl(e.target.value); })} />
          </Field>
          <Field label="Agent API Format">
            <FormatSelect value={bridge.agent.api_format}
              onChange={(value) => updateDraft((next) => { next.bridges[name].agent.api_format = value; })} />
          </Field>
          <Field label="Provider">
            <select className="zed-input font-mono text-[12px]" value={bridge.provider.name}
              onChange={(e) => updateDraft((next) => { next.bridges[name].provider.name = e.target.value; })}>
              {Object.keys(config.providers).map((providerName) => <option key={providerName} value={providerName}>{providerName}</option>)}
            </select>
          </Field>
        </div>
        <div className="bridge-mapping">
          <div className="bridge-mapping-head">
            <span className="zed-kicker">Model Mapping</span>
            <IconButton label="Add mapping" onClick={() => updateDraft((next) => addModelMapping(next, name))}>+</IconButton>
          </div>
          <div className="toml-models">
            {Object.entries(bridge.models).map(([agentModel, providerModel]) => (
              <div className="toml-model-row" key={agentModel}>
                <input className="zed-input font-mono text-[12px]" value={agentModel}
                  onChange={(e) => updateDraft((next) => renameModel(next, name, agentModel, e.target.value))} />
                <span className="arrow">→</span>
                <input className="zed-input font-mono text-[12px]" value={providerModel}
                  onChange={(e) => updateDraft((next) => { next.bridges[name].models[agentModel] = e.target.value; })} />
                <IconButton
                  label="Delete mapping"
                  danger
                  onClick={() => updateDraft((next) => { delete next.bridges[name].models[agentModel]; })}
                >
                  ×
                </IconButton>
              </div>
            ))}
          </div>
        </div>
      </div>
    </article>
  );
}

function TomlPreview({
  toml,
  lineCount,
  dirty,
  issues,
  saveDraft,
  resetDraft,
  isBusy,
}: {
  toml: string;
  lineCount: number;
  dirty: boolean;
  issues: ValidationIssue[];
  saveDraft: () => void;
  resetDraft: () => void;
  isBusy: boolean;
}) {
  return (
    <section data-ai-region="proxy-toml-editor" className="proxy-editor-shell">
      <div className="proxy-panel-head">
        <span className="zed-kicker">proxy.toml</span>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-muted-foreground">{lineCount} lines</span>
          {dirty && <Button variant="primary" size="sm" disabled={isBusy || issues.length > 0} onClick={saveDraft}>Save</Button>}
          <Button variant="outline" size="sm" disabled={isBusy} onClick={resetDraft}>Reset</Button>
        </div>
      </div>
      <div className="proxy-editor-wrap">
        <pre className="line-numbers">{Array.from({ length: lineCount }, (_, i) => i + 1).join("\n")}</pre>
        <pre className="toml-editor-preview">{toml}</pre>
      </div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="toml-field"><span>{label}</span>{children}</label>;
}

function IconButton({
  label,
  children,
  danger,
  disabled,
  onClick,
}: {
  label: string;
  children: ReactNode;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      disabled={disabled}
      className={`proxy-icon-button ${danger ? "danger" : ""}`}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

function ConfirmDelete({
  onCancel,
  onConfirm,
}: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="proxy-confirm-delete" role="group" aria-label="Confirm delete">
      <span>Delete?</span>
      <Button type="button" variant="ghost" size="sm" className="proxy-confirm-button" onClick={onCancel}>Cancel</Button>
      <Button type="button" variant="ghost" size="sm" className="proxy-confirm-button danger" onClick={onConfirm}>Delete</Button>
    </div>
  );
}

function FormatSelect({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <select className="zed-input font-mono text-[12px]" value={value} onChange={(e) => onChange(e.target.value)}>
      {API_FORMATS.map((format) => <option key={format} value={format}>{format}</option>)}
    </select>
  );
}

function cloneConfig(config: ProxyConfig): ProxyConfig {
  return JSON.parse(JSON.stringify(config)) as ProxyConfig;
}

function normalizeBaseUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "/";
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

function uniqueName(existing: Record<string, unknown>, prefix: string): string {
  let index = Object.keys(existing).length + 1;
  let name = `${prefix}_${index}`;
  while (existing[name]) {
    index += 1;
    name = `${prefix}_${index}`;
  }
  return name;
}

function addProvider(config: ProxyConfig) {
  const name = uniqueName(config.providers, "provider");
  config.providers[name] = {
    base_url: "https://api.example.com/anthropic",
    api_format: "anthropic_messages",
    api_key_env: "PROVIDER_API_KEY",
  };
}

function addBridge(config: ProxyConfig) {
  const name = uniqueName(config.bridges, "bridge");
  const providerName = Object.keys(config.providers)[0] ?? "";
  config.bridges[name] = {
    agent: { base_url: `/${name}`, api_format: "anthropic_messages" },
    provider: { name: providerName },
    models: { "claude-sonnet": "provider-model" },
  };
}

function renameProvider(config: ProxyConfig, oldName: string, newNameRaw: string) {
  const newName = newNameRaw.trim();
  if (!newName || newName === oldName || config.providers[newName]) return;
  config.providers[newName] = config.providers[oldName];
  delete config.providers[oldName];
  for (const bridge of Object.values(config.bridges)) {
    if (bridge.provider.name === oldName) bridge.provider.name = newName;
  }
}

function renameBridge(config: ProxyConfig, oldName: string, newNameRaw: string) {
  const newName = newNameRaw.trim();
  if (!newName || newName === oldName || config.bridges[newName]) return;
  config.bridges[newName] = config.bridges[oldName];
  delete config.bridges[oldName];
}

function addModelMapping(config: ProxyConfig, bridgeName: string) {
  const models = config.bridges[bridgeName].models;
  const key = uniqueName(models, "agent_model");
  models[key] = "provider-model";
}

function renameModel(config: ProxyConfig, bridgeName: string, oldName: string, newNameRaw: string) {
  const newName = newNameRaw.trim();
  const models = config.bridges[bridgeName].models;
  if (!newName || newName === oldName || models[newName]) return;
  models[newName] = models[oldName];
  delete models[oldName];
}

function validateConfig(config: ProxyConfig): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  if (!config.server.listen_addr) issues.push({ scope: "server", message: "Missing listen_addr" });
  for (const [name, provider] of Object.entries(config.providers)) {
    if (!provider.base_url) issues.push({ scope: name, message: "Missing base_url" });
    if (!provider.api_format) issues.push({ scope: name, message: "Missing api_format" });
  }
  for (const [name, bridge] of Object.entries(config.bridges)) {
    if (!bridge.agent.base_url) issues.push({ scope: name, message: "Missing agent.base_url" });
    if (!bridge.agent.api_format) issues.push({ scope: name, message: "Missing agent.api_format" });
    if (!bridge.provider.name) issues.push({ scope: name, message: "Missing provider.name" });
    if (bridge.provider.name && !config.providers[bridge.provider.name]) issues.push({ scope: name, message: `Provider ${bridge.provider.name} is not defined` });
    if (!Object.keys(bridge.models).length) issues.push({ scope: name, message: "Missing model mappings" });
  }
  return issues;
}

function serializeProxyConfig(config: ProxyConfig): string {
  const lines: string[] = [
    "[server]",
    `listen_addr = ${quote(config.server.listen_addr)}`,
    "",
  ];

  for (const [name, bridge] of Object.entries(config.bridges)) {
    lines.push(`[bridges.${name}.agent]`);
    lines.push(`base_url = ${quote(bridge.agent.base_url)}`);
    lines.push(`api_format = ${quote(bridge.agent.api_format)}`);
    lines.push("");
    lines.push(`[bridges.${name}.provider]`);
    lines.push(`name = ${quote(bridge.provider.name)}`);
    lines.push("");
    lines.push(`[bridges.${name}.models]`);
    for (const [agentModel, providerModel] of Object.entries(bridge.models)) {
      lines.push(`${quote(agentModel)} = ${quote(providerModel)}`);
    }
    lines.push("");
  }

  for (const [name, provider] of Object.entries(config.providers)) {
    lines.push(`[providers.${name}]`);
    lines.push(`base_url = ${quote(provider.base_url)}`);
    lines.push(`api_format = ${quote(provider.api_format)}`);
    if (provider.api_key_env) lines.push(`api_key_env = ${quote(provider.api_key_env)}`);
    lines.push("");
  }

  return lines.join("\n").trimEnd();
}

function quote(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
