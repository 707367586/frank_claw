import {
  createContext, useCallback, useContext, useEffect, useMemo, useRef, useState,
  type ReactNode,
} from "react";
import { fetchHermesInfo, getSession, type HermesInfo } from "./hermes-rest";
import { HermesSocket } from "./hermes-socket";
import { ChatStore } from "./chat-store";
import {
  listAgents, listToolsets, createAgent as restCreateAgent,
  deleteAgent as restDeleteAgent, rotateAgentSession,
  type Agent, type AgentCreate, type Toolset,
} from "./agents-rest";

const TOKEN_KEY = "clawx.dashboard_token";
const ACTIVE_AGENT_KEY = "clawx.active_agent";

export interface ClawContextValue {
  token: string | null;
  wsUrl: string | null;
  enabled: boolean;
  configured: boolean;
  provider: string | null;
  missingEnvVar: string | null;
  agents: Agent[];
  toolsets: Toolset[];
  activeAgentId: string | null;
  activeAgent: Agent | null;
  chat: ChatStore;
  setToken: (token: string) => void;
  clearToken: () => void;
  refreshInfo: () => Promise<HermesInfo | null>;
  selectAgent: (id: string) => void;
  createAgent: (payload: AgentCreate) => Promise<Agent>;
  deleteAgent: (id: string) => Promise<void>;
  newConversation: () => Promise<void>;
  sendUserMessage: (content: string) => void;
}

const Ctx = createContext<ClawContextValue | null>(null);

