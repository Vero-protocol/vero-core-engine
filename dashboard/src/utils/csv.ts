/**
 * csv.ts — RFC 4180 compliant CSV serialization plus a browser download
 * helper. Kept dependency-free so it can be unit-tested with vitest in jsdom
 * without having to polyfill Blob/URL.createObjectURL at the module level.
 */

export interface CsvColumn<T> {
  /** Property key on the row. */
  key: Extract<keyof T, string>;
  /** Header label rendered as the first row. */
  header: string;
  /** Optional formatter receiving the resolved value and the full row. */
  format?: (
    value: T[Extract<keyof T, string>],
    row: T
  ) => string | number | boolean;
}

/** RFC 4180 §2 field escape. Quoting only when required keeps the output tidy. */
export function escapeCsvField(value: unknown): string {
  if (value === null || value === undefined) return "";
  const str = typeof value === "string" ? value : String(value);
  if (/[",\r\n]/.test(str)) {
    return `"${str.replace(/"/g, '""')}"`;
  }
  return str;
}/** Build a CRLF-delimited CSV string from `rows` using the supplied columns. */
export function toCsv<T>(
  rows: readonly T[],
  columns: readonly CsvColumn<T>[]
): string {
  const headerLine = columns.map((c) => escapeCsvField(c.header)).join(",");
  const bodyLines = rows.map((row) =>
    columns
      .map((col) => {
        const raw = row[col.key];
        const value = col.format ? col.format(raw, row) : raw;
        return escapeCsvField(value);
      })
      .join(",")
  );
  // Always end with a trailing CRLF per the spec.
  return [headerLine, ...bodyLines].join("\r\n") + "\r\n";
}

/**
 * Trigger a browser download for the supplied CSV content.
 *
 * `URL.createObjectURL` is widely supported but occasionally undefined in
 * older jsdom builds; we guard it so the helper remains safe to import in
 * non-browser test environments.
 */
export function downloadCsv(filename: string, csv: string): void {
  if (typeof document === "undefined" || typeof URL === "undefined") {
    throw new Error("downloadCsv must be called in a browser environment");
  }
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  // Some browsers require the element to be in the DOM before .click() works.
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  // Defer revoke so Safari has a chance to start the download.
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Format epoch ms as `YYYY-MM-DD_HH-mm-ss` for filename-safe timestamps. */
export function timestampForFilename(ts: number): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const d = new Date(ts);
  return (
    `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}` +
    `_${pad(d.getUTCHours())}-${pad(d.getUTCMinutes())}-${pad(d.getUTCSeconds())}`
  );
}
