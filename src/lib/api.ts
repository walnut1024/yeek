import { getTransport } from "./transport";

// Types

export interface SessionRecord {
  id: string;
  agent: string;
  project_path: string | null;
  title: string | null;
  model: string | null;
  git_branch: string | null;
  started_at: string | null;
  ended_at: string | null;
  status: "active" | "complete" | "partial";
  visibility: "visible" | "hidden" | "archived";
  pinned: boolean;
  archived_at: string | null;
  deleted_at: string | null;
  delete_mode: "none" | "soft_deleted" | "source_deleted";
  message_count: number;
  updated_at: string;
}

export interface SessionListResponse {
  sessions: SessionRecord[];
  total: number;
  has_more: boolean;
}

export interface MessagePreview {
  role: string;
  content_preview: string;
}

export interface MessageRecord {
  id: string;
  session_id: string;
  parent_id: string | null;
  role: string;
  kind: string;
  content_preview: string;
  timestamp: string | null;
  is_sidechain: boolean;
  entry_type: string;
  subtype: string | null;
  tool_name: string | null;
  subagent_id: string | null;
  model: string | null;
  metadata: string | null;
}

export interface SourceRef {
  source_id: string;
  source_type: string;
  path: string;
  delete_policy: "not_allowed" | "hide_only" | "file_safe" | "needs_review";
}

export interface SessionPreviewPayload {
  record: SessionRecord;
  preview_messages: MessagePreview[];
  source_count: number;
}

export interface SessionDetailPayload {
  record: SessionRecord;
  messages: MessageRecord[];
  sources: SourceRef[];
}

export interface SystemStatusPayload {
  db_path: string;
  total_sessions: number;
  total_sources: number;
  last_sync_at: string | null;
  status: string;
}

export interface ActionResult {
  success: boolean;
  affected_count: number;
}

export interface ActionLogEntry {
  id: number;
  session_id: string | null;
  action: string;
  detail: string | null;
  created_at: string;
}

// Browse params

export interface BrowseRequest {
  sort?: string;
  limit?: number;
  offset?: number;
}

export interface SearchRequest {
  query: string;
  limit?: number;
  offset?: number;
}

// API functions

export async function getSystemStatus(): Promise<SystemStatusPayload> {
  return getTransport().command<SystemStatusPayload>("get_system_status");
}

export async function browseSessions(
  params: BrowseRequest = {}
): Promise<SessionListResponse> {
  return getTransport().command<SessionListResponse>("browse_sessions", { request: params });
}

export async function searchSessions(
  params: SearchRequest
): Promise<SessionListResponse> {
  return getTransport().command<SessionListResponse>("search_sessions", { request: params });
}

export async function getSessionPreview(
  sessionId: string
): Promise<SessionPreviewPayload> {
  return getTransport().command<SessionPreviewPayload>("get_session_preview", { sessionId });
}

export async function getSessionDetail(
  sessionId: string
): Promise<SessionDetailPayload> {
  return getTransport().command<SessionDetailPayload>("get_session_detail", { sessionId });
}

export async function softDeleteSessions(
  ids: string[]
): Promise<ActionResult> {
  return getTransport().command<ActionResult>("soft_delete_sessions", { ids });
}

export async function softDeleteProject(
  projectPath: string
): Promise<ActionResult> {
  return getTransport().command<ActionResult>("soft_delete_project", { projectPath });
}

export async function rescanSources(): Promise<ActionResult> {
  return getTransport().command<ActionResult>("rescan_sources");
}

export async function releaseAndResync(): Promise<ActionResult> {
  return getTransport().command<ActionResult>("release_and_resync");
}

export async function getActionLog(
  limit?: number
): Promise<{ actions: ActionLogEntry[] }> {
  return getTransport().command<{ actions: ActionLogEntry[] }>("get_action_log", { limit });
}

// Delete planning

export interface SourceDeletePlan {
  source: SourceRef;
  can_delete: boolean;
  target_path: string;
  reason: string;
}

export interface DeletePlan {
  session_id: string;
  sources: SourceDeletePlan[];
  allowed: boolean;
  reason: string;
}

export interface DestructiveDeleteResult {
  success: boolean;
  deleted_files: number;
  failed_files: number;
  errors: string[];
}

export async function getDeletePlan(
  sessionId: string
): Promise<DeletePlan> {
  return getTransport().command<DeletePlan>("get_delete_plan", { sessionId });
}

