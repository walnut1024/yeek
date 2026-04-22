import { invoke } from "@tauri-apps/api/core";

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
    const res = await fetch(url, options);
    if (!res.ok) {
      const err = await res.json().catch(() => ({ message: res.statusText }));
      throw new Error(err.message || `HTTP ${res.status}`);
    }
    const text = await res.text();
    if (!text) return undefined as T;
    return JSON.parse(text);
  }
}

const isTauri = !!(window as any).__TAURI_INTERNALS__;

let transport: Transport = isTauri
  ? new TauriTransport()
  : new HttpTransport();

export function getTransport(): Transport {
  return transport;
}

export function setHttpBaseUrl(url: string) {
  transport = new HttpTransport(url);
}
