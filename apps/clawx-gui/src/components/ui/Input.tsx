import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";

interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  size?: "sm" | "md";
}

const Input = forwardRef<HTMLInputElement, Props>(function Input(
  { leftIcon, rightIcon, size = "md", className = "", ...rest },
  ref,
) {
  return (
    <div className={`ui-input ui-input--${size} ${className}`.trim()}>
      {leftIcon && <span className="ui-input__icon">{leftIcon}</span>}
      <input ref={ref} className="ui-input__control" {...rest} />
      {rightIcon && <span className="ui-input__icon">{rightIcon}</span>}
    </div>
  );
});

export default Input;
