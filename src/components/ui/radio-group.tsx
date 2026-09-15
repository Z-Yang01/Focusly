import * as React from "react";

import { cn } from "@/lib/utils";

/**
 * 原生实现（项目未安装 @radix-ui/react-radio-group，不引入新依赖）：
 * - RadioGroup：role="radiogroup" 容器，管理 name / value / onValueChange
 * - RadioGroupItem：input[type=radio]（sr-only）+ 自绘圆点样式
 * API 与 shadcn 对齐：value / defaultValue / onValueChange / orientation / disabled
 */

interface RadioGroupContextValue {
  name: string;
  value?: string;
  disabled?: boolean;
  onValueChange?: (value: string) => void;
}

const RadioGroupContext = React.createContext<RadioGroupContextValue>({ name: "" });

export interface RadioGroupProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  orientation?: "horizontal" | "vertical";
  name?: string;
  disabled?: boolean;
}

const RadioGroup = React.forwardRef<HTMLDivElement, RadioGroupProps>(
  (
    {
      className,
      value,
      defaultValue,
      onValueChange,
      orientation = "vertical",
      name,
      disabled,
      children,
      ...props
    },
    ref,
  ) => {
    const autoName = React.useId();
    const groupName = name ?? `radio-group-${autoName}`;
    const [internalValue, setInternalValue] = React.useState<string | undefined>(defaultValue);
    const isControlled = value !== undefined;
    const currentValue = isControlled ? value : internalValue;

    const handleChange = React.useCallback(
      (next: string) => {
        if (!isControlled) setInternalValue(next);
        onValueChange?.(next);
      },
      [isControlled, onValueChange],
    );

    return (
      <RadioGroupContext.Provider
        value={{ name: groupName, value: currentValue, disabled, onValueChange: handleChange }}
      >
        <div
          ref={ref}
          role="radiogroup"
          aria-disabled={disabled || undefined}
          className={cn("grid gap-2", orientation === "horizontal" && "grid-flow-col gap-4", className)}
          {...props}
        >
          {children}
        </div>
      </RadioGroupContext.Provider>
    );
  },
);
RadioGroup.displayName = "RadioGroup";

export interface RadioGroupItemProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "type" | "checked"> {
  value: string;
}

const RadioGroupItem = React.forwardRef<HTMLInputElement, RadioGroupItemProps>(
  ({ className, value, disabled, id, ...props }, ref) => {
    const group = React.useContext(RadioGroupContext);
    const autoId = React.useId();
    const inputId = id ?? `radio-item-${autoId}`;
    const inputRef = React.useRef<HTMLInputElement | null>(null);

    React.useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

    const isDisabled = disabled ?? group.disabled;

    return (
      <label
        htmlFor={inputId}
        className={cn(
          "inline-flex cursor-pointer select-none items-center justify-center",
          isDisabled && "cursor-not-allowed opacity-50",
        )}
      >
        <input
          ref={inputRef}
          id={inputId}
          type="radio"
          name={group.name || undefined}
          value={value}
          checked={group.value === value}
          disabled={isDisabled}
          onChange={() => group.onValueChange?.(value)}
          className="peer sr-only"
          {...props}
        />
        <span
          aria-hidden="true"
          className={cn(
            "block h-4 w-4 rounded-full border border-primary bg-background shadow-sm transition-colors",
            "peer-checked:bg-primary peer-checked:shadow-[inset_0_0_0_3px_var(--background)]",
            "peer-focus-visible:outline-none peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2",
            className,
          )}
        />
      </label>
    );
  },
);
RadioGroupItem.displayName = "RadioGroupItem";

export { RadioGroup, RadioGroupItem };
