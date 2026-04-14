import { Search, FileText, FileCode, Image as ImageIcon } from "lucide-react";
import Input from "./ui/Input";
import Button from "./ui/Button";

interface Artifact { id: string; kind: "md" | "py" | "img"; title: string; date: string; excerpt: string; lang: string }

const MOCK: Artifact[] = [
  { id: "1", kind: "md",  title: "竞品总结 - 第 3 版", date: "Mar 15, 2026", lang: "md", excerpt: "本次总结了当下 3 大主流助手的差异，识别了 10 个 bug，并对差异部分进行综合分析 35%，干货对营业报告有参考价值和指导意义。" },
  { id: "2", kind: "py",  title: "数据处理脚本", date: "Mar 14, 2026", lang: "Python", excerpt: "自动化 Python 脚本，用于自动化执行数据处理/数据批量存储的 PostgreSQL 函数。完成 500 条数据…" },
  { id: "3", kind: "img", title: "产品功能架构图", date: "Mar 12, 2026", lang: "SVG", excerpt: "架构图内容产品功能模块，功能模块详情/内部结构，设计模型和 API 调用关系的真实图。" },
];

const ICON: Record<Artifact["kind"], typeof FileText> = { md: FileText, py: FileCode, img: ImageIcon };

export default function ArtifactsPanel({ conversationId: _conversationId }: { conversationId?: string }) {
  const items = MOCK;
  return (
    <div className="artifacts">
      <header className="artifacts__head">
        <Input leftIcon={<Search size={14} />} placeholder="搜索产物..." size="sm" />
        <div className="artifacts__actions">
          <Button variant="outline" size="sm">测试</Button>
          <Button variant="default" size="sm">新增</Button>
        </div>
      </header>
      <ul className="artifacts__list">
        {items.map((a) => {
          const Icon = ICON[a.kind];
          return (
            <li key={a.id} className="artifact-card">
              <div className={`artifact-card__icon artifact-card__icon--${a.kind}`}><Icon size={16} /></div>
              <div className="artifact-card__body">
                <div className="artifact-card__top">
                  <span className="artifact-card__title">{a.title}</span>
                  <span className="artifact-card__date">{a.date}</span>
                </div>
                <p className="artifact-card__excerpt">{a.excerpt}</p>
                <span className="artifact-card__lang">{a.lang}</span>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
