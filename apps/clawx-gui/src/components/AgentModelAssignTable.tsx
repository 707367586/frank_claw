import { useEffect, useState } from "react";
import Select from "./ui/Select";
import { listAgents, updateAgent } from "../lib/api";
import type { Agent, ModelProvider } from "../lib/types";

interface Props {
  providers: ModelProvider[];
}

export default function AgentModelAssignTable({ providers }: Props) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savingId, setSavingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listAgents()
      .then((list) => {
        if (!cancelled) setAgents(list);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "加载 Agent 失败");
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const options = providers.map((p) => ({
    value: p.id,
    label: `${p.name} · ${p.model_name}`,
  }));

  const providerById = new Map(providers.map((p) => [p.id, p]));

  const handleChange = async (agent: Agent, newId: string) => {
    if (!newId || newId === agent.model_id) return;
    setSavingId(agent.id);
    setError(null);
    try {
      const updated = await updateAgent(agent.id, { model_id: newId });
      setAgents((prev) => prev.map((a) => (a.id === agent.id ? updated : a)));
    } catch (err) {
      setError(err instanceof Error ? err.message : "更新失败");
    } finally {
      setSavingId(null);
    }
  };

  if (loading) return <p className="settings-page__placeholder">加载中…</p>;

  return (
    <>
      {error && <p className="settings-page__placeholder">{error}</p>}
      <div className="mm-table">
        <div className="mm-table__head">
          <span>Agent</span>
          <span>角色</span>
          <span>Provider</span>
          <span />
        </div>
        {agents.map((a) => {
          const current = a.model_id ? providerById.get(a.model_id) : undefined;
          return (
            <div key={a.id} className="mm-table__row">
              <span>{a.name}</span>
              <span>{a.role}</span>
              <span>
                {options.length === 0 ? (
                  <em style={{ color: "var(--muted-foreground)" }}>
                    {current ? `${current.name} · ${current.model_name}` : "无可用 Provider"}
                  </em>
                ) : (
                  <Select
                    options={options}
                    value={a.model_id ?? ""}
                    disabled={savingId === a.id}
                    onChange={(e) => handleChange(a, e.target.value)}
                  />
                )}
              </span>
              <span style={{ fontSize: 11, color: "var(--muted-foreground)" }}>
                {savingId === a.id ? "保存中…" : ""}
              </span>
            </div>
          );
        })}
      </div>
    </>
  );
}
