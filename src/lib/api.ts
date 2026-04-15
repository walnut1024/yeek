import { invoke } from "@tauri-apps/api/core";

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
  group?: string;
  limit?: number;
  offset?: number;
  visibility?: string;
  agent?: string;
  project_path?: string;
  pinned_only?: boolean;
}

export interface SearchRequest {
  query: string;
  limit?: number;
  offset?: number;
  visibility?: string;
  agent?: string;
}

// API functions

export async function getSystemStatus(): Promise<SystemStatusPayload> {
  return invoke("get_system_status");
}

export async function browseSessions(
  params: BrowseRequest = {}
): Promise<SessionListResponse> {
  return invoke("browse_sessions", { request: params });
}

export async function searchSessions(
  params: SearchRequest
): Promise<SessionListResponse> {
  return invoke("search_sessions", { request: params });
}

export async function getSessionPreview(
  sessionId: string
): Promise<SessionPreviewPayload> {
  return invoke("get_session_preview", { sessionId });
}

export async function getSessionDetail(
  sessionId: string
): Promise<SessionDetailPayload> {
  return invoke("get_session_detail", { sessionId });
}

export async function setPinned(
  ids: string[],
  value: boolean
): Promise<ActionResult> {
  return invoke("set_pinned", { ids, value });
}

export async function setArchived(
  ids: string[],
  value: boolean
): Promise<ActionResult> {
  return invoke("set_archived", { ids, value });
}

export async function setHidden(
  ids: string[],
  value: boolean
): Promise<ActionResult> {
  return invoke("set_hidden", { ids, value });
}

export async function softDeleteSessions(
  ids: string[]
): Promise<ActionResult> {
  return invoke("soft_delete_sessions", { ids });
}

export async function rescanSources(): Promise<ActionResult> {
  return invoke("rescan_sources");
}

export async function getActionLog(
  limit?: number
): Promise<{ actions: ActionLogEntry[] }> {
  return invoke("get_action_log", { limit });
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
  return invoke("get_delete_plan", { sessionId });
}

export async function destructiveDeleteSession(
  sessionId: string
): Promise<DestructiveDeleteResult> {
  return invoke("destructive_delete_session", { sessionId });
}

export async function getSubagentMessages(
  sessionId: string,
  subagentId: string
): Promise<MessageRecord[]> {
  return invoke("get_subagent_messages", { sessionId, subagentId });
}
