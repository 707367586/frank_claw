import { useEffect, useState } from "react";
import Dialog from "./ui/Dialog";
import Input from "./ui/Input";
import Select from "./ui/Select";
import Button from "./ui/Button";
import { createModel, updateModel } from "../lib/api";
import type { ModelProvider } from "../lib/types";

type ProviderType = ModelProvider["provider_type"];

interface ProviderPreset {
  label: string;
  base_url: string;
  default_model: string;
}

const PRESETS: Record<ProviderType, ProviderPreset> = {
  zhipu: {
    label: "智谱 AI (GLM)",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    default_model: "glm-4.6",
  },
  anthropic: {
    label: "Anthropic (Claude)",
    base_url: "https://api.anthropic.com",
    default_model: "claude-sonnet-4-6",
  },
  openai: {
    label: "OpenAI (GPT)",
    base_url: "https://api.openai.com",
    default_model: "gpt-4o",
  },
  ollama: {
    label: "Ollama (本地)",
    base_url: "http://localhost:11434",
    default_model: "llama3",
  },
  custom: {
    label: "自定义",
    base_url: "",
    default_model: "",
  },
};

const TYPE_OPTIONS = (Object.keys(PRESETS) as ProviderType[]).map((k) => ({
  value: k,
  label: PRESETS[k].label,
}));

interface Props {
  open: boolean;
  onClose: () => void;
  onSaved: (provider: ModelProvider) => void;
  initial?: ModelProvider;
}

export default function AddProviderModal({ open, onClose, onSaved, initial }: Props) {
  const [type, setType] = useState<ProviderType>("zhipu");
  const [name, setName] = useState("智谱 AI");
  const [baseUrl, setBaseUrl] = useState(PRESETS.zhipu.base_url);
  const [modelName, setModelName] = useState(PRESETS.zhipu.default_model);
  const [apiKey, setApiKey] = useState("");
  const [isDefault, setIsDefault] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset to defaults (or prefill from `initial`) whenever the modal opens.
  useEffect(() => {
    if (!open) return;
    if (initial) {
      setType(initial.provider_type);
      setName(initial.name);
      setBaseUrl(initial.base_url);
      setModelName(initial.model_name);
      const params = (initial.parameters ?? {}) as { api_key?: string };
      setApiKey(typeof params.api_key === "string" ? params.api_key : "");
      setIsDefault(initial.is_default);
    } else {
      setType("zhipu");
      setName(PRESETS.zhipu.label);
      setBaseUrl(PRESETS.zhipu.base_url);
      setModelName(PRESETS.zhipu.default_model);
      setApiKey("");
      setIsDefault(false);
    }
    setError(null);
    setSubmitting(false);
  }, [open, initial]);

  const handleTypeChange = (next: ProviderType) => {
    const preset = PRESETS[next];
    setType(next);
    setName(preset.label);
    setBaseUrl(preset.base_url);
    setModelName(preset.default_model);
  };

  const canSubmit =
    name.trim() !== "" &&
    modelName.trim() !== "" &&
    (type === "ollama" || type === "custom" || apiKey.trim() !== "") &&
    !submitting;

  const handleSubmit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const payload = {
        name: name.trim(),
        provider_type: type,
        base_url: baseUrl.trim(),
        model_name: modelName.trim(),
        parameters: apiKey.trim() ? { api_key: apiKey.trim() } : {},
        is_default: isDefault,
      };
      const saved = initial
        ? await updateModel(initial.id, payload)
        : await createModel(payload);
      onSaved(saved);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSubmitting(false);
    }
  };

  const needsApiKey = type !== "ollama" && type !== "custom";

  return (
    <Dialog open={open} onClose={onClose} width={520}>
      <header className="agent-template__head">
        <h2>{initial ? "编辑模型 Provider" : "添加模型 Provider"}</h2>
        <p>配置一个 LLM 服务端，保存后即刻生效。</p>
      </header>

      <label className="field">
        <span className="field__label">Provider 类型</span>
        <Select
          options={TYPE_OPTIONS}
          value={type}
          onChange={(e) => handleTypeChange(e.target.value as ProviderType)}
        />
      </label>

      <label className="field">
        <span className="field__label">名称</span>
        <Input
          placeholder="例如: 我的智谱账号"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </label>

      <label className="field">
        <span className="field__label">Base URL</span>
        <Input
          placeholder="https://open.bigmodel.cn/api/paas/v4"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
        />
      </label>

      <label className="field">
        <span className="field__label">模型名</span>
        <Input
          placeholder="glm-4.6"
          value={modelName}
          onChange={(e) => setModelName(e.target.value)}
        />
      </label>

      {needsApiKey && (
        <label className="field">
          <span className="field__label">API Key</span>
          <Input
            type="password"
            placeholder="sk-..."
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
      )}

      <label className="field" style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <input
          type="checkbox"
          checked={isDefault}
          onChange={(e) => setIsDefault(e.target.checked)}
        />
        <span>设为默认 Provider</span>
      </label>

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
          {submitting ? "保存中…" : "保存"}
        </Button>
      </footer>
    </Dialog>
  );
}
