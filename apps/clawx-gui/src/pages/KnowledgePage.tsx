import { useState, useEffect, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Trash2, RefreshCw, FolderOpen } from "lucide-react";
import {
  listKnowledgeSources,
  deleteKnowledgeSource,
  addKnowledgeSource,
  searchKnowledge,
  listAgents,
} from "../lib/api";
import type { KnowledgeSource, KnowledgeSearchResult, Agent } from "../lib/types";

const KB_STATUS_COLORS: Record<KnowledgeSource["status"], string> = {
  indexing: "#facc15",
  ready: "#4ade80",
  error: "#f87171",
};

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function KnowledgePage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const sourceId = searchParams.get("source");

  const [source, setSource] = useState<KnowledgeSource | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [reindexing, setReindexing] = useState(false);

  // Search state
  const [searchQuery, setSearchQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("");
  const [results, setResults] = useState<KnowledgeSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  // Agents for filter dropdown
  const [agents, setAgents] = useState<Agent[]>([]);

  useEffect(() => {
    listAgents()
      .then(setAgents)
      .catch(() => {
        /* agent list is optional for filter */
      });
  }, []);

  const loadSource = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const all = await listKnowledgeSources();
      const found = all.find((s) => s.id === id) ?? null;
      setSource(found);
      if (!found) setError("Source not found");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load source",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!sourceId) {
      setSource(null);
      return;
    }
    loadSource(sourceId);
  }, [sourceId, loadSource]);

  const handleDelete = useCallback(async () => {
    if (!source) return;
    const confirmed = window.confirm(
      `Delete knowledge source "${source.path}"? This cannot be undone.`,
    );
    if (!confirmed) return;

    setMutationError(null);
    try {
      await deleteKnowledgeSource(source.id);
      setSource(null);
      setSearchParams({});
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to delete source",
      );
    }
  }, [source, setSearchParams]);

  const handleReindex = useCallback(async () => {
    if (!source) return;
    setReindexing(true);
    setMutationError(null);
    try {
      await deleteKnowledgeSource(source.id);
      const reindexed = await addKnowledgeSource(source.path, source.agent_id);
      setSource(reindexed);
      // Update the URL param to the new source id
      setSearchParams({ source: reindexed.id });
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to reindex source",
      );
    } finally {
      setReindexing(false);
    }
  }, [source, setSearchParams]);

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) return;
    setSearching(true);
    setSearchError(null);
    setHasSearched(true);
    try {
      const data = await searchKnowledge(
        searchQuery.trim(),
        agentFilter || undefined,
      );
      setResults(data);
    } catch (err) {
      setSearchError(
        err instanceof Error ? err.message : "Search failed",
      );
      setResults([]);
    } finally {
      setSearching(false);
    }
  }, [searchQuery, agentFilter]);

  const handleSearchKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") handleSearch();
    },
    [handleSearch],
  );

  return (
    <div className="kb-page">
      {/* Source detail section */}
      {sourceId && (
        <div className="kb-detail-section">
          {loading && <p className="list-placeholder">Loading source...</p>}
          {error && <p className="form-error">{error}</p>}
          {mutationError && <p className="form-error">{mutationError}</p>}
          {source && (
            <div className="kb-detail-card">
              <div className="kb-detail-header">
                <div className="kb-detail-header-left">
                  <FolderOpen size={24} />
                  <div>
                    <h3 className="kb-detail-path">{source.path}</h3>
                    <span className="kb-detail-created">
                      Added {formatDate(source.created_at)}
                    </span>
                  </div>
                  <span
                    className="kb-status-badge-lg"
                    style={{ background: KB_STATUS_COLORS[source.status] }}
                  >
                    {source.status}
                  </span>
                </div>
                <div className="agent-detail-actions">
                  <button
                    className="btn-icon"
                    onClick={handleReindex}
                    disabled={reindexing}
                    title="Reindex source"
                    aria-label="Reindex knowledge source"
                  >
                    <RefreshCw
                      size={16}
                      className={reindexing ? "spin" : ""}
                    />
                  </button>
                  <button
                    className="btn-icon btn-danger"
                    onClick={handleDelete}
                    title="Delete source"
                    aria-label="Delete knowledge source"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
              <div className="kb-detail-stats">
                <div className="kb-stat">
                  <span className="kb-stat-value">{source.doc_count}</span>
                  <span className="kb-stat-label">Documents</span>
                </div>
                <div className="kb-stat">
                  <span className="kb-stat-value">{source.chunk_count}</span>
                  <span className="kb-stat-label">Chunks</span>
                </div>
                <div className="kb-stat">
                  <span className="kb-stat-value">{source.agent_id.slice(0, 8)}</span>
                  <span className="kb-stat-label">Agent</span>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Search workbench */}
      <div className="kb-search-section">
        <h3 className="kb-section-title">Knowledge Search</h3>
        <div className="kb-search-bar">
          <div className="kb-search-input-wrap">
            <Search size={16} className="kb-search-icon" />
            <input
              type="text"
              className="form-input kb-search-input"
              placeholder="Search knowledge base..."
              aria-label="Search knowledge base"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={handleSearchKeyDown}
            />
          </div>
          <select
            className="form-input kb-agent-filter"
            value={agentFilter}
            onChange={(e) => setAgentFilter(e.target.value)}
            aria-label="Filter by agent"
          >
            <option value="">All agents</option>
            {agents.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          <button
            className="btn-primary"
            onClick={handleSearch}
            disabled={searching || !searchQuery.trim()}
            aria-label="Execute knowledge search"
          >
            {searching ? "Searching..." : "Search"}
          </button>
        </div>

        {searchError && <p className="form-error">{searchError}</p>}

        <div className="kb-results">
          {!hasSearched && (
            <div className="empty-state">
              <p>Enter a query above to search the knowledge base.</p>
            </div>
          )}
          {hasSearched && !searching && results.length === 0 && !searchError && (
            <div className="empty-state">
              <p>No results found.</p>
            </div>
          )}
          {results.map((result) => (
            <div key={result.chunk_id} className="kb-result-card">
              <div className="kb-result-header">
                <span className="kb-result-source" title={result.source_path}>
                  {result.source_path}
                </span>
                <span className="kb-result-score">
                  {Math.round(result.score * 100)}%
                </span>
              </div>
              <div className="kb-result-score-bar">
                <div
                  className="kb-result-score-fill"
                  style={{ width: `${Math.round(result.score * 100)}%` }}
                />
              </div>
              <p className="kb-result-content">{result.content}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
