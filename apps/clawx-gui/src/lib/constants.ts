import type { Agent, Memory, Task, Run, KnowledgeSource, Channel } from "./types";

export const STATUS_COLORS: Record<Agent["status"], string> = {
  idle: "#4ade80",
  working: "#facc15",
  error: "#f87171",
  offline: "#6b7280",
};

export const MEMORY_TYPE_COLORS: Record<Memory["memory_type"], string> = {
  fact: "#60a5fa",
  preference: "#a78bfa",
  event: "#34d399",
  skill: "#fbbf24",
};

export const LIFECYCLE_COLORS: Record<Task["lifecycle_status"], string> = {
  active: "#4ade80",
  paused: "#facc15",
  archived: "#6b7280",
};

export const RUN_STATUS_COLORS: Record<Run["status"], string> = {
  queued: "#6b7280",
  planning: "#a78bfa",
  running: "#60a5fa",
  waiting_confirmation: "#facc15",
  completed: "#4ade80",
  failed: "#f87171",
  interrupted: "#fb923c",
};

export const KB_STATUS_COLORS: Record<KnowledgeSource["status"], string> = {
  indexing: "#facc15",
  ready: "#4ade80",
  error: "#f87171",
};

export const CHANNEL_STATUS_COLORS: Record<Channel["status"], string> = {
  connected: "#4ade80",
  disconnected: "#6b7280",
  error: "#f87171",
};
