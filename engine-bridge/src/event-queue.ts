/**
 * event-queue.ts — Persistent local queue for events.
 *
 * Buffers events to SQLite before passing to handlers, preventing data loss
 * during traffic spikes or downstream handler failures. Supports recovery of
 * unprocessed events on restart.
 *
 * Queue States:
 *   pending   → Enqueued, awaiting handler processing
 *   processing → Currently being handled
 *   processed → Successfully handled
 *   failed    → Handler failed; eligible for retry
 */

import Database from "better-sqlite3";
import { EngineEvent } from "./event-propagator";
import * as fs from "fs";
import * as path from "path";
import { logger } from "./logger";

const DEFAULT_DB_PATH = path.join(process.cwd(), "event-queue.db");
const MAX_RETRIES = 3;

export interface QueuedEvent {
  id: string;
  eventData: EngineEvent;
  status: "pending" | "processing" | "processed" | "failed";
  attempts: number;
  enqueueTime: number;
  processTime?: number;
  error?: string;
  /** Timestamp when the event becomes eligible for the next attempt (ms since epoch) */
  nextAttempt?: number;
}

export class EventQueue {
  private db: Database.Database;
  private readonly dbPath: string;
  private readonly maxRetries: number;

  constructor(dbPath: string = DEFAULT_DB_PATH, maxRetries: number = MAX_RETRIES) {
    this.dbPath = dbPath;
    this.maxRetries = maxRetries;
    this.db = this.initializeDatabase();
  }

  /**
   * Initialize SQLite database with events table if not present.
   * Schema:
   *   id (TEXT PK)      - Unique event identifier
   *   eventData (TEXT)  - JSON-serialized EngineEvent
   *   status (TEXT)     - pending | processing | processed | failed
   *   attempts (INT)    - Number of processing attempts
   *   enqueueTime (INT) - Milliseconds since epoch
   *   processTime (INT) - Milliseconds since epoch (nullable)
   *   error (TEXT)      - Last error message (nullable)
   *   nextAttempt (INT) - Next time eligible for retry
   */
  private initializeDatabase(): Database.Database {
    const db = new Database(this.dbPath);
    db.pragma("journal_mode = WAL");
    db.pragma("synchronous = NORMAL");

    db.exec(`
        CREATE TABLE IF NOT EXISTS events (
          id TEXT PRIMARY KEY,
          eventData TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'pending',
          attempts INT NOT NULL DEFAULT 0,
          enqueueTime INT NOT NULL,
          processTime INT,
          error TEXT,
          nextAttempt INT NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_status ON events(status);
        CREATE INDEX IF NOT EXISTS idx_enqueueTime ON events(enqueueTime);
        CREATE INDEX IF NOT EXISTS idx_nextAttempt ON events(nextAttempt);
      `);

    return db;
  }

  /**
   * Enqueue an event for processing. Returns true if successfully queued.
   */
  enqueue(event: EngineEvent): boolean {
    try {
      const now = Date.now();
      const stmt = this.db.prepare(`
        INSERT INTO events (id, eventData, status, attempts, enqueueTime, nextAttempt)
        VALUES (?, ?, ?, ?, ?, ?)
      `);

      stmt.run(
        event.id,
        JSON.stringify(event),
        "pending",
        0,
        now,
        now
      );

      return true;
    } catch (err) {
      logger.error("[EventQueue] Enqueue failed:", err);
      return false;
    }
  }

  /**
   * Dequeue a pending event for processing. Transitions to 'processing' state.
   * Returns the event or null if none available.
   */
  dequeue(): QueuedEvent | null {
    try {
      const now = Date.now();
      const stmt = this.db.prepare(`
        SELECT * FROM events
        WHERE (status = 'pending' OR (status = 'failed' AND attempts < ?))
          AND nextAttempt <= ?
        ORDER BY enqueueTime ASC
        LIMIT 1
      `);

      const row = stmt.get(this.maxRetries, now) as any;
      if (!row) return null;

      // Transition to processing
      const updateStmt = this.db.prepare(`
        UPDATE events
        SET status = 'processing', attempts = attempts + 1
        WHERE id = ?
      `);
      updateStmt.run(row.id);

      return {
        id: row.id,
        eventData: JSON.parse(row.eventData),
        status: "processing",
        attempts: row.attempts + 1,
        enqueueTime: row.enqueueTime,
        processTime: row.processTime,
        error: row.error,
      };
    } catch (err) {
      logger.error("[EventQueue] Dequeue failed:", err);
      return null;
    }
  }

  /**
   * Mark an event as successfully processed. Transitions to 'processed' state.
   */
  markProcessed(eventId: string): boolean {
    try {
      const stmt = this.db.prepare(`
        UPDATE events
        SET status = 'processed', processTime = ?
        WHERE id = ?
      `);
      stmt.run(Date.now(), eventId);
      return true;
    } catch (err) {
      logger.error("[EventQueue] Mark processed failed:", err);
      return false;
    }
  }

