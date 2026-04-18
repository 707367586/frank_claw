import { useCallback, useEffect, useState } from "react";
import { Plus } from "lucide-react";
import Button from "../components/ui/Button";
import SettingsNav from "../components/SettingsNav";
import ModelProviderCard from "../components/ModelProviderCard";
import AddProviderModal from "../components/AddProviderModal";
import AgentModelAssignTable from "../components/AgentModelAssignTable";
import { listModels, deleteModel } from "../lib/api";
import type { ModelProvider } from "../lib/types";

export default function SettingsPage() {
  const [section, setSection] = useState("model");
  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ModelProvider | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await listModels();
      setProviders(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (section === "model") {
      refresh();
    }
  }, [section, refresh]);

  const handleDelete = async (id: string) => {
    if (!confirm("确认删除该 Provider？Agent 若正在使用它将无法调用。")) return;
    setDeletingId(id);
    try {
      await deleteModel(id);
      setProviders((prev) => prev.filter((p) => p.id !== id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "删除失败");
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <div className="settings-page">
      <SettingsNav value={section} onChange={setSection} />
      <section className="settings-page__main">
        {section === "model" && (
          <>
            <header className="settings-page__head">
              <h2>模型 Provider</h2>
              <Button
                leftIcon={<Plus size={14} />}
                size="sm"
                onClick={() => setModalOpen(true)}
              >
                添加
              </Button>
            </header>

            {error && <p className="settings-page__placeholder">{error}</p>}
            {loading && <p className="settings-page__placeholder">加载中…</p>}

            {!loading && providers.length === 0 && (
              <p className="settings-page__placeholder">
                还没有配置任何 Provider，点击"添加"填入智谱 API Key 以启用 GLM 模型。
              </p>
            )}

            <div className="settings-page__providers">
              {providers.map((p) => (
                <ModelProviderCard
                  key={p.id}
                  provider={p}
                  onEdit={(prov) => {
                    setEditing(prov);
                    setModalOpen(true);
                  }}
                  onDelete={handleDelete}
                  busy={deletingId === p.id}
                />
              ))}
            </div>

            <h2 className="settings-page__section-title">Agent 模型分配</h2>
            <AgentModelAssignTable providers={providers} />

            <AddProviderModal
              open={modalOpen}
              initial={editing ?? undefined}
              onClose={() => {
                setModalOpen(false);
                setEditing(null);
              }}
              onSaved={(p) => {
                setProviders((prev) => {
                  const exists = prev.some((x) => x.id === p.id);
                  return exists
                    ? prev.map((x) => (x.id === p.id ? p : x))
                    : [p, ...prev];
                });
                setEditing(null);
              }}
            />
          </>
        )}
        {section !== "model" && (
          <div className="settings-page__placeholder">该分组将在后续迭代实现。</div>
        )}
      </section>
    </div>
  );
}
