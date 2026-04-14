import type {
  Agent,
  Channel,
  Conversation,
  KnowledgeSearchResult,
  KnowledgeSource,
  Memory,
  Message,
  ModelProvider,
  PermissionProfile,
  Run,
  Skill,
  SystemHealth,
  SystemStats,
  Task,
  Trigger,
  VaultSnapshot,
} from "./types";

// ── Configuration ──

const BASE_URL = import.meta.env.VITE_API_URL ?? "http://localhost:9090";
const AUTH_TOKEN = import.meta.env.VITE_AUTH_TOKEN ?? "dev-token";

// ── Core fetch wrappers ──

async function baseFetch(
  path: string,
  options: RequestInit = {},
): Promise<Response> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${AUTH_TOKEN}`,
    ...(options.headers as Record<string, string>),
  };

  const method = (options.method ?? "GET").toUpperCase();
  if (method === "POST" || method === "PUT") {
    headers["Content-Type"] = headers["Content-Type"] ?? "application/json";
  }

  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    const body = await res.text();
    let message = `${res.status} ${res.statusText}`;
    try {
      const parsed = JSON.parse(body);
      if (parsed.message) message = parsed.message;
    } catch {
      if (body) message = body;
    }
    throw new Error(message);
  }

  return res;
}

export async function fetchApi<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const res = await baseFetch(path, options);
  const text = await res.text();
  if (!text) {
    throw new Error(`Expected JSON response body from ${path}, got empty`);
  }
  return JSON.parse(text) as T;
}

export async function fetchApiVoid(
  path: string,
  options: RequestInit = {},
): Promise<void> {
  await baseFetch(path, options);
}

// ── SSE streaming ──

export function connectSSE(
  path: string,
  onMessage: (data: string) => void,
  onDone?: () => void,
  onError?: (err: Error) => void,
  body?: unknown,
): AbortController {
  const controller = new AbortController();
  const handleError = onError ?? console.error;

  fetch(`${BASE_URL}${path}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${AUTH_TOKEN}`,
      "Content-Type": "application/json",
      Accept: "text/event-stream",
    },
    body: body ? JSON.stringify(body) : undefined,
    signal: controller.signal,
  })
    .then(async (res) => {
      if (!res.ok) {
        throw new Error(`SSE error: ${res.status} ${res.statusText}`);
      }
      const reader = res.body?.getReader();
      if (!reader) throw new Error("No response body");

      const decoder = new TextDecoder();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          if (line.startsWith("data: ")) {
            const data = line.slice(6);
            if (data === "[DONE]") {
              onDone?.();
              return;
            }
            onMessage(data);
          }
        }
      }
      onDone?.();
    })
    .catch((err) => {
      if (err.name !== "AbortError") {
        handleError(err);
      }
    });

  return controller;
}

// ── Agents ──

export function listAgents(): Promise<Agent[]> {
  return fetchApi("/agents");
}

export function getAgent(id: string): Promise<Agent> {
  return fetchApi(`/agents/${encodeURIComponent(id)}`);
}

export function createAgent(
  data: Partial<Omit<Agent, "id" | "created_at" | "updated_at">>,
): Promise<Agent> {
  return fetchApi("/agents", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function updateAgent(
  id: string,
  data: Partial<Omit<Agent, "id" | "created_at" | "updated_at">>,
): Promise<Agent> {
  return fetchApi(`/agents/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteAgent(id: string): Promise<void> {
  return fetchApiVoid(`/agents/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function getPermissionProfile(
  agentId: string,
): Promise<PermissionProfile> {
  return fetchApi(`/agents/${encodeURIComponent(agentId)}/permission-profile`);
}

// ── Conversations ──

export function listConversations(agentId?: string): Promise<Conversation[]> {
  const params = new URLSearchParams();
  if (agentId) params.set("agent_id", agentId);
  const qs = params.toString();
  return fetchApi(`/conversations${qs ? `?${qs}` : ""}`);
}

export function createConversation(agentId: string): Promise<Conversation> {
  return fetchApi("/conversations", {
    method: "POST",
    body: JSON.stringify({ agent_id: agentId }),
  });
}

export function deleteConversation(id: string): Promise<void> {
  return fetchApiVoid(`/conversations/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

// ── Messages ──

export function listMessages(convId: string): Promise<Message[]> {
  return fetchApi(`/conversations/${encodeURIComponent(convId)}/messages`);
}

export function sendMessage(
  convId: string,
  content: string,
): Promise<Message> {
  return fetchApi(`/conversations/${encodeURIComponent(convId)}/messages`, {
    method: "POST",
    body: JSON.stringify({ content }),
  });
}

export function sendMessageStream(
  convId: string,
  content: string,
  onMessage: (data: string) => void,
  onDone?: () => void,
  onError?: (err: Error) => void,
): AbortController {
  return connectSSE(
    `/conversations/${encodeURIComponent(convId)}/messages`,
    onMessage,
    onDone,
    onError,
    { role: "user", content, stream: true },
  );
}

// ── Memories ──

export function listMemories(
  agentId?: string,
  scope?: string,
  query?: string,
): Promise<Memory[]> {
  const params = new URLSearchParams();
  if (agentId) params.set("agent_id", agentId);
  if (scope) params.set("scope", scope);
  if (query) params.set("query", query);
  const qs = params.toString();
  return fetchApi(`/memories${qs ? `?${qs}` : ""}`);
}

export function getMemory(id: string): Promise<Memory> {
  return fetchApi(`/memories/${encodeURIComponent(id)}`);
}

export function deleteMemory(id: string): Promise<void> {
  return fetchApiVoid(`/memories/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function pinMemory(id: string): Promise<Memory> {
  return fetchApi(`/memories/${encodeURIComponent(id)}/pin`, {
    method: "POST",
  });
}

// ── Knowledge ──

export function listKnowledgeSources(): Promise<KnowledgeSource[]> {
  return fetchApi("/knowledge");
}

export function addKnowledgeSource(
  path: string,
  agentId: string,
): Promise<KnowledgeSource> {
  return fetchApi("/knowledge", {
    method: "POST",
    body: JSON.stringify({ path, agent_id: agentId }),
  });
}

export function deleteKnowledgeSource(id: string): Promise<void> {
  return fetchApiVoid(`/knowledge/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function searchKnowledge(
  query: string,
  agentId?: string,
): Promise<KnowledgeSearchResult[]> {
  const params = new URLSearchParams({ query });
  if (agentId) params.set("agent_id", agentId);
  return fetchApi(`/knowledge/search?${params}`);
}

// ── Vault ──

export function listVaultSnapshots(): Promise<VaultSnapshot[]> {
  return fetchApi("/vault");
}

export function getSnapshot(id: string): Promise<VaultSnapshot> {
  return fetchApi(`/vault/${encodeURIComponent(id)}`);
}

export function rollbackSnapshot(id: string): Promise<void> {
  return fetchApiVoid(`/vault/${encodeURIComponent(id)}/rollback`, {
    method: "POST",
  });
}

// ── Models ──

export function listModels(): Promise<ModelProvider[]> {
  return fetchApi("/models");
}

export function createModel(
  data: Partial<Omit<ModelProvider, "id" | "created_at">>,
): Promise<ModelProvider> {
  return fetchApi("/models", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function deleteModel(id: string): Promise<void> {
  return fetchApiVoid(`/models/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

// ── System ──

export function getHealth(): Promise<SystemHealth> {
  return fetchApi("/system/health");
}

export function getStats(): Promise<SystemStats> {
  return fetchApi("/system/stats");
}

// ── Tasks ──

export function listTasks(agentId?: string): Promise<Task[]> {
  const params = new URLSearchParams();
  if (agentId) params.set("agent_id", agentId);
  const qs = params.toString();
  return fetchApi(`/tasks${qs ? `?${qs}` : ""}`);
}

export function createTask(
  data: Partial<Omit<Task, "id" | "created_at">>,
): Promise<Task> {
  return fetchApi("/tasks", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function getTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${encodeURIComponent(id)}`);
}

export function updateTask(
  id: string,
  data: Partial<Omit<Task, "id" | "created_at">>,
): Promise<Task> {
  return fetchApi(`/tasks/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteTask(id: string): Promise<void> {
  return fetchApiVoid(`/tasks/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function pauseTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${encodeURIComponent(id)}/pause`, {
    method: "POST",
  });
}

export function resumeTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${encodeURIComponent(id)}/resume`, {
    method: "POST",
  });
}

export function archiveTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${encodeURIComponent(id)}/archive`, {
    method: "POST",
  });
}

// ── Triggers ──

export function listTriggers(taskId: string): Promise<Trigger[]> {
  const params = new URLSearchParams({ task_id: taskId });
  return fetchApi(`/task-triggers?${params}`);
}

export function addTrigger(
  taskId: string,
  data: Partial<Omit<Trigger, "id" | "task_id" | "created_at">>,
): Promise<Trigger> {
  return fetchApi("/task-triggers", {
    method: "POST",
    body: JSON.stringify({ ...data, task_id: taskId }),
  });
}

export function updateTrigger(
  id: string,
  data: Partial<Omit<Trigger, "id" | "task_id" | "created_at">>,
): Promise<Trigger> {
  return fetchApi(`/task-triggers/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteTrigger(id: string): Promise<void> {
  return fetchApiVoid(`/task-triggers/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

// ── Runs ──

export function listRuns(taskId: string): Promise<Run[]> {
  const params = new URLSearchParams({ task_id: taskId });
  return fetchApi(`/task-runs?${params}`);
}

export function getRun(id: string): Promise<Run> {
  return fetchApi(`/task-runs/${encodeURIComponent(id)}`);
}

export function submitFeedback(
  runId: string,
  kind: string,
  reason?: string,
): Promise<void> {
  return fetchApiVoid(
    `/task-runs/${encodeURIComponent(runId)}/feedback`,
    {
      method: "POST",
      body: JSON.stringify({ kind, reason }),
    },
  );
}

// ── Channels ──

export function listChannels(): Promise<Channel[]> {
  return fetchApi("/channels");
}

export function createChannel(
  data: Partial<Omit<Channel, "id" | "created_at">>,
): Promise<Channel> {
  return fetchApi("/channels", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export function getChannel(id: string): Promise<Channel> {
  return fetchApi(`/channels/${encodeURIComponent(id)}`);
}

export function updateChannel(
  id: string,
  data: Partial<Omit<Channel, "id" | "created_at">>,
): Promise<Channel> {
  return fetchApi(`/channels/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteChannel(id: string): Promise<void> {
  return fetchApiVoid(`/channels/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function connectChannel(id: string): Promise<Channel> {
  return fetchApi(`/channels/${encodeURIComponent(id)}/connect`, {
    method: "POST",
  });
}

export function disconnectChannel(id: string): Promise<Channel> {
  return fetchApi(`/channels/${encodeURIComponent(id)}/disconnect`, {
    method: "POST",
  });
}

// ── Skills ──

export function listSkills(): Promise<Skill[]> {
  return fetchApi("/skills");
}

export function getSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${encodeURIComponent(id)}`);
}

export function deleteSkill(id: string): Promise<void> {
  return fetchApiVoid(`/skills/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export function enableSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${encodeURIComponent(id)}/enable`, {
    method: "POST",
  });
}

export function disableSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${encodeURIComponent(id)}/disable`, {
    method: "POST",
  });
}
