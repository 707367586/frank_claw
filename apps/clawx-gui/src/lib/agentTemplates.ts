export interface AgentTemplate {
  id: string;
  name: string;
  icon: string;
  role: string;
  description: string;
  systemPrompt: string;
  skills: string[];
}

export const AGENT_TEMPLATES: AgentTemplate[] = [
  {
    id: "developer",
    name: "编程助手",
    icon: "💻",
    role: "Developer",
    description: "专注于代码编写、调试和架构设计",
    systemPrompt: "You are a professional developer assistant...",
    skills: ["Web Search", "Code Gen", "PDF Reader"],
  },
  {
    id: "researcher",
    name: "研究助手",
    icon: "🔍",
    role: "Researcher",
    description: "Web 搜索与数据分析、报告撰写",
    systemPrompt: "You are a research assistant...",
    skills: ["Web Search", "PDF Reader", "YouTube"],
  },
  {
    id: "writer",
    name: "写作助手",
    icon: "✍️",
    role: "Writer",
    description: "内容创作、编辑与润色",
    systemPrompt: "You are a writing assistant...",
    skills: ["Image Gen", "Translator"],
  },
  {
    id: "analyst",
    name: "数据分析",
    icon: "📊",
    role: "Analyst",
    description: "数据处理、可视化与报表生成",
    systemPrompt: "You are a data analyst...",
    skills: ["Code Gen", "PDF Reader"],
  },
  {
    id: "assistant",
    name: "智能助手",
    icon: "🤖",
    role: "Assistant",
    description: "智能管家，工作生活全方位助手",
    systemPrompt: "You are a smart assistant...",
    skills: ["Web Search", "Translator"],
  },
  {
    id: "automation",
    name: "自动化助手",
    icon: "⚡",
    role: "Automation",
    description: "自动化任务执行与工作流编排",
    systemPrompt: "You are an automation assistant...",
    skills: ["Web Search", "Calendar", "Code Gen"],
  },
];
