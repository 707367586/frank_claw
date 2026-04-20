import type {
  ErrorPayload,
  MessageCreatePayload,
  PicoMessage,
} from "./pico-types";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  thought: boolean;
  requestId?: string;
  ts: number;
}

export class ChatStore {
  messages: ChatMessage[] = [];
  typing = false;
  lastError: ErrorPayload | null = null;
  private subs = new Set<() => void>();

  subscribe(fn: () => void): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }

  private emit(): void {
    this.subs.forEach((f) => f());
  }

  addUser(content: string, requestId?: string): string {
    const id = requestId ?? crypto.randomUUID();
    this.messages = [
      ...this.messages,
      { id, role: "user", content, thought: false, requestId, ts: Date.now() },
    ];
    this.emit();
    return id;
  }

  applyServer(msg: PicoMessage): void {
    switch (msg.type) {
      case "message.create": {
        const p = msg.payload as unknown as MessageCreatePayload;
        this.messages = [
          ...this.messages,
          {
            id: p.message_id,
            role: "assistant",
            content: p.content,
            thought: !!p.thought,
            ts: Date.now(),
          },
        ];
        break;
      }
      case "message.update": {
        const p = msg.payload as unknown as MessageCreatePayload;
        this.messages = this.messages.map((m) =>
          m.id === p.message_id
            ? { ...m, content: p.content, thought: !!p.thought }
            : m,
        );
        break;
      }
      case "typing.start": this.typing = true; break;
      case "typing.stop": this.typing = false; break;
      case "error": {
        const p = msg.payload as unknown as ErrorPayload;
        this.lastError = p;
        if (p.request_id) {
          this.messages = this.messages.filter((m) => m.requestId !== p.request_id);
        }
        break;
      }
      default: return;
    }
    this.emit();
  }
}
