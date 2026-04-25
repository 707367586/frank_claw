import { HermesApiError } from "./hermes-rest";

export interface Agent {
  id: string;
  name: string;
  description: string;
  color: string;
  icon: string;
  system_prompt: string;
  model: string | null;
  enabled_toolsets: string[];
  workspace_dir: string;
  current_session_id: string;
  created_at: number;
}

export interface AgentCreate {
  name: string;
  description: string;
  color: string;
  icon: string;
  system_prompt: string;
  model: string | null;
  enabled_toolsets: string[];
  workspace_dir?: string;
}

export interface Toolset {
  name: string;
  description: string;
  tools: string[];
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
  if (rest.body && !headers["Content-Type"]) headers["Content-Type"] = "application/json";
  const res = await fetch(path, { ...rest, headers });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.message) msg = body.message;
    } catch { /* ignore */ }
    throw new HermesApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function listAgents(token: string): Promise<Agent[]> {
  return call<Agent[]>("/api/agents", { token });
}

export function createAgent(payload: AgentCreate, token: string): Promise<Agent> {
  return call<Agent>("/api/agents", { method: "POST", token, body: JSON.stringify(payload) });
}

export function deleteAgent(id: string, token: string): Promise<void> {
  return call<void>(`/api/agents/${encodeURIComponent(id)}`, { method: "DELETE", token });
}

export function rotateAgentSession(
  id: string,
  token: string,
): Promise<{ session_id: string }> {
  return call<{ session_id: string }>(
    `/api/agents/${encodeURIComponent(id)}/sessions`,
    { method: "POST", token },
  );
}

export function listToolsets(token: string): Promise<Toolset[]> {
  return call<Toolset[]>("/api/toolsets", { token });
}
