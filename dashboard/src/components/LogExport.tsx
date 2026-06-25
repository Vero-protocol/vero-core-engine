import React, { useId, useMemo } from "react";
import { Download, FileSpreadsheet, Filter } from "lucide-react";
import { Card } from "./Card";
import { useAuditLogs, type StatusFilter, type TypeFilter } from "../hooks/useAuditLogs";
import { downloadCsv, toCsv, timestampForFilename } from "../utils/csv";

const STATUS_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "success", label: "Success" },
  { value: "rejected", label: "Rejected" },
  { value: "pending", label: "Pending" },
];

const TYPE_OPTIONS: { value: TypeFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "commit", label: "Commit" },
  { value: "snapshot", label: "Snapshot" },
  { value: "replay", label: "Replay" },
  { value: "mismatch", label: "Mismatch" },
];

const statusBadge = (status: string) => {
  const map: Record<string, string> = {
    success: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300",
    rejected: "bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300",
    pending: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300",
  };
  return map[status] ?? "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200";
};

/** A small, focusable select used for filters. */
const FilterSelect = <T extends string>({
  id,
  value,
  onChange,
  options,
  label,
}: {
  id: string;
  value: T;
  onChange: (next: T) => void;
  options: { value: T; label: string }[];
  label: string;
}) => (
  <label htmlFor={id} className="inline-flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
    <Filter size={12} />
    <span>{label}</span>
    <select
      id={id}
      value={value}
      onChange={(e) => onChange(e.target.value as T)}
      className="px-2 py-1 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500/60"
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  </label>
);

/**
 * LogExport — Audit log table + CSV export.
 *
 * Acceptance criteria from issue #37:
 *   - File download logic
 *   - Valid CSV (RFC 4180 compliant via csv.ts)
 *   - Data integrity OK (rows match exactly what's rendered)
 */
export const LogExport: React.FC = () => {
  const {
    filtered,
    logs,
    statusFilter,
    setStatusFilter,
    typeFilter,
    setTypeFilter,
  } = useAuditLogs();

  const counts = useMemo(
    () => ({
      success: logs.filter((l) => l.status === "success").length,
      rejected: logs.filter((l) => l.status === "rejected").length,
      pending: logs.filter((l) => l.status === "pending").length,
    }),
    [logs]
  );

  // Stable ids for linking screen-reader announcements to the regions
  // that hold them — keeps `aria-live` and `aria-describedby` deterministic
  // across renders and avoids colliding with adjacent panels.
  const filterHelpId = useId();
  const resultCountId = useId();

  const csv = useMemo(
    () =>
      toCsv(filtered, [
        { key: "timestamp", header: "Timestamp (UTC)", format: (v) => new Date(Number(v)).toISOString() },
        { key: "sequence", header: "Sequence" },
        { key: "ledger", header: "Ledger" },
        { key: "author", header: "Author" },
        { key: "eventType", header: "Event Type" },
        { key: "status", header: "Status" },
        { key: "stateHash", header: "State Hash" },
        { key: "payloadBytes", header: "Payload Bytes" },
        { key: "feeStroops", header: "Fee (stroops)" },
      ]),
    [filtered]
  );

  const handleExport = () => {
    const filename = `vero-audit-log_${timestampForFilename(Date.now())}.csv`;
    downloadCsv(filename, csv);
  };

  return (
    <Card
      title="Audit Log Export"
      description="Filter the generated audit entries and download them as RFC 4180 compliant CSV."
      actions={
        <button
          type="button"
          onClick={handleExport}
          disabled={filtered.length === 0}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md bg-emerald-600 hover:bg-emerald-500 text-white disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-emerald-500/60 transition-colors duration-150"
        >
          <Download size={14} />
          Export CSV
        </button>
      }
    >
      <div
        className="flex flex-wrap items-center gap-4 mb-4"
        role="group"
        aria-describedby={filterHelpId}
      >
        <FilterSelect
          id="status-filter"
          value={statusFilter}
          onChange={setStatusFilter}
          options={STATUS_OPTIONS}
          label="Status"
        />
        <FilterSelect
          id="type-filter"
          value={typeFilter}
          onChange={setTypeFilter}
          options={TYPE_OPTIONS}
          label="Type"
        />
        <p id={filterHelpId} className="sr-only">
          Filter the audit entries shown in the table below.
        </p>
        <div
          id={resultCountId}
          className="ml-auto flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400"
          aria-live="polite"
          aria-atomic="true"
        >
          <span className="inline-flex items-center gap-1">
            <FileSpreadsheet size={12} /> {filtered.length} / {logs.length} entries
          </span>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 mb-4 text-xs">
        <span className="px-2 py-0.5 rounded-full bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300">
          ✓ {counts.success} success
        </span>
        <span className="px-2 py-0.5 rounded-full bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300">
          ✗ {counts.rejected} rejected
        </span>
        <span className="px-2 py-0.5 rounded-full bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300">
          • {counts.pending} pending
        </span>
      </div>

      <div
        className="overflow-x-auto border border-gray-200 dark:border-gray-700 rounded-lg"
        aria-describedby={resultCountId}
      >
        <table className="w-full text-sm" aria-rowcount={filtered.length}>
          <thead className="bg-gray-50 dark:bg-gray-900/40 text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">
            <tr>
              <th className="px-3 py-2 text-left font-medium">Time</th>
              <th className="px-3 py-2 text-right font-medium">Seq</th>
              <th className="px-3 py-2 text-right font-medium">Ledger</th>
              <th className="px-3 py-2 text-left font-medium">Author</th>
              <th className="px-3 py-2 text-left font-medium">Type</th>
              <th className="px-3 py-2 text-left font-medium">Status</th>
              <th className="px-3 py-2 text-right font-medium">Fee</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-700/60">
            {filtered.length === 0 ? (
              <tr>
                <td colSpan={7} className="px-3 py-8 text-center text-gray-500 dark:text-gray-400">
                  No entries match the current filters.
                </td>
              </tr>
            ) : (
              filtered.slice(0, 12).map((row) => (
                <tr key={row.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/20 transition-colors duration-150">
                  <td className="px-3 py-2 whitespace-nowrap tabular-nums">{new Date(row.timestamp).toISOString().slice(11, 19)}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{row.sequence}</td>
                  <td className="px-3 py-2 text-right tabular-nums">{row.ledger}</td>
                  <td className="px-3 py-2 font-mono text-[11px] truncate max-w-[9rem]" title={row.author}>{row.author.slice(0, 8)}…</td>
                  <td className="px-3 py-2 capitalize">{row.eventType}</td>
                  <td className="px-3 py-2">
                    <span className={"px-1.5 py-0.5 rounded text-[10px] font-semibold " + statusBadge(row.status)}>
                      {row.status}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">{row.feeStroops}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
        {filtered.length > 12 && (
          <div className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-900/40">
            Showing first 12 of {filtered.length} rows. The CSV export contains every filtered row.
          </div>
        )}
      </div>
    </Card>
  );
};
