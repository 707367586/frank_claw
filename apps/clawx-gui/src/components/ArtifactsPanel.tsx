import { Search, SlidersHorizontal, ArrowUpDown } from "lucide-react";
import { useState } from "react";

interface Artifact {
  id: string;
  title: string;
  type: "Document" | "Code" | "Image" | "Data";
  summary: string;
  agentName: string;
  agentColor: string;
  date: string;
}

const MOCK_ARTIFACTS: Artifact[] = [
  {
    id: "1",
    title: "API Integration Guide",
    type: "Document",
    summary:
      "Step-by-step guide for integrating the ClawX REST API with external services, covering authentication and rate limiting.",
    agentName: "Writer",
    agentColor: "#7C5CFC",
    date: "Apr 12",
  },
  {
    id: "2",
    title: "data_pipeline.py",
    type: "Code",
    summary:
      "Python script for ETL pipeline that processes incoming webhook events and stores them in the analytics database.",
    agentName: "Coder",
    agentColor: "#22C55E",
    date: "Apr 11",
  },
  {
    id: "3",
    title: "Q3 Performance Report",
    type: "Document",
    summary:
      "Quarterly performance analysis with key metrics, user growth trends, and recommendations for infrastructure scaling.",
    agentName: "Analyst",
    agentColor: "#F59E0B",
    date: "Apr 10",
  },
  {
    id: "4",
    title: "schema_migration.sql",
    type: "Code",
    summary:
      "Database migration adding conversation_metadata table and updating indexes for improved query performance.",
    agentName: "Coder",
    agentColor: "#22C55E",
    date: "Apr 09",
  },
];

const TYPE_COLORS: Record<string, string> = {
  Document: "#3B82F6",
  Code: "#22C55E",
  Image: "#F59E0B",
  Data: "#8B5CF6",
};

export default function ArtifactsPanel() {
  const [search, setSearch] = useState("");

  const filtered = MOCK_ARTIFACTS.filter(
    (a) =>
      a.title.toLowerCase().includes(search.toLowerCase()) ||
      a.summary.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="artifacts-panel">
      <div className="artifacts-toolbar">
        <div className="artifacts-search">
          <Search size={14} />
          <input
            type="text"
            placeholder="Search artifacts..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <button className="artifacts-btn" title="Filter">
          <SlidersHorizontal size={14} />
        </button>
        <button className="artifacts-btn" title="Sort">
          <ArrowUpDown size={14} />
        </button>
      </div>

      <div className="artifacts-list">
        {filtered.map((artifact) => (
          <div key={artifact.id} className="artifact-card">
            <div className="artifact-card-top">
              <div className="artifact-card-left">
                <span
                  className="artifact-agent-dot"
                  style={{ background: artifact.agentColor }}
                  title={artifact.agentName}
                />
                <span className="artifact-title">{artifact.title}</span>
              </div>
              <span className="artifact-date">{artifact.date}</span>
            </div>
            <div className="artifact-card-body">
              <span
                className="artifact-type-badge"
                style={{
                  background: `${TYPE_COLORS[artifact.type] ?? "#666"}22`,
                  color: TYPE_COLORS[artifact.type] ?? "#666",
                }}
              >
                {artifact.type}
              </span>
              <p className="artifact-summary">{artifact.summary}</p>
            </div>
          </div>
        ))}
        {filtered.length === 0 && (
          <p className="list-placeholder">No artifacts found.</p>
        )}
      </div>
    </div>
  );
}
