import { RefreshCw, Plus } from "lucide-react";
import Button from "../components/ui/Button";
import ConnectorCard from "../components/ConnectorCard";
import AvailableChannelChip from "../components/AvailableChannelChip";

export default function ConnectorsPage() {
  return (
    <div className="connectors-page">
      <header className="connectors-page__head">
        <div className="connectors-page__title"><RefreshCw size={16} /><h1>渠道管理</h1></div>
      </header>

      <div className="connectors-page__ctx">
        <span className="dot dot--success" />
        <span className="ctx-text"><strong>编程助手</strong> · 运行中</span>
        <span className="ctx-desc">管理此 Agent 的消息渠道连接，不同 Agent 支持不同的渠道类型</span>
      </div>

      <Button leftIcon={<Plus size={14} />} size="md" variant="default">为此 Agent 添加渠道</Button>

      <section>
        <h2 className="connectors-page__group">已连接渠道</h2>
        <div className="connectors-page__connected">
          <ConnectorCard
            emoji="💬"
            name="飞书 - 产品团队群"
            typeLine="飞书群聊 · 自动回复模式"
            status="connected"
            metricLine="今日消息: 12 条 · 最近活跃: 10 分钟前"
          />
          <ConnectorCard
            emoji="💼"
            name="Slack - #engineering"
            typeLine="Slack 频道 · Webhook 模式"
            status="disconnected"
            metricLine=""
            errorLine="连接异常: Token 已过期，请重新授权"
          />
        </div>
      </section>

      <section>
        <h2 className="connectors-page__group">此 Agent 可用渠道</h2>
        <div className="connectors-page__available">
          <AvailableChannelChip name="飞书" available />
          <AvailableChannelChip name="Telegram" available />
          <AvailableChannelChip name="Slack" available />
          <AvailableChannelChip name="Discord" available={false} />
          <AvailableChannelChip name="微信" available={false} />
        </div>
      </section>
    </div>
  );
}
