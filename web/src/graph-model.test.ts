import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/analysis-soshite.json";
import type { AnalysisDocument } from "./types";
import { buildGraphModel, buildOrderedTree } from "./graph-model";

function documentCopy(): AnalysisDocument {
  return structuredClone(fixture) as AnalysisDocument;
}

describe("buildOrderedTree", () => {
  it("preserves edge order recursively", () => {
    const root = buildOrderedTree(documentCopy().tree);
    expect(root.node.id).toBe("sentence-0");
    expect(root.children.map((child) => child.node.id)).toEqual([
      "match-0-0",
      "match-1-3",
    ]);
    expect(root.children[1]?.children.map((child) => child.node.id)).toEqual([
      "token-1",
      "token-2",
      "token-3",
    ]);
  });

  it("rejects duplicate node IDs", () => {
    const document = documentCopy();
    document.tree.nodes.push(structuredClone(document.tree.nodes[0]!));
    expect(() => buildOrderedTree(document.tree)).toThrow(
      "duplicate tree node: sentence-0",
    );
  });

  it("rejects missing roots and edge references", () => {
    const missingRoot = documentCopy();
    missingRoot.tree.root_id = "missing";
    expect(() => buildOrderedTree(missingRoot.tree)).toThrow(
      "missing tree root: missing",
    );

    const missingChild = documentCopy();
    missingChild.tree.edges[0]!.child_id = "missing";
    expect(() => buildOrderedTree(missingChild.tree)).toThrow(
      "missing tree child: missing",
    );
  });

  it("rejects multiple parents", () => {
    const document = documentCopy();
    document.tree.edges.push({
      parent_id: "match-0-0",
      child_id: "token-1",
      order: 1,
    });
    expect(() => buildOrderedTree(document.tree)).toThrow(
      "multiple parents for tree node: token-1",
    );
  });

  it("rejects cycles and disconnected nodes", () => {
    const cycle = documentCopy();
    cycle.tree.edges.push({
      parent_id: "token-0",
      child_id: "sentence-0",
      order: 0,
    });
    expect(() => buildOrderedTree(cycle.tree)).toThrow("tree contains a cycle");

    const disconnected = documentCopy();
    disconnected.tree.edges = disconnected.tree.edges.filter(
      (edge) => edge.child_id !== "token-3",
    );
    expect(() => buildOrderedTree(disconnected.tree)).toThrow(
      "disconnected tree node: token-3",
    );
  });
});

describe("buildGraphModel", () => {
  it("derives faithful labels from the committed document", () => {
    const root = buildGraphModel(documentCopy());
    expect([root.primaryLabel, root.secondaryLabel]).toEqual(["", ""]);
    expect(root.children[0]).toMatchObject({
      id: "match-0-0",
      primaryLabel: "そして",
      secondaryLabel: "Used to connect two sentences; 'and then', 'and'.",
    });
    expect(root.children[1]).toMatchObject({
      id: "match-1-3",
      primaryLabel: "なによりも",
      secondaryLabel: "Above all else, more than anything",
    });
    expect(root.children[1]?.children.map((node) => node.primaryLabel)).toEqual([
      "なに",
      "より",
      "も",
    ]);
  });

  it("uses a gloss before a non-redundant reading", () => {
    const withGloss = documentCopy();
    withGloss.tokens[1]!.surface = "何";
    withGloss.tokens[1]!.glosses = [
      { entry_seq: 1, gloss: "what", pos: ["pronoun"] },
    ];
    expect(
      buildGraphModel(withGloss).children[1]?.children[0]?.secondaryLabel,
    ).toBe("what");

    withGloss.tokens[1]!.glosses = [];
    expect(
      buildGraphModel(withGloss).children[1]?.children[0]?.secondaryLabel,
    ).toBe("なに");
  });

  it("does not repeat a reading identical to the surface", () => {
    expect(
      buildGraphModel(documentCopy()).children[0]?.children[0]?.secondaryLabel,
    ).toBe("");
  });

  it("requires a sentence root", () => {
    const document = documentCopy();
    document.tree.nodes[0]!.kind = "segment";
    expect(() => buildGraphModel(document)).toThrow(
      "tree root must be a sentence",
    );
  });

  it("derives segment surfaces without invented translations", () => {
    const document = documentCopy();
    document.tree.nodes[1]!.kind = "segment";
    document.tree.nodes[1]!.match_id = null;
    expect(buildGraphModel(document).children[0]).toMatchObject({
      primaryLabel: "そして",
      secondaryLabel: "",
    });
  });

  it("rejects missing references and invalid spans", () => {
    const missingToken = documentCopy();
    missingToken.tree.nodes[2]!.token_id = "missing";
    expect(() => buildGraphModel(missingToken)).toThrow(
      "missing analyzed token: missing",
    );

    const missingMatch = documentCopy();
    missingMatch.tree.nodes[1]!.match_id = "missing";
    expect(() => buildGraphModel(missingMatch)).toThrow(
      "missing primary match: missing",
    );

    const missingSecondary = documentCopy();
    missingSecondary.tree.nodes[3]!.secondary_match_ids = ["missing"];
    expect(() => buildGraphModel(missingSecondary)).toThrow(
      "missing secondary match: missing",
    );

    const invalidSpan = documentCopy();
    invalidSpan.tree.nodes[1]!.token_end = 99;
    invalidSpan.primary_matches[0]!.token_end = 99;
    expect(() => buildGraphModel(invalidSpan)).toThrow(
      "invalid token span: 0..99",
    );
  });
});
