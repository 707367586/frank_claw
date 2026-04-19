import { useState, useEffect, useRef, useCallback, useSyncExternalStore } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import {
  listMessages,
  listModels,
  sendMessageStream,
  listConversations,
  createConversation,
} from "../lib/api";
import { useAgents } from "../lib/store";
import { rememberConvForAgent } from "../lib/agent-conv-memory";
import {
  appendDelta,
  beginStream,
  clearStream,
  failStream,
  finishStream,
  getStreamState,
  subscribe,
} from "../lib/chat-stream-store";
import type { Agent, Conversation, Message, ModelProvider } from "../lib/types";
import MessageBubble from "../components/MessageBubble";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import SourceReferences from "../components/SourceReferences";
import ArtifactsPanel from "../components/ArtifactsPanel";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../components/ui/Tabs";

export default function ChatPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const convId = searchParams.get("conv");
  const agentId = searchParams.get("agent");

  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Live stream state for the currently viewed conv, sourced from the global
  // store so that a stream started in another (now-unmounted) ChatPage
  // instance keeps rendering its typing indicator + accumulated text when the
  // user navigates back.
  const streamState = useSyncExternalStore(
    subscribe,
    () => getStreamState(convId),
    () => getStreamState(convId),
  );
  const isStreaming = streamState.isStreaming;
  const streamingContent = streamState.content;

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

  // Persist the current (agent, conv) pair so switching agents can resume.
  useEffect(() => {
    rememberConvForAgent(agentId, convId);
  }, [agentId, convId]);

  const [activeTab, setActiveTab] = useState<"conversation" | "artifacts">("conversation");

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  // When the welcome flow creates a conversation + starts a stream in one go,
  // the convId URL param flips null → new-id and would otherwise trigger the
  // load-messages effect below, aborting our just-started stream. This ref
  // records the conv id we're mid-stream into so the effect can skip itself
  // for that transition only. Single-shot, cleared on every convId change
  // (both matching and non-matching) so a stale marker can't silently skip a
  // later navigation back to the same conv.
  const streamingConvRef = useRef<string | null>(null);

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
    // Skip the reset/fetch if we just started streaming into this conv ourselves
    // (welcome-flow: createConversation → setSearchParams → stream). Clearing the
    // ref here guarantees the *next* convId change (real navigation) still
    // aborts and reloads as expected.
    if (convId && streamingConvRef.current === convId) {
      streamingConvRef.current = null;
      return;
    }

    // User navigated to a different conv: drop any stale welcome handoff marker
    // so we don't silently skip the load on a later return to it.
    streamingConvRef.current = null;

    // Deliberately do NOT abort any in-flight stream when the conversation
    // changes. The global chat-stream-store keeps the delta feed alive so
    // returning to this conv (possibly in another ChatPage instance) picks
    // up the typing indicator + accumulated content automatically via
    // useSyncExternalStore above. We only drop our local controller ref.
    abortRef.current = null;

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
      // Only cancel the in-progress listMessages/listConversations fetch
      // on effect teardown. Leave any SSE stream in flight.
      cancelled = true;
    };
  }, [convId, agents]);

  // Auto-scroll on new messages or streaming content
  useEffect(() => {
    scrollToBottom(isStreaming);
  }, [messages, streamingContent, scrollToBottom, isStreaming]);

  // We intentionally do NOT abort SSE streams on unmount. Navigating away
  // from the chat page (e.g. to /settings) should leave the current agent's
  // generation running in the background; the backend persists the final
  // reply on completion, so it is visible next time the user opens that conv.

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
      beginStream(convId);

      abortRef.current = sendMessageStream(
        convId,
        userText,
        (data: string) => {
          try {
            const parsed = JSON.parse(data) as { delta?: string; content?: string };
            const text = parsed.delta ?? parsed.content;
            if (text) appendDelta(convId, text);
          } catch {
            appendDelta(convId, data);
          }
        },
        async () => {
          finishStream(convId);
          try {
            const msgs = await listMessages(convId);
            setMessages(msgs);
          } catch (e) {
            console.error("Failed to refresh messages:", e);
          }
          // Once messages include the persisted assistant reply, drop the
          // store slot so the typing indicator doesn't reappear.
          clearStream(convId);
        },
        (err: Error) => {
          console.error("Stream error:", err);
          failStream(convId, err.message || "Failed to send message. Please try again.");
          setError(err.message || "Failed to send message. Please try again.");
        },
      );
    },
    [],
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
            {error && (
              <div className="chat-page__placeholder">
                <p>{error}</p>
                {/\bno provider registered\b/i.test(error) && (
                  <p style={{ marginTop: 8 }}>
                    <button
                      type="button"
                      className="chat-welcome__tag"
                      onClick={() => navigate("/settings")}
                    >
                      前往设置 → 填入 API Key
                    </button>
                  </p>
                )}
              </div>
            )}
            {!loading && !error && (
              <div className="chat-page__stream">
                {messages.map((m) => (
                  <div key={m.id}>
                    <MessageBubble message={m} />
                    <SourceReferences refs={m.refs ?? []} />
                  </div>
                ))}
                {isStreaming && (
                  <div className="msg msg--assistant">
                    <div className="msg__bubble msg__bubble--streaming">
                      {streamingContent}
                      <span className="typing-indicator" aria-label="正在生成">
                        <span />
                        <span />
                        <span />
                      </span>
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
