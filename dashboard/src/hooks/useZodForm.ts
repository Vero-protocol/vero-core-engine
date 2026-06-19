import { useCallback, useMemo, useState } from 'react';
import type { FormEvent } from 'react';
import type { z } from 'zod';

export type FieldErrors = Partial<Record<string, string>>;
export type VisibleFieldErrors = Partial<Record<string, string | undefined>>;
export type TouchedMap = Partial<Record<string, boolean>>;

/**
 * `useZodFormOptions` — strongly-typed call signature so callers get
 * a precise `z.input<T>` for their `initialValues`.
 */
export type UseZodFormOptions<T extends z.ZodTypeAny> = {
  schema: T;
  initialValues: z.input<T>;
};

/**
 * `UseZodFormReturn` — strongly-typed result. `setValue`/`handleBlur`
 * use `K extends keyof z.input<T>` so TS narrows field names at every
 * call site.
 */
export type UseZodFormReturn<T extends z.ZodTypeAny> = {
  values: z.input<T>;
  /** Raw errors derived from the current `values` (always computed). */
  errors: FieldErrors;
  /**
   * Errors that should actually be shown to the user — only surfaced for
   * fields that have been blurred, or for all fields once the form has
   * been submitted at least once. Avoids yelling at the user before
   * they've had a chance to type.
   */
  visibleErrors: VisibleFieldErrors;
  isValid: boolean;
  hasAttemptedSubmit: boolean;
  touched: TouchedMap;
  setValue: <K extends keyof z.input<T>>(name: K, value: z.input<T>[K]) => void;
  handleBlur: (name: keyof z.input<T>) => void;
  handleSubmit: (
    onValid: (parsed: z.output<T>) => void | Promise<void>,
  ) => (e?: FormEvent) => Promise<void>;
  reset: (next?: z.input<T>) => void;
};

/**
 * `useZodForm` — minimal Zod-driven form state.
 *
 * Behaviour:
 *   • `errors` is always derived from `safeParse(values)`, so a parent
 *     component never needs to call zod manually.
 *   • `visibleErrors` only exposes errors for fields the user has
 *     touched (blurred), or for *all* fields once submit has been
 *     attempted — the standard "don't intrude" UX for forms.
 *   • `handleSubmit(...)` blocks the callback when the parsed result
 *     doesn't validate — bad input is never propagated to onValid.
 *
 * The hook is intentionally dependency-free (no react-hook-form) so the
 * security-critical dashboard keeps a tight, auditable surface.
 *
 * Implementation note: zod 4's `z.input<T>` is a conditional that TS
 * cannot always resolve to a concrete `object` type when `T extends
 * z.ZodTypeAny`. Internally we store values as `Record<string, unknown>`
 * and cast at the public boundary, which keeps the call-site types
 * precise while avoiding TS2698 / TS2769 errors at runtime-safe casts.
 */
export function useZodForm<T extends z.ZodTypeAny>(
  options: UseZodFormOptions<T>,
): UseZodFormReturn<T> {
  type Input = z.input<T>;
  type Output = z.output<T>;
  type Key = keyof Input;

  // Boundary cast: schema is whatever the caller provided; values are
  // their input shape. The `useState` initialiser must be a real object
  // for TS' `useState` overloads to accept it.
  const initialValues = options.initialValues as Input;
  const schema = options.schema as z.ZodTypeAny;

  // Internal storage uses a permissive shape — callers see strict types
  // through the returned object.
  const [values, setValues] = useState<Record<string, unknown>>(
    initialValues as unknown as Record<string, unknown>,
  );
  const [touched, setTouched] = useState<TouchedMap>({});
  const [submitAttempted, setSubmitAttempted] = useState(false);

  const errors = useMemo<FieldErrors>(() => {
    const parsed = schema.safeParse(values);
    if (parsed.success) return {};
    const out: FieldErrors = {};
    for (const issue of parsed.error.issues) {
      const raw = issue.path[0];
      const key = raw === undefined ? '_root' : String(raw);
      // first-issue-wins per field; this is friendlier than dumping every
      // problem a complex object can produce for one bad leaf.
      if (out[key] === undefined) {
        out[key] = issue.message;
      }
    }
    return out;
  }, [schema, values]);

  const visibleErrors = useMemo<VisibleFieldErrors>(() => {
    const out: VisibleFieldErrors = { ...errors };
    if (!submitAttempted) {
      for (const key of Object.keys(errors)) {
        if (key !== '_root' && !touched[key]) {
          out[key] = undefined;
        }
      }
    }
    return out;
  }, [errors, touched, submitAttempted]);

  const isValid = Object.keys(errors).length === 0;

  const setValue = useCallback(
    <K extends Key>(name: K, value: Input[K]) => {
      setValues((prev) => ({ ...prev, [name as string]: value }));
    },
    [],
  );

  const handleBlur = useCallback((name: Key) => {
    setTouched((prev) => ({ ...prev, [name as string]: true }));
  }, []);

  const handleSubmit = useCallback(
    (onValid: (parsed: Output) => void | Promise<void>) =>
      async (e?: FormEvent) => {
        if (e && typeof e.preventDefault === 'function') e.preventDefault();
        setSubmitAttempted(true);
        // Mark all fields touched so all errors become visible.
        setTouched(
          (Object.keys(values) as string[]).reduce(
            (acc, k) => ({ ...acc, [k]: true }),
            {} as TouchedMap,
          ),
        );
        const parsed = schema.safeParse(values);
        if (!parsed.success) {
          // Bad input is intentionally blocked — we never call onValid.
          return;
        }
        await onValid(parsed.data as Output);
      },
    [schema, values],
  );

  const reset = useCallback(
    (next?: Input) => {
      setValues((next ?? initialValues) as unknown as Record<string, unknown>);
      setTouched({});
      setSubmitAttempted(false);
    },
    [initialValues],
  );

  return {
    values: values as Input,
    errors,
    visibleErrors,
    isValid,
    hasAttemptedSubmit: submitAttempted,
    touched,
    setValue,
    handleBlur,
    handleSubmit,
    reset,
  };
}
