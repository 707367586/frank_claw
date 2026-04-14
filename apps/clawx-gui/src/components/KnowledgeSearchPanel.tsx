import { Search, UploadCloud } from "lucide-react";
import Input from "./ui/Input";

const RESULTS = [
  { id: "1", title: "API 接口设计规范", source: "产品文档库 · api-design-guide.md · 本地",   excerpt: "API 设计应遵循 RESTful 原则，所有端点返回 JSON 格式，支持版本控制。认证使用 JWT Bearer Token，有效期 24 小时。" },
  { id: "2", title: "认证系统安全规范",  source: "技术规范 · auth-spec-v2.md",             excerpt: "密码必须使用 bcrypt 加密（salt rounds ≥ 10），禁用 MD5/SHA1 作为哈希函数。" },
  { id: "3", title: "产品路线图讨论",   source: "产品讨论 #42 · chat-2024-03-15.md",       excerpt: "Q2 重点方向：智能化 Agent 能力增强、多渠道接入、企业级知识库集成。" },
  { id: "4", title: "数据流重构方案",   source: "技术评审 #18 · review-session.md",        excerpt: "将旧有 REST 接口统一迁移到 GraphQL 网关，关键路径保留向后兼容。" },
];

export default function KnowledgeSearchPanel() {
  return (
    <div className="kn-search">
      <Input leftIcon={<Search size={14} />} placeholder="在所有知识中搜索..." size="md" />
      <ul className="kn-search__results">
        {RESULTS.map((r) => (
          <li key={r.id} className="kn-search__item">
            <div className="kn-search__item-head">
              <span className="kn-search__title">{r.title}</span>
              <a className="kn-search__view">查看原文 →</a>
            </div>
            <p className="kn-search__excerpt">{r.excerpt}</p>
            <span className="kn-search__source">{r.source}</span>
          </li>
        ))}
      </ul>
      <div className="kn-search__drop">
        <UploadCloud size={20} />
        <p>拖放文件到此处添加到知识库</p>
        <span>支持 PDF、Markdown、代码文件等</span>
      </div>
    </div>
  );
}
