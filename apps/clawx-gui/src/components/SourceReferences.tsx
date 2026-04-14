import { FileText, ChevronRight, ChevronLeft } from "lucide-react";
import { useState } from "react";

export interface SourceReference {
  filename: string;
  path: string;
  snippet: string;
  timestamp: string;
}

interface SourceReferencesProps {
  references: SourceReference[];
}

const MOCK_REFERENCES: SourceReference[] = [
  {
    filename: "product-roadmap.pdf",
    path: "/docs/planning/product-roadmap.pdf",
    snippet:
      "Q3 objectives include expanding the AI assistant capabilities with multi-modal support and improved context retention across sessions...",
    timestamp: "2026-04-10 14:30",
  },
  {
    filename: "api-design-spec.md",
    path: "/docs/specs/api-design-spec.md",
    snippet:
      "The streaming endpoint uses Server-Sent Events (SSE) to deliver incremental token output to the client in real time...",
    timestamp: "2026-04-08 09:15",
  },
  {
    filename: "meeting-notes-0405.md",
    path: "/docs/meetings/meeting-notes-0405.md",
    snippet:
      "Action items: finalize connector schema, review knowledge base indexing pipeline, deploy staging environment by Friday...",
    timestamp: "2026-04-05 16:45",
  },
];

export default function SourceReferences({ references }: SourceReferencesProps) {
  const [collapsed, setCollapsed] = useState(false);
  const items = references.length > 0 ? references : MOCK_REFERENCES;

  return (
    <div className={`source-references ${collapsed ? "collapsed" : ""}`}>
      <button
        className="source-references-toggle"
        onClick={() => setCollapsed(!collapsed)}
        title={collapsed ? "Show references" : "Hide references"}
      >
        {collapsed ? <ChevronLeft size={14} /> : <ChevronRight size={14} />}
      </button>

      {!collapsed && (
        <>
          <div className="source-references-header">
            <FileText size={16} />
            <span>Source References</span>
          </div>
          <div className="source-references-list">
            {items.map((ref, i) => (
              <div key={i} className="source-ref-card">
                <div className="source-ref-top">
                  <FileText size={14} className="source-ref-icon" />
                  <div className="source-ref-meta">
                    <span className="source-ref-filename">{ref.filename}</span>
                    <span className="source-ref-timestamp">{ref.timestamp}</span>
                  </div>
                </div>
                <div className="source-ref-path">{ref.path}</div>
                <div className="source-ref-snippet">{ref.snippet}</div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
