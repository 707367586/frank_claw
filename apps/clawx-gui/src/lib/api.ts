import type {
  Agent,
  Channel,
  Conversation,
  KnowledgeSource,
  Memory,
  Message,
  ModelProvider,
  Run,
  Skill,
  SystemHealth,
  SystemStats,
  Task,
  Trigger,
  VaultSnapshot,
} from "./types";

// ── Configuration ──

const BASE_URL = "http://localhost:9090";
const AUTH_TOKEN = "dev-token";

// ── Core fetch wrapper ──

export async function fetchApi<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
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

  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
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
        onError?.(err);
      }
    });

  return controller;
}

// ── Agents ──

export function listAgents(): Promise<Agent[]> {
  return fetchApi("/agents");
}

export function getAgent(id: string): Promise<Agent> {
  return fetchApi(`/agents/${id}`);
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
  return fetchApi(`/agents/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteAgent(id: string): Promise<void> {
  return fetchApi(`/agents/${id}`, { method: "DELETE" });
}

// ── Conversations ──

export function listConversations(agentId?: string): Promise<Conversation[]> {
  const params = agentId ? `?agent_id=${agentId}` : "";
  return fetchApi(`/conversations${params}`);
}

export function createConversation(agentId: string): Promise<Conversation> {
  return fetchApi("/conversations", {
    method: "POST",
    body: JSON.stringify({ agent_id: agentId }),
  });
}

export function deleteConversation(id: string): Promise<void> {
  return fetchApi(`/conversations/${id}`, { method: "DELETE" });
}

// ── Messages ──

export function listMessages(convId: string): Promise<Message[]> {
  return fetchApi(`/conversations/${convId}/messages`);
}

export function sendMessage(
  convId: string,
  content: string,
): Promise<Message> {
  return fetchApi(`/conversations/${convId}/messages`, {
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
    `/conversations/${convId}/messages?stream=true`,
    onMessage,
    onDone,
    onError,
    { content },
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
  return fetchApi(`/memories/${id}`);
}

export function deleteMemory(id: string): Promise<void> {
  return fetchApi(`/memories/${id}`, { method: "DELETE" });
}

export function pinMemory(id: string): Promise<Memory> {
  return fetchApi(`/memories/${id}/pin`, { method: "POST" });
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
  return fetchApi(`/knowledge/${id}`, { method: "DELETE" });
}

export function searchKnowledge(
  query: string,
  agentId?: string,
): Promise<unknown[]> {
  const params = new URLSearchParams({ query });
  if (agentId) params.set("agent_id", agentId);
  return fetchApi(`/knowledge/search?${params}`);
}

// ── Vault ──

export function listVaultSnapshots(): Promise<VaultSnapshot[]> {
  return fetchApi("/vault");
}

export function getSnapshot(id: string): Promise<VaultSnapshot> {
  return fetchApi(`/vault/${id}`);
}

export function rollbackSnapshot(id: string): Promise<void> {
  return fetchApi(`/vault/${id}/rollback`, { method: "POST" });
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
  return fetchApi(`/models/${id}`, { method: "DELETE" });
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
  const params = agentId ? `?agent_id=${agentId}` : "";
  return fetchApi(`/tasks${params}`);
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
  return fetchApi(`/tasks/${id}`);
}

export function updateTask(
  id: string,
  data: Partial<Omit<Task, "id" | "created_at">>,
): Promise<Task> {
  return fetchApi(`/tasks/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteTask(id: string): Promise<void> {
  return fetchApi(`/tasks/${id}`, { method: "DELETE" });
}

export function pauseTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${id}/pause`, { method: "POST" });
}

export function resumeTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${id}/resume`, { method: "POST" });
}

export function archiveTask(id: string): Promise<Task> {
  return fetchApi(`/tasks/${id}/archive`, { method: "POST" });
}

// ── Triggers ──

export function listTriggers(taskId: string): Promise<Trigger[]> {
  return fetchApi(`/task-triggers?task_id=${taskId}`);
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
  return fetchApi(`/task-triggers/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteTrigger(id: string): Promise<void> {
  return fetchApi(`/task-triggers/${id}`, { method: "DELETE" });
}

// ── Runs ──

export function listRuns(taskId: string): Promise<Run[]> {
  return fetchApi(`/task-runs?task_id=${taskId}`);
}

export function getRun(id: string): Promise<Run> {
  return fetchApi(`/task-runs/${id}`);
}

export function submitFeedback(
  runId: string,
  kind: string,
  reason?: string,
): Promise<void> {
  return fetchApi(`/task-runs/${runId}/feedback`, {
    method: "POST",
    body: JSON.stringify({ kind, reason }),
  });
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
  return fetchApi(`/channels/${id}`);
}

export function updateChannel(
  id: string,
  data: Partial<Omit<Channel, "id" | "created_at">>,
): Promise<Channel> {
  return fetchApi(`/channels/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export function deleteChannel(id: string): Promise<void> {
  return fetchApi(`/channels/${id}`, { method: "DELETE" });
}

export function connectChannel(id: string): Promise<Channel> {
  return fetchApi(`/channels/${id}/connect`, { method: "POST" });
}

export function disconnectChannel(id: string): Promise<Channel> {
  return fetchApi(`/channels/${id}/disconnect`, { method: "POST" });
}

// ── Skills ──

export function listSkills(): Promise<Skill[]> {
  return fetchApi("/skills");
}

export function getSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${id}`);
}

export function deleteSkill(id: string): Promise<void> {
  return fetchApi(`/skills/${id}`, { method: "DELETE" });
}

export function enableSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${id}/enable`, { method: "POST" });
}

export function disableSkill(id: string): Promise<Skill> {
  return fetchApi(`/skills/${id}/disable`, { method: "POST" });
}
