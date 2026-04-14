import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { listAgents } from './api';
import type { Agent } from './types';

interface AgentContextValue {
  agents: Agent[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

const AgentContext = createContext<AgentContextValue>({
  agents: [],
  loading: true,
  error: null,
  refresh: () => {},
});

export function AgentProvider({ children }: { children: ReactNode }) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      const data = await listAgents();
      setAgents(data);
      setError(null);
    } catch (e: any) {
      setError(e.message || 'Failed to load agents');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  return (
    <AgentContext.Provider value={{ agents, loading, error, refresh }}>
      {children}
    </AgentContext.Provider>
  );
}

export function useAgents() {
  return useContext(AgentContext);
}
