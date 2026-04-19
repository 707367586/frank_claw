/**
 * Global per-conversation streaming store.
 *
 * ChatPage mounts/unmounts as the user navigates between agents, but the
 * backend SSE fetch for an in-flight generation keeps running (see T7
 * accumulator + fix to not abort on conv change). This store lives above the
 * page so the live delta text stays accessible: returning to the conv
 * re-renders the typing indicator and accumulated content even though the
 * React subtree that started the stream has long unmounted.
 *
 * Thin on purpose: one Map keyed by convId + a subscribe/snapshot pair for
 * useSyncExternalStore. No tight coupling to React; tests exercise it
 * directly.
 */

export interface StreamState {
  content: string;
  isStreaming: boolean;
  error: string | null;
}

type Listener = () => void;

const EMPTY: StreamState = {
  content: "",
  isStreaming: false,
  error: null,
};

const store = new Map<string, StreamState>();
const listeners = new Set<Listener>();

function notify(): void {
  for (const l of listeners) l();
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getStreamState(convId: string | null | undefined): StreamState {
  if (!convId) return EMPTY;
  return store.get(convId) ?? EMPTY;
}

export function beginStream(convId: string): void {
  store.set(convId, { content: "", isStreaming: true, error: null });
  notify();
}

export function appendDelta(convId: string, text: string): void {
  const prev = store.get(convId) ?? EMPTY;
  store.set(convId, {
    content: prev.content + text,
    isStreaming: true,
    error: prev.error,
  });
  notify();
}

export function finishStream(convId: string): void {
  const prev = store.get(convId);
  if (!prev) return;
  store.set(convId, { ...prev, isStreaming: false });
  notify();
}

export function failStream(convId: string, message: string): void {
  const prev = store.get(convId) ?? EMPTY;
  store.set(convId, { content: prev.content, isStreaming: false, error: message });
  notify();
}

/** Clear a conv's slot (e.g. after successful messages refresh). */
export function clearStream(convId: string): void {
  if (store.delete(convId)) notify();
}

/** Test-only reset. */
export function __resetStreamStore(): void {
  store.clear();
  listeners.clear();
}
