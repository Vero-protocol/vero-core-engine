import { timingSafeEqual } from "crypto";
import type { IncomingMessage } from "http";

export interface RelayerAuthOptions {
  apiKeys?: string[];
  jwtSecret?: string;
}

export interface VerifyClientInfo {
  origin: string;
  req: IncomingMessage;
  secure: boolean;
}

export type VerifyClientCallback = (result: boolean, code?: number, message?: string) => void;

export interface AuthResult {
  allowed: boolean;
  statusCode?: number;
  message?: string;
}

function extractApiKey(req: IncomingMessage): string | null {
  const authHeader = req.headers["authorization"];
  if (authHeader) {
    const parts = (Array.isArray(authHeader) ? authHeader[0] : authHeader).split(" ");
    if (parts.length === 2 && parts[0].toLowerCase() === "bearer") {
      return parts[1];
    }
  }

  const apiKeyHeader = req.headers["x-api-key"];
  if (apiKeyHeader) {
    return Array.isArray(apiKeyHeader) ? apiKeyHeader[0] : apiKeyHeader;
  }

  return null;
}

export class RelayerAuth {
  private readonly apiKeys: string[];

  constructor(options: RelayerAuthOptions = {}) {
    this.apiKeys = options.apiKeys ?? [];
  }

  verifyClient(info: VerifyClientInfo, cb: VerifyClientCallback): void {
    const key = extractApiKey(info.req);
    if (key && this.apiKeys.some(apiKey => {
      const candidate = Buffer.from(key);
      const expected = Buffer.from(apiKey);
      return candidate.length === expected.length && timingSafeEqual(candidate, expected);
    })) {
      cb(true);
      return;
    }
    cb(false, 401, "Unauthorized: missing or invalid API key");
  }
}
