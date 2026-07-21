import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/analysis-soshite.json";
import { parseAnalysisDocument } from "./types";

describe("parseAnalysisDocument", () => {
  it("accepts the committed schema version 1 fixture", () => {
    expect(parseAnalysisDocument(fixture)).toBe(fixture);
  });

  it("rejects unsupported schema versions", () => {
    expect(() =>
      parseAnalysisDocument({ ...fixture, schema_version: 2 }),
    ).toThrow("unsupported analysis schema version: 2");
  });

  it.each([null, [], { schema_version: 1 }, { ...fixture, tokens: null }])(
    "rejects a malformed top-level document: %j",
    (value) => {
      expect(() => parseAnalysisDocument(value)).toThrow("invalid analysis document");
    },
  );
});
