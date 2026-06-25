import React, { useState } from 'react';
import { Field } from './Field';
import {
  guardianSettingsSchema,
  GUARDIAN_SETTINGS_DEFAULTS,
  type GuardianSettings,
  type GuardianSettingsInput,
} from '../lib/validation';
import { useZodForm } from '../hooks/useZodForm';

type SubmitState =
  | { status: 'idle' }
  | { status: 'success'; value: GuardianSettings }
  | { status: 'error'; message: string };

export type GuardianConfigFormProps = {
  /**
   * Optional save handler. Called only when the form passes validation —
   * the hook guarantees bad input is never propagated upstream.
   */
  onSave?: (settings: GuardianSettings) => void | Promise<void>;
  initialValues?: Partial<GuardianSettingsInput>;
};

/**
 * `GuardianConfigForm` — concrete demonstration of the validation layer.
 *
 * Renders four fields (rpcEndpoint, signerThreshold, timelockSeconds,
 * webhookUrl) and a submit button. Errors are surfaced inline beneath
 * the offending field once the user has interacted with it (or after a
 * submit attempt), using the `Field` component for accessibility.
 *
 * The submit button also reflects `isValid` so users get immediate
 * feedback that "something is wrong" before they click.
 */
export const GuardianConfigForm: React.FC<GuardianConfigFormProps> = ({
  onSave,
  initialValues,
}) => {
  const form = useZodForm({
    schema: guardianSettingsSchema,
    initialValues: {
      ...GUARDIAN_SETTINGS_DEFAULTS,
      ...initialValues,
    },
  });

  const [submitState, setSubmitState] = useState<SubmitState>({ status: 'idle' });
  const [isSaving, setIsSaving] = useState(false);

  const onSubmit = form.handleSubmit(async (parsed) => {
    setIsSaving(true);
    setSubmitState({ status: 'idle' });
    try {
      if (onSave) {
        await onSave(parsed);
      }
      setSubmitState({ status: 'success', value: parsed });
    } catch (err) {
      setSubmitState({
        status: 'error',
        message: err instanceof Error ? err.message : 'Save failed',
      });
    } finally {
      setIsSaving(false);
    }
  });

  return (
    <form
      noValidate
      onSubmit={onSubmit}
      aria-label="Guardian configuration"
      className="space-y-4"
      data-testid="guardian-config-form"
    >
      <Field
        label="RPC endpoint"
        type="url"
        placeholder="https://soroban-testnet.stellar.org"
        value={form.values.rpcEndpoint}
        onChange={(v) => form.setValue('rpcEndpoint', v)}
        onBlur={() => form.handleBlur('rpcEndpoint')}
        error={form.visibleErrors.rpcEndpoint}
        hint="HTTPS URL of the Soroban/Horizon RPC node."
        autoComplete="off"
        inputMode="url"
      />

      <Field
        label="Signer threshold"
        type="number"
        min={1}
        max={10}
        step={1}
        value={String(form.values.signerThreshold ?? '')}
        onChange={(v) => form.setValue('signerThreshold', v)}
        onBlur={() => form.handleBlur('signerThreshold')}
        error={form.visibleErrors.signerThreshold}
        hint="Multi-sig threshold for governance proposals (1–10)."
        inputMode="numeric"
        autoComplete="off"
      />

      <Field
        label="Time-lock (seconds)"
        type="number"
        min={60}
        max={86_400}
        step={60}
        value={String(form.values.timelockSeconds ?? '')}
        onChange={(v) => form.setValue('timelockSeconds', v)}
        onBlur={() => form.handleBlur('timelockSeconds')}
        error={form.visibleErrors.timelockSeconds}
        hint="Mandatory delay between approval and execution (60s–24h)."
        inputMode="numeric"
        autoComplete="off"
      />

      <Field
        label="Notification webhook URL"
        type="url"
        placeholder="https://hooks.example.com/guardian"
        value={form.values.webhookUrl ?? ''}
        onChange={(v) => form.setValue('webhookUrl', v)}
        onBlur={() => form.handleBlur('webhookUrl')}
        error={form.visibleErrors.webhookUrl}
        hint="Optional. Called when the circuit-breaker trips."
        optional
        autoComplete="off"
        inputMode="url"
      />

      {form.visibleErrors._root && (
        <div
          role="alert"
          className="rounded-md border border-red-300 dark:border-red-700 bg-red-50 dark:bg-red-900/30 px-3 py-2 text-sm text-red-700 dark:text-red-200"
        >
          {form.visibleErrors._root}
        </div>
      )}

      <div className="flex items-center gap-3 pt-2">
        <button
          type="submit"
          disabled={isSaving}
          aria-disabled={isSaving}
          data-testid="guardian-config-submit"
          className="inline-flex items-center justify-center rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:opacity-60 disabled:cursor-not-allowed"
        >
          {isSaving ? 'Saving…' : 'Save settings'}
        </button>
        <button
          type="button"
          onClick={() => {
            form.reset();
            setSubmitState({ status: 'idle' });
          }}
          className="text-sm font-medium text-gray-700 dark:text-gray-300 hover:underline"
        >
          Reset
        </button>
        {!form.isValid && form.hasAttemptedSubmit && (
          <span
            className="text-xs text-red-600 dark:text-red-400"
            data-testid="guardian-config-blocked"
          >
            Fix the highlighted fields to save.
          </span>
        )}
      </div>

      {submitState.status === 'success' && (
        <div
          role="status"
          data-testid="guardian-config-success"
          className="rounded-md border border-green-300 dark:border-green-700 bg-green-50 dark:bg-green-900/30 px-3 py-2 text-sm text-green-700 dark:text-green-200"
        >
          ✓ Settings saved. Threshold{' '}
          <strong>{submitState.value.signerThreshold}</strong>, time-lock{' '}
          <strong>{submitState.value.timelockSeconds}s</strong>.
        </div>
      )}

      {submitState.status === 'error' && (
        <div
          role="alert"
          className="rounded-md border border-red-300 dark:border-red-700 bg-red-50 dark:bg-red-900/30 px-3 py-2 text-sm text-red-700 dark:text-red-200"
        >
          {submitState.message}
        </div>
      )}
    </form>
  );
};

export default GuardianConfigForm;
