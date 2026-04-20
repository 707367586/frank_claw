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
    return (
      <div className="empty-state">
        Pico channel disabled. Edit <code>~/.picoclaw/config.json</code>: set
        <code className="mx-1">channels.pico.enabled = true</code> and restart the launcher.
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
