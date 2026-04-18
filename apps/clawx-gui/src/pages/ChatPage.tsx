import { useState, useEffect, useRef, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import {
  listMessages,
  listModels,
  sendMessageStream,
  listConversations,
  createConversation,
} from "../lib/api";
import { useAgents } from "../lib/store";
import type { Agent, Conversation, Message, ModelProvider } from "../lib/types";
import MessageBubble from "../components/MessageBubble";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import SourceReferences from "../components/SourceReferences";
import ArtifactsPanel from "../components/ArtifactsPanel";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../components/ui/Tabs";

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

  // Providers list for resolving agent.model_id → provider.model_name
  const [providers, setProviders] = useState<ModelProvider[]>([]);

  useEffect(() => {
    let cancelled = false;
    listModels()
      .then((p) => { if (!cancelled) setProviders(p); })
      .catch(() => { /* silent; composer falls back to 未选择 */ });
    return () => { cancelled = true; };
  }, []);

  const modelName = agent
    ? providers.find((p) => p.id === agent.model_id)?.model_name
    : undefined;

  const [activeTab, setActiveTab] = useState<"conversation" | "artifacts">("conversation");

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  // When the welcome flow creates a conversation + starts a stream in one go,
  // the convId URL param flips null → new-id and would otherwise trigger the
  // load-messages effect below, aborting our just-started stream. This ref
  // records the conv id we're mid-stream into so the effect can skip itself
  // for that transition only.
  const streamingConvRef = useRef<string | null>(null);

  const scrollToBottom = useCallback(
    (instant = false) => {
      messagesEndRef.current?.scrollIntoView({
        behavior: instant ? "instant" : "smooth",
      });
    },
    [],
  );

  const refreshMessages = useCallback(async (id: string) => {
    try {
      const msgs = await listMessages(id);
      setMessages(msgs);
    } catch (e) {
      console.error("Failed to refresh messages:", e);
      setError("Failed to refresh messages. Please reload.");
    }
  }, []);

  // Resolve selected agent from agentId param
  useEffect(() => {
    if (!agentId || !agentsLoaded) return;
    const found = agents.find((a) => a.id === agentId) ?? null;
    setAgent(found);
  }, [agentId, agents, agentsLoaded]);

  // Load messages when conversation changes
  useEffect(() => {
    // Skip the reset/fetch if we just started streaming into this conv ourselves
    // (welcome-flow: createConversation → setSearchParams → stream). Clearing the
    // ref here guarantees the *next* convId change (real navigation) still
    // aborts and reloads as expected.
    if (convId && streamingConvRef.current === convId) {
      streamingConvRef.current = null;
      return;
    }

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

  const streamToConversation = useCallback(
    (convId: string, userText: string) => {
      const userMsg: Message = {
        id: `temp-${Date.now()}`,
        conversation_id: convId,
        role: "user",
        content: userText,
        created_at: new Date().toISOString(),
      };
      setMessages((prev) => [...prev, userMsg]);
      setIsStreaming(true);
      setStreamingContent("");

      abortRef.current = sendMessageStream(
        convId,
        userText,
        (data: string) => {
          try {
            const parsed = JSON.parse(data) as { delta?: string; content?: string };
            const text = parsed.delta ?? parsed.content;
            if (text) {
              setStreamingContent((prev) => prev + text);
            }
          } catch {
            setStreamingContent((prev) => prev + data);
          }
        },
        async () => {
          setIsStreaming(false);
          setStreamingContent("");
          await refreshMessages(convId);
        },
        (err: Error) => {
          console.error("Stream error:", err);
          setIsStreaming(false);
          setStreamingContent("");
          setError("Failed to send message. Please try again.");
        },
      );
    },
    [refreshMessages],
  );

  const handleSend = useCallback(
    (content: string) => {
      if (!convId || isStreaming) return;
      streamToConversation(convId, content);
    },
    [convId, isStreaming, streamToConversation],
  );

  // Send from welcome page: create conversation first, then send
  const handleWelcomeSend = useCallback(
    async (content: string) => {
      if (!agent || isStreaming) return;
      try {
        const conv = await createConversation(agent.id);
        setConversation(conv);
        // Mark this conv as already-streaming so the URL param change below
        // does not retrigger the load-messages effect (which would abort our
        // stream). The effect clears the ref when it observes the match.
        streamingConvRef.current = conv.id;
        setSearchParams({ conv: conv.id });
        streamToConversation(conv.id, content);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to create conversation");
      }
    },
    [agent, isStreaming, setSearchParams, streamToConversation],
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

  // Agent selected but no conversation → show welcome page
  if (!convId && agent) {
    return <ChatWelcome agent={agent} onSend={handleWelcomeSend} />;
  }

  // No conversation selected and no specific agent
  if (!convId) {
    return (
      <div className="empty-state">
        <h2>选择一个 Agent 开始对话</h2>
        <p>从左侧列表选择一个 Agent，或创建新的对话。</p>
      </div>
    );
  }

  return (
    <div className="chat-page">
      <TabsRoot value={activeTab} onChange={(v) => setActiveTab(v as "conversation" | "artifacts")}>
        <header className="chat-page__head">
          <TabsList>
            <TabsTrigger value="conversation">对话</TabsTrigger>
            <TabsTrigger value="artifacts">产物</TabsTrigger>
          </TabsList>
        </header>

        <TabsContent value="conversation">
          <div className="chat-page__body">
            {loading && <p className="chat-page__placeholder">加载中…</p>}
            {error && <p className="chat-page__placeholder">{error}</p>}
            {!loading && !error && (
              <div className="chat-page__stream">
                {messages.map((m) => (
                  <div key={m.id}>
                    <MessageBubble message={m} />
                    <SourceReferences refs={m.refs ?? []} />
                  </div>
                ))}
                {isStreaming && streamingContent && (
                  <MessageBubble
                    message={{
                      id: "streaming",
                      conversation_id: convId!,
                      role: "assistant",
                      content: streamingContent,
                      created_at: new Date().toISOString(),
                    }}
                  />
                )}
                {isStreaming && !streamingContent && (
                  <div className="msg msg--assistant">
                    <div className="msg__bubble">
                      <span className="typing-indicator"><span /><span /><span /></span>
                    </div>
                  </div>
                )}
                <div ref={messagesEndRef} />
              </div>
            )}
          </div>
        </TabsContent>

        <TabsContent value="artifacts">
          <div className="chat-page__body">
            <ArtifactsPanel conversationId={convId ?? undefined} />
          </div>
        </TabsContent>

        <footer className="chat-page__foot">
          <ChatInput onSend={handleSend} disabled={isStreaming || loading} model={modelName} />
        </footer>
      </TabsRoot>
    </div>
  );
}
