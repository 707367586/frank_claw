import { useState } from "react";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import MessageBubble from "../components/MessageBubble";
import { useClaw } from "../lib/store";

type ChatTab = "chat" | "artifacts";

export default function ChatPage() {
  const claw = useClaw();
  const [tab, setTab] = useState<ChatTab>("chat");

  if (!claw.token) {
    return (
      <div className="empty-state">
        No dashboard token. Open <a href="/settings" className="underline">Settings</a> to paste yours.
      </div>
    );
  }

  if (!claw.enabled) {
    if (claw.missingEnvVar) {
      return (
        <div className="empty-state">
          Hermes is not ready: <code>{claw.missingEnvVar}</code> is not set.
          Add it to <code>~/.hermes/.env</code> (one line, e.g.
          <code className="mx-1">{claw.missingEnvVar}=...</code>)
          and restart the backend.
        </div>
      );
    }
    return (
      <div className="empty-state">
        Hermes is not configured. Run the bootstrap
        (<code>uv run --project backend python backend/scripts/init_config.py</code>)
        and restart <code className="mx-1">hermes_bridge</code>.
      </div>
    );
  }

  const { messages, typing } = claw.chat;

  return (
    <div className="chat-page">
      <header className="chat-page__head">
        <nav className="page-tabs" role="tablist" aria-label="主区视图">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "chat"}
            className={`page-tabs__trigger ${tab === "chat" ? "is-active" : ""}`}
            onClick={() => setTab("chat")}
          >
            对话
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "artifacts"}
            className={`page-tabs__trigger ${tab === "artifacts" ? "is-active" : ""}`}
            onClick={() => setTab("artifacts")}
          >
            产物
          </button>
        </nav>
      </header>

      <div className="chat-page__body">
        {tab === "artifacts" ? (
          <div className="chat-page__placeholder">暂无产物</div>
        ) : messages.length === 0 ? (
          <ChatWelcome />
        ) : (
          messages.map((m) => (
            <MessageBubble
              key={m.id}
              role={m.role}
              content={m.content}
              thought={m.thought}
            />
          ))
        )}
        {tab === "chat" && typing && (
          <div className="msg msg--assistant" data-testid="typing">
            <div className="msg__bubble msg__bubble--streaming">
              <span className="typing-indicator" aria-label="正在生成">
                <span />
                <span />
                <span />
              </span>
            </div>
          </div>
        )}
      </div>

      <footer className="chat-page__foot">
        <ChatInput onSubmit={(text) => claw.sendUserMessage(text)} />
      </footer>
    </div>
  );
}
