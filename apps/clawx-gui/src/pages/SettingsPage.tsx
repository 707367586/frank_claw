import { useState } from "react";
import { useClaw } from "../lib/store";

export default function SettingsPage() {
  const claw = useClaw();
  const [draft, setDraft] = useState("");

  const save = (e: React.FormEvent) => {
    e.preventDefault();
    const t = draft.trim();
    if (!t) return;
    claw.setToken(t);
    setDraft("");
  };

  return (
    <div className="p-6 space-y-6 max-w-xl">
      <h1 className="text-xl font-semibold">Settings</h1>

      {!claw.token ? (
        <section>
          <h2 className="font-medium">Connect to hermes_bridge</h2>
          <p className="mt-1 text-xs text-neutral-500">
            Start the launcher with <code>pnpm dev</code>, copy the line
            <code className="mx-1">dashboardToken: …</code> from its stdout, paste it below.
          </p>
          <form onSubmit={save} className="mt-3 flex gap-2">
            <label className="flex-1">
              <span className="sr-only">Dashboard token</span>
              <input
                aria-label="Dashboard token"
                type="text"
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                placeholder="paste dashboardToken…"
                className="w-full rounded border px-3 py-2 text-sm font-mono"
                autoFocus
              />
            </label>
            <button type="submit" className="rounded bg-blue-600 px-4 py-2 text-sm text-white">
              Save
            </button>
          </form>
        </section>
      ) : (
        <section>
          <h2 className="font-medium">Hermes Connection</h2>
          <dl className="mt-2 grid grid-cols-[140px_1fr] gap-x-4 gap-y-1 text-sm">
            <dt>Token</dt>
            <dd className="font-mono break-all">{maskToken(claw.token)}</dd>
            <dt>WebSocket URL</dt>
            <dd className="font-mono">{claw.wsUrl ?? "(unknown — refresh)"}</dd>
            <dt>Configured</dt>
            <dd>{claw.configured ? "yes" : "no"}</dd>
            <dt>Enabled</dt>
            <dd>{claw.enabled ? "yes" : "no"}</dd>
          </dl>
          <div className="mt-3 flex gap-2">
            <button
              className="rounded bg-neutral-200 px-3 py-1 text-sm"
              onClick={() => void claw.refreshInfo()}
            >
              Refresh
            </button>
            <button
              className="rounded bg-neutral-200 px-3 py-1 text-sm"
              onClick={() => claw.clearToken()}
            >
              Clear token
            </button>
          </div>
          {!claw.enabled && (
            <p className="mt-3 text-xs text-amber-600">
              Hermes is not configured. Run
              <code className="mx-1">uv run --project backend python backend/scripts/init_config.py</code>
              or edit <code>~/.hermes/config.yaml</code>, then restart
              <code className="mx-1">hermes_bridge</code>.
            </p>
          )}
        </section>
      )}
    </div>
  );
}

function maskToken(t: string): string {
  if (t.length <= 8) return t;
  return t.slice(0, 4) + "…" + t.slice(-4);
}
