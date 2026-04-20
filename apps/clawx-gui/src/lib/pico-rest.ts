export interface PicoInfo {
  configured: boolean;
  enabled: boolean;
  ws_url: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  preview: string;
  message_count: number;
  created: number;
  updated: number;
}

export interface SessionMessage {
  role: "user" | "assistant" | "system";
  content: string;
  media?: unknown;
}

export interface SessionDetail extends SessionSummary {
  messages: SessionMessage[];
  summary: string;
}

export interface SkillInfo {
  name: string;
  description?: string;
  installed?: boolean;
}

export type ToolStatus = "enabled" | "disabled" | "blocked";

export interface ToolInfo {
  name: string;
  enabled: boolean;          // derived from status === "enabled"
  status: ToolStatus;        // server-authoritative (handles blocked tools too)
  description?: string;
  category?: string;
  config_key?: string;
  reason_code?: string;      // present when status === "blocked"
}

interface ToolWireFormat {
  name: string;
  status: ToolStatus;
  description?: string;
  category?: string;
  config_key?: string;
  reason_code?: string;
}

export class PicoApiError extends Error {
  constructor(public readonly status: number, message: string) {
    super(message);
  }
}

async function call<T>(
  path: string,
  init: RequestInit & { token?: string } = {},
): Promise<T> {
  const { token, ...rest } = init;
  const headers: Record<string, string> = {
    ...(rest.headers as Record<string, string> | undefined),
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  if (rest.body && !headers["Content-Type"])
    headers["Content-Type"] = "application/json";
  const res = await fetch(path, { ...rest, headers });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.message) msg = body.message;
    } catch {
      /* ignore */
    }
    throw new PicoApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function fetchPicoInfo(token: string): Promise<PicoInfo> {
  return call<PicoInfo>("/api/pico/info", { token });
}

export function listSessions(opts: {
  offset?: number;
  limit?: number;
  token: string;
}): Promise<SessionSummary[]> {
  const params = new URLSearchParams();
  if (opts.offset != null) params.set("offset", String(opts.offset));
  if (opts.limit != null) params.set("limit", String(opts.limit));
  const q = params.toString() ? `?${params}` : "";
  return call<SessionSummary[]>(`/api/sessions${q}`, { token: opts.token });
}

export function getSession(id: string, token: string): Promise<SessionDetail> {
  return call<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`, { token });
}

export function deleteSession(id: string, token: string): Promise<void> {
  return call<void>(`/api/sessions/${encodeURIComponent(id)}`, {
    method: "DELETE",
    token,
  });
}

export async function listSkills(token: string): Promise<SkillInfo[]> {
  // Upstream wraps the list as { skills: [...] } (verified in surface audit).
  const wrap = await call<{ skills: SkillInfo[] }>("/api/skills", { token });
  return wrap.skills ?? [];
}

export async function listTools(token: string): Promise<ToolInfo[]> {
  // Upstream wraps as { tools: [...] } and uses status field, not enabled
  // (verified in surface audit; see backend/web/backend/api/tools.go).
  const wrap = await call<{ tools: ToolWireFormat[] }>("/api/tools", { token });
  return (wrap.tools ?? []).map((t) => ({
    ...t,
    enabled: t.status === "enabled",
  }));
}

export function setToolEnabled(
  name: string,
  enabled: boolean,
  token: string,
): Promise<void> {
  return call<void>(`/api/tools/${encodeURIComponent(name)}/state`, {
    method: "PUT",
    token,
    body: JSON.stringify({ enabled }),
  });
}
