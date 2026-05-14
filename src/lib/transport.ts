import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export interface Transport {
  command<T>(name: string, args?: Record<string, unknown>): Promise<T>;
}

class TauriTransport implements Transport {
  async command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
    return invoke<T>(name, args);
  }
}

interface RouteMapping {
  method: string;
  path: string;
  buildBody?: (args: Record<string, unknown>) => Record<string, unknown>;
}

type TransportArgs = Record<string, unknown> | undefined;

const ROUTES: Record<string, RouteMapping> = {
  get_system_status:           { method: "GET",  path: "/api/system/status" },
  browse_sessions:             { method: "GET",  path: "/api/sessions" },
  search_sessions:             { method: "GET",  path: "/api/sessions/search" },
  get_session_preview:         { method: "GET",  path: "/api/sessions/{sessionId}/preview" },
  get_session_detail:          { method: "GET",  path: "/api/sessions/{sessionId}/detail" },
  get_session_transcript:      { method: "GET",  path: "/api/sessions/{sessionId}/transcript" },
  soft_delete_sessions:        { method: "POST", path: "/api/sessions/soft-delete" },
  soft_delete_project:         { method: "POST", path: "/api/sessions/soft-delete-project" },
  rescan_sources:              { method: "POST", path: "/api/system/rescan" },
  release_and_resync:          { method: "POST", path: "/api/system/release-and-resync" },
  get_action_log:              { method: "GET",  path: "/api/system/action-log" },
  get_delete_plan:             { method: "GET",  path: "/api/sessions/{sessionId}/delete-plan" },
  destructive_delete_session:  { method: "POST", path: "/api/sessions/{sessionId}/destructive-delete" },
  destructive_delete_sessions: { method: "POST", path: "/api/sessions/destructive-delete-batch" },
  get_delete_job:              { method: "GET",  path: "/api/sessions/delete-job/{jobId}" },
  get_subagent_messages:       { method: "GET",  path: "/api/sessions/{sessionId}/subagents/{subagentId}" },
  resume_session:              { method: "POST", path: "/api/sessions/resume" },
  list_plugins:                { method: "GET",  path: "/api/plugins" },
  toggle_plugin:               { method: "POST", path: "/api/plugins/toggle" },
  uninstall_plugin:            { method: "POST", path: "/api/plugins/uninstall" },
  clean_plugin:                { method: "POST", path: "/api/plugins/clean" },
  reinstall_plugin:            { method: "POST", path: "/api/plugins/reinstall" },
  list_marketplaces:           { method: "GET",  path: "/api/marketplaces" },
  add_marketplace:             { method: "POST", path: "/api/marketplaces" },
  update_marketplace:          { method: "POST", path: "/api/marketplaces/{name}/update" },
  remove_marketplace:          { method: "DELETE", path: "/api/marketplaces/{name}" },
  list_marketplace_plugins:    { method: "GET",  path: "/api/marketplaces/{marketplaceName}/plugins" },
  install_marketplace_plugin:  { method: "POST", path: "/api/marketplaces/install-plugin" },
  // Proxy
  get_proxy_status:    { method: "GET",  path: "/api/proxy/status" },
  start_proxy:         { method: "POST", path: "/api/proxy/start" },
  stop_proxy:          { method: "POST", path: "/api/proxy/stop" },
  restart_proxy:       { method: "POST", path: "/api/proxy/restart" },
  get_proxy_config:    { method: "GET",  path: "/api/proxy/config" },
  update_proxy_config: { method: "PUT",  path: "/api/proxy/config" },
  get_proxy_metrics:   { method: "GET",  path: "/api/proxy/metrics" },
  get_proxy_logs:      { method: "GET",  path: "/api/proxy/logs" },
  get_proxy_error_events: { method: "GET", path: "/api/proxy/errors" },
};

