import { useState } from "react";

export interface PermissionRequest {
  id: string;
  type: "fs_write" | "fs_delete" | "net_http" | "exec_shell";
  target: string;
  risk: "high" | "medium" | "low";
  description: string;
}

export interface PermissionModalProps {
  agentName: string;
  requests: PermissionRequest[];
  onApprove: (approved: string[], denied: string[]) => void;
  onDenyAll: () => void;
  onClose: () => void;
}

const TYPE_ICONS: Record<PermissionRequest["type"], string> = {
  fs_write: "\u270F",
  fs_delete: "\uD83D\uDDD1",
  net_http: "\uD83C\uDF10",
  exec_shell: ">_",
};

const TYPE_LABELS: Record<PermissionRequest["type"], string> = {
  fs_write: "写入文件",
  fs_delete: "删除文件",
  net_http: "访问网络",
  exec_shell: "执行命令",
};

const RISK_LABELS: Record<PermissionRequest["risk"], string> = {
  high: "高危",
  medium: "中",
  low: "低",
};

type Decision = "allow" | "deny" | "pending";

export default function PermissionModal({
  agentName,
  requests,
  onApprove,
  onDenyAll,
  onClose,
}: PermissionModalProps) {
  const [decisions, setDecisions] = useState<Record<string, Decision>>(() => {
    const init: Record<string, Decision> = {};
    for (const r of requests) {
      init[r.id] = "pending";
    }
    return init;
  });

  const [expandedId, setExpandedId] = useState<string | null>(null);

  const setDecision = (id: string, d: Decision) => {
    setDecisions((prev) => ({ ...prev, [id]: d }));
  };

  const handleConfirm = () => {
    const approved: string[] = [];
    const denied: string[] = [];
    for (const r of requests) {
      const d = decisions[r.id];
      if (d === "allow") approved.push(r.id);
      else denied.push(r.id);
    }
    onApprove(approved, denied);
  };

  return (
    <div className="permission-modal-overlay" onClick={onClose}>
      <div className="permission-modal" onClick={(e) => e.stopPropagation()}>
        <div className="permission-modal-header">
          <h2>Agent 请求审批</h2>
          <p className="permission-modal-subtitle">
            {agentName} 需执行 {requests.length} 个动作
          </p>
        </div>

        <div className="permission-modal-body">
          {requests.map((req, idx) => {
            const riskClass = `risk-${req.risk}`;
            const isExpanded = expandedId === req.id;

            return (
              <div key={req.id} className="permission-request-item">
                <div className="permission-request-main">
                  <span className="permission-request-num">{idx + 1}</span>
                  <span className="permission-request-icon">
                    {TYPE_ICONS[req.type]}
                  </span>
                  <div className="permission-request-info">
                    <span className="permission-request-desc">
                      {TYPE_LABELS[req.type]}
                    </span>
                    <span className="permission-request-target">{req.target}</span>
                  </div>
                  <span className={`risk-badge ${riskClass}`}>
                    {RISK_LABELS[req.risk]}
                  </span>
                  <div className="permission-request-actions">
                    <button
                      className={`permission-btn-allow ${decisions[req.id] === "allow" ? "active" : ""}`}
                      onClick={() => setDecision(req.id, "allow")}
                    >
                      Allow
                    </button>
                    <button
                      className={`permission-btn-deny ${decisions[req.id] === "deny" ? "active" : ""}`}
                      onClick={() => setDecision(req.id, "deny")}
                    >
                      Deny
                    </button>
                  </div>
                  <button
                    className="permission-expand-btn"
                    onClick={() => setExpandedId(isExpanded ? null : req.id)}
                  >
                    {isExpanded ? "\u25B2" : "\u25BC"}
                  </button>
                </div>
                {isExpanded && (
                  <div className="permission-request-detail">
                    <p>{req.description}</p>
                  </div>
                )}
              </div>
            );
          })}
        </div>

        <div className="permission-modal-footer">
          <button className="btn-destructive" onClick={onDenyAll}>
            全部拒绝
          </button>
          <button className="btn-primary" onClick={handleConfirm}>
            按选择结果继续
          </button>
        </div>
      </div>
    </div>
  );
}
