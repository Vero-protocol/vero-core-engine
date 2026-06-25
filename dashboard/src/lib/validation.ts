import { z } from 'zod';

/**
 * Schema validators for the Guardian Dashboard.
 *
 * These are intentionally restrictive on the wire-side (e.g. require HTTPS
 * for RPC endpoints) so that malformed / dangerous values are rejected
 * before they ever reach the engine-bridge or on-chain layer.
 *
 * All messages are user-facing; keep them short and action-oriented.
 */

/**
 * Stellar RPC endpoints MUST use HTTPS in production. We allow `http://`
 * only when callers explicitly opt in (used by the local dev validator in
 * docker-compose). Otherwise every other URL must be `https://...`.
 */
const isHttpUrl = (s: string): boolean => {
  try {
    const protocol = new URL(s).protocol;
    return protocol === 'http:' || protocol === 'https:';
  } catch {
    return false;
  }
};

export const httpsUrl = z
  .string()
  .min(1, 'URL is required')
  .refine(isHttpUrl, 'Must be a valid URL')
  .refine((u) => u.startsWith('https://'), {
    message: 'RPC endpoint must use HTTPS',
  });

export const httpUrl = (allowHttp = false) => {
  const base = z
    .string()
    .min(1, 'URL is required')
    .refine(isHttpUrl, 'Must be a valid URL');
  if (allowHttp) return base;
  return base.refine((u) => u.startsWith('https://'), {
    message: 'Must use HTTPS (use httpUrl(true) to allow http://)',
  });
};

/**
 * Stellar address format — Ed25519 accounts (`G...`) or contract IDs (`C...`).
 * 56 base32 characters from the alphabet `[A-Z2-7]` after the prefix.
 * Solana-style or invalid addresses are rejected.
 */
const STELLAR_BASE32 = /^[A-Z2-7]{55}$/;
export const stellarAddress = z.string().refine(
  (s) =>
    (s.startsWith('G') || s.startsWith('C')) && STELLAR_BASE32.test(s.slice(1)),
  'Invalid Stellar address (expected G... or C..., 56 chars base32)',
);

/**
 * Bounded integer with a friendly label. Use through `z.coerce.number()` so
 * HTML <input type="number"> values (always strings on the wire) parse
 * cleanly into native numbers.
 */
export const boundedInt = (min: number, max: number, label = 'Value') =>
  z.coerce
    .number({ message: `${label} must be a number` })
    .int(`${label} must be a whole number`)
    .min(min, `${label} must be at least ${min}`)
    .max(max, `${label} must be at most ${max}`);

/**
 * Bounded percentage / ratio stored as a decimal number (0–100).
 */
export const boundedPercent = (min: number, max: number, label = 'Percentage') =>
  z.coerce
    .number({ message: `${label} must be a number` })
    .min(min, `${label} must be at least ${min}`)
    .max(max, `${label} must be at most ${max}`);

/**
 * Optional URL — empty strings are treated as "not set".
 */
export const optionalUrl = z
  .string()
  .refine((s) => s === '' || isHttpUrl(s), 'Must be a valid URL or empty')
  .optional();

/**
 * `guardianSettingsSchema` — used by the GuardianConfigForm demo and
 * reusable across future dashboard config forms (issue #38 custom RPC,
 * #42 vote weight calc, etc.).
 */
export const guardianSettingsSchema = z.object({
  rpcEndpoint: httpsUrl,
  signerThreshold: boundedInt(1, 10, 'Signer threshold'),
  timelockSeconds: boundedInt(60, 86_400, 'Time-lock'),
  webhookUrl: optionalUrl,
});

export type GuardianSettingsInput = z.input<typeof guardianSettingsSchema>;
export type GuardianSettings = z.output<typeof guardianSettingsSchema>;

export const GUARDIAN_SETTINGS_DEFAULTS: GuardianSettingsInput = {
  rpcEndpoint: '',
  signerThreshold: 3,
  timelockSeconds: 720,
  webhookUrl: '',
};
