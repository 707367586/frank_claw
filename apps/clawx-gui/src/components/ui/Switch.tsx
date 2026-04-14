import { forwardRef, type InputHTMLAttributes } from "react";
type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "type">;
const Switch = forwardRef<HTMLInputElement, Props>(function Switch({ className = "", ...rest }, ref) {
  return (
    <label className={`ui-switch ${className}`.trim()}>
      <input ref={ref} type="checkbox" {...rest} />
      <span className="ui-switch__track"><span className="ui-switch__thumb" /></span>
    </label>
  );
});
export default Switch;
