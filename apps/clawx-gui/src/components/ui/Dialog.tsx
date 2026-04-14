import { useEffect, type ReactNode } from "react";
interface Props { open: boolean; onClose: () => void; title?: string; children: ReactNode; width?: number }
export default function Dialog({ open, onClose, title, children, width = 520 }: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="ui-dialog-backdrop" onClick={onClose}>
      <div className="ui-dialog" style={{ width }} onClick={(e) => e.stopPropagation()}>
        {title && <h2 className="ui-dialog__title">{title}</h2>}
        {children}
      </div>
    </div>
  );
}
