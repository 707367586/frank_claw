import { FileCode, BookOpen, FileText } from "lucide-react";
import type { SourceRef } from "../lib/types";

export type { SourceRef };

const ICONS = { code: FileCode, doc: BookOpen, text: FileText } as const;

export default function SourceReferences({ refs }: { refs: SourceRef[] }) {
  if (!refs.length) return null;
  return (
    <ul className="src-refs">
      {refs.map((r) => {
        const Icon = ICONS[r.kind];
        return (
          <li key={r.id} className="src-ref">
            <div className="src-ref__head">
              <Icon size={14} className="src-ref__icon" />
              <span className="src-ref__filename">{r.filename}</span>
              {r.lineRange && <span className="src-ref__range">{r.lineRange}</span>}
            </div>
            <pre className="src-ref__snippet">{r.snippet}</pre>
          </li>
        );
      })}
    </ul>
  );
}
