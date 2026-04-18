import { useEffect, useMemo, useState } from "react";
import { Code, Search, PenLine, Upload } from "lucide-react";
import Dialog from "./ui/Dialog";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "./ui/Tabs";
import Input from "./ui/Input";
import Textarea from "./ui/Textarea";
import Select from "./ui/Select";
import Button from "./ui/Button";
import { createAgent, listModels } from "../lib/api";
import { useAgents } from "../lib/store";
import type { ModelProvider } from "../lib/types";

interface Template {
  id: string;
  icon: typeof Code;
  name: string;
  role: string;
  desc: string;
  systemPrompt: string;
  capabilities: string[];
}

const TEMPLATES: Template[] = [
  {
    id: "dev",
    icon: Code,
    name: "开发助手",
    role: "Developer",
    desc: "代码审查、调试、架构设计",
    systemPrompt:
      "You are a professional developer assistant. Help the user with code review, debugging, and architecture design. Be concise and direct.",
    capabilities: ["web_search", "code_gen", "pdf_reader"],
  },
  {
    id: "research",
    icon: Search,
    name: "研究分析",
    role: "Researcher",
    desc: "网络搜索、数据分析、报告撰写",
    systemPrompt:
      "You are a research assistant. Help the user find information, analyze data, and write reports.",
    capabilities: ["web_search", "pdf_reader"],
  },
  {
    id: "writing",
    icon: PenLine,
    name: "写作创作",
    role: "Writer",
    desc: "内容创作、编辑、文案撰写",
    systemPrompt:
      "You are a writing assistant. Help the user draft, edit, and polish content.",
    capabilities: ["translator"],
  },
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function AgentTemplateModal({ open, onClose }: Props) {
  const { refresh } = useAgents();

  const [tab, setTab] = useState("template");
  const [tplId, setTplId] = useState("dev");
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [prompt, setPrompt] = useState("");
  const [modelId, setModelId] = useState("");

  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset + reload providers whenever modal opens.
  useEffect(() => {
    if (!open) return;
    setTab("template");
    setTplId("dev");
    setName("");
    setDesc("");
    setPrompt("");
    setModelId("");
    setError(null);
    setSubmitting(false);

    setLoadingProviders(true);
    listModels()
      .then((list) => {
        setProviders(list);
        const preferred = list.find((p) => p.is_default) ?? list[0];
        if (preferred) setModelId(preferred.id);
      })
      .catch((err) =>
        setError(err instanceof Error ? err.message : "加载 Provider 失败"),
      )
      .finally(() => setLoadingProviders(false));
  }, [open]);

  const modelOptions = useMemo(
    () =>
      providers.map((p) => ({
        value: p.id,
        label: `${p.name} · ${p.model_name}`,
      })),
    [providers],
  );

  const selectedTemplate = TEMPLATES.find((t) => t.id === tplId) ?? TEMPLATES[0];

  const canSubmit =
    !submitting &&
    !loadingProviders &&
    modelId !== "" &&
    name.trim() !== "" &&
    (tab === "template" || prompt.trim() !== "");

  const handleSubmit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const payload =
        tab === "template"
          ? {
              name: name.trim(),
              role: selectedTemplate.role,
              system_prompt: selectedTemplate.systemPrompt,
              model_id: modelId,
              capabilities: selectedTemplate.capabilities,
            }
          : {
              name: name.trim(),
              role: "Custom",
              system_prompt: prompt.trim(),
              model_id: modelId,
              capabilities: [],
            };
      await createAgent(payload);
      await refresh();
      // Avoid leaking `desc` to a linter — currently only used in custom UI.
      void desc;
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onClose={onClose} width={560}>
      <header className="agent-template__head">
        <h2>创建新 Agent</h2>
        <p>选择模板或自定义一个 Agent，并绑定到一个模型 Provider。</p>
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
                  className={`tpl-card ${tplId === t.id ? "is-active" : ""}`}
                  onClick={() => setTplId(t.id)}
                  type="button"
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
            <Input
              placeholder="例如: 我的开发助手"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label className="field">
            <span className="field__label">模型 Provider</span>
            {providers.length === 0 ? (
              <p className="settings-page__placeholder">
                {loadingProviders
                  ? "加载中…"
                  : "还没有配置任何 Provider，请先到 设置 → 模型 Provider 添加一个。"}
              </p>
            ) : (
              <Select
                options={modelOptions}
                value={modelId}
                onChange={(e) => setModelId(e.target.value)}
              />
            )}
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
            <Input
              placeholder="例如: 我的自定义 Agent"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label className="field">
            <span className="field__label">描述</span>
            <Textarea
              placeholder="简要描述该 Agent 的任务"
              value={desc}
              onChange={(e) => setDesc(e.target.value)}
            />
          </label>
          <label className="field">
            <span className="field__label">系统提示词</span>
            <Textarea
              placeholder="输入 Agent 的指令..."
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
            />
          </label>
          <label className="field">
            <span className="field__label">模型 Provider</span>
            {providers.length === 0 ? (
              <p className="settings-page__placeholder">
                {loadingProviders
                  ? "加载中…"
                  : "还没有配置任何 Provider，请先到 设置 → 模型 Provider 添加一个。"}
              </p>
            ) : (
              <Select
                options={modelOptions}
                value={modelId}
                onChange={(e) => setModelId(e.target.value)}
              />
            )}
          </label>
        </TabsContent>
      </TabsRoot>

      {error && (
        <p className="field" style={{ color: "var(--destructive, #dc2626)" }}>
          {error}
        </p>
      )}

      <footer className="agent-template__foot">
        <Button variant="ghost" onClick={onClose} disabled={submitting}>
          取消
        </Button>
        <Button variant="default" onClick={handleSubmit} disabled={!canSubmit}>
          {submitting ? "创建中…" : "创建 Agent"}
        </Button>
      </footer>
    </Dialog>
  );
}
