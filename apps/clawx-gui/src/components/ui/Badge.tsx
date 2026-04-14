import type { ReactNode } from "react";
type Tone = "neutral" | "success" | "warning" | "error" | "info" | "primary";
export default function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return <span className={`ui-badge ui-badge--${tone}`}>{children}</span>;
}
