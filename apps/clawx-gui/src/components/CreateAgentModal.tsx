import { useMemo, useState } from "react";
import {
  Code2, PenTool, BarChart3, Bot, MessageSquare, FileText, Lightbulb,
  Sparkles, Database, Globe, Wrench, Search,
} from "lucide-react";
import type { ComponentType } from "react";
import Dialog from "./ui/Dialog";
import Input from "./ui/Input";
import { useClaw } from "../lib/store";

const COLORS = [
  "#5749F4", "#3B82F6", "#EC4899", "#F59E0B",
  "#22C55E", "#EF4444", "#14B8A6", "#8B5CF6",
  "#F97316", "#06B6D4", "#84CC16", "#6366F1",
];

const ICON_OPTIONS: { name: string; Icon: ComponentType<{ size?: number }> }[] = [
  { name: "Code2", Icon: Code2 }, { name: "Search", Icon: Search },
  { name: "PenTool", Icon: PenTool }, { name: "BarChart3", Icon: BarChart3 },
  { name: "Bot", Icon: Bot }, { name: "MessageSquare", Icon: MessageSquare },
  { name: "FileText", Icon: FileText }, { name: "Lightbulb", Icon: Lightbulb },
  { name: "Sparkles", Icon: Sparkles }, { name: "Database", Icon: Database },
  { name: "Globe", Icon: Globe }, { name: "Wrench", Icon: Wrench },
];

const MODEL_PRESETS = [
  "Sonnet 4.6", "Opus 4.6", "Haiku 4.5",
  "GLM-4.5-Air", "GLM-4.5-Plus", "GPT-4o", "DeepSeek-V3",
];

interface Props { open: boolean; onClose: () => void }

