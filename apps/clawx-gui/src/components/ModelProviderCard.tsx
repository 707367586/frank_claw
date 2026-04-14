import Badge from "./ui/Badge";
import Button from "./ui/Button";

interface Props {
  emoji: string;
  name: string;
  available: boolean;
  summary: string;
  apiKey?: string;
}

export default function ModelProviderCard({ emoji, name, available, summary, apiKey }: Props) {
  return (
    <div className="mp-card">
      <div className="mp-card__head">
        <span className="mp-card__emoji">{emoji}</span>
        <div className="mp-card__name">{name}</div>
        <Badge tone={available ? "success" : "neutral"}>{available ? "可用" : "不可用"}</Badge>
      </div>
      <div className="mp-card__summary">{summary}</div>
      {apiKey && <div className="mp-card__key">API Key: <code>{apiKey}</code></div>}
      <div className="mp-card__actions">
        {available ? <>
          <Button size="sm" variant="outline">测试连接</Button>
          <Button size="sm" variant="ghost">编辑</Button>
        </> : <Button size="sm" variant="default">配置</Button>}
      </div>
    </div>
  );
}
