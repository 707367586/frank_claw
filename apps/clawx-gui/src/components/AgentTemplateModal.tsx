import { useState } from "react";
import { Code, Search, PenLine, Upload } from "lucide-react";
import Dialog from "./ui/Dialog";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "./ui/Tabs";
import Input from "./ui/Input";
import Textarea from "./ui/Textarea";
import Select from "./ui/Select";
import Button from "./ui/Button";

const TEMPLATES = [
  { id: "dev",      icon: Code,    name: "开发助手", desc: "代码审查、调试、架构设计" },
  { id: "research", icon: Search,  name: "研究分析", desc: "网络搜索、数据分析、报告撰写" },
  { id: "writing",  icon: PenLine, name: "写作创作", desc: "内容创作、编辑、文案撰写" },
];

const MODELS = [
  { value: "claude-opus-4-6",   label: "Claude Opus 4" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
  { value: "gpt-4o",            label: "GPT-4o" },
];

interface Props { open: boolean; onClose: () => void }

export default function AgentTemplateModal({ open, onClose }: Props) {
  const [tab, setTab] = useState("template");
  const [tpl, setTpl] = useState("dev");
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [prompt, setPrompt] = useState("");
  const [model, setModel] = useState("claude-opus-4-6");

  return (
    <Dialog open={open} onClose={onClose} width={560}>
      <header className="agent-template__head">
        <h2>创建新 Agent</h2>
        <p>创建一个专属的多功能智能 AI Agent</p>
      </header>

      <TabsRoot value={tab} onChange={setTab}>
        <TabsList>
          <TabsTrigger value="template">从模板</TabsTrigger>
          <TabsTrigger value="custom">自定义</TabsTrigger>
        </TabsList>

        <TabsContent value="template">
          <ul className="agent-template__grid">
            {TEMPLATES.map((t) => (
              <li key={t.id}>
                <button
                  className={`tpl-card ${tpl === t.id ? "is-active" : ""}`}
                  onClick={() => setTpl(t.id)}
                >
                  <div className="tpl-card__icon"><t.icon size={16} /></div>
                  <div className="tpl-card__text">
                    <div className="tpl-card__name">{t.name}</div>
                    <div className="tpl-card__desc">{t.desc}</div>
                  </div>
                </button>
              </li>
            ))}
          </ul>
          <label className="field">
            <span className="field__label">Agent 名称</span>
            <Input placeholder="例如: 我的开发助手" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">模型</span>
            <Select options={MODELS} value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
        </TabsContent>

        <TabsContent value="custom">
          <div className="agent-template__avatar">
            <div className="agent-template__avatar-slot">PC</div>
            <div className="agent-template__avatar-meta">
              <Button size="sm" leftIcon={<Upload size={14} />} variant="outline">上传头像</Button>
              <span>PNG, JPG 最大 2MB</span>
            </div>
          </div>
          <label className="field">
            <span className="field__label">Agent 名称</span>
            <Input placeholder="例如: 我的自定义 Agent" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">描述</span>
            <Textarea placeholder="简要描述该 Agent 的任务" value={desc} onChange={(e) => setDesc(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">系统提示词</span>
            <Textarea placeholder="输入 Agent 的指令..." value={prompt} onChange={(e) => setPrompt(e.target.value)} />
          </label>
          <label className="field">
            <span className="field__label">模型</span>
            <Select options={MODELS} value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
        </TabsContent>
      </TabsRoot>

      <footer className="agent-template__foot">
        <Button variant="ghost" onClick={onClose}>取消</Button>
        <Button variant="default">创建 Agent</Button>
      </footer>
    </Dialog>
  );
}
