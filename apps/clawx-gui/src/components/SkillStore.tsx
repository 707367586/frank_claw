import { useState, useMemo } from "react";

interface Skill {
  id: string;
  name: string;
  description: string;
  status: "installed" | "review" | "available";
}

interface SkillCategory {
  name: string;
  skills: Skill[];
}

const MOCK_DATA: SkillCategory[] = [
  {
    name: "推荐",
    skills: [
      { id: "web-search", name: "Web Search", description: "互联网搜索和信息获取", status: "installed" },
      { id: "image-gen", name: "Image Gen", description: "图片生成 AI 绘图", status: "installed" },
      { id: "pdf-reader-rec", name: "PDF Reader", description: "解析 PDF 内容并提取信息", status: "available" },
    ],
  },
  {
    name: "信息获取",
    skills: [
      { id: "web-search-2", name: "Web Search", description: "互联网搜索和信息获取", status: "installed" },
      { id: "pdf-reader", name: "PDF Reader", description: "解析 PDF 内容并提取信息", status: "available" },
      { id: "youtube-summary", name: "YouTube Summary", description: "YouTube 视频内容总结", status: "available" },
    ],
  },
];

function getIconColor(name: string): string {
  const colors = [
    "#7C5CFC", "#3B82F6", "#22C55E", "#F59E0B",
    "#EF4444", "#EC4899", "#8B5CF6", "#06B6D4",
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

export default function SkillStore() {
  const [search, setSearch] = useState("");
  const [localStatuses, setLocalStatuses] = useState<Record<string, Skill["status"]>>({});

  const filteredCategories = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return MOCK_DATA;
    return MOCK_DATA
      .map((cat) => ({
        ...cat,
        skills: cat.skills.filter((s) => s.name.toLowerCase().includes(q)),
      }))
      .filter((cat) => cat.skills.length > 0);
  }, [search]);

  const getStatus = (skill: Skill): Skill["status"] => {
    return localStatuses[skill.id] ?? skill.status;
  };

  const toggleInstall = (skill: Skill) => {
    const current = getStatus(skill);
    setLocalStatuses((prev) => ({
      ...prev,
      [skill.id]: current === "installed" ? "available" : "installed",
    }));
  };

  return (
    <div className="skill-store">
      <input
        type="text"
        className="form-input skill-store-search"
        placeholder="搜索 Skill..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />

      {filteredCategories.length === 0 && (
        <div className="empty-state">
          <p>没有匹配的 Skill</p>
        </div>
      )}

      {filteredCategories.map((category) => (
        <div key={category.name} className="skill-category">
          <h3 className="skill-category-header">{category.name}</h3>
          <div className="skill-category-list">
            {category.skills.map((skill) => {
              const status = getStatus(skill);
              const iconColor = getIconColor(skill.name);
              const initial = skill.name.charAt(0).toUpperCase();

              return (
                <div key={skill.id} className="skill-item">
                  <div className="skill-item-icon" style={{ background: iconColor }}>
                    {initial}
                  </div>
                  <div className="skill-item-info">
                    <span className="skill-item-name">{skill.name}</span>
                    <span className="skill-item-desc">{skill.description}</span>
                  </div>
                  <div className="skill-item-action">
                    {status === "installed" ? (
                      <span
                        className="skill-status skill-status-installed"
                        onClick={() => toggleInstall(skill)}
                      >
                        已安装
                      </span>
                    ) : status === "review" ? (
                      <span className="skill-status skill-status-review">审核</span>
                    ) : (
                      <button
                        className="btn-primary skill-install-btn"
                        onClick={() => toggleInstall(skill)}
                      >
                        安装
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
