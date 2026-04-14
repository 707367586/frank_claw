import { LayoutTemplate, Plus, Settings, Code, Search, Pen, ChevronRight } from "lucide-react";

interface EmptyStateProps {
  onCreateFromTemplate: () => void;
  onCreateCustom: () => void;
}

interface TemplateCard {
  icon: typeof Code;
  iconColor: string;
  name: string;
  description: string;
}

const TEMPLATES: TemplateCard[] = [
  { icon: Code, iconColor: "#7C5CFC", name: "Developer", description: "Code review, debugging, and architecture design" },
  { icon: Search, iconColor: "#3B82F6", name: "Researcher", description: "Web search, data analysis, report writing" },
  { icon: Pen, iconColor: "#F59E0B", name: "Writer", description: "Content creation, editing, copywriting" },
];

export default function EmptyState({
  onCreateFromTemplate,
  onCreateCustom,
}: EmptyStateProps) {
  return (
    <div className="empty-welcome">
      <div className="empty-welcome-center">
        <h1 className="empty-welcome-title">欢迎使用 ZettClaw</h1>
        <p className="empty-welcome-subtitle">
          你还没有创建任何 Agent，开始吧
        </p>

        <div className="empty-welcome-actions">
          <button className="btn-primary-pill" onClick={onCreateFromTemplate}>
            <LayoutTemplate size={16} />
            从模板创建
          </button>
          <button className="btn-secondary-pill" onClick={onCreateCustom}>
            <Plus size={16} />
            从零开始
          </button>
          <button className="btn-outline-pill" onClick={onCreateCustom}>
            <Settings size={16} />
            先配置模型
          </button>
        </div>

        <div className="empty-welcome-templates">
          <h3 className="empty-welcome-templates-title">Recommended Templates</h3>
          {TEMPLATES.map((tpl) => (
            <button
              key={tpl.name}
              className="empty-welcome-template-card"
              onClick={onCreateFromTemplate}
            >
              <div className="template-card-icon" style={{ background: tpl.iconColor }}>
                <tpl.icon size={20} color="#fff" />
              </div>
              <div className="template-card-info">
                <span className="template-card-name">{tpl.name}</span>
                <span className="template-card-desc">{tpl.description}</span>
              </div>
              <ChevronRight size={16} className="template-card-arrow" />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
