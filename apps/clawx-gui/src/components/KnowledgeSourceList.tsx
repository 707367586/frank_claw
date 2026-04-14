import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus, FolderOpen } from "lucide-react";
import { listKnowledgeSources, addKnowledgeSource } from "../lib/api";
import { KB_STATUS_COLORS } from "../lib/constants";
import type { KnowledgeSource } from "../lib/types";

const FOLDER_COLORS = [
  "#7C5CFC", "#60a5fa", "#34d399", "#fbbf24", "#f87171", "#a78bfa", "#fb923c", "#22d3ee",
];

function truncatePath(path: string, maxLen = 28): string {
  if (path.length <= maxLen) return path;
  return "..." + path.slice(path.length - maxLen + 3);
}

export default function KnowledgeSourceList() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("source");

  const [sources, setSources] = useState<KnowledgeSource[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [showAddForm, setShowAddForm] = useState(false);
  const [newPath, setNewPath] = useState("");
  const [newAgentId, setNewAgentId] = useState("");
  const [addError, setAddError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const loadSources = useCallback(async () => {
    try {
      setError(null);
      const data = await listKnowledgeSources();
      setSources(data);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load knowledge sources",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSources();
  }, [loadSources]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return sources;
    return sources.filter((s) => s.path.toLowerCase().includes(q));
  }, [sources, search]);

  const handleSelect = useCallback(
    (id: string) => {
      setSearchParams({ source: id });
    },
    [setSearchParams],
  );

  const handleAdd = useCallback(async () => {
    if (!newPath.trim() || !newAgentId.trim()) {
      setAddError("Both folder path and agent ID are required.");
      return;
    }
    setAdding(true);
    setAddError(null);
    try {
      const created = await addKnowledgeSource(newPath.trim(), newAgentId.trim());
      setSources((prev) => [created, ...prev]);
      setNewPath("");
      setNewAgentId("");
      setShowAddForm(false);
    } catch (err) {
      setAddError(
        err instanceof Error ? err.message : "Failed to add knowledge source",
      );
    } finally {
      setAdding(false);
    }
  }, [newPath, newAgentId]);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <div className="list-panel-header-row">
          <h2 className="list-panel-title">Knowledge Sources</h2>
          <button
            className="btn-add-green"
            onClick={() => setShowAddForm((v) => !v)}
            aria-label="Add knowledge source"
          >
            <Plus size={14} /> 添加知识源
          </button>
        </div>
      </div>

      {showAddForm && (
        <div className="kb-add-form">
          <input
            type="text"
            className="form-input"
            placeholder="Folder path (e.g. /data/docs)"
            aria-label="Folder path"
            value={newPath}
            onChange={(e) => setNewPath(e.target.value)}
          />
          <input
            type="text"
            className="form-input"
            placeholder="Agent ID"
            aria-label="Agent ID for knowledge source"
            value={newAgentId}
            onChange={(e) => setNewAgentId(e.target.value)}
          />
          {addError && <p className="form-error">{addError}</p>}
          <div className="form-actions">
            <button
              className="btn-secondary"
              onClick={() => {
                setShowAddForm(false);
                setAddError(null);
              }}
              disabled={adding}
            >
              Cancel
            </button>
            <button
              className="btn-primary"
              onClick={handleAdd}
              disabled={adding}
              aria-label="Confirm add knowledge source"
            >
              {adding ? "Adding..." : "Add"}
            </button>
          </div>
        </div>
      )}

      <div className="list-panel-search">
        <Search size={14} className="search-icon" />
        <input
          type="text"
          className="search-input"
          aria-label="Search knowledge sources"
          placeholder="Search knowledge..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="list-panel-content">
        {loading && <p className="list-placeholder">Loading...</p>}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "No matches" : "No knowledge sources yet"}
          </p>
        )}
        {filtered.map((source, idx) => (
          <button
            key={source.id}
            className={`kb-source-card ${selectedId === source.id ? "selected" : ""}`}
            onClick={() => handleSelect(source.id)}
            aria-label={`Select knowledge source ${source.path}`}
          >
            <div
              className="kb-source-icon"
              style={{ background: FOLDER_COLORS[idx % FOLDER_COLORS.length] + "22", color: FOLDER_COLORS[idx % FOLDER_COLORS.length] }}
            >
              <FolderOpen size={18} />
            </div>
            <div className="kb-source-info">
              <span className="kb-source-path" title={source.path}>
                {truncatePath(source.path)}
              </span>
              <span className="kb-source-stats">
                {source.doc_count} docs
                <span className="kb-source-agent-tag">{source.agent_id.slice(0, 6)}</span>
              </span>
            </div>
            <span
              className="kb-status-badge"
              style={{ background: KB_STATUS_COLORS[source.status] }}
              title={source.status}
            >
              {source.status}
            </span>
          </button>
        ))}
      </div>
    </aside>
  );
}
