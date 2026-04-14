import Badge from "./ui/Badge";
import Button from "./ui/Button";
import { RefreshCw } from "lucide-react";

interface Props {
  emoji: string;
  name: string;
  typeLine: string;
  status: "connected" | "disconnected";
  metricLine: string;
  errorLine?: string;
}

export default function ConnectorCard({ emoji, name, typeLine, status, metricLine, errorLine }: Props) {
  return (
    <div className="conn-card">
      <div className="conn-card__head">
        <span className="conn-card__emoji">{emoji}</span>
        <div className="conn-card__title-col">
          <div className="conn-card__name">{name}</div>
          <div className="conn-card__type">{typeLine}</div>
        </div>
        <Badge tone={status === "connected" ? "success" : "error"}>
          {status === "connected" ? "已连接" : "已断开"}
        </Badge>
      </div>
      <div className="conn-card__meta">{metricLine}</div>
      {errorLine && (
        <div className="conn-card__error">
          <span>{errorLine}</span>
          <Button size="sm" variant="outline" leftIcon={<RefreshCw size={12} />}>重新连接</Button>
        </div>
      )}
    </div>
  );
}
