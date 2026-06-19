/**
 * useAuditLogs — Exposes the synthetic audit entries used by the Log Export
 * panel. Kept as a tiny hook so the data source can later be swapped for a
 * real fetch without touching consumers.
 */

import { useMemo, useState } from "react";
import { generateAuditLog } from "../utils/auditLog";
import type { AuditEntry, AuditEventType, AuditStatus } from "../types";

export type StatusFilter = "all" | AuditStatus;
export type TypeFilter = "all" | AuditEventType;

export function useAuditLogs(initialCount: number = 60) {
  const [logs] = useState<AuditEntry[]>(() => generateAuditLog(initialCount));
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [typeFilter, setTypeFilter] = useState<TypeFilter>("all");

  const filtered = useMemo(
    () =>
      logs.filter((entry) => {
        if (statusFilter !== "all" && entry.status !== statusFilter) {
          return false;
        }
        if (typeFilter !== "all" && entry.eventType !== typeFilter) {
          return false;
        }
        return true;
      }),
    [logs, statusFilter, typeFilter]
  );

  return {
    logs,
    filtered,
    statusFilter,
    setStatusFilter,
    typeFilter,
    setTypeFilter,
  };
}
