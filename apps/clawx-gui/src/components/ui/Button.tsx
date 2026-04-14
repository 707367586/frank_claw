import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "default" | "secondary" | "destructive" | "outline" | "ghost";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
}

const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  { variant = "default", size = "md", leftIcon, rightIcon, className = "", children, ...rest },
  ref,
) {
  const cls = `ui-btn ui-btn--${variant} ui-btn--${size} ${className}`.trim();
  return (
    <button ref={ref} className={cls} {...rest}>
      {leftIcon && <span className="ui-btn__icon">{leftIcon}</span>}
      {children && <span className="ui-btn__label">{children}</span>}
      {rightIcon && <span className="ui-btn__icon">{rightIcon}</span>}
    </button>
  );
});

export default Button;
