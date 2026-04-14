import { useState } from "react";
import { Plus } from "lucide-react";
import Button from "../components/ui/Button";
import SettingsNav from "../components/SettingsNav";
import ModelProviderCard from "../components/ModelProviderCard";
import AgentModelAssignTable from "../components/AgentModelAssignTable";

export default function SettingsPage() {
  const [section, setSection] = useState("model");
  return (
    <div className="settings-page">
      <SettingsNav value={section} onChange={setSection} />
      <section className="settings-page__main">
        {section === "model" && (
          <>
            <header className="settings-page__head">
              <h2>模型 Provider</h2>
              <Button leftIcon={<Plus size={14} />} size="sm">添加</Button>
            </header>
            <div className="settings-page__providers">
              <ModelProviderCard emoji="☁️" name="Anthropic (Claude)" available summary="模型: Claude Opus 4, Claude Sonnet 4.6" apiKey="sk-ant-•••••••" />
              <ModelProviderCard emoji="🏠" name="Ollama (本地)" available summary="模型: llama3:70b, codellama:34b" apiKey="地址: http://localhost:11434" />
              <ModelProviderCard emoji="🔌" name="OpenAI" available={false} summary="API Key 未配置" />
            </div>
            <h2 className="settings-page__section-title">Agent 模型分配</h2>
            <AgentModelAssignTable />
          </>
        )}
        {section !== "model" && <div className="settings-page__placeholder">该分组将在后续迭代实现。</div>}
      </section>
    </div>
  );
}
