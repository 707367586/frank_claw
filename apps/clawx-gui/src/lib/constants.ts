import type { Agent, Memory } from "./types";

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
