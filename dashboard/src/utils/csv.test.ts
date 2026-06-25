import { describe, it, expect } from "vitest";
import {
  escapeCsvField,
  toCsv,
  timestampForFilename,
} from "./csv";

describe("escapeCsvField", () => {
  it("returns empty string for null/undefined", () => {
    expect(escapeCsvField(null)).toBe("");
    expect(escapeCsvField(undefined)).toBe("");
  });

  it("passes through plain strings untouched", () => {
    expect(escapeCsvField("hello")).toBe("hello");
    expect(escapeCsvField("hello world")).toBe("hello world");
  });

  it("quotes fields containing commas", () => {
    expect(escapeCsvField("a,b")).toBe('"a,b"');
  });

  it("quotes and doubles inner quotes", () => {
    expect(escapeCsvField('she said "hi"')).toBe('"she said ""hi"""');
  });

  it("quotes fields containing newlines", () => {
    expect(escapeCsvField("line1\nline2")).toBe('"line1\nline2"');
    expect(escapeCsvField("line1\r\nline2")).toBe('"line1\r\nline2"');
  });

  it("coerces non-strings without quoting unless needed", () => {
    expect(escapeCsvField(42)).toBe("42");
    expect(escapeCsvField(true)).toBe("true");
    expect(escapeCsvField(3.14)).toBe("3.14");
  });
});

describe("toCsv", () => {
  interface Row {
    name: string;
    count: number;
    note: string;
  }

  const cols: import("./csv").CsvColumn<Row>[] = [
    { key: "name", header: "Name" },
    { key: "count", header: "Count" },
    {
      key: "note",
      header: "Note",
      // `v` is typed as the union of the row's string-keyed fields. Wrap
      // in `String()` so the callable is compatible regardless of whether
      // the underlying value is a number or a string.
      format: (v) => String(v).toUpperCase(),
    },
  ];

  it("emits a header line followed by CRLF rows", () => {
    const csv = toCsv<Row>([{ name: "alpha", count: 1, note: "ok" }], cols);
    expect(csv).toBe("Name,Count,Note\r\nalpha,1,OK\r\n");
  });

  it("quotes special characters correctly", () => {
    const csv = toCsv<Row>(
      [{ name: 'he said "hi"', count: 0, note: "a,b\nc" }],
      cols
    );
    // The Note column applies `v.toUpperCase()`, so the value is escaped
    // and uppercased before being wrapped in quotes.
    expect(csv).toBe(
      'Name,Count,Note\r\n"he said ""hi""",0,"A,B\nC"\r\n'
    );
  });

  it("handles an empty row list with just the header", () => {
    expect(toCsv<Row>([], cols)).toBe("Name,Count,Note\r\n");
  });

  it("coerces missing values to empty strings", () => {
    interface Partial {
      a: string | null;
      b: number | undefined;
    }
    const csv = toCsv<Partial>(
      [{ a: null, b: undefined }],
      [
        { key: "a", header: "A" },
        { key: "b", header: "B" },
      ]
    );
    expect(csv).toBe("A,B\r\n,\r\n");
  });
});

describe("timestampForFilename", () => {
  it("formats epoch ms as UTC YYYY-MM-DD_HH-mm-ss", () => {
    // 2024-01-02 03:04:05 UTC
    const ts = Date.UTC(2024, 0, 2, 3, 4, 5);
    expect(timestampForFilename(ts)).toBe("2024-01-02_03-04-05");
  });

  it("zero-pads single-digit values", () => {
    const ts = Date.UTC(2024, 8, 9, 1, 2, 3); // Sept 9
    expect(timestampForFilename(ts)).toBe("2024-09-09_01-02-03");
  });
});
