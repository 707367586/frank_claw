import { useEffect, useState } from "react";
import {
  listSkills, listTools, setToolEnabled,
  type SkillInfo, type ToolInfo,
} from "../lib/hermes-rest";
import { useClaw } from "../lib/store";

export default function ConnectorsPage() {
  const claw = useClaw();
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!claw.token) return;
    setError(null);
    listSkills(claw.token).then(setSkills).catch((e: Error) => setError(e.message));
    listTools(claw.token).then(setTools).catch((e: Error) => setError(e.message));
  }, [claw.token]);

  if (!claw.token) {
    return (
      <div className="p-8 text-center text-sm text-neutral-500">
        No dashboard token. Open <a href="/settings" className="underline">Settings</a> to paste yours.
      </div>
    );
  }

  const toggle = async (t: ToolInfo) => {
    if (!claw.token) return;
    const next = !t.enabled;
    setTools((arr) => arr.map((x) => (x.name === t.name ? { ...x, enabled: next } : x)));
    try {
      await setToolEnabled(t.name, next, claw.token);
    } catch (e) {
      setTools((arr) => arr.map((x) => (x.name === t.name ? { ...x, enabled: t.enabled } : x)));
      setError((e as Error).message);
    }
  };

  return (
    <div className="p-6 space-y-6 max-w-3xl">
      {error && (
        <div className="rounded border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </div>
      )}

      <section>
        <h2 className="text-lg font-semibold">Skills</h2>
        <ul className="mt-2 divide-y rounded border">
          {skills.length === 0 && (
            <li className="p-3 text-sm text-neutral-400">(no skills installed)</li>
          )}
          {skills.map((s) => (
            <li key={s.name} className="p-3 text-sm">
              <span className="font-mono">{s.name}</span>
              {s.description && (
                <span className="ml-2 text-neutral-500">— {s.description}</span>
              )}
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2 className="text-lg font-semibold">Tools</h2>
        <ul className="mt-2 divide-y rounded border">
          {tools.length === 0 && (
            <li className="p-3 text-sm text-neutral-400">(no tools available)</li>
          )}
          {tools.map((t) => (
            <li key={t.name} className="flex items-center gap-3 p-3 text-sm">
              <input
                id={`tool-${t.name}`}
                type="checkbox"
                aria-label={t.name}
                checked={t.enabled}
                onChange={() => toggle(t)}
              />
              <label htmlFor={`tool-${t.name}`} className="font-mono">{t.name}</label>
              {t.description && (
                <span className="text-neutral-500">— {t.description}</span>
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
