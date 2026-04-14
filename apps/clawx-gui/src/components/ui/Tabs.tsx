import { createContext, useContext, type ReactNode } from "react";

interface Ctx { value: string; onChange: (v: string) => void }
const TabsCtx = createContext<Ctx | null>(null);

export function TabsRoot({ value, onChange, children }: Ctx & { children: ReactNode }) {
  return <TabsCtx.Provider value={{ value, onChange }}><div className="ui-tabs">{children}</div></TabsCtx.Provider>;
}

export function TabsList({ children }: { children: ReactNode }) {
  return <div className="ui-tabs__list" role="tablist">{children}</div>;
}

export function TabsTrigger({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsCtx)!;
  const active = ctx.value === value;
  return (
    <button
      role="tab"
      aria-selected={active}
      className={`ui-tabs__trigger ${active ? "is-active" : ""}`}
      onClick={() => ctx.onChange(value)}
    >
      {children}
    </button>
  );
}

export function TabsContent({ value, children }: { value: string; children: ReactNode }) {
  const ctx = useContext(TabsCtx)!;
  if (ctx.value !== value) return null;
  return <div role="tabpanel" className="ui-tabs__content">{children}</div>;
}

export default { Root: TabsRoot, List: TabsList, Trigger: TabsTrigger, Content: TabsContent };
