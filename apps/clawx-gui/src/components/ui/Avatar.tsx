import type { ReactNode } from "react";
interface Props {
  size?: number;
  rounded?: "md" | "full";
  bg?: string;
  children: ReactNode;
  className?: string;
}
export default function Avatar({ size = 32, rounded = "md", bg, children, className = "" }: Props) {
  return (
    <span
      className={`ui-avatar ui-avatar--${rounded} ${className}`.trim()}
      style={{ width: size, height: size, background: bg, fontSize: Math.round(size * 0.5) }}
    >
      {children}
    </span>
  );
}