function commandToRoute(
  name: string,
  args?: Record<string, unknown>,
): { method: string; path: string; body?: Record<string, unknown> } {
  const route = ROUTES[name];
  if (!route) throw new Error(`Unknown command: ${name}`);

  let path = route.path;

  // Replace path parameters like {sessionId}, {subagentId}, {name}, {marketplaceName}
  if (args) {
    for (const [key, value] of Object.entries(args)) {
      const placeholder = `{${key}}`;
      if (path.includes(placeholder)) {
        path = path.replace(placeholder, String(value));
      }
    }
  }

  // For GET requests, add remaining args as query params
  let body: Record<string, unknown> | undefined;
  if (route.method === "GET" && args) {
    const pathParams = new Set(
      (route.path.match(/\{(\w+)\}/g) || []).map((p) => p.slice(1, -1)),
    );
    const queryParams = Object.entries(args).filter(
      ([k]) => !pathParams.has(k),
    );
    if (queryParams.length > 0) {
      const qs = queryParams
        .map(([k, v]) => `${camelToSnake(k)}=${encodeURIComponent(String(v))}`)
        .join("&");
      path += `?${qs}`;
    }
  } else if (args) {
    // For POST/DELETE, extract body from args that aren't path params
    const pathParams = new Set(
      (route.path.match(/\{(\w+)\}/g) || []).map((p) => p.slice(1, -1)),
    );
    const bodyEntries = Object.entries(args).filter(
      ([k]) => !pathParams.has(k),
    );
    if (bodyEntries.length > 0) {
      body = Object.fromEntries(
        bodyEntries.map(([k, v]) => [camelToSnake(k), v]),
      );
    }
  }

  return { method: route.method, path, body };
}