export default function CreateAgentModal({ open, onClose }: Props) {
  const claw = useClaw();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [color, setColor] = useState(COLORS[0]);
  const [icon, setIcon] = useState(ICON_OPTIONS[0].name);
  const [systemPrompt, setSystemPrompt] = useState("");
  const [modelMode, setModelMode] = useState<"global" | "custom">("global");
  const [modelPreset, setModelPreset] = useState<string>(MODEL_PRESETS[0]);
  const [modelCustom, setModelCustom] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [enabledToolsets, setEnabledToolsets] = useState<Set<string>>(
    () => new Set((claw.toolsets ?? []).map((t) => t.name)),
  );
  const [submitErr, setSubmitErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [showErrors, setShowErrors] = useState(false);

  const allToolsetNames = useMemo(() => (claw.toolsets ?? []).map((t) => t.name), [claw.toolsets]);

  const errors = {
    name: !name.trim() ? "请输入名称" : null,
    systemPrompt: !systemPrompt.trim() ? "请输入 System Prompt" : null,
  };

  function pickedModel(): string | null {
    if (modelMode === "global") return null;
    return modelPreset === "自定义..." ? modelCustom.trim() || null : modelPreset;
  }

  async function submit() {
    setShowErrors(true);
    if (errors.name || errors.systemPrompt) return;
    setSubmitErr(null);
    setBusy(true);
    try {
      await claw.createAgent({
        name: name.trim(),
        description: description.trim(),
        color,
        icon,
        system_prompt: systemPrompt,
        model: pickedModel(),
        enabled_toolsets:
          enabledToolsets.size === allToolsetNames.length
            ? allToolsetNames
            : allToolsetNames.filter((n) => enabledToolsets.has(n)),
      });
      onClose();
    } catch (e) {
      setSubmitErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose} title="新建 Agent" width={560}>
      <div className="create-agent">
        {submitErr && <div className="create-agent__error">{submitErr}</div>}

        <label className="create-agent__field">
          <span>名称</span>
          <Input
            size="md"
            value={name}
            onChange={(e) => setName(e.target.value)}
            aria-label="名称"
            placeholder="给它起个名字"
          />
          {showErrors && errors.name && <em className="create-agent__err">{errors.name}</em>}
        </label>

        <label className="create-agent__field">
          <span>描述</span>
          <Input
            size="md"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            aria-label="描述"
            placeholder="一句话副标题，可空"
          />
        </label>

        <div className="create-agent__field">
          <span>颜色</span>
          <div className="create-agent__swatches">
            {COLORS.map((c) => (
              <button
                key={c}
                type="button"
                aria-label={`颜色 ${c}`}
                aria-pressed={color === c}
                className={`create-agent__swatch ${color === c ? "is-active" : ""}`}
                style={{ background: c }}
                onClick={() => setColor(c)}
              />
            ))}
          </div>
        </div>

        <div className="create-agent__field">
          <span>图标</span>
          <div className="create-agent__icons">
            {ICON_OPTIONS.map(({ name: n, Icon }) => (
              <button
                key={n}
                type="button"
                aria-label={`图标 ${n}`}
                aria-pressed={icon === n}
                className={`create-agent__icon ${icon === n ? "is-active" : ""}`}
                onClick={() => setIcon(n)}
              >
                <Icon size={16} />
              </button>
            ))}
          </div>
        </div>

        <label className="create-agent__field">
          <span>System Prompt</span>
          <textarea
            className="ui-textarea"
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
            rows={8}
            aria-label="System Prompt"
            placeholder="描述这个 Agent 的角色、风格、约束……"
          />
          {showErrors && errors.systemPrompt && (
            <em className="create-agent__err">{errors.systemPrompt}</em>
          )}
        </label>

        <div className="create-agent__field">
          <span>模型</span>
          <div className="create-agent__model-row">
            <button
              type="button"
              className={`create-agent__pill ${modelMode === "global" ? "is-active" : ""}`}
              onClick={() => setModelMode("global")}
            >
              跟随全局
            </button>
            <button
              type="button"
              aria-label="自定义模型"
              className={`create-agent__pill ${modelMode === "custom" ? "is-active" : ""}`}
              onClick={() => setModelMode("custom")}
            >
              自定义
            </button>
          </div>
          {modelMode === "custom" && (
            <>
              <select
                className="ui-select__control create-agent__select"
                aria-label="选择模型"
                value={modelPreset}
                onChange={(e) => setModelPreset(e.target.value)}
              >
                {MODEL_PRESETS.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
                <option value="自定义...">自定义...</option>
              </select>
              {modelPreset === "自定义..." && (
                <Input
                  size="sm"
                  aria-label="自定义模型名"
                  placeholder="例如 anthropic/claude-sonnet-4-6"
                  value={modelCustom}
                  onChange={(e) => setModelCustom(e.target.value)}
                />
              )}
            </>
          )}
        </div>

        <div className="create-agent__field">
          <button
            type="button"
            className="create-agent__advanced"
            onClick={() => setAdvancedOpen((v) => !v)}
          >
            {advancedOpen ? "▾" : "▸"} 高级
          </button>
          {advancedOpen && (
            <div className="create-agent__toolsets">
              <div className="create-agent__toolset-actions">
                <button type="button" onClick={() => setEnabledToolsets(new Set(allToolsetNames))}>全选</button>
                <button type="button" onClick={() => setEnabledToolsets(new Set())}>全不选</button>
              </div>
              {(claw.toolsets ?? []).map((t) => (
                <label key={t.name} className="create-agent__toolset">
                  <input
                    type="checkbox"
                    checked={enabledToolsets.has(t.name)}
                    onChange={(e) => {
                      setEnabledToolsets((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) next.add(t.name);
                        else next.delete(t.name);
                        return next;
                      });
                    }}
                  />
                  <span>
                    <strong>{t.name}</strong>
                    {t.description && <small> — {t.description}</small>}
                  </span>
                </label>
              ))}
            </div>
          )}
        </div>

        <div className="create-agent__footer">
          <button type="button" onClick={onClose} disabled={busy}>取消</button>
          <button
            type="button"
            className="create-agent__submit"
            onClick={() => void submit()}
            disabled={busy}
          >
            {busy ? "创建中..." : "创建"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
