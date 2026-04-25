import type { HermesMessage } from "./hermes-types";

export interface HermesSocketOptions {
  wsBase: string;
  sessionId: string;
  token: string;
  onMessage?: (msg: HermesMessage) => void;
  onOpen?: () => void;
  onClose?: (code: number) => void;
  onError?: (err: unknown) => void;
}

const RECONNECT_INITIAL_MS = 500;
const RECONNECT_MAX_MS = 30_000;

export class HermesSocket {
  private ws: WebSocket | null = null;
  private queue: HermesMessage[] = [];
  private reconnectMs = RECONNECT_INITIAL_MS;
  private closedByUser = false;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private agentId: string | null = null;

  constructor(private readonly opts: HermesSocketOptions) {}

  connect(agentId?: string | null): void {
    this.closedByUser = false;
    if (agentId !== undefined) this.agentId = agentId ?? null;
    const params = new URLSearchParams({ session_id: this.opts.sessionId });
    if (this.agentId) params.set("agent_id", this.agentId);
    const url = `${this.opts.wsBase}?${params.toString()}`;
    const ws = new WebSocket(url, [`token.${this.opts.token}`]);
    this.ws = ws;
    ws.onopen = () => {
      this.reconnectMs = RECONNECT_INITIAL_MS;
      while (this.queue.length) {
        const m = this.queue.shift()!;
        ws.send(JSON.stringify(m));
      }
      this.opts.onOpen?.();
    };
    ws.onmessage = (ev) => {
      let parsed: HermesMessage;
      try {
        parsed = JSON.parse(typeof ev.data === "string" ? ev.data : "") as HermesMessage;
      } catch {
        return;
      }
      this.opts.onMessage?.(parsed);
    };
    ws.onerror = (err) => this.opts.onError?.(err);
    ws.onclose = (ev) => {
      this.opts.onClose?.(ev.code);
      if (this.closedByUser) return;
      this.timer = setTimeout(() => this.connect(), this.reconnectMs);
      this.reconnectMs = Math.min(this.reconnectMs * 2, RECONNECT_MAX_MS);
    };
  }

  send(msg: HermesMessage): void {
    const enriched: HermesMessage = {
      ...msg,
      id: msg.id ?? crypto.randomUUID(),
      session_id: msg.session_id ?? this.opts.sessionId,
      timestamp: msg.timestamp ?? Date.now(),
    };
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(enriched));
    } else {
      this.queue.push(enriched);
    }
  }

  close(): void {
    this.closedByUser = true;
    if (this.timer) clearTimeout(this.timer);
    this.ws?.close(1000);
  }
}