function camelToSnake(s: string): string {
  return s.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

const PREVIEW_NOW = "2026-05-13T10:30:00.000Z";

const PREVIEW_SESSIONS = [
  {
    id: "preview-session-1",
    agent: "claude_code",
    project_path: "/Users/demo/Projects/yeek",
    title: "Refine dashboard density and review proxy preview fallback",
    model: "claude-sonnet-4",
    git_branch: "main",
    started_at: "2026-05-13T08:30:00.000Z",
    ended_at: null,
    status: "active",
    visibility: "visible",
    pinned: false,
    archived_at: null,
    deleted_at: null,
    delete_mode: "none",
    message_count: 42,
    updated_at: "2026-05-13T10:24:00.000Z",
  },
  {
    id: "preview-session-2",
    agent: "claude_code",
    project_path: "/Users/demo/Projects/vendor_proxy",
    title: "Audit plugin marketplace layout for compact high-density view",
    model: "claude-haiku-4",
    git_branch: "feat/compact-ui",
    started_at: "2026-05-12T09:00:00.000Z",
    ended_at: "2026-05-12T10:10:00.000Z",
    status: "complete",
    visibility: "visible",
    pinned: false,
    archived_at: null,
    deleted_at: null,
    delete_mode: "none",
    message_count: 18,
    updated_at: "2026-05-12T10:10:00.000Z",
  },
  {
    id: "preview-session-3",
    agent: "codex",
    project_path: "/Users/demo/Projects/design-lab",
    title: "Compare graph node spacing against updated design system",
    model: "gpt-5.4",
    git_branch: "design/review",
    started_at: "2026-05-11T16:00:00.000Z",
    ended_at: "2026-05-11T16:45:00.000Z",
    status: "partial",
    visibility: "visible",
    pinned: false,
    archived_at: null,
    deleted_at: null,
    delete_mode: "none",
    message_count: 27,
    updated_at: "2026-05-11T16:45:00.000Z",
  },
];

const PREVIEW_MESSAGES = [
  {
    id: "m1",
    session_id: "preview-session-1",
    parent_id: null,
    role: "human",
    kind: "message",
    content_preview: "Please tighten the app layout and remove wasted vertical whitespace.",
    timestamp: "2026-05-13T09:40:00.000Z",
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: null,
    subagent_id: null,
    model: null,
    metadata: null,
  },
  {
    id: "m2",
    session_id: "preview-session-1",
    parent_id: "m1",
    role: "assistant",
    kind: "message",
    content_preview: "I tightened the sidebar, headers, pills, and session cards. Next I am checking Proxy and Marketplace for dense layouts.",
    timestamp: "2026-05-13T09:41:00.000Z",
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: null,
    subagent_id: null,
    model: "claude-sonnet-4",
    metadata: null,
  },
  {
    id: "m3",
    session_id: "preview-session-1",
    parent_id: "m2",
    role: "assistant",
    kind: "tool_use",
    content_preview: "Read src/pages/proxy/proxy-page.tsx",
    timestamp: "2026-05-13T09:42:00.000Z",
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: "Read",
    subagent_id: null,
    model: "claude-sonnet-4",
    metadata: null,
  },
  {
    id: "m4",
    session_id: "preview-session-1",
    parent_id: "m3",
    role: "user",
    kind: "tool_result",
    content_preview: "Loaded proxy-page.tsx (520 lines).",
    timestamp: "2026-05-13T09:42:01.000Z",
    is_sidechain: false,
    entry_type: "message",
    subtype: null,
    tool_name: "Read",
    subagent_id: null,
    model: null,
    metadata: null,
  },
  {
    id: "m5",
    session_id: "preview-session-1",
    parent_id: "m4",
    role: "assistant",
    kind: "message",
    content_preview: "Proxy labels can move into a fixed-width left column so inputs stay on one line more often.",
    timestamp: "2026-05-13T09:43:00.000Z",
    is_sidechain: false,
    entry_type: "summary",
    subtype: null,
    tool_name: null,
    subagent_id: null,
    model: "claude-sonnet-4",
    metadata: null,
  },
];

const PREVIEW_SOURCES = [
  {
    source_id: "src-1",
    source_type: "workspace",
    path: "/Users/demo/Projects/yeek/src/pages/proxy/proxy-page.tsx",
    delete_policy: "hide_only",
  },
  {
    source_id: "src-2",
    source_type: "workspace",
    path: "/Users/demo/Projects/yeek/src/index.css",
    delete_policy: "hide_only",
  },
];

function buildPreviewResponse(name: string, args?: TransportArgs): unknown {
  switch (name) {
    case "get_system_status":
      return {
        db_path: "/Users/demo/Library/Application Support/yeek/yeek.db",
        total_sessions: PREVIEW_SESSIONS.length,
        total_sources: 94,
        total_projects: 7,
        total_messages: 223,
        active_sessions: 1,
        complete_sessions: 1,
        partial_sessions: 1,
        last_sync_at: PREVIEW_NOW,
        status: "ok",
      };
    case "browse_sessions": {
      const agent = typeof args?.agent === "string" ? args.agent : undefined;
      const sessions = agent
        ? PREVIEW_SESSIONS.filter((session) => session.agent === agent)
        : PREVIEW_SESSIONS;
      return { sessions, total: sessions.length, has_more: false };
    }
    case "search_sessions": {
      const query = String(args?.query ?? "").toLowerCase();
      const sessions = PREVIEW_SESSIONS.filter((session) =>
        !query
          || session.title?.toLowerCase().includes(query)
          || session.project_path?.toLowerCase().includes(query),
      );
      return { sessions, total: sessions.length, has_more: false };
    }
    case "get_session_preview": {
      const sessionId = String(args?.sessionId ?? PREVIEW_SESSIONS[0].id);
      const record = PREVIEW_SESSIONS.find((session) => session.id === sessionId) ?? PREVIEW_SESSIONS[0];
      return {
        record,
        preview_messages: PREVIEW_MESSAGES.slice(0, 2).map((msg) => ({
          role: msg.role,
          content_preview: msg.content_preview,
        })),
        source_count: PREVIEW_SOURCES.length,
      };
    }
    case "get_session_detail": {
      const sessionId = String(args?.sessionId ?? PREVIEW_SESSIONS[0].id);
      const record = PREVIEW_SESSIONS.find((session) => session.id === sessionId) ?? PREVIEW_SESSIONS[0];
      return { record, messages: PREVIEW_MESSAGES, sources: PREVIEW_SOURCES };
    }
    case "get_session_transcript":
      return {
        messages: PREVIEW_MESSAGES,
        main_path: PREVIEW_MESSAGES.map((msg) => msg.id),
        branches: [],
      };
    case "get_action_log":
      return {
        actions: [
          {
            id: 3,
            session_id: "preview-session-1",
            action: "ui.review",
            detail: "Compressed sidebar, graph header, and proxy form rows.",
            created_at: "2026-05-13T09:58:00.000Z",
          },
          {
            id: 2,
            session_id: "preview-session-2",
            action: "marketplace.sync",
            detail: "Updated compact plugin rows and cleaned badge density.",
            created_at: "2026-05-13T09:44:00.000Z",
          },
          {
            id: 1,
            session_id: "preview-session-3",
            action: "proxy.preview",
            detail: "Loaded fallback preview data because Tauri backend is unavailable.",
            created_at: "2026-05-13T09:32:00.000Z",
          },
        ],
      };
    case "list_plugins":
      return {
        plugins: [
          {
            key: "compact-audit@official",
            name: "compact-audit",
            version: "1.4.0",
            scope: "global",
            marketplace: { name: "official", repo: "https://example.com/official" },
            install_path: "/Users/demo/.yeek/plugins/compact-audit",
            enabled: true,
            health: "ok",
            health_issues: [],
            skills: [
              {
                name: "layout-density",
                description: "Checks long rows and wasted whitespace.",
                skill_type: "skill",
                tools: "Read, Grep",
                file_path: "/preview/layout-density",
                health: "ok",
              },
            ],
            agents: [],
            installed_at: "2026-05-11T09:00:00.000Z",
            last_updated: "2026-05-13T09:20:00.000Z",
          },
        ],
        total_plugins: 1,
        total_skills: 1,
        total_agents: 0,
        health_summary: { ok: 1, partial: 0, hook: 0, broken: 0 },
      };
    case "list_marketplaces":
      return {
        marketplaces: [
          {
            name: "official",
            repo: "https://example.com/official",
            install_location: "/Users/demo/.yeek/marketplaces/official",
            last_updated: "2026-05-13T09:20:00.000Z",
            plugin_count: 6,
          },
        ],
      };
    case "list_marketplace_plugins":
      return [
        {
          name: "compact-audit",
          description: "Review long labels, whitespace, and dense panel layouts.",
          skill_count: 1,
          agent_count: 0,
          has_hooks: false,
          installed: true,
        },
        {
          name: "session-lens",
          description: "Inspect transcripts and graph readability.",
          skill_count: 2,
          agent_count: 1,
          has_hooks: true,
          installed: false,
        },
      ];
    case "get_proxy_status":
      return {
        running: false,
        listen_addr: "127.0.0.1:8787",
        uptime_secs: null,
        version: "preview",
      };
    case "get_proxy_config":
      return {
        server: { listen_addr: "127.0.0.1:8787" },
        providers: {
          deepseek_preview: {
            base_url: "https://api.deepseek.com/anthropic",
            api_format: "anthropic_messages",
            api_key_env: "DEEPSEEK_API_KEY",
          },
        },
        bridges: {
          claude_preview: {
            agent: { base_url: "/deepseek_anthropic", api_format: "anthropic_messages" },
            provider: { name: "deepseek_preview" },
            models: { "claude-sonnet": "deepseek-v4-pro" },
          },
        },
      };
    case "get_proxy_metrics":
      return {
        version: "preview",
        uptime_secs: 5420,
        request_count: 182,
        error_count: 3,
        active_connections: 2,
        rps: 1.4,
        avg_latency_ms: 482,
      };
    case "get_proxy_error_events":
      return [
        {
          timestamp: 1747130820000,
          provider: "deepseek_preview",
          model: "deepseek-v4-pro",
          status: 429,
          message: "Rate limited during preview fallback.",
        },
      ];
    case "get_proxy_logs":
      return "[preview] Backend unavailable. Using browser preview fixture.\n";
    default:
      return undefined;
  }
}

class HttpTransport implements Transport {
  private baseUrl: string;
  constructor(baseUrl = "http://localhost:17321") {
    this.baseUrl = baseUrl;
  }

  async command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
    const { method, path, body } = commandToRoute(name, args);
    const url = `${this.baseUrl}${path}`;
    const options: RequestInit = {
      method,
      headers: { "Content-Type": "application/json" },
    };
    if (body) options.body = JSON.stringify(body);
    let res: Response;
    try {
      res = await fetch(url, options);
    } catch (error) {
      const preview = buildPreviewResponse(name, args);
      if (preview !== undefined) return preview as T;
      throw error;
    }
    if (!res.ok) {
      const err = await res.json().catch(() => ({ message: res.statusText }));
      throw new Error(err.message || `HTTP ${res.status}`);
    }
    const text = await res.text();
    if (!text) return undefined as T;
    return JSON.parse(text);
  }
}

const isTauri = !!window.__TAURI_INTERNALS__;

let transport: Transport = isTauri
  ? new TauriTransport()
  : new HttpTransport();

export function getTransport(): Transport {
  return transport;
}

export function setHttpBaseUrl(url: string) {
  transport = new HttpTransport(url);
}
