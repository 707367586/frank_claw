import { useSearchParams } from "react-router-dom";
import { Settings, MoreVertical, Zap } from "lucide-react";
import Avatar from "../components/ui/Avatar";
import Button from "../components/ui/Button";
import IconButton from "../components/ui/IconButton";
import { useAgents } from "../lib/store";

const CAPS = ["Python", "JavaScript", "代码生成", "调试", "代码审查"];

export default function ContactsPage() {
  const [params] = useSearchParams();
  const selectedId = params.get("agent");
  const { agents } = useAgents();
  const agent = agents.find((a) => a.id === selectedId) ?? agents[0];

  if (!agent) return <div className="contacts-page__empty">请选择一个 Agent</div>;

  return (
    <div className="contacts-page">
      <header className="contacts-page__head">
        <h1>{agent.name}</h1>
        <div className="contacts-page__actions">
          <IconButton icon={<Settings size={14} />} aria-label="配置" variant="ghost" size="sm" />
          <IconButton icon={<MoreVertical size={14} />} aria-label="更多" variant="ghost" size="sm" />
        </div>
      </header>

      <section className="contacts-page__hero">
        <Avatar size={72} rounded="md" bg="var(--primary)"><span>{"</>"}</span></Avatar>
        <h2>{agent.name}</h2>
        <span className="contacts-page__running">● 运行中</span>
        <p className="contacts-page__desc">一个擅长编程的助手，专注多种编程语言的解决方案，提供代码生成、代码审查、性能优化等能力。</p>
        <div className="contacts-page__cta">
          <Button variant="default">开始对话</Button>
          <Button variant="outline">配置</Button>
        </div>
      </section>

      <section>
        <h3 className="contacts-page__section">能力标签</h3>
        <div className="contacts-page__caps">
          {CAPS.map((c) => <span key={c} className="cap-chip"><Zap size={12} />{c}</span>)}
        </div>
      </section>

      <section>
        <h3 className="contacts-page__section">基本信息</h3>
        <dl className="contacts-page__info">
          <dt>创建者</dt><dd>ZettClaw Team</dd>
          <dt>版本</dt><dd>v2.1.0</dd>
          <dt>模型</dt><dd>GPT-4o</dd>
          <dt>最近使用</dt><dd>2 分钟前</dd>
          <dt>对话次数</dt><dd>1,284</dd>
        </dl>
      </section>

      <section>
        <h3 className="contacts-page__section">常用提示词</h3>
        <ul className="contacts-page__prompts">
          <li>帮我写一个 REST API 接口</li>
          <li>优化这段代码的性能</li>
          <li>解释这段代码的工作原理</li>
        </ul>
      </section>
    </div>
  );
}
