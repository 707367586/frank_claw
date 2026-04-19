/**
 * localStorage-backed memory of the last conversation the user viewed for
 * each agent. AgentSidebar reads this on switch; ChatPage writes it whenever
 * a convId changes. Keeps agent browsing state across reloads without
 * requiring a server round trip.
 */
export const lastConvKey = (agentId: string): string =>
  `clawx:lastConv:${agentId}`;

export function rememberConvForAgent(
  agentId: string | null | undefined,
  convId: string | null | undefined,
): void {
  if (typeof window === "undefined") return;
  if (!agentId || !convId) return;
  try {
    window.localStorage.setItem(lastConvKey(agentId), convId);
  } catch {
    // localStorage may be unavailable (Safari private mode, quota). Ignore.
  }
}

export function forgetConvForAgent(agentId: string | null | undefined): void {
  if (typeof window === "undefined") return;
  if (!agentId) return;
  try {
    window.localStorage.removeItem(lastConvKey(agentId));
  } catch {
    // ignore
  }
}

export function recallConvForAgent(
  agentId: string | null | undefined,
): string | null {
  if (typeof window === "undefined") return null;
  if (!agentId) return null;
  try {
    return window.localStorage.getItem(lastConvKey(agentId));
  } catch {
    return null;
  }
}
