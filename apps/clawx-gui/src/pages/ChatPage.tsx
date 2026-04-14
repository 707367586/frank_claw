import { useState, useEffect, useRef, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import {
  listMessages,
  sendMessageStream,
  listConversations,
  createConversation,
} from "../lib/api";
import { useAgents } from "../lib/store";
import type { Agent, Conversation, Message } from "../lib/types";
import MessageBubble from "../components/MessageBubble";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import EmptyState from "../components/EmptyState";
import SourceReferences from "../components/SourceReferences";
import ArtifactsPanel from "../components/ArtifactsPanel";

export default function ChatPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const convId = searchParams.get("conv");
  const agentId = searchParams.get("agent");

  const [messages, setMessages] = useState<Message[]>([]);
  const [streamingContent, setStreamingContent] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Conversation and agent info for header
  const [_conversation, setConversation] = useState<Conversation | null>(null);
  const [agent, setAgent] = useState<Agent | null>(null);

  // All agents list (from shared context)
  const { agents, loading: agentsLoading } = useAgents();
  const agentsLoaded = !agentsLoading;

  const [activeTab, setActiveTab] = useState<"conversation" | "artifacts">("conversation");

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

  // Resolve selected agent from agentId param
  useEffect(() => {
    if (!agentId || !agentsLoaded) return;
    const found = agents.find((a) => a.id === agentId) ?? null;
    setAgent(found);
  }, [agentId, agents, agentsLoaded]);

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
      // Don't clear agent here — it may be set via agentId param for welcome page
      return;
    }

    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const [msgs, convs] = await Promise.all([
          listMessages(convId!),
          listConversations(),
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
  }, [convId, agents]);

  // Auto-scroll on new messages or streaming content
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
            const parsed = JSON.parse(data) as { delta?: string; content?: string };
            const text = parsed.delta ?? parsed.content;
            if (text) {
              setStreamingContent((prev) => prev + text);
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

  // Send from welcome page: create conversation first, then send
  const handleWelcomeSend = useCallback(
    async (content: string) => {
      if (!agent) return;
      try {
        const conv = await createConversation(agent.id);
        setConversation(conv);
        // Navigate to the new conversation
        setSearchParams({ conv: conv.id });
        // Small delay to let state settle, then send
        setTimeout(() => {
          // We need to send after convId is set; using the conv.id directly
          const userMsg: Message = {
            id: `temp-${Date.now()}`,
            conversation_id: conv.id,
            role: "user",
            content,
            created_at: new Date().toISOString(),
          };
          setMessages([userMsg]);
          setIsStreaming(true);
          setStreamingContent("");

          abortRef.current = sendMessageStream(
            conv.id,
            content,
            (data: string) => {
              try {
                const parsed = JSON.parse(data) as { content?: string };
                if (parsed.content) {
                  setStreamingContent((prev) => prev + parsed.content);
                }
              } catch {
                setStreamingContent((prev) => prev + data);
              }
            },
            () => {
              setIsStreaming(false);
              setStreamingContent("");
              listMessages(conv.id).then(
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
        }, 50);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to create conversation");
      }
    },
    [agent, setSearchParams],
  );

  // --- Routing logic ---

  // Still loading agents list
  if (!agentsLoaded) {
    return (
      <div className="empty-state">
        <p>Loading...</p>
      </div>
    );
  }

  // No agents exist → show first-time empty state
  if (agents.length === 0) {
    return (
      <EmptyState
        onCreateFromTemplate={() => {
          // TODO: navigate to agent creation from template
        }}
        onCreateCustom={() => {
          // TODO: navigate to agent creation
        }}
      />
    );
  }

  // Agent selected but no conversation → show welcome page
  if (!convId && agent) {
    return <ChatWelcome agent={agent} onSend={handleWelcomeSend} />;
  }

  // No conversation selected and no specific agent
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
      {/* Top bar with tabs */}
      <div className="chat-top-bar">
        <div className="chat-tabs">
          <button
            className={`chat-tab ${activeTab === "conversation" ? "active" : ""}`}
            onClick={() => setActiveTab("conversation")}
          >
            Conversation
          </button>
          <button
            className={`chat-tab ${activeTab === "artifacts" ? "active" : ""}`}
            onClick={() => setActiveTab("artifacts")}
          >
            Artifacts
          </button>
        </div>
      </div>

      {/* Content area */}
      {activeTab === "conversation" ? (
        <div className="chat-content-area">
          {/* Messages */}
          <div className="chat-messages-col">
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

          {/* Source References panel */}
          <SourceReferences refs={[]} />
        </div>
      ) : (
        <ArtifactsPanel />
      )}
    </div>
  );
}
