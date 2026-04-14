import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Plus, Search } from "lucide-react";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../components/ui/Tabs";
import Input from "../components/ui/Input";
import Button from "../components/ui/Button";
import AgentGridCard from "../components/AgentGridCard";
import SkillStore from "../components/SkillStore";
import AgentTemplateModal from "../components/AgentTemplateModal";
import { useAgents } from "../lib/store";

export default function AgentsPage() {
  const { agents } = useAgents();
  const navigate = useNavigate();
  const [tab, setTab] = useState("agent");
  const [query, setQuery] = useState("");
  const [openNew, setOpenNew] = useState(false);

  const filtered = agents.filter((a) => a.name.toLowerCase().includes(query.toLowerCase()));

  return (
    <div className="agents-page">
      <TabsRoot value={tab} onChange={setTab}>
        <header className="agents-page__head">
          <TabsList>
            <TabsTrigger value="agent">Agent</TabsTrigger>
            <TabsTrigger value="skill">Skill</TabsTrigger>
          </TabsList>
          <div className="agents-page__head-right">
            <Input size="sm" leftIcon={<Search size={14} />} placeholder="搜索 Agent..." value={query} onChange={(e) => setQuery(e.target.value)} />
            <Button leftIcon={<Plus size={14} />} size="sm" onClick={() => setOpenNew(true)}>新建 Agent</Button>
          </div>
        </header>

        <TabsContent value="agent">
          <div className="agents-page__grid">
            {filtered.map((a) => (
              <AgentGridCard
                key={a.id}
                agent={a}
                onEnter={() => navigate(`/?agent=${a.id}`)}
                onEdit={() => navigate(`/agents/${a.id}/edit`)}
              />
            ))}
          </div>
        </TabsContent>
        <TabsContent value="skill">
          <SkillStore />
        </TabsContent>
      </TabsRoot>

      <AgentTemplateModal open={openNew} onClose={() => setOpenNew(false)} />
    </div>
  );
}
