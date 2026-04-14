import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "default" | "secondary" | "destructive" | "outline" | "ghost";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon: ReactNode;
  "aria-label": string;
}

const IconButton = forwardRef<HTMLButtonElement, Props>(function IconButton(
  { variant = "ghost", size = "md", icon, className = "", ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      className={`ui-icon-btn ui-icon-btn--${variant} ui-icon-btn--${size} ${className}`.trim()}
      {...rest}
    >
      {icon}
    </button>
  );
});

export default IconButton;
