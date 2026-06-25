import { describe, it, expect } from 'vitest';
import {
  httpsUrl,
  httpUrl,
  stellarAddress,
  boundedInt,
  boundedPercent,
  optionalUrl,
  guardianSettingsSchema,
  GUARDIAN_SETTINGS_DEFAULTS,
} from './validation';

describe('httpsUrl', () => {
  it('accepts a valid https URL', () => {
    expect(httpsUrl.safeParse('https://soroban-testnet.stellar.org').success).toBe(true);
  });

  it('rejects plain http URLs', () => {
    const result = httpsUrl.safeParse('http://example.com');
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0].message).toMatch(/HTTPS/i);
    }
  });

  it('rejects non-URL strings', () => {
    expect(httpsUrl.safeParse('not-a-url').success).toBe(false);
    expect(httpsUrl.safeParse('').success).toBe(false);
  });
});

describe('httpUrl(allowHttp)', () => {
  it('rejects http URLs when allowHttp=false', () => {
    expect(httpUrl(false).safeParse('http://localhost:8000').success).toBe(false);
  });

  it('accepts http URLs when allowHttp=true', () => {
    expect(httpUrl(true).safeParse('http://localhost:8000').success).toBe(true);
    expect(httpUrl(true).safeParse('https://example.com').success).toBe(true);
  });
});

describe('stellarAddress', () => {
  // Programmatic fixtures — 55 base32 chars, alphabet [A-Z2-7].
  const validG = 'G' + 'A'.repeat(55);
  const validC = 'C' + 'A'.repeat(55);

  it('accepts a syntactically valid G... address', () => {
    expect(stellarAddress.safeParse(validG).success).toBe(true);
  });

  it('accepts a syntactically valid C... contract id', () => {
    expect(stellarAddress.safeParse(validC).success).toBe(true);
  });

  it('rejects a too-short string', () => {
    expect(stellarAddress.safeParse('GAAAA').success).toBe(false);
  });

  it('rejects lowercase characters', () => {
    const lc = 'g' + 'a'.repeat(55);
    expect(stellarAddress.safeParse(lc).success).toBe(false);
  });

  it('rejects unsupported prefixes', () => {
    const bad = 'M' + 'A'.repeat(55);
    expect(stellarAddress.safeParse(bad).success).toBe(false);
  });
});

describe('boundedInt', () => {
  const s = boundedInt(1, 10, 'Value');

  it('coerces numeric strings and validates range', () => {
    expect(s.safeParse('5').success).toBe(true);
  });

  it('rejects values below the minimum', () => {
    expect(s.safeParse('0').success).toBe(false);
  });

  it('rejects values above the maximum', () => {
    expect(s.safeParse('11').success).toBe(false);
  });

  it('rejects non-integers', () => {
    expect(s.safeParse('3.5').success).toBe(false);
  });
});

describe('boundedPercent', () => {
  const s = boundedPercent(0, 100, 'Percentage');

  it('accepts in-range values', () => {
    expect(s.safeParse('0').success).toBe(true);
    expect(s.safeParse('42.5').success).toBe(true);
    expect(s.safeParse('100').success).toBe(true);
  });

  it('rejects out-of-range values', () => {
    expect(s.safeParse('-1').success).toBe(false);
    expect(s.safeParse('101').success).toBe(false);
  });
});

describe('optionalUrl', () => {
  it('accepts empty string', () => {
    expect(optionalUrl.safeParse('').success).toBe(true);
  });

  it('accepts valid URL', () => {
    expect(optionalUrl.safeParse('https://hooks.example.com').success).toBe(true);
  });

  it('rejects malformed URL', () => {
    expect(optionalUrl.safeParse('not a url').success).toBe(false);
  });
});

describe('guardianSettingsSchema', () => {
  const valid = {
    rpcEndpoint: 'https://soroban-testnet.stellar.org',
    signerThreshold: '3',
    timelockSeconds: '720',
    webhookUrl: 'https://hooks.example.com/guardian',
  };

  it('parses valid settings and coerces numeric fields', () => {
    const result = guardianSettingsSchema.safeParse(valid);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.signerThreshold).toBe(3);
      expect(result.data.timelockSeconds).toBe(720);
      expect(typeof result.data.signerThreshold).toBe('number');
    }
  });

  it('blocks an http RPC endpoint', () => {
    const r = guardianSettingsSchema.safeParse({
      ...valid,
      rpcEndpoint: 'http://soroban-testnet.stellar.org',
    });
    expect(r.success).toBe(false);
  });

  it('blocks a threshold out of bounds', () => {
    const r = guardianSettingsSchema.safeParse({
      ...valid,
      signerThreshold: '11',
    });
    expect(r.success).toBe(false);
    if (!r.success) {
      const fieldErrors = r.error.issues.map((i) => i.path[0]);
      expect(fieldErrors).toContain('signerThreshold');
    }
  });

  it('allows omitting the optional webhook', () => {
    const { webhookUrl: _omit, ...rest } = valid;
    const r = guardianSettingsSchema.safeParse(rest);
    expect(r.success).toBe(true);
  });

  it('exposes stable defaults', () => {
    expect(GUARDIAN_SETTINGS_DEFAULTS.signerThreshold).toBe(3);
    expect(GUARDIAN_SETTINGS_DEFAULTS.timelockSeconds).toBe(720);
  });
});
