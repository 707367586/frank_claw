import {
  createContext, useContext, useEffect, useMemo, useRef, useState,
  type ReactNode,
} from "react";
import { fetchPicoInfo, type PicoInfo } from "./pico-rest";
import { PicoSocket } from "./pico-socket";
import { ChatStore } from "./chat-store";

const TOKEN_KEY = "clawx.dashboard_token";

export interface ClawContextValue {
  token: string | null;
  wsUrl: string | null;
  enabled: boolean;
  configured: boolean;
  sessionId: string | null;
  chat: ChatStore;
  setToken: (token: string) => void;
  clearToken: () => void;
  startNewSession: () => void;
  sendUserMessage: (content: string) => void;
  refreshInfo: () => Promise<PicoInfo | null>;
}

const Ctx = createContext<ClawContextValue | null>(null);

export function ClawProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => {
    try { return localStorage.getItem(TOKEN_KEY); } catch { return null; }
  });
  const [wsUrl, setWsUrl] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const chatRef = useRef(new ChatStore());
  const sockRef = useRef<PicoSocket | null>(null);
  const [, forceRender] = useState(0);

  useEffect(() => chatRef.current.subscribe(() => forceRender((n) => n + 1)), []);

  const refreshInfo = async (): Promise<PicoInfo | null> => {
    if (!token) {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      return null;
    }
    try {
      const info = await fetchPicoInfo(token);
      setWsUrl(info.ws_url);
      setEnabled(info.enabled);
      setConfigured(info.configured);
      return info;
    } catch {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      return null;
    }
  };

  useEffect(() => { void refreshInfo(); /* eslint-disable-next-line */ }, [token]);

  // (Re)connect WS when we have token + ws_url + sessionId
  useEffect(() => {
    if (!token || !wsUrl || !sessionId) return;
    sockRef.current?.close();
    const s = new PicoSocket({
      wsBase: wsUrl, sessionId, token,
      onMessage: (m) => chatRef.current.applyServer(m),
    });
    s.connect();
    sockRef.current = s;
    return () => s.close();
  }, [token, wsUrl, sessionId]);

  const setToken = (t: string) => {
    try { localStorage.setItem(TOKEN_KEY, t); } catch { /* private mode */ }
    setTokenState(t);
  };

  const clearToken = () => {
    try { localStorage.removeItem(TOKEN_KEY); } catch { /* */ }
    setTokenState(null);
  };

  const startNewSession = () => setSessionId(crypto.randomUUID());

  const sendUserMessage = (content: string) => {
    if (!sockRef.current || !sessionId) return;
    const reqId = chatRef.current.addUser(content);
    sockRef.current.send({ type: "message.send", id: reqId, payload: { content } });
  };

  const value = useMemo<ClawContextValue>(
    () => ({
      token, wsUrl, enabled, configured, sessionId,
      chat: chatRef.current,
      setToken, clearToken,
      startNewSession, sendUserMessage, refreshInfo,
    }),
    [token, wsUrl, enabled, configured, sessionId],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useClaw(): ClawContextValue {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useClaw must be used inside <ClawProvider>");
  return ctx;
}
