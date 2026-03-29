import { useState, useEffect } from "react";

interface Agent {
  id: string;
  name: string;
  role: string;
  status: string;
}

const API_BASE = "http://localhost:9090";

function App() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [messages, setMessages] = useState<{ role: string; content: string }[]>([]);
  const [input, setInput] = useState("");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  useEffect(() => {
    fetchAgents();
  }, []);

  async function fetchAgents() {
    try {
      const res = await fetch(`${API_BASE}/agents`, {
        headers: { Authorization: "Bearer dev-token" },
      });
      if (res.ok) {
        const data = await res.json();
        setAgents(data);
      }
    } catch {
      // Service not running
    }
  }

  async function sendMessage() {
    if (!input.trim() || !selectedAgent) return;

    const userMsg = { role: "user", content: input };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");

    try {
      const res = await fetch(`${API_BASE}/conversations`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: "Bearer dev-token",
        },
        body: JSON.stringify({
          agent_id: selectedAgent,
          message: input,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: data.content || "[no response]" },
        ]);
      }
    } catch {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "[error: service unavailable]" },
      ]);
    }
  }

  return (
    <div className="app">
      <aside className={`sidebar ${sidebarCollapsed ? "collapsed" : ""}`}>
        <div className="sidebar-header">
          <h1 className="logo">ClawX</h1>
          <button
            className="collapse-btn"
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
          >
            {sidebarCollapsed ? ">" : "<"}
          </button>
        </div>
        {!sidebarCollapsed && (
          <>
            <div className="agent-list">
              <h2>Agents</h2>
              {agents.length === 0 && (
                <p className="empty">No agents found. Start the service first.</p>
              )}
              {agents.map((agent) => (
                <div
                  key={agent.id}
                  className={`agent-card ${selectedAgent === agent.id ? "selected" : ""}`}
                  onClick={() => setSelectedAgent(agent.id)}
                >
                  <div className="agent-name">{agent.name}</div>
                  <div className="agent-role">{agent.role}</div>
                </div>
              ))}
            </div>
            <div className="sidebar-footer">
              <div className="nav-item">Knowledge Base</div>
              <div className="nav-item">Tasks</div>
              <div className="nav-item">Connectors</div>
              <div className="nav-item">Settings</div>
            </div>
          </>
        )}
      </aside>
      <main className="main-content">
        {selectedAgent ? (
          <div className="chat-view">
            <div className="messages">
              {messages.map((msg, i) => (
                <div key={i} className={`message ${msg.role}`}>
                  <div className="message-content">{msg.content}</div>
                </div>
              ))}
            </div>
            <div className="input-bar">
              <input
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendMessage()}
                placeholder="Type a message..."
              />
              <button onClick={sendMessage}>Send</button>
            </div>
          </div>
        ) : (
          <div className="empty-state">
            <h2>Select an Agent to start chatting</h2>
            <p>Choose an agent from the sidebar, or create a new one.</p>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
