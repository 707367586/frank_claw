import { forwardRef, type SelectHTMLAttributes } from "react";
import { ChevronDown } from "lucide-react";

interface Option { value: string; label: string }
interface Props extends SelectHTMLAttributes<HTMLSelectElement> { options: Option[] }

const Select = forwardRef<HTMLSelectElement, Props>(function Select(
  { options, className = "", ...rest },
  ref,
) {
  return (
    <div className={`ui-select ${className}`.trim()}>
      <select ref={ref} className="ui-select__control" {...rest}>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
      <ChevronDown size={16} className="ui-select__chevron" />
    </div>
  );
});

export default Select;
