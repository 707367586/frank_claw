import type { PicoMessage } from "./pico-types";

export interface PicoSocketOptions {
  wsBase: string;
  sessionId: string;
  token: string;
  onMessage?: (msg: PicoMessage) => void;
  onOpen?: () => void;
  onClose?: (code: number) => void;
  onError?: (err: unknown) => void;
}

const RECONNECT_INITIAL_MS = 500;
const RECONNECT_MAX_MS = 30_000;

export class PicoSocket {
  private ws: WebSocket | null = null;
  private queue: PicoMessage[] = [];
  private reconnectMs = RECONNECT_INITIAL_MS;
  private closedByUser = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly opts: PicoSocketOptions) {}

  connect(): void {
    this.closedByUser = false;
    const url = `${this.opts.wsBase}?session_id=${encodeURIComponent(this.opts.sessionId)}`;
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
      let parsed: PicoMessage;
      try {
        parsed = JSON.parse(typeof ev.data === "string" ? ev.data : "") as PicoMessage;
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

  send(msg: PicoMessage): void {
    const enriched: PicoMessage = {
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
