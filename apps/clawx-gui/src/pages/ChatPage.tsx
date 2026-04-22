import { useEffect } from "react";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import MessageBubble from "../components/MessageBubble";
import { useClaw } from "../lib/store";

export default function ChatPage() {
  const claw = useClaw();

  useEffect(() => {
    if (!claw.sessionId && claw.enabled) claw.startNewSession();
  }, [claw.enabled, claw.sessionId]);

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
      <div className="chat-page__body">
        {messages.length === 0 ? (
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
        {typing && (
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