export function ClawProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => {
    try { return localStorage.getItem(TOKEN_KEY); } catch { return null; }
  });
  const [wsUrl, setWsUrl] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [provider, setProvider] = useState<string | null>(null);
  const [missingEnvVar, setMissingEnvVar] = useState<string | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [toolsets, setToolsets] = useState<Toolset[]>([]);
  const [activeAgentId, setActiveAgentId] = useState<string | null>(() => {
    try { return localStorage.getItem(ACTIVE_AGENT_KEY); } catch { return null; }
  });

  const chatRef = useRef(new ChatStore());
  const sockRef = useRef<HermesSocket | null>(null);
  // Track session IDs that were freshly created (no history to hydrate)
  const freshSessionsRef = useRef<Set<string>>(new Set());
  const [chatVersion, forceRender] = useState(0);

  useEffect(() => chatRef.current.subscribe(() => forceRender((n) => n + 1)), []);

  const refreshInfo = useCallback(async (): Promise<HermesInfo | null> => {
    if (!token) {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      setProvider(null); setMissingEnvVar(null);
      return null;
    }
    try {
      const info = await fetchHermesInfo(token);
      setWsUrl(info.ws_url); setEnabled(info.enabled); setConfigured(info.configured);
      setProvider(info.provider ?? null); setMissingEnvVar(info.missing_env_var ?? null);
      return info;
    } catch {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      setProvider(null); setMissingEnvVar(null);
      return null;
    }
  }, [token]);

  useEffect(() => { void refreshInfo(); }, [refreshInfo]);

  // Bootstrap agents + toolsets after token resolves
  useEffect(() => {
    if (!token) { setAgents([]); setToolsets([]); return; }
    (async () => {
      try { setAgents(await listAgents(token)); } catch { /* ignore */ }
      try { setToolsets(await listToolsets(token)); } catch { /* ignore */ }
    })();
  }, [token]);

  // Auto-select first agent if none selected (or selected one disappeared)
  useEffect(() => {
    if (agents.length === 0) return;
    if (activeAgentId && agents.some((a) => a.id === activeAgentId)) return;
    setActiveAgentId(agents[0].id);
  }, [agents, activeAgentId]);

  // Persist active agent id to localStorage
  useEffect(() => {
    try {
      if (activeAgentId) localStorage.setItem(ACTIVE_AGENT_KEY, activeAgentId);
      else localStorage.removeItem(ACTIVE_AGENT_KEY);
    } catch { /* ignore */ }
  }, [activeAgentId]);

  const activeAgent = useMemo<Agent | null>(
    () => agents.find((a) => a.id === activeAgentId) ?? null,
    [agents, activeAgentId],
  );

  // Hydrate history when active agent changes
  useEffect(() => {
    if (!token || !activeAgent) {
      chatRef.current.replaceMessages([]);
      return;
    }
    const sid = activeAgent.current_session_id;
    // Skip hydration for freshly-rotated sessions (they have no history)
    if (freshSessionsRef.current.has(sid)) {
      freshSessionsRef.current.delete(sid);
      return;
    }
    (async () => {
      try {
        const detail = await getSession(sid, token);
        chatRef.current.replaceMessages(detail.messages);
      } catch {
        chatRef.current.replaceMessages([]);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, activeAgent?.id, activeAgent?.current_session_id]);

  // Connect WS for the active agent's current session
  useEffect(() => {
    if (!token || !wsUrl || !activeAgent) return;
    sockRef.current?.close();
    const s = new HermesSocket({
      wsBase: wsUrl,
      sessionId: activeAgent.current_session_id,
      token,
      onMessage: (m) => chatRef.current.applyServer(m),
    });
    s.connect(activeAgent.id);
    sockRef.current = s;
    return () => s.close();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, wsUrl, activeAgent?.id, activeAgent?.current_session_id]);

  const setToken = (t: string) => {
    try { localStorage.setItem(TOKEN_KEY, t); } catch { /* */ }
    setTokenState(t);
  };
  const clearToken = () => {
    try { localStorage.removeItem(TOKEN_KEY); } catch { /* */ }
    setTokenState(null);
  };

  const selectAgent = useCallback((id: string) => setActiveAgentId(id), []);

  const createAgentFn = useCallback(async (payload: AgentCreate) => {
    if (!token) throw new Error("not authenticated");
    const a = await restCreateAgent(payload, token);
    setAgents((prev) => [...prev, a]);
    setActiveAgentId(a.id);
    return a;
  }, [token]);

  const deleteAgentFn = useCallback(async (id: string) => {
    if (!token) return;
    await restDeleteAgent(id, token);
    setAgents((prev) => {
      const next = prev.filter((a) => a.id !== id);
      if (activeAgentId === id) setActiveAgentId(next[0]?.id ?? null);
      return next;
    });
  }, [token, activeAgentId]);

  const newConversation = useCallback(async () => {
    if (!token || !activeAgent) return;
    const { session_id } = await rotateAgentSession(activeAgent.id, token);
    // Mark as fresh so the hydration effect skips it
    freshSessionsRef.current.add(session_id);
    chatRef.current.replaceMessages([]);
    setAgents((prev) =>
      prev.map((a) => a.id === activeAgent.id ? { ...a, current_session_id: session_id } : a),
    );
  }, [token, activeAgent]);

  const sendUserMessage = (content: string) => {
    const reqId = chatRef.current.addUser(content);
    sockRef.current?.send({ type: "message.send", id: reqId, payload: { content } });
  };

  const value = useMemo<ClawContextValue>(
    () => ({
      token, wsUrl, enabled, configured, provider, missingEnvVar,
      agents, toolsets, activeAgentId, activeAgent,
      chat: chatRef.current,
      setToken, clearToken, refreshInfo,
      selectAgent, createAgent: createAgentFn, deleteAgent: deleteAgentFn,
      newConversation, sendUserMessage,
    }),
    // chatVersion ensures context consumers re-render when messages/typing change
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [token, wsUrl, enabled, configured, provider, missingEnvVar,
     agents, toolsets, activeAgentId, activeAgent, refreshInfo,
     selectAgent, createAgentFn, deleteAgentFn, newConversation, chatVersion],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useClaw(): ClawContextValue {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useClaw must be used inside <ClawProvider>");
  return ctx;
}