export async function destructiveDeleteSession(
  sessionId: string
): Promise<DestructiveDeleteResult> {
  return getTransport().command<DestructiveDeleteResult>("destructive_delete_session", { sessionId });
}

export async function getSubagentMessages(
  sessionId: string,
  subagentId: string
): Promise<MessageRecord[]> {
  return getTransport().command<MessageRecord[]>("get_subagent_messages", { sessionId, subagentId });
}

// Transcript (tree-aware)

export interface SiblingInfo {
  message_id: string;
  label: string;
}

export interface BranchPoint {
  parent_id: string;
  siblings: SiblingInfo[];
  active_index: number;
}

export interface TranscriptPayload {
  messages: MessageRecord[];
  main_path: string[];
  branches: BranchPoint[];
}

export async function getSessionTranscript(
  sessionId: string
): Promise<TranscriptPayload> {
  return getTransport().command<TranscriptPayload>("get_session_transcript", { sessionId });
}

// ── Skills / Plugins ──────────────────────────────────────────

export interface SkillInfo {
  name: string;
  description: string;
  skill_type: string;
  tools?: string;
  file_path: string;
  health: string;
  health_detail?: string;
}

export interface MarketplaceInfo {
  name: string;
  repo: string;
  last_updated?: string;
}

export interface PluginInfo {
  key: string;
  name: string;
  version: string;
  scope: string;
  marketplace?: MarketplaceInfo;
  install_path: string;
  enabled: boolean;
  health: string;
  health_issues: string[];
  skills: SkillInfo[];
  agents: SkillInfo[];
  installed_at?: string;
  last_updated?: string;
}

export interface HealthSummary {
  ok: number;
  partial: number;
  hook: number;
  broken: number;
}

export interface SkillsOverview {
  plugins: PluginInfo[];
  total_plugins: number;
  total_skills: number;
  total_agents: number;
  health_summary: HealthSummary;
}

export async function listPlugins(scope: string): Promise<SkillsOverview> {
  return getTransport().command<SkillsOverview>("list_plugins", { scope });
}

export async function togglePlugin(key: string): Promise<void> {
  return getTransport().command<void>("toggle_plugin", { key });
}

export async function uninstallPlugin(key: string): Promise<void> {
  return getTransport().command<void>("uninstall_plugin", { key });
}

export interface FixPluginResult {
  action: string;
  message: string;
}

export async function cleanPlugin(key: string): Promise<FixPluginResult> {
  return getTransport().command<FixPluginResult>("clean_plugin", { key });
}

export async function reinstallPlugin(key: string): Promise<FixPluginResult> {
  return getTransport().command<FixPluginResult>("reinstall_plugin", { key });
}

// ── Marketplace ───────────────────────────────────────────

export interface MarketplaceEntry {
  name: string;
  repo: string;
  install_location: string;
  last_updated?: string;
  plugin_count: number;
}

export interface MarketplaceListResult {
  marketplaces: MarketplaceEntry[];
}

export async function listMarketplaces(): Promise<MarketplaceListResult> {
  return getTransport().command<MarketplaceListResult>("list_marketplaces");
}

export async function addMarketplace(name: string, repo: string): Promise<void> {
  return getTransport().command<void>("add_marketplace", { name, repo });
}

export async function updateMarketplace(name: string): Promise<void> {
  return getTransport().command<void>("update_marketplace", { name });
}

export async function removeMarketplace(name: string, removePlugins: boolean): Promise<void> {
  return getTransport().command<void>("remove_marketplace", { name, removePlugins });
}

export interface MarketplacePlugin {
  name: string;
  description: string;
  skill_count: number;
  agent_count: number;
  has_hooks: boolean;
  installed: boolean;
}

export async function listMarketplacePlugins(marketplaceName: string): Promise<MarketplacePlugin[]> {
  return getTransport().command<MarketplacePlugin[]>("list_marketplace_plugins", { marketplaceName });
}

export async function installMarketplacePlugin(marketplaceName: string, pluginName: string): Promise<void> {
  return getTransport().command<void>("install_marketplace_plugin", { marketplaceName, pluginName });
}

// ── Resume ────────────────────────────────────────────────

export async function resumeSession(sessionId: string, agent: string, cwd: string | null): Promise<void> {
  return getTransport().command<void>("resume_session", { sessionId, agent, cwd });
}
