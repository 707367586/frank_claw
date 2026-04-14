// ── Core entity types matching Rust backend ──

export interface SourceRef {
  id: string;
  filename: string;
  kind: "code" | "doc" | "text";
  lineRange?: string;
  snippet: string;
}

export interface Agent {
  id: string;
  name: string;
  role: string;
  system_prompt: string;
  model_id: string | null;
  model?: string;
  status: "idle" | "working" | "error" | "offline";
  created_at: string;
  updated_at: string;
}

export interface Conversation {
  id: string;
  agent_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  conversation_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
  refs?: SourceRef[];
}

export interface Memory {
  id: string;
  agent_id: string;
  scope: "agent" | "user";
  memory_type: "fact" | "preference" | "event" | "skill";
  summary: string;
  detail: string;
  importance: number;
  freshness: number;
  pinned: boolean;
  created_at: string;
  last_accessed_at: string;
  access_count: number;
}

export interface KnowledgeSource {
  id: string;
  agent_id: string;
  path: string;
  status: "indexing" | "ready" | "error";
  doc_count: number;
  chunk_count: number;
  created_at: string;
}

export interface VaultSnapshot {
  id: string;
  agent_id: string;
  task_id: string | null;
  label: string;
  created_at: string;
  file_count: number;
}

export interface ModelProvider {
  id: string;
  name: string;
  provider_type: "anthropic" | "openai" | "zhipu" | "ollama" | "custom";
  base_url: string;
  model_name: string;
  parameters: unknown;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface Task {
  id: string;
  agent_id: string;
  name: string;
  goal: string;
  source_kind: string;
  lifecycle_status: "active" | "paused" | "archived";
  notification_policy: string;
  default_max_steps: number;
  default_timeout_secs: number;
  created_at: string;
}

export interface Trigger {
  id: string;
  task_id: string;
  kind: "time" | "event" | "context" | "policy";
  config: Record<string, unknown>;
  status: "active" | "paused";
  next_fire_at: string | null;
  last_fired_at: string | null;
  created_at: string;
}

export interface Run {
  id: string;
  task_id: string;
  trigger_id: string | null;
  status:
    | "queued"
    | "planning"
    | "running"
    | "waiting_confirmation"
    | "completed"
    | "failed"
    | "interrupted";
  checkpoint: Record<string, unknown> | null;
  started_at: string | null;
  completed_at: string | null;
  feedback_kind: string | null;
}

export interface Channel {
  id: string;
  name: string;
  channel_type:
    | "telegram"
    | "lark"
    | "slack"
    | "whatsapp"
    | "discord"
    | "wecom";
  agent_id: string;
  config: Record<string, unknown>;
  status: "connected" | "disconnected" | "error";
  created_at: string;
}

export interface Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  status: "installed" | "enabled" | "disabled";
  capabilities: Record<string, unknown>;
  created_at: string;
}

export interface PermissionProfile {
  agent_id: string;
  capability_scores: Record<string, unknown>;
  trust_level: "L0" | "L1" | "L2" | "L3";
  safety_incidents: number;
}

export interface SystemHealth {
  status: string;
  uptime: number;
  version: string;
}

export interface SystemStats {
  agent_count: number;
  conversation_count: number;
  memory_count: number;
  knowledge_doc_count: number;
  disk_usage_bytes: number;
}

// ── Pagination & error ──

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  offset: number;
  limit: number;
}

export interface ApiError {
  code: string;
  message: string;
}

export interface KnowledgeSearchResult {
  chunk_id: string;
  content: string;
  source_path: string;
  score: number;
}