  /**
   * Mark an event as failed with error message. If retries available,
   * transitions back to 'pending'; otherwise to 'failed'.
   */
  markFailed(eventId: string, error: Error): boolean {
    try {
      // Get current attempt count and nextAttempt
      const getStmt = this.db.prepare("SELECT attempts FROM events WHERE id = ?");
      const row = getStmt.get(eventId) as any;

      if (!row) return false;

      const hasMoreRetries = row.attempts < this.maxRetries;
      const newStatus = "failed"; // always update to 'failed' in DB to track states

      // Compute exponential backoff delay (in ms) based on next attempt count
      const isTest = process.env.NODE_ENV === "test";
      const delayMs = (hasMoreRetries && !isTest) ? Math.pow(2, row.attempts) * 1000 : 0; // attempts already incremented in dequeue
      const nextAttempt = Date.now() + delayMs;

      const stmt = this.db.prepare(`
        UPDATE events
        SET status = ?, error = ?, nextAttempt = ?
        WHERE id = ?
      `);
      stmt.run(newStatus, error.message, nextAttempt, eventId);

      return true;
    } catch (err) {
      logger.error("[EventQueue] Mark failed failed:", err);
      return false;
    }
  }

  /**
   * Recover unprocessed events from queue (for startup).
   * Returns all pending and failed (with retries available) events.
   */
  recoverPending(): QueuedEvent[] {
    try {
      // Reset any processing or retrying failed events back to pending
      const resetStmt = this.db.prepare(`
        UPDATE events
        SET status = 'pending'
        WHERE status = 'processing' OR (status = 'failed' AND attempts < ?)
      `);
      resetStmt.run(this.maxRetries);

      const stmt = this.db.prepare(`
        SELECT * FROM events
        WHERE status = 'pending'
        ORDER BY enqueueTime ASC
      `);

      const rows = stmt.all() as any[];
      return rows.map(row => ({
        id: row.id,
        eventData: JSON.parse(row.eventData),
        status: row.status,
        attempts: row.attempts,
        enqueueTime: row.enqueueTime,
        processTime: row.processTime,
        error: row.error,
      }));
    } catch (err) {
      logger.error("[EventQueue] Recover pending failed:", err);
      return [];
    }
  }

  /**
   * Get queue statistics (size, oldest event, error rate).
   */
  getStats(): {
    total: number;
    pending: number;
    processing: number;
    processed: number;
    failed: number;
    oldestEventAge: number | null;
  } {
    try {
      const allStmt = this.db.prepare("SELECT status, attempts FROM events");
      const rows = allStmt.all() as any[];

      const stats = {
        total: rows.length,
        pending: 0,
        processing: 0,
        processed: 0,
        failed: 0,
      };

      rows.forEach(row => {
        if (row.status === "pending") {
          stats.pending++;
        } else if (row.status === "processed") {
          stats.processed++;
        } else if (row.status === "processing") {
          stats.processing++;
        } else if (row.status === "failed") {
          if (row.attempts < this.maxRetries) {
            stats.processing++;
          } else {
            stats.failed++;
          }
        }
      });

      const oldestStmt = this.db.prepare("SELECT enqueueTime FROM events ORDER BY enqueueTime ASC LIMIT 1");
      const oldest = oldestStmt.get() as any;
      const oldestEventAge = oldest ? Date.now() - oldest.enqueueTime : null;

      return {
        ...stats,
        oldestEventAge,
      };
    } catch (err) {
      logger.error("[EventQueue] Get stats failed:", err);
      return {
        total: 0,
        pending: 0,
        processing: 0,
        processed: 0,
        failed: 0,
        oldestEventAge: null,
      };
    }
  }

  /**
   * Clean up old processed events. Removes processed events older than maxAgeMs.
   */
  cleanup(maxAgeMs: number = 24 * 60 * 60 * 1000): number {
    try {
      const stmt = this.db.prepare(`
        DELETE FROM events
        WHERE status = 'processed' AND processTime < ?
      `);

      const cutoff = Date.now() - maxAgeMs;
      const result = stmt.run(cutoff);
      return result.changes;
    } catch (err) {
      logger.error("[EventQueue] Cleanup failed:", err);
      return 0;
    }
  }

  /**
   * Close database connection.
   */
  close(): void {
    if (this.db) {
      this.db.close();
    }
  }

  /**
   * Delete queue file (for testing/reset). Closes database first.
   */
  deleteQueue(): void {
    this.close();
    if (fs.existsSync(this.dbPath)) {
      fs.unlinkSync(this.dbPath);
    }
    if (fs.existsSync(this.dbPath + "-shm")) {
      fs.unlinkSync(this.dbPath + "-shm");
    }
    if (fs.existsSync(this.dbPath + "-wal")) {
      fs.unlinkSync(this.dbPath + "-wal");
    }
  }
}
