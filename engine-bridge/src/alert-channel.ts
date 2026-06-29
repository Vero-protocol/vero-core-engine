import { logger } from "./logger";

export interface Alert {
  id: string;
  severity: "INFO" | "WARNING" | "CRITICAL";
  title: string;
  message: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

export interface AlertChannel {
  send(alert: Alert): Promise<boolean>;
}

export interface AlertChannelServiceOptions {
  channels?: AlertChannel[];
}

export class AlertChannelService {
  private channels: AlertChannel[];

  constructor(options: AlertChannelServiceOptions = {}) {
    this.channels = options.channels ?? [new ConsoleAlertChannel()];
  }

  addChannel(channel: AlertChannel): void {
    this.channels.push(channel);
  }

  async send(alert: Alert): Promise<void> {
    const results = await Promise.allSettled(
      this.channels.map(ch => ch.send(alert))
    );

    for (let i = 0; i < results.length; i++) {
      const result = results[i];
      if (result.status === "rejected") {
        logger.error(`[AlertChannelService] Channel ${i} failed:`, result.reason);
      } else if (!result.value) {
        logger.warn(`[AlertChannelService] Channel ${i} returned false`);
      }
    }
  }
}

export class ConsoleAlertChannel implements AlertChannel {
  async send(alert: Alert): Promise<boolean> {
    const prefix = alert.severity === "CRITICAL" ? "[ALERT CRITICAL]" :
                   alert.severity === "WARNING"  ? "[ALERT WARNING]" :
                                                   "[ALERT INFO]";

    const meta = alert.metadata ? ` ${JSON.stringify(alert.metadata)}` : "";

    if (alert.severity === "CRITICAL") {
      logger.error(`${prefix} ${alert.title} — ${alert.message}${meta}`, { alert });
    } else if (alert.severity === "WARNING") {
      logger.warn(`${prefix} ${alert.title} — ${alert.message}${meta}`, { alert });
    } else {
      logger.info(`${prefix} ${alert.title} — ${alert.message}${meta}`, { alert });
    }

    return true;
  }
}

export interface WebhookAlertChannelOptions {
  url: string;
  headers?: Record<string, string>;
  timeoutMs?: number;
}

export class WebhookAlertChannel implements AlertChannel {
  private readonly url: string;
  private readonly headers: Record<string, string>;
  private readonly timeoutMs: number;

  constructor(options: WebhookAlertChannelOptions) {
    this.url = options.url;
    this.headers = {
      "Content-Type": "application/json",
      ...options.headers,
    };
    this.timeoutMs = options.timeoutMs ?? 10_000;
  }

  async send(alert: Alert): Promise<boolean> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      const response = await fetch(this.url, {
        method: "POST",
        headers: this.headers,
        body: JSON.stringify({
          id: alert.id,
          severity: alert.severity,
          title: alert.title,
          message: alert.message,
          timestamp: alert.timestamp,
          metadata: alert.metadata,
        }),
        signal: controller.signal,
      });

      if (!response.ok) {
        logger.warn(`[WebhookAlertChannel] HTTP ${response.status} for alert ${alert.id}`);
        return false;
      }

      return true;
    } catch (err) {
      logger.error(`[WebhookAlertChannel] Request failed for alert ${alert.id}:`, err);
      return false;
    } finally {
      clearTimeout(timer);
    }
  }
}
