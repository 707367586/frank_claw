import { Check } from "lucide-react";

export default function AvailableChannelChip({ name, available }: { name: string; available: boolean }) {
  return (
    <span className={`ch-chip ${available ? "" : "is-disabled"}`}>
      {available && <Check size={12} />}
      {name}{!available && " (不支持)"}
    </span>
  );
}
