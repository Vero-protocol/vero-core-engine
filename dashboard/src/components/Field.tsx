import React, { useId } from 'react';

type FieldBaseProps = {
  label: string;
  error?: string | undefined;
  hint?: string;
  optional?: boolean;
  className?: string;
  disabled?: boolean;
};

type InputFieldProps = FieldBaseProps & {
  type?: 'text' | 'url' | 'number';
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  placeholder?: string;
  min?: number;
  max?: number;
  step?: number;
  inputMode?: 'text' | 'numeric' | 'decimal' | 'url';
  autoComplete?: string;
};

const baseInputClasses =
  'w-full rounded-md border px-3 py-2 text-sm transition-colors ' +
  'bg-white dark:bg-gray-900 ' +
  'text-gray-900 dark:text-gray-100 ' +
  'border-gray-300 dark:border-gray-700 ' +
  'focus:outline-none focus:ring-2 focus:ring-blue-500 ' +
  'disabled:opacity-60 disabled:cursor-not-allowed';

const errorInputClasses =
  'border-red-500 dark:border-red-500 focus:ring-red-500';

/**
 * `Field` — accessible form input with label, optional hint, and inline
 * error display. Errors are wired with `role="alert"` and `aria-invalid`
 * so screen-readers announce validation failures immediately.
 */
export const Field: React.FC<InputFieldProps> = ({
  label,
  error,
  hint,
  optional,
  type = 'text',
  value,
  onChange,
  onBlur,
  placeholder,
  min,
  max,
  step,
  className,
  disabled,
  inputMode,
  autoComplete,
}) => {
  const reactId = useId();
  const inputId = `field-${reactId}`;
  const hintId = `${inputId}-hint`;
  const errorId = `${inputId}-error`;
  const describedBy =
    [error ? errorId : null, hint ? hintId : null]
      .filter(Boolean)
      .join(' ') || undefined;

  return (
    <div className={className}>
      <label
        htmlFor={inputId}
        data-testid={`${inputId}-label`}
        className="block text-sm font-medium mb-1 text-gray-900 dark:text-gray-100"
      >
        {label}
        {optional && (
          <span className="ml-2 text-xs font-normal opacity-60">(optional)</span>
        )}
      </label>
      <input
        id={inputId}
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        placeholder={placeholder}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        inputMode={inputMode}
        autoComplete={autoComplete}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        data-testid={inputId}
        className={`${baseInputClasses} ${error ? errorInputClasses : ''}`.trim()}
      />
      {hint && !error && (
        <p
          id={hintId}
          data-testid={`${inputId}-hint`}
          className="mt-1 text-xs opacity-60 text-gray-700 dark:text-gray-300"
        >
          {hint}
        </p>
      )}
      {error && (
        <p
          id={errorId}
          role="alert"
          data-testid={`${inputId}-error`}
          className="mt-1 text-xs font-medium text-red-600 dark:text-red-400"
        >
          {error}
        </p>
      )}
    </div>
  );
};
