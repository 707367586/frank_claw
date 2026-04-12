import { useState, useEffect, useRef, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import {
  listMessages,
  sendMessageStream,
  listAgents,
  listConversations,
} from "../lib/api";
import type { Agent, Conversation, Message } from "../lib/types";
import MessageBubble from "../components/MessageBubble";
import ChatInput from "../components/ChatInput";

export default function ChatPage() {
  const [searchParams] = useSearchParams();
  const convId = searchParams.get("conv");

  const [messages, setMessages] = useState<Message[]>([]);
  const [streamingContent, setStreamingContent] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Conversation and agent info for header
  const [conversation, setConversation] = useState<Conversation | null>(null);
  const [agent, setAgent] = useState<Agent | null>(null);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  const scrollToBottom = useCallback(
    (instant = false) => {
      messagesEndRef.current?.scrollIntoView({
        behavior: instant ? "instant" : "smooth",
      });
    },
    [],
  );

  // Load messages when conversation changes
  useEffect(() => {
    // Abort any active stream when conversation changes
    abortRef.current?.abort();
    abortRef.current = null;
    setIsStreaming(false);
    setStreamingContent("");

    if (!convId) {
      setMessages([]);
      setConversation(null);
      setAgent(null);
      return;
    }

    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        // TODO: Replace with getConversation(id) / getAgent(id) when available
        // to avoid over-fetching all conversations and agents.
        const [msgs, convs, agents] = await Promise.all([
          listMessages(convId!),
          listConversations(),
          listAgents(),
        ]);
        if (cancelled) return;
        setMessages(msgs);

        const conv = convs.find((c) => c.id === convId) ?? null;
        setConversation(conv);
        if (conv) {
          const ag = agents.find((a) => a.id === conv.agent_id) ?? null;
          setAgent(ag);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
      abortRef.current?.abort();
      abortRef.current = null;
    };
  }, [convId]);

  // Auto-scroll on new messages or streaming content
  // Use instant scroll during streaming to avoid jank; smooth after completion.
  useEffect(() => {
    scrollToBottom(isStreaming);
  }, [messages, streamingContent, scrollToBottom, isStreaming]);

  // Clean up SSE on unmount
  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  const handleSend = useCallback(
    (content: string) => {
      if (!convId || isStreaming) return;

      // Optimistically add user message
      const userMsg: Message = {
        id: `temp-${Date.now()}`,
        conversation_id: convId,
        role: "user",
        content,
        created_at: new Date().toISOString(),
      };
      setMessages((prev) => [...prev, userMsg]);
      setIsStreaming(true);
      setStreamingContent("");

      abortRef.current = sendMessageStream(
        convId,
        content,
        (data: string) => {
          try {
            const parsed = JSON.parse(data) as { content?: string };
            if (parsed.content) {
              setStreamingContent((prev) => prev + parsed.content);
            }
          } catch {
            // If it's plain text, append directly
            setStreamingContent((prev) => prev + data);
          }
        },
        () => {
          // Stream complete — refresh messages from server
          setIsStreaming(false);
          setStreamingContent("");
          listMessages(convId).then(
            (msgs) => setMessages(msgs),
            (err) => console.error("Failed to refresh messages:", err),
          );
        },
        (err: Error) => {
          console.error("Stream error:", err);
          setIsStreaming(false);
          setStreamingContent("");
          setError("Failed to send message. Please try again.");
        },
      );
    },
    [convId, isStreaming],
  );

  // No conversation selected
  if (!convId) {
    return (
      <div className="empty-state">
        <h2>Select a conversation</h2>
        <p>Choose a conversation from the sidebar or start a new chat.</p>
      </div>
    );
  }

  return (
    <div className="chat-view">
      {/* Header */}
      <div className="chat-header">
        <div className="chat-header-info">
          <span className="chat-header-agent">{agent?.name ?? "Agent"}</span>
          <span className="chat-header-title">
            {conversation?.title || "New conversation"}
          </span>
        </div>
      </div>

      {/* Messages */}
      <div className="messages">
        {loading && (
          <p className="list-placeholder">Loading messages...</p>
        )}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading &&
          !error &&
          messages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}
        {/* Streaming assistant message */}
        {isStreaming && streamingContent && (
          <MessageBubble
            message={{
              id: "streaming",
              conversation_id: convId,
              role: "assistant",
              content: streamingContent,
              created_at: new Date().toISOString(),
            }}
          />
        )}
        {isStreaming && !streamingContent && (
          <div className="message assistant">
            <div className="typing-indicator">
              <span />
              <span />
              <span />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <ChatInput onSend={handleSend} disabled={isStreaming} />
    </div>
  );
}
