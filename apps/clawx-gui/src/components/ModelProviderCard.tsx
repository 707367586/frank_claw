import Badge from "./ui/Badge";
import Button from "./ui/Button";
import type { ModelProvider } from "../lib/types";

const EMOJI_BY_TYPE: Record<ModelProvider["provider_type"], string> = {
  zhipu: "🤖",
  anthropic: "☁️",
  openai: "🔌",
  ollama: "🏠",
  custom: "⚙️",
};

function maskKey(raw: string): string {
  if (raw.length <= 8) return "••••••";
  return `${raw.slice(0, 4)}••••${raw.slice(-4)}`;
}

interface Props {
  provider: ModelProvider;
  onDelete?: (id: string) => void;
  busy?: boolean;
}

export default function ModelProviderCard({ provider, onDelete, busy }: Props) {
  const params = (provider.parameters ?? {}) as { api_key?: string };
  const apiKey = typeof params.api_key === "string" ? params.api_key : "";
  const available = provider.provider_type === "ollama"
    || provider.provider_type === "custom"
    || apiKey.length > 0;

  return (
    <div className="mp-card">
      <div className="mp-card__head">
        <span className="mp-card__emoji">{EMOJI_BY_TYPE[provider.provider_type]}</span>
        <div className="mp-card__name">{provider.name}</div>
        {provider.is_default && <Badge tone="success">默认</Badge>}
        <Badge tone={available ? "success" : "neutral"}>
          {available ? "可用" : "未配置"}
        </Badge>
      </div>
      <div className="mp-card__summary">
        类型: {provider.provider_type} · 模型: {provider.model_name}
      </div>
      {provider.base_url && (
        <div className="mp-card__summary">Base URL: {provider.base_url}</div>
      )}
      {apiKey && (
        <div className="mp-card__key">
          API Key: <code>{maskKey(apiKey)}</code>
        </div>
      )}
      <div className="mp-card__actions">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => onDelete?.(provider.id)}
          disabled={busy}
        >
          删除
        </Button>
      </div>
    </div>
  );
}
